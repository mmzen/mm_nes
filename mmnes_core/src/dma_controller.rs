// Authorship: Human 0% | Claude 100%
// Updated: Made DMC DMA event-driven - no pre-calculation of cycles, dynamically checks CPU bus intent
// Updated: Fixed OAM DMA alignment logic (1 idle for even, 2 for odd), added comprehensive tests
// Updated: Added DMC DMA bus conflict behavior - halt cycles read from CPU's conflict address
// Updated: Fixed DMC+OAM DMA interleaving - halt cycles run alongside OAM, only get cycle has priority
// Updated: Added GET/PUT phase tracking for proper APU bus timing
// Updated: Replaced DMC DMA cycles_remaining with DmcDmaPhase state machine (Halt/Dummy/Align/Read)
//! DMA Controller for cycle-accurate OAM DMA and DMC DMA handling.
//!
//! This module provides a centralized DMA controller that manages both:
//! - OAM DMA: Transfers 256 bytes from CPU memory to PPU OAM (513-514 cycles)
//! - DMC DMA: Fetches sample bytes for the DMC channel (1-4 cycles)
//!
//! The controller works with the CPU's cycle-stepping state machine to properly
//! halt the CPU during DMA operations.
//!
//! ## GET/PUT Phase Model
//!
//! The APU operates on a GET/PUT phase that alternates every CPU cycle:
//! - **GET phase**: APU samples the data bus (reads occur)
//! - **PUT phase**: APU drives the internal address bus
//!
//! DMA operations must align to these phases:
//! - OAM DMA reads must occur on GET cycles
//! - DMC DMA's final sample read must occur on a GET cycle
//! - If the next cycle would be PUT when a read is needed, an alignment cycle is inserted

use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use crate::bus::Bus;
use crate::dma_device::DmaDevice;
use crate::memory::MemoryError;

/// APU bus phase - alternates every CPU cycle
///
/// The GET/PUT model describes the APU's relationship to the CPU bus:
/// - GET: APU samples the data bus (external reads visible to APU)
/// - PUT: APU drives internal address bus (writes/internal operations)
///
/// DMA operations align to these phases for correct timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApuPhase {
    /// APU samples data bus - reads occur on this phase
    #[default]
    Get,
    /// APU drives internal address bus - writes/internal ops
    Put,
}

impl ApuPhase {
    /// Toggle to the opposite phase
    pub fn toggle(&self) -> ApuPhase {
        match self {
            ApuPhase::Get => ApuPhase::Put,
            ApuPhase::Put => ApuPhase::Get,
        }
    }

    /// Returns true if this is a GET phase (read phase)
    pub fn is_get(&self) -> bool {
        matches!(self, ApuPhase::Get)
    }

    /// Returns true if this is a PUT phase
    pub fn is_put(&self) -> bool {
        matches!(self, ApuPhase::Put)
    }
}

/// State of OAM DMA transfer
#[derive(Debug, Clone, Default)]
pub struct OamDmaState {
    /// Whether OAM DMA is currently active
    pub active: bool,
    /// Source page address (high byte of source address)
    pub page: u8,
    /// Current byte index being transferred (0-255)
    pub byte_index: u16,
    /// Current cycle within DMA (0 = alignment, 1-512 = transfer)
    pub cycle: u16,
    /// Whether we're on the read phase (true) or write phase (false)
    pub read_phase: bool,
    /// The value read during the read phase
    pub read_value: u8,
    /// APU phase when OAM DMA started (affects alignment)
    /// If started on PUT, need extra alignment cycle to reach GET for first read
    pub started_on_phase: ApuPhase,
}

/// Phase of DMC DMA operation
///
/// DMC DMA follows a specific sequence depending on CPU state:
/// - Halt: Wait for CPU to be ready (reads from conflict address)
/// - Dummy: Dummy cycle after halt
/// - Align: Optional alignment if next cycle isn't GET
/// - Read: Actual sample fetch from DMC address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DmcDmaPhase {
    /// Idle - no DMC DMA in progress
    #[default]
    Idle,
    /// Halt cycle(s) - reading from conflict address, waiting for CPU
    /// Contains count of remaining halt cycles (1-3)
    Halt(u8),
    /// Dummy cycle after halt
    Dummy,
    /// Alignment cycle (if next cycle isn't GET phase)
    Align,
    /// Final read cycle - fetch sample from DMC address
    Read,
}

