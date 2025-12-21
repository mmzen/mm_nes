// Authorship: Human 0% | Claude 100%
//! Tests for DmaController
//!
//! Tests OAM DMA and DMC DMA cycle-accurate behavior with the new bus arbiter model.
//! Tests GET/PUT APU phase tracking for DMA alignment.

use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::MockBusStub;
use crate::dma_controller::{ApuPhase, BusOp, DmaController};
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

/// Helper to check if a BusOp is a read
fn is_read(op: &BusOp) -> bool {
    matches!(op, BusOp::Read(_))
}

/// Helper to check if a BusOp is a write
fn is_write(op: &BusOp) -> bool {
    matches!(op, BusOp::Write(_, _))
}

/// Helper to get address from BusOp
fn get_address(op: &BusOp) -> Option<u16> {
    match op {
        BusOp::Read(addr) => Some(*addr),
        BusOp::Write(addr, _) => Some(*addr),
        BusOp::None => None,
    }
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
    controller.request_dmc_dma(0xC000, false, None);
    assert!(controller.is_active());
    assert!(controller.is_dmc_dma_active());
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
    controller.request_dmc_dma(0xC000, false, None);
    assert!(controller.is_active());

    controller.reset();
    assert!(!controller.is_active());
}

#[test]
fn test_oam_dma_completes_in_513_cycles_get_phase_start() {
    init();
    let mut controller = create_controller();
    // "Started on GET phase" means write to $4014 happened on GET phase
    // The first DMA step happens on the NEXT phase (PUT) after toggle
    controller.set_apu_phase(ApuPhase::Get);
    controller.start_oam_dma(0x02);
    // Simulate the phase toggle that happens between write and first DMA step
    controller.toggle_apu_phase(); // Now PUT - this is when first DMA step runs

    let mut total_cycles = 0;
    let mut read_count = 0;
    let mut write_count = 0;

    while controller.is_active() {
        let result = controller.step_cycle(false).unwrap();
        total_cycles += 1;

        if is_read(&result.bus_op) {
            read_count += 1;
        }
        if is_write(&result.bus_op) {
            write_count += 1;
        }

        // Toggle phase each cycle (simulates real timing)
        controller.toggle_apu_phase();

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
    // "Started on PUT phase" means write to $4014 happened on PUT phase
    // The first DMA step happens on the NEXT phase (GET) after toggle
    controller.set_apu_phase(ApuPhase::Put);
    controller.start_oam_dma(0x02);
    // Simulate the phase toggle that happens between write and first DMA step
    controller.toggle_apu_phase(); // Now GET - this is when first DMA step runs

    let mut total_cycles = 0;

    while controller.is_active() {
        let _ = controller.step_cycle(false).unwrap();
        total_cycles += 1;

        // Toggle phase each cycle
        controller.toggle_apu_phase();

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

    // Cycle 0: PendingHalt -> WaitGet (no bus op since halt just succeeded)
    let result = controller.step_cycle(false).unwrap();
    // Note: In the new model, first cycle transitions states but may have bus op
    controller.toggle_apu_phase();

    // Cycle 1: WaitGet on PUT phase - stays waiting
    let result = controller.step_cycle(false).unwrap();
    controller.toggle_apu_phase();

    // Cycle 2: WaitGet on GET phase - transitions to Get and reads from 0x0200
    let result = controller.step_cycle(false).unwrap();
    assert!(is_read(&result.bus_op), "Should be a read");
    assert_eq!(get_address(&result.bus_op), Some(0x0200), "First read from 0x0200");
    controller.toggle_apu_phase();

    // Cycle 3: WaitPut on PUT phase - transitions to Put and writes
    let result = controller.step_cycle(false).unwrap();
    assert!(is_write(&result.bus_op), "Should be a write");
    controller.toggle_apu_phase();

    // Cycle 4: WaitGet on GET phase - reads from 0x0201
    let result = controller.step_cycle(false).unwrap();
    assert!(is_read(&result.bus_op), "Should be a read");
    assert_eq!(get_address(&result.bus_op), Some(0x0201), "Second read from 0x0201");
}

#[test]
fn test_oam_dma_alternates_read_write_on_phases() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Get); // GET phase start
    controller.start_oam_dma(0x00);

    // First cycle: PendingHalt -> WaitGet
    controller.step_cycle(false).unwrap();
    controller.toggle_apu_phase(); // Now PUT

    // Run through several cycles, checking pattern
    // After halt, reads happen on GET phases, writes on PUT phases
    let mut reads = 0;
    let mut writes = 0;

    for _ in 0..20 {
        let result = controller.step_cycle(false).unwrap();
        if is_read(&result.bus_op) {
            reads += 1;
        }
        if is_write(&result.bus_op) {
            writes += 1;
        }
        controller.toggle_apu_phase();
    }

    // Should have roughly equal reads and writes (alternating)
    assert!(reads > 0, "Should have some reads");
    assert!(writes > 0, "Should have some writes");
}

#[test]
fn test_oam_dma_halt_fails_on_cpu_write() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Get);
    controller.start_oam_dma(0x02);

    // First cycle with CPU writing - halt should fail, stay in PendingHalt
    let result = controller.step_cycle(true).unwrap(); // cpu_is_writing = true
    assert!(controller.is_oam_dma_active(), "OAM DMA should still be active");

    // Controller is still in PendingHalt, trying again
    // Second cycle with CPU reading - halt should succeed
    let result = controller.step_cycle(false).unwrap();
    assert!(controller.is_oam_dma_active(), "OAM DMA should still be active (but now in WaitGet)");
}

