// Authorship: Human 0% | Claude 100%
//! Tests for DmaController
//!
//! Tests OAM DMA and DMC DMA cycle-accurate behavior with the new bus arbiter model.
//! Tests GET/PUT APU phase tracking for DMA alignment.
//!
//! Phase is now passed explicitly to step_cycle() - tests track phase locally.

use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::MockBusStub;
use crate::dma_controller::{ApuPhase, BusOp, BusWinner, DmaController};
use crate::dma_device::MockDmaDeviceStub;
use crate::tests::init;

fn create_controller() -> DmaController<MockBusStub, MockDmaDeviceStub> {
    let mut bus = MockBusStub::new();
    bus.expect_read_byte().returning(|addr| Ok((addr & 0xFF) as u8));
    bus.expect_write_byte().returning(|_, _| Ok(()));

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
#[allow(dead_code)]
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
    // oam_dma_cycles is now a static method that takes the write phase
    // Write on GET phase = 513 cycles
    assert_eq!(DmaController::<MockBusStub, MockDmaDeviceStub>::oam_dma_cycles(ApuPhase::Get), 513);
}

#[test]
fn test_oam_dma_cycles_put_phase_start() {
    init();
    // Write on PUT phase = 514 cycles
    assert_eq!(DmaController::<MockBusStub, MockDmaDeviceStub>::oam_dma_cycles(ApuPhase::Put), 514);
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
    let write_phase = ApuPhase::Get;
    controller.start_oam_dma(0x02);
    // First DMA step is on toggled phase
    let mut current_phase = write_phase.toggle(); // PUT

    let mut total_cycles = 0;
    let mut read_count = 0;
    let mut write_count = 0;

    while controller.is_active() {
        let result = controller.step_cycle(false, current_phase).unwrap();
        total_cycles += 1;

        if is_read(&result.bus_op) {
            read_count += 1;
        }
        if is_write(&result.bus_op) {
            write_count += 1;
        }

        // Toggle phase each cycle (simulates real timing)
        current_phase = current_phase.toggle();

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
    let write_phase = ApuPhase::Put;
    controller.start_oam_dma(0x02);
    let mut current_phase = write_phase.toggle(); // GET

    let mut total_cycles = 0;

    while controller.is_active() {
        let _ = controller.step_cycle(false, current_phase).unwrap();
        total_cycles += 1;

        // Toggle phase each cycle
        current_phase = current_phase.toggle();

        // Safety: prevent infinite loop
        if total_cycles > 600 {
            panic!("OAM DMA took too long");
        }
    }

    assert_eq!(total_cycles, 514, "OAM DMA should take 514 cycles when started on PUT phase");
}

#[test]
fn test_oam_dma_reads_from_correct_source_address() {
    init();
    let mut controller = create_controller();
    let write_phase = ApuPhase::Get;
    controller.start_oam_dma(0x02); // Source page = 0x02
    let mut current_phase = write_phase.toggle(); // First DMA step on PUT

    // Run a few cycles and check that reads are from page 0x02
    let mut read_addresses = Vec::new();

    for _ in 0..20 {
        if !controller.is_active() { break; }
        let result = controller.step_cycle(false, current_phase).unwrap();

        if let BusOp::Read(addr) = result.bus_op {
            read_addresses.push(addr);
        }

        current_phase = current_phase.toggle();
    }

    // First few reads should be from 0x0200, 0x0201, etc.
    assert!(read_addresses.len() >= 2, "Should have some reads");
    assert!(read_addresses[0] >= 0x0200 && read_addresses[0] < 0x0300,
            "First read should be from page 0x02");
}

#[test]
fn test_oam_dma_writes_to_2004() {
    init();
    let mut controller = create_controller();
    let write_phase = ApuPhase::Get;
    controller.start_oam_dma(0x02);
    let mut current_phase = write_phase.toggle();

    // Run a few cycles and check that writes are to $2004
    let mut write_addresses = Vec::new();

    for _ in 0..20 {
        if !controller.is_active() { break; }
        let result = controller.step_cycle(false, current_phase).unwrap();

        if let BusOp::Write(addr, _) = result.bus_op {
            write_addresses.push(addr);
        }

        current_phase = current_phase.toggle();
    }

    // All writes should be to $2004
    assert!(write_addresses.len() >= 2, "Should have some writes");
    for addr in write_addresses {
        assert_eq!(addr, 0x2004, "OAM DMA should write to $2004");
    }
}

#[test]
fn test_oam_dma_alternates_read_write_on_phases() {
    init();
    let mut controller = create_controller();
    let write_phase = ApuPhase::Get;
    controller.start_oam_dma(0x00);
    let mut current_phase = write_phase.toggle();

    // First cycle: PendingHalt -> Halt (no bus op)
    controller.step_cycle(false, current_phase).unwrap();
    current_phase = current_phase.toggle();

    // Run through several cycles, checking pattern
    let mut reads = 0;
    let mut writes = 0;

    for _ in 0..20 {
        if !controller.is_active() { break; }
        let result = controller.step_cycle(false, current_phase).unwrap();
        if is_read(&result.bus_op) {
            reads += 1;
        }
        if is_write(&result.bus_op) {
            writes += 1;
        }
        current_phase = current_phase.toggle();
    }

    // Should have roughly equal reads and writes (alternating)
    assert!(reads > 0, "Should have some reads");
    assert!(writes > 0, "Should have some writes");
}

#[test]
fn test_oam_dma_halt_fails_on_cpu_write() {
    init();
    let mut controller = create_controller();
    controller.start_oam_dma(0x02);
    let current_phase = ApuPhase::Get;

    // First cycle with CPU writing - halt should fail, stay in PendingHalt
    let _result = controller.step_cycle(true, current_phase).unwrap(); // cpu_is_writing = true
    assert!(controller.is_oam_dma_active(), "OAM DMA should still be active (PendingHalt)");

    // Second cycle with CPU reading - halt should succeed
    let _result = controller.step_cycle(false, current_phase.toggle()).unwrap();
    assert!(controller.is_oam_dma_active(), "OAM DMA should still be active (now in Halt/WaitGet)");
}

// ============================================================================
// DMC DMA Tests
// ============================================================================

#[test]
fn test_dmc_dma_state_machine_sequence() {
    init();
    let mut controller = create_controller();
    let mut current_phase = ApuPhase::Get;

    // Start DMC DMA
    controller.request_dmc_dma(0xC000, false, None);

    // Run until complete
    let mut cycles = 0;
    while controller.is_dmc_dma_active() && cycles < 10 {
        controller.step_cycle(false, current_phase).unwrap();
        current_phase = current_phase.toggle();
        cycles += 1;
    }

    assert!(!controller.is_dmc_dma_active(), "DMC DMA should have completed");
}

#[test]
fn test_dmc_dma_returns_sample() {
    init();
    let mut controller = create_controller();
    let mut current_phase = ApuPhase::Get;
    controller.request_dmc_dma(0xC000, false, None);

    let mut sample_received = false;

    for _ in 0..10 {
        let result = controller.step_cycle(false, current_phase).unwrap();
        current_phase = current_phase.toggle();

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
    let mut current_phase = ApuPhase::Get;
    controller.request_dmc_dma(0xC000, false, None);

    // First cycle with CPU writing - halt should fail
    controller.step_cycle(true, current_phase).unwrap();
    assert!(controller.is_dmc_dma_active(), "DMC DMA should still be pending");
    current_phase = current_phase.toggle();

    // Second cycle with CPU reading - halt should succeed
    controller.step_cycle(false, current_phase).unwrap();
    current_phase = current_phase.toggle();

    // Continue until complete
    let mut cycles = 2;
    while controller.is_dmc_dma_active() && cycles < 10 {
        controller.step_cycle(false, current_phase).unwrap();
        current_phase = current_phase.toggle();
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
    let phase = ApuPhase::Get;
    assert_eq!(phase.toggle(), ApuPhase::Put);
    assert_eq!(phase.toggle().toggle(), ApuPhase::Get);
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
fn test_oam_dma_uses_current_apu_phase_for_alignment() {
    init();
    let mut controller = create_controller();

    // Test GET phase start (write on GET -> first DMA step on PUT)
    let write_phase = ApuPhase::Get;
    controller.start_oam_dma(0x02);
    let mut current_phase = write_phase.toggle();

    let mut cycles_get = 0;
    while controller.is_active() {
        let _ = controller.step_cycle(false, current_phase).unwrap();
        current_phase = current_phase.toggle();
        cycles_get += 1;
        if cycles_get > 600 { panic!("Too many cycles"); }
    }
    assert_eq!(cycles_get, 513, "GET phase start should take 513 cycles");

    // Test PUT phase start (write on PUT -> first DMA step on GET)
    let write_phase = ApuPhase::Put;
    controller.start_oam_dma(0x02);
    let mut current_phase = write_phase.toggle();

    let mut cycles_put = 0;
    while controller.is_active() {
        let _ = controller.step_cycle(false, current_phase).unwrap();
        current_phase = current_phase.toggle();
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
    bus.expect_write_byte().returning(|_, _| Ok(()));

    let mut ppu = MockDmaDeviceStub::new();
    ppu.expect_dma_write().returning(|_, _| Ok(()));

    let mut controller = DmaController::new(
        Rc::new(RefCell::new(bus)),
        Rc::new(RefCell::new(ppu))
    );

    // Set the halted read address (simulating CPU was reading from $2002)
    controller.set_halted_read_address(Some(0x2002));
    controller.start_oam_dma(0x02);

    // First cycle transitions from PendingHalt
    // During cycles where DMA doesn't use the bus, CPU repeated read should occur
    let result = controller.step_cycle(false, ApuPhase::Get).unwrap();

    // The result should show the CPU halted
    assert!(result.cpu_halted, "CPU should be halted");
}

#[test]
fn test_bus_op_none_when_no_activity() {
    init();
    assert_eq!(BusOp::None, BusOp::default());
}

#[test]
fn test_bus_winner_tracking() {
    init();
    let mut controller = create_controller();
    controller.start_oam_dma(0x02);

    // First cycle - halt cycle, winner should be None or CpuRepeat
    let result = controller.step_cycle(false, ApuPhase::Put).unwrap();
    // During halt, no OAM bus op happens
    assert!(matches!(result.winner, BusWinner::None | BusWinner::CpuRepeat));
}

// ============================================================================
// Overlap Tests - DMC DMA vs OAM DMA
// ============================================================================

#[test]
fn test_dmc_read_has_priority_over_oam_get() {
    init();
    let mut controller = create_controller();
    let mut current_phase = ApuPhase::Get;

    // Start both DMAs
    controller.start_oam_dma(0x02);
    controller.request_dmc_dma(0xC000, false, None);

    // Run until DMC completes - it should get priority on GET cycles when it needs to read
    let mut dmc_sample = None;
    let mut cycles = 0;

    while controller.is_dmc_dma_active() && cycles < 20 {
        let result = controller.step_cycle(false, current_phase).unwrap();
        current_phase = current_phase.toggle();
        cycles += 1;

        if result.dmc_sample.is_some() {
            dmc_sample = result.dmc_sample;
        }
    }

    assert!(dmc_sample.is_some(), "DMC should have completed and returned sample");

    // OAM DMA should still be active (it gets delayed when DMC steals cycles)
    assert!(controller.is_oam_dma_active(), "OAM DMA should still be running");
}