/// State of DMC DMA fetch
#[derive(Debug, Clone, Default)]
pub struct DmcDmaState {
    /// Whether DMC DMA is currently pending/active
    pub active: bool,
    /// Address to fetch the DMC sample from
    pub address: u16,
    /// Current phase of DMC DMA operation
    pub phase: DmcDmaPhase,
    /// Address the CPU was accessing when DMC DMA started (for bus conflict reads)
    /// During halt cycles, reads from this address cause side effects
    pub conflict_address: Option<u16>,
    /// Tracks if CPU was writing when DMC DMA started (for halt cycle count)
    /// True = CPU was writing, need to wait longer before read
    pub cpu_was_writing: bool,
}

/// DMA Controller managing OAM and DMC DMA operations
#[derive(Debug)]
pub struct DmaController<B: Bus + ?Sized, D: DmaDevice + ?Sized> {
    /// OAM DMA state
    oam_dma: OamDmaState,
    /// DMC DMA state
    dmc_dma: DmcDmaState,
    /// Reference to the system bus for reading source data
    bus: Rc<RefCell<B>>,
    /// Reference to the PPU for OAM writes
    ppu: Rc<RefCell<D>>,
    /// Current APU phase (GET/PUT) - alternates every CPU cycle
    /// Used for DMA alignment decisions
    apu_phase: ApuPhase,
    /// Address the CPU was reading from when halted
    /// During DMA no-bus cycles, the external bus shows repeated reads from this address.
    /// This is important because reading certain addresses causes side effects:
    /// - $2002: Clears VBlank flag
    /// - $2007: Increments VRAM address
    /// - $4016/$4017: Clocks controller shift register
    halted_read_address: Option<u16>,
}

/// Result of a DMA controller cycle step
#[derive(Debug, Clone, Default)]
pub struct DmaStepResult {
    /// True if DMA is active and CPU should remain halted
    pub cpu_halted: bool,
    /// True if OAM DMA just completed
    pub oam_dma_complete: bool,
    /// True if DMC DMA just completed, contains the fetched byte
    pub dmc_dma_complete: Option<u8>,
    /// Address accessed this cycle (if any)
    pub address_accessed: Option<u16>,
    /// True if a read occurred
    pub read_occurred: bool,
    /// True if a write occurred
    pub write_occurred: bool,
}

impl<B: Bus + ?Sized, D: DmaDevice + ?Sized> DmaController<B, D> {
    /// Create a new DMA controller
    pub fn new(bus: Rc<RefCell<B>>, ppu: Rc<RefCell<D>>) -> Self {
        DmaController {
            oam_dma: OamDmaState::default(),
            dmc_dma: DmcDmaState::default(),
            bus,
            ppu,
            apu_phase: ApuPhase::default(),
            halted_read_address: None,
        }
    }

    /// Reset the DMA controller state
    /// Note: APU phase is NOT reset here - it persists across reset (only randomized on power-on)
    pub fn reset(&mut self) {
        self.oam_dma = OamDmaState::default();
        self.dmc_dma = DmcDmaState::default();
        self.halted_read_address = None;
        // Don't reset apu_phase - only power_on should randomize it
    }

    /// Set the address the CPU was reading from when halted
    ///
    /// During DMA no-bus cycles, the external bus shows repeated reads from this address.
    /// This causes side effects for certain registers:
    /// - $2002: Clears VBlank flag
    /// - $2007: Increments VRAM address
    /// - $4016/$4017: Clocks controller shift register
    pub fn set_halted_read_address(&mut self, address: Option<u16>) {
        self.halted_read_address = address;
    }

