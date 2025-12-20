// Authorship: Human 0% | Claude 100%
// Updated: Fixed OAM DMA alignment logic (1 idle for even, 2 for odd), added comprehensive tests
// Updated: Added DMC DMA bus conflict behavior - halt cycles read from CPU's conflict address
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
        controller.start_dmc_dma(0xC000, 4, None);
        assert!(controller.is_active());
        assert!(controller.is_dmc_dma_active());
    }

    #[test]
    fn test_calculate_dmc_dma_cycles_cpu_writing() {
        // CPU is writing - 4 cycles
        let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(true, false, false, false);
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_calculate_dmc_dma_cycles_cpu_reading() {
        // CPU is reading - 3 cycles
        let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(false, false, false, false);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_calculate_dmc_dma_cycles_oam_dma_write_phase() {
        // OAM DMA is active on write phase - 2 cycles
        let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(true, false, true, false);
        assert_eq!(cycles, 2);
        let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(false, false, true, false);
        assert_eq!(cycles, 2);
    }

    #[test]
    fn test_calculate_dmc_dma_cycles_oam_dma_read_phase() {
        // OAM DMA is active on read phase - 1 cycle
        let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(true, false, true, true);
        assert_eq!(cycles, 1);
        let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(false, false, true, true);
        assert_eq!(cycles, 1);
    }

    #[test]
    fn test_calculate_dmc_dma_cycles_cpu_halted() {
        // CPU is halted (not OAM DMA) - 1 cycle
        let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(false, true, false, false);
        assert_eq!(cycles, 1);
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
        controller.start_dmc_dma(0xC000, 4, None);
        assert!(controller.is_active());

        controller.reset();
        assert!(!controller.is_active());
    }

    #[test]
    fn test_oam_dma_completes_in_513_cycles_even_start() {
        let mut controller = create_controller();
        controller.set_cpu_cycle_odd(false);
        controller.start_oam_dma(0x02);

        let mut total_cycles = 0;
        let mut read_count = 0;
        let mut write_count = 0;

        while controller.is_active() {
            let result = controller.step_cycle().unwrap();
            total_cycles += 1;

            if result.read_occurred {
                read_count += 1;
            }
            if result.write_occurred {
                write_count += 1;
            }

            // Safety: prevent infinite loop
            if total_cycles > 600 {
                panic!("OAM DMA took too long");
            }
        }

        assert_eq!(total_cycles, 513, "OAM DMA should take 513 cycles on even start");
        assert_eq!(read_count, 256, "Should have 256 reads");
        assert_eq!(write_count, 256, "Should have 256 writes");
    }

    #[test]
    fn test_oam_dma_completes_in_514_cycles_odd_start() {
        let mut controller = create_controller();
        controller.set_cpu_cycle_odd(true);
        controller.start_oam_dma(0x02);

        let mut total_cycles = 0;

        while controller.is_active() {
            let _ = controller.step_cycle().unwrap();
            total_cycles += 1;

            // Safety: prevent infinite loop
            if total_cycles > 600 {
                panic!("OAM DMA took too long");
            }
        }

        assert_eq!(total_cycles, 514, "OAM DMA should take 514 cycles on odd start");
    }

    #[test]
    fn test_oam_dma_reads_from_correct_addresses() {
        let mut controller = create_controller();
        controller.set_cpu_cycle_odd(false); // Even start

        // Start DMA from page 0x02 (addresses 0x0200-0x02FF)
        controller.start_oam_dma(0x02);

        // Cycle 0: First idle cycle (even start has only 1 idle)
        let result = controller.step_cycle().unwrap();
        assert!(!result.read_occurred && !result.write_occurred, "Idle cycle");

        // Cycle 1: First read should be from 0x0200
        let result = controller.step_cycle().unwrap();
        assert!(result.read_occurred, "First op should be a read");
        assert_eq!(result.address_accessed, Some(0x0200), "First read from 0x0200");

        // Cycle 2: Write byte 0 to OAM
        let result = controller.step_cycle().unwrap();
        assert!(result.write_occurred, "Second op should be a write");

        // Cycle 3: Read from 0x0201
        let result = controller.step_cycle().unwrap();
        assert!(result.read_occurred, "Third op should be a read");
        assert_eq!(result.address_accessed, Some(0x0201), "Second read from 0x0201");
    }

    #[test]
    fn test_oam_dma_alternates_read_write() {
        let mut controller = create_controller();
        controller.set_cpu_cycle_odd(false); // Even start
        controller.start_oam_dma(0x00);

        // Skip the 1 idle cycle for even start
        let result = controller.step_cycle().unwrap();
        assert!(!result.read_occurred && !result.write_occurred, "Should be idle cycle");

        // Check first 10 transfer operations alternate between read and write
        for i in 0..10 {
            let result = controller.step_cycle().unwrap();
            if i % 2 == 0 {
                assert!(result.read_occurred, "Cycle {} should be read", i);
                assert!(!result.write_occurred, "Cycle {} should not be write", i);
            } else {
                assert!(result.write_occurred, "Cycle {} should be write", i);
                assert!(!result.read_occurred, "Cycle {} should not be read", i);
            }
        }
    }

    #[test]
    fn test_oam_dma_odd_start_has_two_idle_cycles() {
        let mut controller = create_controller();
        controller.set_cpu_cycle_odd(true); // Odd start
        controller.start_oam_dma(0x00);

        // First idle cycle
        let result = controller.step_cycle().unwrap();
        assert!(!result.read_occurred && !result.write_occurred, "First idle cycle");

        // Second idle cycle (only on odd start)
        let result = controller.step_cycle().unwrap();
        assert!(!result.read_occurred && !result.write_occurred, "Second idle cycle");

        // Now the first read
        let result = controller.step_cycle().unwrap();
        assert!(result.read_occurred, "Third cycle should be first read");
    }

    #[test]
    fn test_dmc_dma_bus_conflict_reads_from_conflict_address() {
        let mut controller = create_controller();

        // Start DMC DMA with 4 cycles and a conflict address of 0x2007
        controller.start_dmc_dma(0xC000, 4, Some(0x2007));

        // Cycle 1, 2, 3: Halt cycles should read from conflict address (0x2007)
        for i in 0..3 {
            let result = controller.step_cycle().unwrap();
            assert!(result.read_occurred, "Halt cycle {} should perform read", i);
            assert_eq!(result.address_accessed, Some(0x2007), "Halt cycle {} should read from conflict address", i);
            assert!(result.dmc_dma_complete.is_none(), "Halt cycle {} should not complete DMA", i);
        }

        // Cycle 4: Final cycle should read from DMC sample address (0xC000)
        let result = controller.step_cycle().unwrap();
        assert!(result.read_occurred, "Final cycle should perform read");
        assert_eq!(result.address_accessed, Some(0xC000), "Final cycle should read from DMC address");
        assert!(result.dmc_dma_complete.is_some(), "Final cycle should complete DMA");

        // DMA should now be inactive
        assert!(!controller.is_dmc_dma_active());
    }

    #[test]
    fn test_dmc_dma_without_conflict_address_has_idle_halt_cycles() {
        let mut controller = create_controller();

        // Start DMC DMA with 4 cycles but no conflict address
        controller.start_dmc_dma(0xC000, 4, None);

        // Cycle 1, 2, 3: Halt cycles should be idle (no bus activity)
        for i in 0..3 {
            let result = controller.step_cycle().unwrap();
            assert!(!result.read_occurred, "Halt cycle {} should NOT perform read without conflict address", i);
            assert!(result.address_accessed.is_none(), "Halt cycle {} should have no address", i);
            assert!(result.dmc_dma_complete.is_none(), "Halt cycle {} should not complete DMA", i);
        }

        // Cycle 4: Final cycle should read from DMC sample address
        let result = controller.step_cycle().unwrap();
        assert!(result.read_occurred, "Final cycle should perform read");
        assert_eq!(result.address_accessed, Some(0xC000), "Final cycle should read from DMC address");
        assert!(result.dmc_dma_complete.is_some(), "Final cycle should complete DMA");
    }
}
