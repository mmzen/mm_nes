// Authorship: Human 0% | Claude 100%
// Rewritten: Proper bus arbiter model - exactly one bus op per CPU cycle
// Rewritten: OAM DMA phase-gated (reads on GET, writes on PUT)
// Rewritten: DMC DMA correct state machine (PendingHalt, Halt, Dummy, Align, Read)
// Rewritten: Correct overlap handling - DMC Read wins over OAM Get
//! DMA Controller for cycle-accurate OAM DMA and DMC DMA handling.
//!
//! This module provides a centralized DMA controller that enforces the fundamental
//! constraint: **exactly one bus operation per CPU cycle**.
//!
//! ## Bus Arbiter Model
//!
//! Each CPU cycle, the DMA controller determines a single `BusOp`:
//! - `BusOp::Read(addr)` - Read from address (DMA or CPU repeated read)
//! - `BusOp::Write(addr, data)` - Write to address (OAM DMA to PPU)
//! - `BusOp::None` - No DMA bus op (CPU executes normally)
//!
//! ## GET/PUT Phase Model
//!
//! The APU operates on a GET/PUT phase that alternates every CPU cycle:
//! - **GET phase**: Reads occur (OAM DMA source read, DMC sample read)
//! - **PUT phase**: Writes occur (OAM DMA to PPU)
//!
//! DMA operations are gated by phase:
//! - OAM DMA reads only on GET cycles
//! - OAM DMA writes only on PUT cycles
//! - DMC DMA sample read only on GET cycles
//!
//! ## Overlap Handling
//!
//! When DMC DMA needs to read and OAM DMA also needs GET:
//! - DMC Read wins (higher priority)
//! - OAM stays in WaitGet state
//! - Natural realignment occurs

use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use crate::bus::Bus;
use crate::dma_device::DmaDevice;
use crate::memory::MemoryError;

/// APU bus phase - alternates every CPU cycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApuPhase {
    /// Reads occur on GET phase
    #[default]
    Get,
    /// Writes occur on PUT phase
    Put,
}

impl ApuPhase {
    pub fn toggle(&self) -> ApuPhase {
        match self {
            ApuPhase::Get => ApuPhase::Put,
            ApuPhase::Put => ApuPhase::Get,
        }
    }

    pub fn is_get(&self) -> bool {
        matches!(self, ApuPhase::Get)
    }

    pub fn is_put(&self) -> bool {
        matches!(self, ApuPhase::Put)
    }
}

/// Single bus operation for one CPU cycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusOp {
    /// Read from address
    Read(u16),
    /// Write data to address
    Write(u16, u8),
    /// No DMA bus operation - CPU executes normally
    None,
}

impl Default for BusOp {
    fn default() -> Self {
        BusOp::None
    }
}

/// OAM DMA operation state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OamDmaOp {
    /// Not active
    #[default]
    Idle,
    /// Attempting to halt CPU - waiting for CPU read cycle
    PendingHalt,
    /// Halt succeeded - this is the halt cycle itself (1 cycle, no bus op)
    Halt,
    /// Waiting for GET phase to read
    WaitGet,
    /// Performing GET read from source
    Get,
    /// Waiting for PUT phase to write
    WaitPut,
    /// Performing PUT write to OAM
    Put,
}

/// State of OAM DMA transfer
#[derive(Debug, Clone, Default)]
pub struct OamDmaState {
    /// Current operation state
    pub op: OamDmaOp,
    /// Source page address (high byte)
    pub page: u8,
    /// Current byte index (0-255)
    pub byte_index: u16,
    /// Value read during GET phase, to be written during PUT
    pub read_value: u8,
    /// Total bytes transferred (for completion detection)
    pub bytes_transferred: u16,
}

/// DMC DMA phase - correct hardware sequence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DmcDmaPhase {
    /// Not active
    #[default]
    Idle,
    /// Attempting to halt CPU - retries until CPU read cycle or already stalled
    PendingHalt,
    /// Halt cycle - 1 cycle, no DMA bus op
    Halt,
    /// Dummy cycle - 1 cycle, no DMA bus op (NOT optional)
    Dummy,
    /// Alignment cycle - 1 cycle if current phase is PUT, no DMA bus op
    Align,
    /// Read cycle - GET phase read from DMC address
    Read,
}

/// State of DMC DMA fetch
#[derive(Debug, Clone, Default)]
pub struct DmcDmaState {
    /// Current phase
    pub phase: DmcDmaPhase,
    /// Address to fetch sample from
    pub address: u16,
    /// Fetched sample value (set after Read phase)
    pub sample: Option<u8>,
}

