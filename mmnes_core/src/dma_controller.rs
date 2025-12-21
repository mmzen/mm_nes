// Authorship: Human 0% | Claude 100%
// Rewritten: Proper bus arbiter model - exactly one bus op per CPU cycle
// Rewritten: OAM DMA phase-gated (reads on GET, writes on PUT)
// Rewritten: DMC DMA correct state machine (PendingHalt, Halt, Dummy, Align, Read)
// Rewritten: Correct overlap handling - DMC Read wins over OAM Get
// Fixed: OAM PUT now writes to $2004 (not byte_index)
// Fixed: Phase passed as parameter, alignment uses next_phase
// Fixed: Explicit BusWinner tracking
//! DMA Controller for cycle-accurate OAM DMA and DMC DMA handling.
//!
//! This module provides a centralized DMA controller that enforces the fundamental
//! constraint: **exactly one bus operation per CPU cycle**.
//!
//! ## Bus Arbiter Model
//!
//! Each CPU cycle, the DMA controller determines a single `BusOp`:
//! - `BusOp::Read(addr)` - Read from address (DMA or CPU repeated read)
//! - `BusOp::Write(addr, data)` - Write to address (OAM DMA to $2004)
//! - `BusOp::None` - No DMA bus op (CPU executes normally)
//!
//! ## GET/PUT Phase Model
//!
//! The APU operates on a GET/PUT phase that alternates every CPU cycle:
//! - **GET phase**: Reads occur (OAM DMA source read, DMC sample read)
//! - **PUT phase**: Writes occur (OAM DMA write to $2004)
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

/// PPU OAM data register address
const PPU_OAMDATA: u16 = 0x2004;

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

/// Who won the bus arbitration this cycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BusWinner {
    /// DMC DMA performed a read
    DmcRead,
    /// OAM DMA performed a GET read
    OamGet,
    /// OAM DMA performed a PUT write
    OamPut,
    /// CPU repeated read (during DMA idle cycles)
    CpuRepeat,
    /// No bus operation
    #[default]
    None,
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
    /// Performing PUT write to OAM via $2004
    Put,
}