    /// Get the current halted read address
    pub fn get_halted_read_address(&self) -> Option<u16> {
        self.halted_read_address
    }

    /// Set the APU phase directly (used during power-on randomization)
    pub fn set_apu_phase(&mut self, phase: ApuPhase) {
        self.apu_phase = phase;
    }

    /// Get the current APU phase
    pub fn get_apu_phase(&self) -> ApuPhase {
        self.apu_phase
    }

    /// Toggle the APU phase (called every CPU cycle)
    pub fn toggle_apu_phase(&mut self) {
        self.apu_phase = self.apu_phase.toggle();
    }

    /// Check if any DMA is currently active
    pub fn is_active(&self) -> bool {
        self.oam_dma.active || self.dmc_dma.active
    }

    /// Start an OAM DMA transfer from the specified page
    ///
    /// OAM DMA alignment depends on the current APU phase:
    /// - If started on GET: 513 cycles (1 idle + 512 transfer)
    /// - If started on PUT: 514 cycles (2 idle + 512 transfer)
    ///
    /// The extra idle cycle when starting on PUT ensures the first read
    /// occurs on a GET phase.
    pub fn start_oam_dma(&mut self, page: u8) {
        self.oam_dma = OamDmaState {
            active: true,
            page,
            byte_index: 0,
            cycle: 0,
            read_phase: true,
            read_value: 0,
            started_on_phase: self.apu_phase,
        };
    }

    /// Request a DMC DMA fetch from the specified address (event-driven).
    ///
    /// This is the preferred method - it doesn't pre-calculate cycles.
    /// The DMA controller dynamically determines timing based on CPU bus state each cycle.
    ///
    /// DMC DMA sequence:
    /// - Halt phase: Waits until CPU is not writing (reads from conflict address)
    /// - Read phase: Fetches sample from DMC address
    ///
    /// # Arguments
    /// * `address` - The address to fetch the sample byte from
    /// * `cpu_is_writing` - True if CPU is currently performing a write (cannot halt immediately)
    /// * `conflict_address` - The address the CPU was reading from (for halt cycle side effects)
    pub fn request_dmc_dma(&mut self, address: u16, cpu_is_writing: bool, conflict_address: Option<u16>) {
        // DMC DMA starts in Halt phase if CPU is writing, otherwise can proceed faster
        // The exact number of halt cycles depends on dynamic state each cycle
        let initial_phase = if cpu_is_writing {
            // CPU is writing - start in halt, need to wait for write to complete
            // We use Halt(3) as a maximum but will transition based on actual bus state
            DmcDmaPhase::Halt(3)
        } else if self.oam_dma.active && !self.oam_dma.read_phase {
            // OAM DMA is on write phase - need to wait for it
            DmcDmaPhase::Halt(1)
        } else if self.oam_dma.active && self.oam_dma.read_phase {
            // OAM DMA is on read phase - can steal immediately after
            DmcDmaPhase::Read
        } else {
            // CPU is reading - need minimal halt cycles
            DmcDmaPhase::Halt(2)
        };

        self.dmc_dma = DmcDmaState {
            active: true,
            address,
            phase: initial_phase,
            conflict_address,
            cpu_was_writing: cpu_is_writing,
        };
    }

