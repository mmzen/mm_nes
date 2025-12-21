// Authorship: Human 0% | Claude 100%
//! Tests for DmaController
//!
//! Tests OAM DMA (513/514 cycles) and DMC DMA (1-4 cycles) cycle-accurate behavior.
//! Also tests GET/PUT APU phase tracking for DMA alignment.

use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::MockBusStub;
use crate::dma_controller::{ApuPhase, DmaController};
use crate::dma_device::MockDmaDeviceStub;
use crate::tests::init;

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
    init();
    let controller = create_controller();
    assert!(!controller.is_active());
    assert!(!controller.is_oam_dma_active());
    assert!(!controller.is_dmc_dma_active());
}

#[test]
fn test_start_oam_dma_activates_controller() {
    init();
    let mut controller = create_controller();
    controller.start_oam_dma(0x02);
    assert!(controller.is_active());
    assert!(controller.is_oam_dma_active());
}

#[test]
fn test_start_dmc_dma_activates_controller() {
    init();
    let mut controller = create_controller();
    controller.start_dmc_dma(0xC000, 4, None);
    assert!(controller.is_active());
    assert!(controller.is_dmc_dma_active());
}

#[test]
fn test_calculate_dmc_dma_cycles_cpu_writing() {
    init();
    // CPU is writing - 4 cycles
    let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(true, false, false, false);
    assert_eq!(cycles, 4);
}

#[test]
fn test_calculate_dmc_dma_cycles_cpu_reading() {
    init();
    // CPU is reading - 3 cycles
    let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(false, false, false, false);
    assert_eq!(cycles, 3);
}

#[test]
fn test_calculate_dmc_dma_cycles_oam_dma_write_phase() {
    init();
    // OAM DMA is active on write phase - 2 cycles
    let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(true, false, true, false);
    assert_eq!(cycles, 2);
    let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(false, false, true, false);
    assert_eq!(cycles, 2);
}

#[test]
fn test_calculate_dmc_dma_cycles_oam_dma_read_phase() {
    init();
    // OAM DMA is active on read phase - 1 cycle
    let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(true, false, true, true);
    assert_eq!(cycles, 1);
    let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(false, false, true, true);
    assert_eq!(cycles, 1);
}

#[test]
fn test_calculate_dmc_dma_cycles_cpu_halted() {
    init();
    // CPU is halted (not OAM DMA) - 1 cycle
    let cycles = DmaController::<MockBusStub, MockDmaDeviceStub>::calculate_dmc_dma_cycles(false, true, false, false);
    assert_eq!(cycles, 1);
}

#[test]
fn test_oam_dma_cycles_get_phase_start() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Get);
    // Starting on GET phase: 513 cycles (1 idle + 512 transfer)
    assert_eq!(controller.oam_dma_cycles(), 513);
}

#[test]
fn test_oam_dma_cycles_put_phase_start() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Put);
    // Starting on PUT phase: 514 cycles (2 idle + 512 transfer)
    assert_eq!(controller.oam_dma_cycles(), 514);
}

#[test]
fn test_reset_clears_dma_state() {
    init();
    let mut controller = create_controller();
    controller.start_oam_dma(0x02);
    controller.start_dmc_dma(0xC000, 4, None);
    assert!(controller.is_active());

    controller.reset();
    assert!(!controller.is_active());
}

#[test]
fn test_oam_dma_completes_in_513_cycles_get_phase_start() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Get);
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

    assert_eq!(total_cycles, 513, "OAM DMA should take 513 cycles when started on GET phase");
    assert_eq!(read_count, 256, "Should have 256 reads");
    assert_eq!(write_count, 256, "Should have 256 writes");
}

#[test]
fn test_oam_dma_completes_in_514_cycles_put_phase_start() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Put);
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

    assert_eq!(total_cycles, 514, "OAM DMA should take 514 cycles when started on PUT phase");
}

#[test]
fn test_oam_dma_reads_from_correct_addresses() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Get); // GET phase start

    // Start DMA from page 0x02 (addresses 0x0200-0x02FF)
    controller.start_oam_dma(0x02);

    // Cycle 0: First idle cycle (GET start has only 1 idle)
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
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Get); // GET phase start
    controller.start_oam_dma(0x00);

    // Skip the 1 idle cycle for GET phase start
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
fn test_oam_dma_put_phase_start_has_two_idle_cycles() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Put); // PUT phase start
    controller.start_oam_dma(0x00);

    // First idle cycle
    let result = controller.step_cycle().unwrap();
    assert!(!result.read_occurred && !result.write_occurred, "First idle cycle");

    // Second idle cycle (only on PUT phase start - aligns to GET)
    let result = controller.step_cycle().unwrap();
    assert!(!result.read_occurred && !result.write_occurred, "Second idle cycle");

    // Now the first read (occurs on GET phase)
    let result = controller.step_cycle().unwrap();
    assert!(result.read_occurred, "Third cycle should be first read");
}

