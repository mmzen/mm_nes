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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// OAM DMA operation states.
///
/// ## Design: Pure Intent + Commit Pattern
///
/// The state machine uses only "Wait" states. State transitions happen AFTER
/// bus operations succeed, not when determining intent. This eliminates the
/// need for rollback when arbitration steals a cycle.
///
/// ## Cycle Structure (513 or 514 cycles total)
///
/// 1. **Halt cycle** (1 cycle): `Halt` state, no bus operation
/// 2. **Alignment cycle** (0-1 cycles): `WaitGet` state if first read would land on PUT
///    - This optional cycle ensures the first GET read occurs on a GET phase
///    - If $4014 write occurs when next phase is GET: 513 cycles (no alignment needed)
///    - If $4014 write occurs when next phase is PUT: 514 cycles (1 alignment cycle)
/// 3. **Transfer cycles** (512 cycles): 256 GET/PUT pairs
///    - `WaitGet`: Ready to read on next GET phase
///    - `WaitPut`: Ready to write on next PUT phase (has data from previous read)
///
/// ## Phase Correctness
///
/// Phase correctness is structurally guaranteed:
/// - `WaitGet` only emits Read intent when phase is GET
/// - `WaitPut` only emits Write intent when phase is PUT
/// - State transitions only occur after successful bus operation
pub enum OamDmaOp {
    /// Not active
    #[default]
    Idle,
    /// Attempting to halt CPU - waiting for CPU read cycle
    PendingHalt,
    /// Halt succeeded - this is the mandatory halt cycle (1 cycle, no bus op)
    Halt,
    /// Ready to read from source address when GET phase arrives
    WaitGet,
    /// Ready to write to $2004 when PUT phase arrives (holds read value)
    WaitPut,
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
///
/// OAM DMA writes go through the bus to $2004 (OAMDATA), so no direct PPU
/// reference is needed. The bus handles routing to the correct device.
#[derive(Debug)]
pub struct DmaController<B: Bus + ?Sized> {
    /// OAM DMA state
    oam: OamDmaState,
    /// DMC DMA state
    dmc: DmcDmaState,
    /// Reference to system bus (all DMA operations go through here)
    bus: Rc<RefCell<B>>,
    /// CPU's halted read address (for repeated reads when DMA doesn't use bus)
    cpu_halted_addr: Option<u16>,
}

impl<B: Bus + ?Sized> DmaController<B> {
    pub fn new(bus: Rc<RefCell<B>>) -> Self {
        DmaController {
            oam: OamDmaState::default(),
            dmc: DmcDmaState::default(),
            bus,
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
        matches!(self.oam.op, OamDmaOp::WaitGet)
    }

    /// Returns true only when CPU is actually stalled by DMA.
    ///
    /// **Important:** `PendingHalt` does NOT stall the CPU. The CPU continues
    /// executing normally until a read cycle allows the halt to succeed.
    ///
    /// CPU is stalled when:
    /// - OAM DMA: `Halt`, `WaitGet`, `WaitPut`
    /// - DMC DMA: `Halt`, `Dummy`, `Align`, `Read`
    pub fn is_cpu_stalled(&self) -> bool {
        self.is_cpu_stalled_by_oam() || self.is_cpu_stalled_by_dmc()
    }

    /// Returns true if CPU is stalled specifically by OAM DMA.
    ///
    /// This is needed for DMC PendingHalt logic: DMC can proceed with halt
    /// if CPU is already stalled by OAM, even if CPU's bus intent is "writing".
    pub fn is_cpu_stalled_by_oam(&self) -> bool {
        matches!(self.oam.op, OamDmaOp::Halt | OamDmaOp::WaitGet | OamDmaOp::WaitPut)
    }

    /// Returns true if CPU is stalled specifically by DMC DMA.
    fn is_cpu_stalled_by_dmc(&self) -> bool {
        matches!(self.dmc.phase,
            DmcDmaPhase::Halt | DmcDmaPhase::Dummy | DmcDmaPhase::Align | DmcDmaPhase::Read)
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
    /// Returns true if the request was accepted, false if ignored (already active).
    pub fn request_dmc_dma(&mut self, address: u16) -> bool {
        // Ignore duplicate requests if DMC DMA is already in progress
        if self.is_dmc_dma_active() {
            return false;
        }
        self.dmc = DmcDmaState {
            phase: DmcDmaPhase::PendingHalt,
            address,
            sample: None,
        };
        true
    }

    // Legacy compatibility
    #[deprecated(note = "Use request_dmc_dma")]
    #[allow(dead_code)]
    pub fn start_dmc_dma(&mut self, address: u16, _cycles: u8, _conflict_address: Option<u16>) {
        self.request_dmc_dma(address);
    }

    #[deprecated(note = "Use request_dmc_dma - no pre-calculation needed")]
    #[allow(dead_code)]
    pub fn calculate_dmc_dma_cycles(
        _cpu_is_writing: bool,
        _cpu_is_halted: bool,
        _oam_dma_active: bool,
        _oam_dma_on_read: bool,
    ) -> u8 {
        panic!("DEPRECATED: DMC DMA is now event-driven via request_dmc_dma(). \
                Do not pre-calculate cycles - the DMA controller determines timing dynamically.")
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
        // Pre-step invariant: DMC Read state must only exist on GET phase
        // ====================================================================
        // If DMC is in Read state but we're on PUT phase, the Dummy→Align→Read
        // sequencing is broken. This catches alignment bugs early rather than
        // silently waiting an extra cycle.
        debug_assert!(
            self.dmc.phase != DmcDmaPhase::Read || current_phase.is_get(),
            "DMC in Read state on PUT phase - Dummy/Align sequencing is broken. \
             The alignment logic should guarantee Read only occurs on GET."
        );

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
        // Must use OAM-only check here: we're asking "is CPU already stalled by something else?"
        // Using is_cpu_stalled() would incorrectly include DMC's own states.
        if self.dmc.phase == DmcDmaPhase::PendingHalt {
            let cpu_already_halted_by_oam = self.is_cpu_stalled_by_oam();
            if !cpu_is_writing || cpu_already_halted_by_oam {
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
        // Debug assertions: catch phase alignment drift early
        // ====================================================================

        // DMC Read must happen on GET phase - if we're reading on PUT, alignment is broken
        debug_assert!(
            winner != BusWinner::DmcRead || current_phase.is_get(),
            "DMC Read occurred on PUT phase - alignment logic is broken"
        );

        // OAM PUT must happen on PUT phase - if we're writing on GET, alignment is broken
        debug_assert!(
            winner != BusWinner::OamPut || current_phase.is_put(),
            "OAM PUT occurred on GET phase - alignment logic is broken"
        );

        // CpuRepeat should only happen when CPU is actually stalled
        // (This is already enforced by arbitrate(), but belt-and-suspenders)
        debug_assert!(
            winner != BusWinner::CpuRepeat || self.is_cpu_stalled(),
            "CpuRepeat returned when CPU is not stalled - should never happen"
        );

        // ====================================================================
        // Step 4: Execute the bus operation based on winner
        // ====================================================================

        // Track read value for OAM commit
        let mut oam_read_value: Option<u8> = None;

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
                    oam_read_value = Some(value);
                }
            }

            BusWinner::OamPut => {
                if let BusOp::Write(addr, data) = bus_op {
                    // Write to $2004 via the bus - PPU handles OAMADDR increment
                    self.bus.borrow_mut().write_byte(addr, data)?;
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
        // Step 4b: Commit OAM state transitions (after bus op succeeded)
        // ====================================================================

        // State transitions happen ONLY after bus operation succeeds
        // This eliminates rollback - if OAM didn't win, state stays unchanged
        result.oam_dma_complete = self.oam_commit(winner, oam_read_value);

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
        result.cpu_halted = self.is_cpu_stalled();

        Ok(result)
    }

    /// Determine what OAM DMA wants to do this cycle (pure - no state mutation).
    ///
    /// This function returns an intent without modifying state. State transitions
    /// happen in `oam_commit()` after arbitration confirms the operation succeeded.
    ///
    /// Phase correctness is structurally guaranteed:
    /// - `WaitGet` only returns Read when phase is GET
    /// - `WaitPut` only returns Write when phase is PUT
    fn oam_wants_bus_op(&self, current_phase: ApuPhase) -> BusOp {
        match self.oam.op {
            OamDmaOp::Idle | OamDmaOp::PendingHalt | OamDmaOp::Halt => BusOp::None,

            OamDmaOp::WaitGet => {
                if current_phase.is_get() {
                    // Phase matches - request read from source
                    let addr = ((self.oam.page as u16) << 8) | self.oam.byte_index;
                    BusOp::Read(addr)
                } else {
                    // Wrong phase - no bus op this cycle
                    BusOp::None
                }
            }

            OamDmaOp::WaitPut => {
                if current_phase.is_put() {
                    // Phase matches - request write to $2004
                    BusOp::Write(PPU_OAMDATA, self.oam.read_value)
                } else {
                    // Wrong phase - no bus op this cycle
                    BusOp::None
                }
            }
        }
    }

    /// Commit OAM DMA state transition after arbitration confirms the operation.
    ///
    /// This is called after the bus operation succeeds. State transitions only
    /// happen here, ensuring no rollback is ever needed.
    ///
    /// # Arguments
    /// * `winner` - Who won the bus arbitration
    /// * `read_value` - Value read from bus (only used for OamGet)
    fn oam_commit(&mut self, winner: BusWinner, read_value: Option<u8>) -> bool {
        match (winner, self.oam.op) {
            (BusWinner::OamGet, OamDmaOp::WaitGet) => {
                // Read succeeded - store value and transition to WaitPut
                self.oam.read_value = read_value.unwrap_or(0);
                self.oam.op = OamDmaOp::WaitPut;
                false // not complete
            }

            (BusWinner::OamPut, OamDmaOp::WaitPut) => {
                // Write succeeded - advance byte index
                self.oam.byte_index += 1;

                if self.oam.byte_index >= 256 {
                    // OAM DMA complete
                    self.oam.op = OamDmaOp::Idle;
                    true // complete!
                } else {
                    // More bytes to transfer
                    self.oam.op = OamDmaOp::WaitGet;
                    false // not complete
                }
            }

            // OAM didn't win or wrong state - no state change needed
            _ => false,
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

    /// Arbitrate between OAM and DMC DMA requests (pure - no state mutation).
    ///
    /// Returns (BusOp, BusWinner) - explicit tracking of who won.
    /// Priority: DMC Read > OAM Get > OAM Put > CPU repeated read
    ///
    /// Note: This function no longer mutates state. OAM state transitions
    /// are handled by `oam_commit()` after the bus operation executes.
    fn arbitrate(&self, oam_wants: BusOp, dmc_wants: BusOp) -> (BusOp, BusWinner) {
        // DMC Read has highest priority
        if let BusOp::Read(_) = dmc_wants {
            // DMC wins - OAM will simply not get its turn, no rollback needed
            return (dmc_wants, BusWinner::DmcRead);
        }

        // OAM operation next
        match oam_wants {
            BusOp::Read(_) => return (oam_wants, BusWinner::OamGet),
            BusOp::Write(_, _) => return (oam_wants, BusWinner::OamPut),
            BusOp::None => {}
        }

        // DMC no-bus phases or OAM waiting - CPU repeated read if stalled
        // IMPORTANT: Must use is_cpu_stalled(), not is_active().
        // During PendingHalt, DMA is active but CPU is NOT stalled - we must not
        // steal the bus with CpuRepeat in that case.
        if self.is_cpu_stalled() {
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
    // Test-only utility methods
    // ========================================================================

    /// Returns expected OAM DMA cycle count assuming immediate halt on write cycle.
    ///
    /// **WARNING**: This is only accurate when halt succeeds immediately after the
    /// $4014 write. If the CPU is writing when halt is attempted, the halt is
    /// delayed and the actual cycle count may differ from this prediction.
    ///
    /// This method is intended for tests with controlled conditions where the
    /// CPU is known to be reading when DMA starts.
    ///
    /// # Arguments
    /// * `write_phase` - The APU phase when the write to $4014 occurred
    #[cfg(test)]
    pub fn oam_dma_cycles(write_phase: ApuPhase) -> u16 {
        // Write on GET -> first DMA step on PUT -> 513 cycles
        // Write on PUT -> first DMA step on GET -> 514 cycles
        // NOTE: Only valid when halt succeeds immediately!
        if write_phase.is_get() { 513 } else { 514 }
    }

    // NOTE: The DMA controller no longer tracks phase internally.
    // The caller (nes_console) must track phase and pass it to step_cycle().
    // Tests should track phase locally and pass it explicitly.
}