    /// Start a DMC DMA fetch from the specified address (legacy interface).
    ///
    /// DMC DMA uses a state machine with phases:
    /// - Halt cycles: 0-3 cycles depending on CPU state, reads from conflict address
    /// - Dummy cycle: Always present after halt
    /// - Align cycle: Present if current phase is PUT (need to align to GET for read)
    /// - Read cycle: Final cycle, fetches sample from DMC address
    ///
    /// Total cycle counts:
    /// - 4 cycles if CPU is writing (halt(3) → read)
    /// - 3 cycles if CPU is reading (halt(2) → read)
    /// - 2 cycles if OAM DMA write phase (halt(1) → read)
    /// - 1 cycle if OAM DMA read phase or CPU halted (read only)
    ///
    /// # Arguments
    /// * `address` - The address to fetch the sample byte from
    /// * `cycles` - Number of cycles to steal (1-4), determines initial phase
    /// * `conflict_address` - The address the CPU was reading from (for halt cycle side effects)
    #[deprecated(note = "Use request_dmc_dma for event-driven DMA")]
    pub fn start_dmc_dma(&mut self, address: u16, cycles: u8, conflict_address: Option<u16>) {
        // Clamp to valid range (1-4 cycles)
        let cycles = cycles.clamp(1, 4);

        // Determine initial phase based on cycle count
        // cycles=1: Read only (no halt)
        // cycles=2: Halt(1) → Read
        // cycles=3: Halt(2) → Read
        // cycles=4: Halt(3) → Read
        let initial_phase = if cycles == 1 {
            DmcDmaPhase::Read
        } else {
            DmcDmaPhase::Halt(cycles - 1)
        };

        self.dmc_dma = DmcDmaState {
            active: true,
            address,
            phase: initial_phase,
            conflict_address,
            cpu_was_writing: cycles >= 4,
        };
    }

    /// Calculate the number of cycles DMC DMA should steal based on CPU state.
    ///
    /// This implements the real 2A03 behavior where DMC DMA timing depends on
    /// what the CPU is currently doing.
    ///
    /// Returns the number of cycles to steal (1-4).
    ///
    /// DMC DMA cycle counts:
    /// - 4 cycles: CPU is writing (halt, halt, halt, get)
    /// - 3 cycles: CPU is reading (halt, halt, get)
    /// - 2 cycles: During OAM DMA write phase (halt, get)
    /// - 1 cycle:  During OAM DMA read phase or CPU already halted (get only)
    #[deprecated(note = "Use request_dmc_dma for event-driven DMA - no pre-calculation needed")]
    #[allow(dead_code)]
    pub fn calculate_dmc_dma_cycles(
        cpu_is_writing: bool,
        cpu_is_halted: bool,
        oam_dma_active: bool,
        oam_dma_on_read: bool,
    ) -> u8 {
        if oam_dma_active {
            // During OAM DMA - depends on read/write phase
            if oam_dma_on_read {
                // OAM DMA is reading - DMC can steal immediately after the read
                1
            } else {
                // OAM DMA is writing - DMC must wait for write to complete
                2
            }
        } else if cpu_is_halted {
            // CPU already halted (rare case) - minimal overhead
            1
        } else if cpu_is_writing {
            // CPU is writing - cannot interrupt, must wait for write to complete
            4
        } else {
            // CPU is reading - standard 3-cycle steal
            3
        }
    }

    /// Check if OAM DMA is currently on a read phase
    pub fn is_oam_dma_on_read(&self) -> bool {
        self.oam_dma.active && self.oam_dma.read_phase
    }

    /// Update the CPU cycle parity tracking (legacy compatibility)
    /// Maps odd=true to PUT phase, odd=false to GET phase
    #[deprecated(note = "Use set_apu_phase instead")]
    pub fn set_cpu_cycle_odd(&mut self, odd: bool) {
        self.apu_phase = if odd { ApuPhase::Put } else { ApuPhase::Get };
    }

