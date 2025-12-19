// Authorship: Human 0% | Claude 100%
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
    /// Address to fetch from
    pub address: u16,
    /// Cycles remaining for this DMC DMA
    pub cycles_remaining: u8,
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

    /// Start a DMC DMA fetch from the specified address
    pub fn start_dmc_dma(&mut self, address: u16) {
        // DMC DMA takes 1-4 cycles depending on CPU state
        // For simplicity, we use 4 cycles (can be refined later)
        self.dmc_dma = DmcDmaState {
            active: true,
            address,
            cycles_remaining: 4,
        };
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
    pub fn step_cycle(&mut self) -> Result<DmaStepResult, MemoryError> {
        let mut result = DmaStepResult::default();

        // DMC DMA has priority over OAM DMA if both are active
        if self.dmc_dma.active {
            result = self.step_dmc_dma()?;
        } else if self.oam_dma.active {
            result = self.step_oam_dma()?;
        }

        result.cpu_halted = self.is_active();
        Ok(result)
    }

    /// Execute one cycle of OAM DMA
    fn step_oam_dma(&mut self) -> Result<DmaStepResult, MemoryError> {
        let mut result = DmaStepResult::default();

        // Cycle 0: Alignment cycle (wait for put cycle if started on odd cycle)
        if self.oam_dma.cycle == 0 {
            self.oam_dma.cycle = 1;
            // If we started on an odd cycle, we have an extra alignment cycle
            if self.oam_dma.started_on_odd {
                return Ok(result);
            }
        }

        // After alignment, we alternate between read and write phases
        // Cycles 1-512 (or 2-513 if started on odd): actual transfer
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
    fn step_dmc_dma(&mut self) -> Result<DmaStepResult, MemoryError> {
        let mut result = DmaStepResult::default();

        self.dmc_dma.cycles_remaining -= 1;

        // On the last cycle, perform the actual read
        if self.dmc_dma.cycles_remaining == 0 {
            let address = self.dmc_dma.address;
            let value = self.bus.borrow().read_byte(address)?;

            result.address_accessed = Some(address);
            result.read_occurred = true;
            result.dmc_dma_complete = Some(value);

            self.dmc_dma.active = false;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::MockBusStub;
    use crate::dma_device::MockDmaDeviceStub;
    use mockall::predicate::*;

    fn create_controller() -> DmaController<MockBusStub, MockDmaDeviceStub> {
        let mut bus = MockBusStub::new();
        bus.expect_read_byte().returning(|addr| Ok((addr & 0xFF) as u8));

        let mut ppu = MockDmaDeviceStub::new();
        ppu.expect_dma_write().returning(|_, _| Ok(()));

        DmaController::new(
            Rc::new(RefCell::new(bus)),
            Rc::new(RefCell::new(ppu))
        )
    }

    #[test]
    fn test_dma_controller_starts_inactive() {
        let controller = create_controller();
        assert!(!controller.is_active());
        assert!(!controller.is_oam_dma_active());
        assert!(!controller.is_dmc_dma_active());
    }

    #[test]
    fn test_start_oam_dma_activates_controller() {
        let mut controller = create_controller();
        controller.start_oam_dma(0x02);
        assert!(controller.is_active());
        assert!(controller.is_oam_dma_active());
    }

    #[test]
    fn test_start_dmc_dma_activates_controller() {
        let mut controller = create_controller();
        controller.start_dmc_dma(0xC000);
        assert!(controller.is_active());
        assert!(controller.is_dmc_dma_active());
    }

    #[test]
    fn test_oam_dma_cycles_even_start() {
        let controller = create_controller();
        // Starting on even cycle: 513 cycles
        assert_eq!(controller.oam_dma_cycles(), 513);
    }

    #[test]
    fn test_oam_dma_cycles_odd_start() {
        let mut controller = create_controller();
        controller.set_cpu_cycle_odd(true);
        // Starting on odd cycle: 514 cycles
        assert_eq!(controller.oam_dma_cycles(), 514);
    }

    #[test]
    fn test_reset_clears_dma_state() {
        let mut controller = create_controller();
        controller.start_oam_dma(0x02);
        controller.start_dmc_dma(0xC000);
        assert!(controller.is_active());

        controller.reset();
        assert!(!controller.is_active());
    }
}