/// Result of a DMA controller cycle
#[derive(Debug, Clone, Default)]
pub struct DmaStepResult {
    /// The bus operation performed this cycle
    pub bus_op: BusOp,
    /// True if CPU should remain halted
    pub cpu_halted: bool,
    /// True if OAM DMA just completed
    pub oam_dma_complete: bool,
    /// DMC sample if DMC DMA just completed
    pub dmc_sample: Option<u8>,
}

/// DMA Controller with proper bus arbitration
#[derive(Debug)]
pub struct DmaController<B: Bus + ?Sized, D: DmaDevice + ?Sized> {
    /// OAM DMA state
    oam: OamDmaState,
    /// DMC DMA state
    dmc: DmcDmaState,
    /// Reference to system bus
    bus: Rc<RefCell<B>>,
    /// Reference to PPU for OAM writes
    ppu: Rc<RefCell<D>>,
    /// Current APU phase
    apu_phase: ApuPhase,
    /// CPU's halted read address (for repeated reads when DMA doesn't use bus)
    cpu_halted_addr: Option<u16>,
}

impl<B: Bus + ?Sized, D: DmaDevice + ?Sized> DmaController<B, D> {
    pub fn new(bus: Rc<RefCell<B>>, ppu: Rc<RefCell<D>>) -> Self {
        DmaController {
            oam: OamDmaState::default(),
            dmc: DmcDmaState::default(),
            bus,
            ppu,
            apu_phase: ApuPhase::default(),
            cpu_halted_addr: None,
        }
    }

    pub fn reset(&mut self) {
        self.oam = OamDmaState::default();
        self.dmc = DmcDmaState::default();
        self.cpu_halted_addr = None;
        // Don't reset apu_phase - only power_on randomizes it
    }

    // ========================================================================
    // Phase Management
    // ========================================================================

    pub fn set_apu_phase(&mut self, phase: ApuPhase) {
        self.apu_phase = phase;
    }

    pub fn get_apu_phase(&self) -> ApuPhase {
        self.apu_phase
    }

    pub fn toggle_apu_phase(&mut self) {
        self.apu_phase = self.apu_phase.toggle();
    }

    // ========================================================================
    // CPU Halted Address (for repeated reads)
    // ========================================================================

    pub fn set_cpu_halted_addr(&mut self, addr: Option<u16>) {
        self.cpu_halted_addr = addr;
    }

    pub fn get_cpu_halted_addr(&self) -> Option<u16> {
        self.cpu_halted_addr
    }

    // Legacy compatibility
    pub fn set_halted_read_address(&mut self, addr: Option<u16>) {
        self.cpu_halted_addr = addr;
    }

    pub fn get_halted_read_address(&self) -> Option<u16> {
        self.cpu_halted_addr
    }

    // ========================================================================
    // Status Queries
    // ========================================================================

    pub fn is_active(&self) -> bool {
        self.oam.op != OamDmaOp::Idle || self.dmc.phase != DmcDmaPhase::Idle
    }

    pub fn is_oam_dma_active(&self) -> bool {
        self.oam.op != OamDmaOp::Idle
    }

    pub fn is_dmc_dma_active(&self) -> bool {
        self.dmc.phase != DmcDmaPhase::Idle
    }

    pub fn is_oam_dma_on_read(&self) -> bool {
        matches!(self.oam.op, OamDmaOp::WaitGet | OamDmaOp::Get)
    }

    // ========================================================================
    // Start DMA Operations
    // ========================================================================

    /// Start OAM DMA from specified page.
    ///
    /// OAM DMA enters PendingHalt state - will wait for CPU read cycle to halt.
    pub fn start_oam_dma(&mut self, page: u8) {
        self.oam = OamDmaState {
            op: OamDmaOp::PendingHalt,
            page,
            byte_index: 0,
            read_value: 0,
            bytes_transferred: 0,
        };
    }

    /// Request DMC DMA to fetch from specified address.
    ///
    /// DMC DMA enters PendingHalt state - will wait for CPU read cycle to halt.
    pub fn request_dmc_dma(&mut self, address: u16, _cpu_is_writing: bool, _conflict_address: Option<u16>) {
        self.dmc = DmcDmaState {
            phase: DmcDmaPhase::PendingHalt,
            address,
            sample: None,
        };
    }

    // Legacy compatibility
    #[deprecated(note = "Use request_dmc_dma")]
    #[allow(dead_code)]
    pub fn start_dmc_dma(&mut self, address: u16, _cycles: u8, _conflict_address: Option<u16>) {
        self.request_dmc_dma(address, false, None);
    }

    #[deprecated(note = "Use request_dmc_dma - no pre-calculation needed")]
    #[allow(dead_code)]
    pub fn calculate_dmc_dma_cycles(
        _cpu_is_writing: bool,
        _cpu_is_halted: bool,
        _oam_dma_active: bool,
        _oam_dma_on_read: bool,
    ) -> u8 {
        3 // Placeholder - not used in new model
    }