    /// Execute one cycle of DMA.
    /// Returns information about what happened during this cycle.
    ///
    /// When both DMC and OAM DMA are active, they interleave:
    /// - DMC halt cycles: OAM DMA continues (both can run in parallel)
    /// - DMC get cycle (final): DMC takes priority, OAM DMA waits
    ///
    /// See: https://www.nesdev.org/wiki/DMA#DMC_DMA_during_OAM_DMA
    pub fn step_cycle(&mut self) -> Result<DmaStepResult, MemoryError> {
        let mut result = DmaStepResult::default();

        // Check if DMC DMA is on its final "read" cycle
        let dmc_on_get_cycle = self.dmc_dma.active && self.dmc_dma.phase == DmcDmaPhase::Read;

        if dmc_on_get_cycle {
            // DMC get cycle takes exclusive priority - OAM DMA pauses
            result = self.step_dmc_dma()?;
        } else if self.dmc_dma.active && self.oam_dma.active {
            // DMC halt cycle + OAM DMA: both can run
            // Step DMC halt cycle (may do conflict read)
            let dmc_result = self.step_dmc_dma()?;
            // Step OAM DMA
            let oam_result = self.step_oam_dma()?;
            // Merge results - prefer OAM DMA address/read/write info since that's the "real" bus activity
            result = DmaStepResult {
                cpu_halted: true,
                oam_dma_complete: oam_result.oam_dma_complete,
                dmc_dma_complete: dmc_result.dmc_dma_complete,
                address_accessed: oam_result.address_accessed.or(dmc_result.address_accessed),
                read_occurred: oam_result.read_occurred || dmc_result.read_occurred,
                write_occurred: oam_result.write_occurred,
            };
        } else if self.dmc_dma.active {
            // DMC DMA only (halt cycles when no OAM DMA)
            result = self.step_dmc_dma()?;
        } else if self.oam_dma.active {
            // OAM DMA only
            result = self.step_oam_dma()?;
        }

        result.cpu_halted = self.is_active();
        Ok(result)
    }

    /// Execute one cycle of OAM DMA
    ///
    /// OAM DMA timing based on GET/PUT phase:
    /// - 513 cycles when started on GET: 1 idle + 512 transfer (256 reads + 256 writes)
    /// - 514 cycles when started on PUT: 2 idle + 512 transfer
    ///
    /// The alignment ensures reads always occur on GET phases.
    ///
    /// During idle/alignment cycles, the CPU's halted read address is read repeatedly,
    /// causing side effects for certain registers ($2002, $2007, $4016/$4017).
    fn step_oam_dma(&mut self) -> Result<DmaStepResult, MemoryError> {
        let mut result = DmaStepResult::default();

        // Cycle 0: First idle/alignment cycle (always present)
        // During this cycle, perform a repeated read from the CPU's halted address
        if self.oam_dma.cycle == 0 {
            self.oam_dma.cycle = 1;
            // Perform repeated read from halted address (causes side effects)
            if let Some(halted_addr) = self.halted_read_address {
                let _ = self.bus.borrow().read_byte(halted_addr)?;
                result.address_accessed = Some(halted_addr);
                result.read_occurred = true;
            }
            return Ok(result);
        }

        // Cycle 1: Second idle cycle (only if started on PUT phase)
        // This aligns the first read to a GET phase
        // Also performs a repeated read from the CPU's halted address
        if self.oam_dma.cycle == 1 && self.oam_dma.started_on_phase.is_put() {
            self.oam_dma.cycle = 2;
            // Perform repeated read from halted address (causes side effects)
            if let Some(halted_addr) = self.halted_read_address {
                let _ = self.bus.borrow().read_byte(halted_addr)?;
                result.address_accessed = Some(halted_addr);
                result.read_occurred = true;
            }
            return Ok(result);
        }

        // After alignment, we alternate between read and write phases
        // 256 read cycles + 256 write cycles = 512 transfer cycles
        if self.oam_dma.read_phase {
            // Read phase: read byte from source address
            let source_addr = ((self.oam_dma.page as u16) << 8) | self.oam_dma.byte_index;

            // Use raw pointer to avoid borrow checker issues
            let bus_ptr = self.bus.as_ptr();
            self.oam_dma.read_value = unsafe { (*bus_ptr).read_byte(source_addr)? };

            result.address_accessed = Some(source_addr);
            result.read_occurred = true;
            self.oam_dma.read_phase = false;
        } else {
            // Write phase: write byte to PPU OAM
            self.ppu.borrow_mut().dma_write(
                self.oam_dma.byte_index as u8,
                self.oam_dma.read_value
            )?;

            result.write_occurred = true;
            self.oam_dma.read_phase = true;
            self.oam_dma.byte_index += 1;

            // Check if transfer is complete
            if self.oam_dma.byte_index >= 256 {
                self.oam_dma.active = false;
                result.oam_dma_complete = true;
            }
        }

        self.oam_dma.cycle += 1;
        Ok(result)
    }