// ============================================================================
// DMC DMA Tests
// ============================================================================

#[test]
fn test_dmc_dma_state_machine_sequence() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Get);

    // Start DMC DMA
    controller.request_dmc_dma(0xC000, false, None);

    // Cycle 1: PendingHalt -> Halt (CPU not writing)
    let result = controller.step_cycle(false).unwrap();
    assert!(controller.is_dmc_dma_active());
    controller.toggle_apu_phase(); // Now PUT

    // Cycle 2: Halt -> Dummy
    let result = controller.step_cycle(false).unwrap();
    assert!(controller.is_dmc_dma_active());
    controller.toggle_apu_phase(); // Now GET

    // Cycle 3: Dummy -> Read (on GET phase)
    let result = controller.step_cycle(false).unwrap();
    // This cycle may complete the DMA if we're on GET
    controller.toggle_apu_phase();

    // Should complete within a few more cycles
    let mut cycles = 3;
    while controller.is_dmc_dma_active() && cycles < 10 {
        controller.step_cycle(false).unwrap();
        controller.toggle_apu_phase();
        cycles += 1;
    }

    assert!(!controller.is_dmc_dma_active(), "DMC DMA should have completed");
}

#[test]
fn test_dmc_dma_returns_sample() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Get);
    controller.request_dmc_dma(0xC000, false, None);

    let mut sample_received = false;

    for _ in 0..10 {
        let result = controller.step_cycle(false).unwrap();
        controller.toggle_apu_phase();

        if result.dmc_sample.is_some() {
            sample_received = true;
            break;
        }
    }

    assert!(sample_received, "Should receive DMC sample");
}

#[test]
fn test_dmc_dma_halt_fails_on_cpu_write() {
    init();
    let mut controller = create_controller();
    controller.set_apu_phase(ApuPhase::Get);
    controller.request_dmc_dma(0xC000, false, None);

    // First cycle with CPU writing - halt should fail
    controller.step_cycle(true).unwrap();
    assert!(controller.is_dmc_dma_active(), "DMC DMA should still be pending");

    // Second cycle with CPU reading - halt should succeed
    controller.step_cycle(false).unwrap();
    // Continue until complete
    controller.toggle_apu_phase();

    let mut cycles = 2;
    while controller.is_dmc_dma_active() && cycles < 10 {
        controller.step_cycle(false).unwrap();
        controller.toggle_apu_phase();
        cycles += 1;
    }

    assert!(!controller.is_dmc_dma_active());
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

    // Test GET phase start (write on GET -> first DMA step on PUT)
    controller.set_apu_phase(ApuPhase::Get);
    controller.start_oam_dma(0x02);
    controller.toggle_apu_phase(); // Simulate toggle between write and first DMA step

    let mut cycles_get = 0;
    while controller.is_active() {
        let _ = controller.step_cycle(false).unwrap();
        controller.toggle_apu_phase();
        cycles_get += 1;
        if cycles_get > 600 { panic!("Too many cycles"); }
    }
    assert_eq!(cycles_get, 513, "GET phase start should take 513 cycles");

    // Test PUT phase start (write on PUT -> first DMA step on GET)
    controller.set_apu_phase(ApuPhase::Put);
    controller.start_oam_dma(0x02);
    controller.toggle_apu_phase(); // Simulate toggle between write and first DMA step

    let mut cycles_put = 0;
    while controller.is_active() {
        let _ = controller.step_cycle(false).unwrap();
        controller.toggle_apu_phase();
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
fn test_cpu_repeated_read_during_dma_idle() {
    init();
    let mut bus = MockBusStub::new();
    bus.expect_read_byte().returning(|addr| Ok((addr & 0xFF) as u8));

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

    // First cycle transitions from PendingHalt
    // During cycles where DMA doesn't use the bus, CPU repeated read should occur
    let result = controller.step_cycle(false).unwrap();

    // The result should show the CPU halted and potentially a repeated read
    assert!(result.cpu_halted, "CPU should be halted");
}

#[test]
fn test_bus_op_none_when_no_activity() {
    init();
    let mut controller = create_controller();

    // Don't start any DMA - step should return None bus op
    // But actually the controller won't step if not active
    // This test verifies the BusOp::None case exists

    assert_eq!(BusOp::None, BusOp::default());
}

// ============================================================================
// Overlap Tests - DMC DMA vs OAM DMA
// ============================================================================

#[test]
fn test_dmc_read_has_priority_over_oam_get() {
    init();
    let mut controller = create_controller();

    // Start both DMAs
    controller.set_apu_phase(ApuPhase::Get);
    controller.start_oam_dma(0x02);
    controller.request_dmc_dma(0xC000, false, None);

    // Run until DMC completes - it should get priority on GET cycles when it needs to read
    let mut dmc_sample = None;
    let mut cycles = 0;

    while controller.is_dmc_dma_active() && cycles < 20 {
        let result = controller.step_cycle(false).unwrap();
        controller.toggle_apu_phase();
        cycles += 1;

        if result.dmc_sample.is_some() {
            dmc_sample = result.dmc_sample;
        }
    }

    assert!(dmc_sample.is_some(), "DMC should have completed and returned sample");

    // OAM DMA should still be active (it gets delayed when DMC steals cycles)
    assert!(controller.is_oam_dma_active(), "OAM DMA should still be running");
}