/// State of OAM DMA transfer
#[derive(Debug, Clone, Default)]
pub struct OamDmaState {
    /// Current operation state
    pub op: OamDmaOp,
    /// Source page address (high byte)
    pub page: u8,
    /// Current byte index (0-255), used for completion detection
    pub byte_index: u16,
    /// Value read during GET phase, to be written during PUT
    pub read_value: u8,
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
    /// Alignment cycle - 1 cycle if next phase would be PUT, no DMA bus op
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
    /// Who won the bus arbitration
    pub winner: BusWinner,
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
    /// Reference to PPU for OAM writes (kept for compatibility, but writes go through bus)
    #[allow(dead_code)]
    ppu: Rc<RefCell<D>>,
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
            cpu_halted_addr: None,
        }
    }

    pub fn reset(&mut self) {
        self.oam = OamDmaState::default();
        self.dmc = DmcDmaState::default();
        self.cpu_halted_addr = None;
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
        };
    }

    /// Request DMC DMA to fetch from specified address.
    ///
    /// DMC DMA enters PendingHalt state - will wait for CPU read cycle to halt.
    ///
    /// If DMC DMA is already active, this request is ignored. This allows the
    /// APU to be authoritative about WHEN to request without the scheduler
    /// needing to track DMA state.
    pub fn request_dmc_dma(&mut self, address: u16, _cpu_is_writing: bool, _conflict_address: Option<u16>) {
        // Ignore duplicate requests if DMC DMA is already in progress
        if self.is_dmc_dma_active() {
            return;
        }
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
    /// * `pending_read_addr` - The CPU's pending read address THIS cycle (captured at halt)
    /// * `current_phase` - The APU phase for THIS cycle (caller must track and pass)
    pub fn step_cycle(&mut self, cpu_is_writing: bool, pending_read_addr: Option<u16>, current_phase: ApuPhase) -> Result<DmaStepResult, MemoryError> {
        let mut result = DmaStepResult::default();
        let next_phase = current_phase.toggle();

        // ====================================================================
        // Step 1: Advance pending halt states
        // ====================================================================

        // OAM DMA: PendingHalt -> Halt (only if CPU is not writing)
        // The Halt state represents the halt cycle itself - 1 cycle with no bus op
        // CRITICAL: Capture the pending read address AT THE MOMENT halt succeeds,
        // not earlier. This ensures correct address for repeated reads.
        if self.oam.op == OamDmaOp::PendingHalt {
            if !cpu_is_writing {
                // Halt succeeds - enter the halt cycle
                // Capture the pending read address NOW (the read being halted)
                self.cpu_halted_addr = pending_read_addr;
                self.oam.op = OamDmaOp::Halt;
            }
            // If CPU is writing, stay in PendingHalt (halt attempt fails)
        }

        // DMC DMA: PendingHalt -> Halt (only if CPU is not writing OR already halted by OAM)
        if self.dmc.phase == DmcDmaPhase::PendingHalt {
            let cpu_already_halted = matches!(self.oam.op, OamDmaOp::Halt | OamDmaOp::WaitGet | OamDmaOp::Get | OamDmaOp::WaitPut | OamDmaOp::Put);
            if !cpu_is_writing || cpu_already_halted {
                // Halt succeeds
                // Only capture address if not already captured by OAM halt
                if self.cpu_halted_addr.is_none() {
                    self.cpu_halted_addr = pending_read_addr;
                }
                self.dmc.phase = DmcDmaPhase::Halt;
            }
            // If CPU is writing and not already halted, stay in PendingHalt
        }

        // ====================================================================
        // Step 2: Determine what each DMA wants to do this cycle
        // ====================================================================

        let oam_wants = self.oam_wants_bus_op(current_phase);
        let dmc_wants = self.dmc_wants_bus_op(current_phase);

        // ====================================================================
        // Step 3: Arbitrate - determine single bus op and winner
        // Priority: DMC Read > OAM Get > OAM Put > CPU repeated read
        // ====================================================================

        let (bus_op, winner) = self.arbitrate(oam_wants, dmc_wants);

        // ====================================================================
        // Step 4: Execute the bus operation based on winner
        // ====================================================================

        match winner {
            BusWinner::DmcRead => {
                if let BusOp::Read(addr) = bus_op {
                    let value = self.bus.borrow().read_byte(addr)?;
                    self.dmc.sample = Some(value);
                    result.dmc_sample = Some(value);
                    self.dmc.phase = DmcDmaPhase::Idle;
                }
            }

            BusWinner::OamGet => {
                if let BusOp::Read(addr) = bus_op {
                    let value = self.bus.borrow().read_byte(addr)?;
                    self.oam.read_value = value;
                    self.oam.op = OamDmaOp::WaitPut;
                }
            }

            BusWinner::OamPut => {
                if let BusOp::Write(addr, data) = bus_op {
                    // Write to $2004 via the bus - PPU handles OAMADDR increment
                    self.bus.borrow_mut().write_byte(addr, data)?;
                    self.oam.byte_index += 1;

                    if self.oam.byte_index >= 256 {
                        // OAM DMA complete
                        self.oam.op = OamDmaOp::Idle;
                        result.oam_dma_complete = true;
                    } else {
                        // More bytes to transfer - go back to waiting for GET
                        self.oam.op = OamDmaOp::WaitGet;
                    }
                }
            }

            BusWinner::CpuRepeat => {
                if let BusOp::Read(addr) = bus_op {
                    // CPU repeated read - side effects happen via bus.read_byte
                    let _ = self.bus.borrow().read_byte(addr)?;
                }
            }

            BusWinner::None => {
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
        self.advance_dmc_no_bus_phases(next_phase);

        // ====================================================================
        // Step 6: Build result
        // ====================================================================

        result.bus_op = bus_op;
        result.winner = winner;
        result.cpu_halted = self.is_active();

        Ok(result)
    }

    /// Determine what OAM DMA wants to do this cycle (may not get it due to arbitration)
    fn oam_wants_bus_op(&mut self, current_phase: ApuPhase) -> BusOp {
        match self.oam.op {
            OamDmaOp::Idle | OamDmaOp::PendingHalt => BusOp::None,

            // Halt cycle - no bus op, just consuming the halt cycle
            OamDmaOp::Halt => BusOp::None,

            OamDmaOp::WaitGet => {
                if current_phase.is_get() {
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
                if current_phase.is_put() {
                    // Phase matches - transition to Put and request write to $2004
                    self.oam.op = OamDmaOp::Put;
                    BusOp::Write(PPU_OAMDATA, self.oam.read_value)
                } else {
                    // Wrong phase - stay waiting
                    BusOp::None
                }
            }

            OamDmaOp::Put => {
                // Already in Put state - return the write we want (to $2004)
                BusOp::Write(PPU_OAMDATA, self.oam.read_value)
            }
        }
    }

    /// Determine what DMC DMA wants to do this cycle
    fn dmc_wants_bus_op(&self, current_phase: ApuPhase) -> BusOp {
        match self.dmc.phase {
            DmcDmaPhase::Idle | DmcDmaPhase::PendingHalt => BusOp::None,

            // These phases don't use the bus
            DmcDmaPhase::Halt | DmcDmaPhase::Dummy | DmcDmaPhase::Align => BusOp::None,

            DmcDmaPhase::Read => {
                // DMC wants to read - but only on GET phase
                if current_phase.is_get() {
                    BusOp::Read(self.dmc.address)
                } else {
                    BusOp::None // Will need to wait/align
                }
            }
        }
    }

    /// Arbitrate between OAM and DMC DMA requests.
    /// Returns (BusOp, BusWinner) - explicit tracking of who won.
    /// Priority: DMC Read > OAM Get > OAM Put > CPU repeated read
    fn arbitrate(&mut self, oam_wants: BusOp, dmc_wants: BusOp) -> (BusOp, BusWinner) {
        // DMC Read has highest priority
        if let BusOp::Read(_) = dmc_wants {
            // DMC wins - if OAM was trying to Get, it stays in Get/WaitGet
            if matches!(self.oam.op, OamDmaOp::Get) {
                // OAM was in Get state but DMC stole the cycle
                // Transition back to WaitGet to try again next GET cycle
                self.oam.op = OamDmaOp::WaitGet;
            }
            return (dmc_wants, BusWinner::DmcRead);
        }

        // OAM operation next
        match oam_wants {
            BusOp::Read(_) => return (oam_wants, BusWinner::OamGet),
            BusOp::Write(_, _) => return (oam_wants, BusWinner::OamPut),
            BusOp::None => {}
        }

        // DMC no-bus phases or OAM waiting - CPU repeated read if halted
        if self.is_active() {
            if let Some(addr) = self.cpu_halted_addr {
                return (BusOp::Read(addr), BusWinner::CpuRepeat);
            }
        }

        (BusOp::None, BusWinner::None)
    }

    /// Advance DMC DMA through no-bus phases (Halt, Dummy, Align)
    ///
    /// # Arguments
    /// * `next_phase` - The phase of the NEXT cycle (used for alignment decision)
    fn advance_dmc_no_bus_phases(&mut self, next_phase: ApuPhase) {
        match self.dmc.phase {
            DmcDmaPhase::Halt => {
                // Halt -> Dummy (always)
                self.dmc.phase = DmcDmaPhase::Dummy;
            }

            DmcDmaPhase::Dummy => {
                // Dummy -> Align or Read depending on NEXT cycle's phase
                // DMC Read needs GET phase
                // If next cycle is PUT, we need to insert Align cycle
                // If next cycle is GET, we can go directly to Read
                if next_phase.is_put() {
                    // Next cycle is PUT, but Read needs GET - insert Align
                    self.dmc.phase = DmcDmaPhase::Align;
                } else {
                    // Next cycle is GET - can go directly to Read
                    self.dmc.phase = DmcDmaPhase::Read;
                }
            }

            DmcDmaPhase::Align => {
                // Align -> Read (always, next cycle should now be GET)
                self.dmc.phase = DmcDmaPhase::Read;
            }

            // Other phases handled elsewhere
            _ => {}
        }
    }

    // ========================================================================
    // Legacy compatibility
    // ========================================================================

    /// Legacy step_cycle that doesn't take current_phase or pending_read_addr.
    /// For backward compatibility - uses defaults.
    /// DEPRECATED: Use step_cycle(cpu_is_writing, pending_read_addr, current_phase) instead.
    #[deprecated(note = "Use step_cycle with explicit parameters")]
    pub fn step_cycle_legacy(&mut self, cpu_is_writing: bool) -> Result<DmaStepResult, MemoryError> {
        // This maintains backward compatibility but should not be used
        self.step_cycle(cpu_is_writing, None, ApuPhase::Get)
    }

    // ========================================================================
    // Utility methods
    // ========================================================================

    /// Returns expected OAM DMA cycle count based on the phase when $4014 was written.
    ///
    /// # Arguments
    /// * `write_phase` - The APU phase when the write to $4014 occurred
    pub fn oam_dma_cycles(write_phase: ApuPhase) -> u16 {
        // Write on GET -> first DMA step on PUT -> 513 cycles
        // Write on PUT -> first DMA step on GET -> 514 cycles
        if write_phase.is_get() { 513 } else { 514 }
    }

    // ========================================================================
    // Phase management (for caller convenience)
    // ========================================================================

    // NOTE: The DMA controller no longer tracks phase internally.
    // The caller (nes_console) must track phase and pass it to step_cycle().
    // These methods are kept for test compatibility but delegate to a default.

    #[cfg(test)]
    pub fn set_apu_phase(&mut self, _phase: ApuPhase) {
        // No-op - phase is now passed to step_cycle
    }

    #[cfg(test)]
    pub fn get_apu_phase(&self) -> ApuPhase {
        // Return default - phase is now passed to step_cycle
        ApuPhase::Get
    }

    #[cfg(test)]
    pub fn toggle_apu_phase(&mut self) {
        // No-op - phase is now passed to step_cycle
    }

    // Legacy compatibility aliases
    #[deprecated(note = "Phase is now passed to step_cycle")]
    #[allow(dead_code)]
    pub fn set_cpu_cycle_odd(&mut self, _odd: bool) {
        // No-op - phase is now passed to step_cycle
    }
}