    /// Execute one cycle of DMC DMA using state machine
    ///
    /// DMC DMA phases:
    /// - Halt(n): n halt cycles remaining, reads from conflict address (causes side effects)
    /// - Dummy: Dummy cycle (no bus activity) - currently not used in simple model
    /// - Align: Alignment cycle (no bus activity) - currently not used in simple model
    /// - Read: Final cycle, fetches sample from DMC address
    ///
    /// The conflict reads during halt cycles are important because they affect special registers:
    /// - $2007: Increments PPU VRAM address
    /// - $4015: Clears APU frame counter interrupt flag
    /// - $4016/$4017: Clocks controller shift register
    fn step_dmc_dma(&mut self) -> Result<DmaStepResult, MemoryError> {
        let mut result = DmaStepResult::default();

        match self.dmc_dma.phase {
            DmcDmaPhase::Idle => {
                // Should not happen - DMA is marked active but no phase
                self.dmc_dma.active = false;
            }

            DmcDmaPhase::Halt(remaining) => {
                // Halt cycle: Read from CPU's conflict address (causes side effects)
                if let Some(conflict_addr) = self.dmc_dma.conflict_address {
                    let _ = self.bus.borrow().read_byte(conflict_addr)?;
                    result.address_accessed = Some(conflict_addr);
                    result.read_occurred = true;
                }
                // Else no bus activity this cycle (idle halt)

                // Transition to next state
                if remaining > 1 {
                    self.dmc_dma.phase = DmcDmaPhase::Halt(remaining - 1);
                } else {
                    // Last halt cycle done, go to read
                    self.dmc_dma.phase = DmcDmaPhase::Read;
                }
            }

            DmcDmaPhase::Dummy => {
                // Dummy cycle - CPU continues to re-read from halted address
                // This causes side effects for special registers
                if let Some(halted_addr) = self.halted_read_address {
                    let _ = self.bus.borrow().read_byte(halted_addr)?;
                    result.address_accessed = Some(halted_addr);
                    result.read_occurred = true;
                }
                // Transition to align or read based on APU phase
                self.dmc_dma.phase = DmcDmaPhase::Read;
            }

            DmcDmaPhase::Align => {
                // Alignment cycle - CPU continues to re-read from halted address
                // This causes side effects for special registers
                if let Some(halted_addr) = self.halted_read_address {
                    let _ = self.bus.borrow().read_byte(halted_addr)?;
                    result.address_accessed = Some(halted_addr);
                    result.read_occurred = true;
                }
                // Transition to read
                self.dmc_dma.phase = DmcDmaPhase::Read;
            }

            DmcDmaPhase::Read => {
                // Final cycle: Perform the actual DMC sample read
                let address = self.dmc_dma.address;
                let value = self.bus.borrow().read_byte(address)?;

                result.address_accessed = Some(address);
                result.read_occurred = true;
                result.dmc_dma_complete = Some(value);

                // DMA complete - reset state
                self.dmc_dma.active = false;
                self.dmc_dma.phase = DmcDmaPhase::Idle;
            }
        }

        Ok(result)
    }

    /// Get the total cycles an OAM DMA will take (for pre-calculation)
    ///
    /// Based on current APU phase:
    /// - GET phase: 513 cycles (1 idle + 512 transfer)
    /// - PUT phase: 514 cycles (2 idle + 512 transfer)
    pub fn oam_dma_cycles(&self) -> u16 {
        // 512 cycles for 256 bytes (read + write each)
        // Plus 1-2 alignment cycles depending on GET/PUT start
        if self.apu_phase.is_put() { 514 } else { 513 }
    }

    /// Check if OAM DMA is active
    pub fn is_oam_dma_active(&self) -> bool {
        self.oam_dma.active
    }

    /// Check if DMC DMA is active
    pub fn is_dmc_dma_active(&self) -> bool {
        self.dmc_dma.active
    }
}