#[test]
fn test_dmc_dma_bus_conflict_reads_from_conflict_address() {
    init();
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
    init();
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

// ============================================================================
// GET/PUT APU Phase Tests
// ============================================================================

#[test]
fn test_apu_phase_toggle() {
    init();
    let mut controller = create_controller();

    // Start at GET
    controller.set_apu_phase(ApuPhase::Get);
    assert_eq!(controller.get_apu_phase(), ApuPhase::Get);

    // Toggle to PUT
    controller.toggle_apu_phase();
    assert_eq!(controller.get_apu_phase(), ApuPhase::Put);

    // Toggle back to GET
    controller.toggle_apu_phase();
    assert_eq!(controller.get_apu_phase(), ApuPhase::Get);
}

#[test]
fn test_apu_phase_is_get_is_put() {
    init();

    assert!(ApuPhase::Get.is_get());
    assert!(!ApuPhase::Get.is_put());

    assert!(!ApuPhase::Put.is_get());
    assert!(ApuPhase::Put.is_put());
}

#[test]
fn test_apu_phase_preserved_across_reset() {
    init();
    let mut controller = create_controller();

    // Set phase to PUT
    controller.set_apu_phase(ApuPhase::Put);
    assert_eq!(controller.get_apu_phase(), ApuPhase::Put);

    // Start some DMA
    controller.start_oam_dma(0x02);
    assert!(controller.is_active());

    // Reset
    controller.reset();

    // DMA should be cleared
    assert!(!controller.is_active());

    // But APU phase should be preserved (only randomized on power-on)
    assert_eq!(controller.get_apu_phase(), ApuPhase::Put);
}

#[test]
fn test_oam_dma_uses_current_apu_phase_for_alignment() {
    init();
    let mut controller = create_controller();

    // Test GET phase start
    controller.set_apu_phase(ApuPhase::Get);
    controller.start_oam_dma(0x02);

    let mut cycles_get = 0;
    while controller.is_active() {
        let _ = controller.step_cycle().unwrap();
        cycles_get += 1;
        if cycles_get > 600 { panic!("Too many cycles"); }
    }
    assert_eq!(cycles_get, 513, "GET phase start should take 513 cycles");

    // Test PUT phase start
    controller.set_apu_phase(ApuPhase::Put);
    controller.start_oam_dma(0x02);

    let mut cycles_put = 0;
    while controller.is_active() {
        let _ = controller.step_cycle().unwrap();
        cycles_put += 1;
        if cycles_put > 600 { panic!("Too many cycles"); }
    }
    assert_eq!(cycles_put, 514, "PUT phase start should take 514 cycles");
}

#[test]
fn test_halted_read_address_tracking() {
    init();
    let mut controller = create_controller();

    // Initially no halted address
    assert!(controller.get_halted_read_address().is_none());

    // Set halted address
    controller.set_halted_read_address(Some(0x2002));
    assert_eq!(controller.get_halted_read_address(), Some(0x2002));

    // Reset should clear it
    controller.reset();
    assert!(controller.get_halted_read_address().is_none());
}

#[test]
fn test_oam_dma_idle_cycle_performs_repeated_read() {
    init();
    // Create a controller with a bus
    let mut bus = MockBusStub::new();
    bus.expect_read_byte().returning(|addr| {
        Ok((addr & 0xFF) as u8)
    });

    let mut ppu = MockDmaDeviceStub::new();
    ppu.expect_dma_write().returning(|_, _| Ok(()));

    let mut controller = DmaController::new(
        Rc::new(RefCell::new(bus)),
        Rc::new(RefCell::new(ppu))
    );

    // Set the halted read address (simulating CPU was reading from $2002)
    controller.set_halted_read_address(Some(0x2002));
    controller.set_apu_phase(ApuPhase::Get);
    controller.start_oam_dma(0x02);

    // First cycle (idle) should read from the halted address
    let result = controller.step_cycle().unwrap();
    assert!(result.read_occurred, "Idle cycle should perform repeated read");
    assert_eq!(result.address_accessed, Some(0x2002), "Should read from halted address");
}

#[test]
fn test_oam_dma_put_phase_both_idle_cycles_perform_repeated_reads() {
    init();
    let mut bus = MockBusStub::new();
    bus.expect_read_byte().returning(|addr| Ok((addr & 0xFF) as u8));

    let mut ppu = MockDmaDeviceStub::new();
    ppu.expect_dma_write().returning(|_, _| Ok(()));

    let mut controller = DmaController::new(
        Rc::new(RefCell::new(bus)),
        Rc::new(RefCell::new(ppu))
    );

    // Set halted address and start on PUT phase (needs 2 idle cycles)
    controller.set_halted_read_address(Some(0x4016));  // Controller read
    controller.set_apu_phase(ApuPhase::Put);
    controller.start_oam_dma(0x02);

    // First idle cycle
    let result1 = controller.step_cycle().unwrap();
    assert!(result1.read_occurred, "First idle cycle should read");
    assert_eq!(result1.address_accessed, Some(0x4016));

    // Second idle cycle (PUT phase alignment)
    let result2 = controller.step_cycle().unwrap();
    assert!(result2.read_occurred, "Second idle cycle should read");
    assert_eq!(result2.address_accessed, Some(0x4016));

    // Third cycle should be actual DMA read
    let result3 = controller.step_cycle().unwrap();
    assert!(result3.read_occurred);
    assert_eq!(result3.address_accessed, Some(0x0200), "Should read from source");
}

#[test]
fn test_oam_dma_no_halted_address_idle_cycles_are_quiet() {
    init();
    let mut bus = MockBusStub::new();
    bus.expect_read_byte().returning(|addr| Ok((addr & 0xFF) as u8));

    let mut ppu = MockDmaDeviceStub::new();
    ppu.expect_dma_write().returning(|_, _| Ok(()));

    let mut controller = DmaController::new(
        Rc::new(RefCell::new(bus)),
        Rc::new(RefCell::new(ppu))
    );

    // No halted address set
    controller.set_apu_phase(ApuPhase::Get);
    controller.start_oam_dma(0x02);

    // Idle cycle without halted address should not read
    let result = controller.step_cycle().unwrap();
    assert!(!result.read_occurred, "Idle without halted address should not read");
    assert!(result.address_accessed.is_none());
}