    // ========================================================================
    // Main Cycle Step - Bus Arbiter
    // ========================================================================

    /// Execute one CPU cycle of DMA.
    ///
    /// This is the bus arbiter - it determines exactly ONE bus operation:
    /// 1. Advance DMA state machines
    /// 2. Determine what each DMA wants to do
    /// 3. Arbitrate: DMC Read > OAM Get > OAM Put > CPU repeated read
    /// 4. Execute the winning bus operation
    /// 5. Return result
    ///
    /// # Arguments
    /// * `cpu_is_writing` - True if CPU's pending bus op is a write (affects halt)
    pub fn step_cycle(&mut self, cpu_is_writing: bool) -> Result<DmaStepResult, MemoryError> {
        let mut result = DmaStepResult::default();

        // ====================================================================
        // Step 1: Advance pending halt states
        // ====================================================================

        // OAM DMA: PendingHalt -> Halt (only if CPU is not writing)
        // The Halt state represents the halt cycle itself - 1 cycle with no bus op
        if self.oam.op == OamDmaOp::PendingHalt {
            if !cpu_is_writing {
                // Halt succeeds - enter the halt cycle
                self.oam.op = OamDmaOp::Halt;
            }
            // If CPU is writing, stay in PendingHalt (halt attempt fails)
        }

        // DMC DMA: PendingHalt -> Halt (only if CPU is not writing OR already halted by OAM)
        if self.dmc.phase == DmcDmaPhase::PendingHalt {
            let cpu_already_halted = matches!(self.oam.op, OamDmaOp::Halt | OamDmaOp::WaitGet | OamDmaOp::Get | OamDmaOp::WaitPut | OamDmaOp::Put);
            if !cpu_is_writing || cpu_already_halted {
                // Halt succeeds
                self.dmc.phase = DmcDmaPhase::Halt;
            }
            // If CPU is writing and not already halted, stay in PendingHalt
        }

        // ====================================================================
        // Step 2: Determine what each DMA wants to do this cycle
        // ====================================================================

        let oam_wants = self.oam_wants_bus_op();
        let dmc_wants = self.dmc_wants_bus_op();

        // ====================================================================
        // Step 3: Arbitrate - determine single bus op
        // Priority: DMC Read > OAM Get > OAM Put > CPU repeated read
        // ====================================================================

        let bus_op = self.arbitrate(oam_wants, dmc_wants);

        // ====================================================================
        // Step 4: Execute the bus operation
        // ====================================================================

        match bus_op {
            BusOp::Read(addr) => {
                let value = self.bus.borrow().read_byte(addr)?;

                // Check who performed this read
                if dmc_wants == BusOp::Read(addr) && self.dmc.phase == DmcDmaPhase::Read {
                    // DMC DMA completed its read
                    self.dmc.sample = Some(value);
                    result.dmc_sample = Some(value);
                    self.dmc.phase = DmcDmaPhase::Idle;
                } else if matches!(self.oam.op, OamDmaOp::Get) {
                    // OAM DMA read
                    self.oam.read_value = value;
                    self.oam.op = OamDmaOp::WaitPut;
                }
                // Otherwise it was a CPU repeated read (side effects happened via bus.read_byte)
            }

            BusOp::Write(addr, data) => {
                // OAM DMA write to PPU
                self.ppu.borrow_mut().dma_write(addr as u8, data)?;
                self.oam.bytes_transferred += 1;
                self.oam.byte_index += 1;

                if self.oam.bytes_transferred >= 256 {
                    // OAM DMA complete
                    self.oam.op = OamDmaOp::Idle;
                    result.oam_dma_complete = true;
                } else {
                    // More bytes to transfer - go back to waiting for GET
                    self.oam.op = OamDmaOp::WaitGet;
                }
            }

            BusOp::None => {
                // No bus operation this cycle
            }
        }

        // ====================================================================
        // Step 5: Advance no-bus phases for both DMAs
        // ====================================================================

        // OAM DMA: Halt -> WaitGet (after the halt cycle completes)
        if self.oam.op == OamDmaOp::Halt {
            self.oam.op = OamDmaOp::WaitGet;
        }

        // DMC DMA: advance through Halt -> Dummy -> Align -> Read
        self.advance_dmc_no_bus_phases();

        // ====================================================================
        // Step 6: Build result
        // ====================================================================

        result.bus_op = bus_op;
        result.cpu_halted = self.is_active();

        Ok(result)
    }

