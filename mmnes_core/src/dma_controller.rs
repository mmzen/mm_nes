// Authorship: Human 0% | Claude 100%
// Updated: Fixed OAM DMA alignment logic (1 idle for even, 2 for odd), added comprehensive tests
// Updated: Added DMC DMA bus conflict behavior - halt cycles read from CPU's conflict address
// Updated: Fixed DMC+OAM DMA interleaving - halt cycles run alongside OAM, only get cycle has priority
//! DMA Controller for cycle-accurate OAM DMA and DMC DMA handling.
//!
//! This module provides a centralized DMA controller that manages both:
//! - OAM DMA: Transfers 256 bytes from CPU memory to PPU OAM (513-514 cycles)
//! - DMC DMA: Fetches sample bytes for the DMC channel (1-4 cycles)
//!
//! The controller works with the CPU's cycle-stepping state machine to properly
//! halt the CPU during DMA operations.

use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use crate::bus::Bus;
use crate::dma_device::DmaDevice;
use crate::memory::MemoryError;

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
    /// Whether we started on an odd CPU cycle (affects alignment)
    pub started_on_odd: bool,
}

/// State of DMC DMA fetch
#[derive(Debug, Clone, Default)]
pub struct DmcDmaState {
    /// Whether DMC DMA is currently pending/active
    pub active: bool,
    /// Address to fetch the DMC sample from
    pub address: u16,
    /// Cycles remaining for this DMC DMA
    pub cycles_remaining: u8,
    /// Address the CPU was accessing when DMC DMA started (for bus conflict reads)
    /// During halt cycles, reads from this address cause side effects
    pub conflict_address: Option<u16>,
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
    /// Tracks whether current CPU cycle is odd (for OAM DMA alignment)
    cpu_cycle_odd: bool,
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
            cpu_cycle_odd: false,
        }
    }

    /// Reset the DMA controller state
    pub fn reset(&mut self) {
        self.oam_dma = OamDmaState::default();
        self.dmc_dma = DmcDmaState::default();
        self.cpu_cycle_odd = false;
    }

    /// Check if any DMA is currently active
    pub fn is_active(&self) -> bool {
        self.oam_dma.active || self.dmc_dma.active
    }

    /// Start an OAM DMA transfer from the specified page
    pub fn start_oam_dma(&mut self, page: u8) {
        self.oam_dma = OamDmaState {
            active: true,
            page,
            byte_index: 0,
            cycle: 0,
            read_phase: true,
            read_value: 0,
            started_on_odd: self.cpu_cycle_odd,
        };
    }

    /// Start a DMC DMA fetch from the specified address.
    ///
    /// The cycle count depends on the CPU's current bus activity:
    /// - 4 cycles if CPU is writing (cannot interrupt mid-write)
    /// - 3 cycles if CPU is reading
    /// - 2 cycles if OAM DMA is in progress
    /// - 1 cycle if CPU is already halted
    ///
    /// # Arguments
    /// * `address` - The address to fetch the sample byte from
    /// * `cycles` - Number of cycles to steal (1-4), determined by caller based on CPU state
    /// * `conflict_address` - The address the CPU was reading from (for halt cycle side effects)
    pub fn start_dmc_dma(&mut self, address: u16, cycles: u8, conflict_address: Option<u16>) {
        // Clamp to valid range (1-4 cycles)
        let cycles = cycles.clamp(1, 4);
        self.dmc_dma = DmcDmaState {
            active: true,
            address,
            cycles_remaining: cycles,
            conflict_address,
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

    /// Update the CPU cycle parity tracking
    pub fn set_cpu_cycle_odd(&mut self, odd: bool) {
        self.cpu_cycle_odd = odd;
    }

    /// Toggle the CPU cycle parity
    pub fn toggle_cpu_cycle_parity(&mut self) {
        self.cpu_cycle_odd = !self.cpu_cycle_odd;
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

        // Check if DMC DMA is on its final "get" cycle (cycles_remaining == 1)
        let dmc_on_get_cycle = self.dmc_dma.active && self.dmc_dma.cycles_remaining == 1;

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
    /// OAM DMA timing (from NESDev):
    /// - 513 cycles on even CPU cycle start: 1 idle + 512 transfer (256 reads + 256 writes)
    /// - 514 cycles on odd CPU cycle start: 2 idle + 512 transfer
    fn step_oam_dma(&mut self) -> Result<DmaStepResult, MemoryError> {
        let mut result = DmaStepResult::default();

        // Cycle 0: First idle/alignment cycle (always present)
        if self.oam_dma.cycle == 0 {
            self.oam_dma.cycle = 1;
            return Ok(result);
        }

        // Cycle 1: Second idle cycle (only if started on odd CPU cycle)
        if self.oam_dma.cycle == 1 && self.oam_dma.started_on_odd {
            self.oam_dma.cycle = 2;
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

    /// Execute one cycle of DMC DMA
    ///
    /// During DMC DMA, the CPU is halted by pulling RDY low. The sequence is:
    /// - Halt cycles (all but last): Read from CPU's conflict address (causes side effects)
    /// - Final cycle: Read from DMC sample address
    ///
    /// The conflict reads are important because they affect special registers:
    /// - $2007: Increments PPU VRAM address
    /// - $4015: Clears APU frame counter interrupt flag
    /// - $4016/$4017: Clocks controller shift register
    fn step_dmc_dma(&mut self) -> Result<DmaStepResult, MemoryError> {
        let mut result = DmaStepResult::default();

        self.dmc_dma.cycles_remaining -= 1;

        if self.dmc_dma.cycles_remaining == 0 {
            // Final cycle: Perform the actual DMC sample read
            let address = self.dmc_dma.address;
            let value = self.bus.borrow().read_byte(address)?;

            result.address_accessed = Some(address);
            result.read_occurred = true;
            result.dmc_dma_complete = Some(value);

            self.dmc_dma.active = false;
        } else if let Some(conflict_addr) = self.dmc_dma.conflict_address {
            // Halt cycle: Read from CPU's conflict address (causes side effects)
            // This is the "bus conflict" behavior where the CPU's read keeps happening
            let _ = self.bus.borrow().read_byte(conflict_addr)?;

            result.address_accessed = Some(conflict_addr);
            result.read_occurred = true;
        }
        // If no conflict address, this is an idle halt cycle (no bus activity)

        Ok(result)
    }

    /// Get the total cycles an OAM DMA will take (for pre-calculation)
    pub fn oam_dma_cycles(&self) -> u16 {
        // 512 cycles for 256 bytes (read + write each)
        // Plus 1-2 alignment cycles depending on odd/even start
        if self.cpu_cycle_odd { 514 } else { 513 }
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