    /// Determine what OAM DMA wants to do this cycle (may not get it due to arbitration)
    fn oam_wants_bus_op(&mut self) -> BusOp {
        match self.oam.op {
            OamDmaOp::Idle | OamDmaOp::PendingHalt => BusOp::None,

            // Halt cycle - no bus op, just consuming the halt cycle
            OamDmaOp::Halt => BusOp::None,

            OamDmaOp::WaitGet => {
                if self.apu_phase.is_get() {
                    // Phase matches - transition to Get and request read
                    self.oam.op = OamDmaOp::Get;
                    let addr = ((self.oam.page as u16) << 8) | self.oam.byte_index;
                    BusOp::Read(addr)
                } else {
                    // Wrong phase - stay waiting
                    BusOp::None
                }
            }

            OamDmaOp::Get => {
                // Already in Get state - return the read we want
                let addr = ((self.oam.page as u16) << 8) | self.oam.byte_index;
                BusOp::Read(addr)
            }

            OamDmaOp::WaitPut => {
                if self.apu_phase.is_put() {
                    // Phase matches - transition to Put and request write
                    self.oam.op = OamDmaOp::Put;
                    BusOp::Write(self.oam.byte_index, self.oam.read_value)
                } else {
                    // Wrong phase - stay waiting
                    BusOp::None
                }
            }

            OamDmaOp::Put => {
                // Already in Put state - return the write we want
                BusOp::Write(self.oam.byte_index, self.oam.read_value)
            }
        }
    }

    /// Determine what DMC DMA wants to do this cycle
    fn dmc_wants_bus_op(&self) -> BusOp {
        match self.dmc.phase {
            DmcDmaPhase::Idle | DmcDmaPhase::PendingHalt => BusOp::None,

            // These phases don't use the bus
            DmcDmaPhase::Halt | DmcDmaPhase::Dummy | DmcDmaPhase::Align => BusOp::None,

            DmcDmaPhase::Read => {
                // DMC wants to read - but only on GET phase
                if self.apu_phase.is_get() {
                    BusOp::Read(self.dmc.address)
                } else {
                    BusOp::None // Will need to wait/align
                }
            }
        }
    }

    /// Arbitrate between OAM and DMC DMA requests.
    /// Priority: DMC Read > OAM operation > CPU repeated read
    fn arbitrate(&mut self, oam_wants: BusOp, dmc_wants: BusOp) -> BusOp {
        // DMC Read has highest priority
        if let BusOp::Read(_) = dmc_wants {
            // DMC wins - if OAM was trying to Get, it stays in Get/WaitGet
            if matches!(self.oam.op, OamDmaOp::Get) {
                // OAM was in Get state but DMC stole the cycle
                // Transition back to WaitGet to try again next GET cycle
                self.oam.op = OamDmaOp::WaitGet;
            }
            return dmc_wants;
        }

        // OAM operation next
        match oam_wants {
            BusOp::Read(_) | BusOp::Write(_, _) => return oam_wants,
            BusOp::None => {}
        }

        // DMC no-bus phases or OAM waiting - CPU repeated read if halted
        if self.is_active() {
            if let Some(addr) = self.cpu_halted_addr {
                return BusOp::Read(addr);
            }
        }

        BusOp::None
    }

    /// Advance DMC DMA through no-bus phases (Halt, Dummy, Align)
    fn advance_dmc_no_bus_phases(&mut self) {
        match self.dmc.phase {
            DmcDmaPhase::Halt => {
                // Halt -> Dummy (always)
                self.dmc.phase = DmcDmaPhase::Dummy;
            }

            DmcDmaPhase::Dummy => {
                // Dummy -> Align or Read depending on phase
                if self.apu_phase.is_put() {
                    // Next cycle will be GET - need to align
                    self.dmc.phase = DmcDmaPhase::Align;
                } else {
                    // Already on GET or will be - go to Read
                    self.dmc.phase = DmcDmaPhase::Read;
                }
            }

            DmcDmaPhase::Align => {
                // Align -> Read (always)
                self.dmc.phase = DmcDmaPhase::Read;
            }

            // Other phases handled elsewhere
            _ => {}
        }
    }

    // ========================================================================
    // Legacy compatibility - old step_cycle without cpu_is_writing
    // ========================================================================

    /// Legacy step_cycle that doesn't take cpu_is_writing.
    /// For backward compatibility - assumes CPU is reading.
    pub fn step_cycle_legacy(&mut self) -> Result<DmaStepResult, MemoryError> {
        self.step_cycle(false)
    }

    // ========================================================================
    // Utility methods
    // ========================================================================

    pub fn oam_dma_cycles(&self) -> u16 {
        if self.apu_phase.is_put() { 514 } else { 513 }
    }

    // Legacy compatibility aliases
    #[deprecated(note = "Use set_apu_phase instead")]
    pub fn set_cpu_cycle_odd(&mut self, odd: bool) {
        self.apu_phase = if odd { ApuPhase::Put } else { ApuPhase::Get };
    }
}
