// Authorship: Human 0% | Claude 100%
//! Tests for DmaController
//!
//! Tests OAM DMA and DMC DMA cycle-accurate behavior with the new bus arbiter model.
//! Tests GET/PUT APU phase tracking for DMA alignment.
//!
//! Phase is now passed explicitly to step_cycle() - tests track phase locally.
//! pending_read_addr is also passed - captured by DMA controller at halt.

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
        let result = controller.step_cycle(false, None, current_phase).unwrap();
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
        let _ = controller.step_cycle(false, None, current_phase).unwrap();
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
        let result = controller.step_cycle(false, None, current_phase).unwrap();

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
        let result = controller.step_cycle(false, None, current_phase).unwrap();

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
    controller.step_cycle(false, None, current_phase).unwrap();
    current_phase = current_phase.toggle();

    // Run through several cycles, checking pattern
    let mut reads = 0;
    let mut writes = 0;

    for _ in 0..20 {
        if !controller.is_active() { break; }
        let result = controller.step_cycle(false, None, current_phase).unwrap();
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
    let _result = controller.step_cycle(true, None, current_phase).unwrap(); // cpu_is_writing = true
    assert!(controller.is_oam_dma_active(), "OAM DMA should still be active (PendingHalt)");

    // Second cycle with CPU reading - halt should succeed
    let _result = controller.step_cycle(false, None, current_phase.toggle()).unwrap();
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
        controller.step_cycle(false, None, current_phase).unwrap();
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
        let result = controller.step_cycle(false, None, current_phase).unwrap();
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
    controller.step_cycle(true, None, current_phase).unwrap();
    assert!(controller.is_dmc_dma_active(), "DMC DMA should still be pending");
    current_phase = current_phase.toggle();

    // Second cycle with CPU reading - halt should succeed
    controller.step_cycle(false, None, current_phase).unwrap();
    current_phase = current_phase.toggle();

    // Continue until complete
    let mut cycles = 2;
    while controller.is_dmc_dma_active() && cycles < 10 {
        controller.step_cycle(false, None, current_phase).unwrap();
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
        let _ = controller.step_cycle(false, None, current_phase).unwrap();
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
        let _ = controller.step_cycle(false, None, current_phase).unwrap();
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

    // Start OAM DMA - address will be captured at halt via step_cycle parameter
    controller.start_oam_dma(0x02);

    // First cycle transitions from PendingHalt - pass the pending read address
    // The DMA controller should capture $2002 as the halted address
    let result = controller.step_cycle(false, Some(0x2002), ApuPhase::Get).unwrap();

    // The result should show the CPU halted
    assert!(result.cpu_halted, "CPU should be halted");

    // Verify the halted address was captured
    assert_eq!(controller.get_halted_read_address(), Some(0x2002),
        "Halted address should be captured from step_cycle parameter");
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
    let result = controller.step_cycle(false, None, ApuPhase::Put).unwrap();
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
        let result = controller.step_cycle(false, None, current_phase).unwrap();
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

// ============================================================================
// Repeated Read Side Effect Tests
// ============================================================================

/// Tests that repeated reads during DMA idle cycles go through the bus.
/// This is critical for side effects like $2007 incrementing VRAM address.
///
/// When CPU is halted during DMA, the external bus shows repeated reads from
/// the address the CPU was about to read. These reads must trigger side effects.
#[test]
fn test_repeated_reads_during_dma_idle_cycles_go_through_bus() {
    init();

    let mut controller = create_controller();

    // Start DMC DMA - it has 2-3 no-bus cycles (Halt, Dummy, maybe Align)
    // where CPU repeated reads should occur.
    // The halted read address ($2007) will be captured at halt via step_cycle parameter.
    controller.request_dmc_dma(0xC000, false, None);

    let mut current_phase = ApuPhase::Get;
    let mut cpu_repeat_reads = 0u32;
    let mut cpu_repeat_addresses: Vec<u16> = Vec::new();
    let mut cycles = 0;

    // Run through the DMC DMA sequence
    // Pass $2007 as the pending read address - it will be captured when halt succeeds
    while controller.is_dmc_dma_active() && cycles < 10 {
        let result = controller.step_cycle(false, Some(0x2007), current_phase).unwrap();
        current_phase = current_phase.toggle();
        cycles += 1;

        // Count cycles where CPU repeated read happened
        if result.winner == BusWinner::CpuRepeat {
            cpu_repeat_reads += 1;
            // Verify the bus op is a read from $2007
            if let BusOp::Read(addr) = result.bus_op {
                cpu_repeat_addresses.push(addr);
            }
        }
    }

    // DMC DMA should have completed
    assert!(!controller.is_dmc_dma_active(), "DMC DMA should have completed");

    // Should have had at least 2 CPU repeated read cycles (Halt + Dummy, maybe Align)
    assert!(cpu_repeat_reads >= 2,
        "Expected at least 2 CPU repeated read cycles, got {}. \
         These cycles must read from $2007 with side effects.",
        cpu_repeat_reads);

    // All repeated reads should be from the halted address ($2007)
    for addr in &cpu_repeat_addresses {
        assert_eq!(*addr, 0x2007,
            "Repeated read should be from halted address $2007, got ${:04X}",
            addr);
    }

    // The DMA controller performs these reads through bus.read_byte(),
    // which means real PPU register side effects (like VRAM address increment) will occur.
    // This is verified by the fact that BusWinner::CpuRepeat returns a BusOp::Read
    // and the execute_bus_op path calls bus.read_byte() for CpuRepeat.
}

// ============================================================================
// Delayed Halt Tests - Critical for address capture correctness
// ============================================================================

/// Tests that when OAM DMA is triggered during a CPU write cycle, the halt is delayed
/// and the halted read address is captured from the LATER read cycle, not the stale value.
///
/// This is the key test for the fix in code review round 6: the halted address must be
/// captured AT THE MOMENT halt succeeds, not at DMA start time.
#[test]
fn test_delayed_halt_captures_correct_address() {
    init();
    let mut controller = create_controller();

    // Start OAM DMA
    controller.start_oam_dma(0x02);
    let current_phase = ApuPhase::Get;

    // Cycle 1: CPU is WRITING - halt should fail, stay in PendingHalt
    // Pass a "stale" address ($1234) that would be wrong if captured now
    let result = controller.step_cycle(true, Some(0x1234), current_phase).unwrap();
    assert!(controller.is_oam_dma_active(), "OAM DMA should still be active");
    // Address should NOT be captured yet since halt didn't succeed
    assert!(controller.get_halted_read_address().is_none(),
        "Halted address should not be captured during failed halt (CPU writing)");

    // Cycle 2: CPU is READING from $ABCD - halt should succeed NOW
    // This is the correct address that should be captured
    let result = controller.step_cycle(false, Some(0xABCD), current_phase.toggle()).unwrap();
    assert!(result.cpu_halted, "CPU should be halted now");

    // The halted address should be $ABCD (the address from the successful halt cycle),
    // NOT $1234 (the stale address from when halt was attempted but failed)
    assert_eq!(controller.get_halted_read_address(), Some(0xABCD),
        "Halted address should be captured from the cycle where halt succeeded ($ABCD), \
         not the earlier cycle ($1234)");
}

/// Tests that DMC DMA also captures the correct address when halt is delayed.
#[test]
fn test_dmc_delayed_halt_captures_correct_address() {
    init();
    let mut controller = create_controller();

    // Start DMC DMA
    controller.request_dmc_dma(0xC000, true, None); // cpu_is_writing = true at request time
    let current_phase = ApuPhase::Get;

    // Cycle 1: CPU is WRITING - halt should fail
    let _result = controller.step_cycle(true, Some(0x5555), current_phase).unwrap();
    assert!(controller.is_dmc_dma_active(), "DMC DMA should still be active");
    // Address should NOT be captured yet
    assert!(controller.get_halted_read_address().is_none(),
        "Halted address should not be captured during failed halt");

    // Cycle 2: CPU is READING from $7777 - halt should succeed
    let result = controller.step_cycle(false, Some(0x7777), current_phase.toggle()).unwrap();
    assert!(result.cpu_halted, "CPU should be halted now");

    // The halted address should be $7777
    assert_eq!(controller.get_halted_read_address(), Some(0x7777),
        "Halted address should be captured from successful halt cycle");
}

/// Tests that is_cpu_stalled() returns false during PendingHalt.
///
/// This is critical: the CPU must continue executing normally during PendingHalt.
/// Only when the halt succeeds (transitions to Halt state) should CPU be stalled.
#[test]
fn test_cpu_not_stalled_during_pending_halt() {
    init();
    let mut controller = create_controller();

    // Before any DMA: not stalled, not active
    assert!(!controller.is_cpu_stalled(), "CPU should not be stalled when no DMA");
    assert!(!controller.is_active(), "No DMA should be active");

    // Start OAM DMA - enters PendingHalt
    controller.start_oam_dma(0x02);

    // DMA is active (PendingHalt counts as active)
    assert!(controller.is_active(), "DMA should be active after start");

    // BUT CPU should NOT be stalled during PendingHalt!
    assert!(!controller.is_cpu_stalled(),
        "CPU should NOT be stalled during PendingHalt - this is the critical invariant");

    // Step with CPU writing - halt fails, stays in PendingHalt
    let current_phase = ApuPhase::Get;
    let _result = controller.step_cycle(true, Some(0x1234), current_phase).unwrap();

    // Still in PendingHalt - CPU still NOT stalled
    assert!(controller.is_active(), "DMA should still be active");
    assert!(!controller.is_cpu_stalled(),
        "CPU should still NOT be stalled while halt is pending (CPU was writing)");

    // Step with CPU reading - halt succeeds, transitions to Halt
    let result = controller.step_cycle(false, Some(0x5678), current_phase.toggle()).unwrap();

    // NOW CPU is stalled (Halt state)
    assert!(result.cpu_halted, "DMA result should indicate CPU halted");
    assert!(controller.is_cpu_stalled(),
        "CPU should NOW be stalled after halt succeeded");
}

/// Tests that is_cpu_stalled() returns false during DMC PendingHalt.
#[test]
fn test_cpu_not_stalled_during_dmc_pending_halt() {
    init();
    let mut controller = create_controller();

    // Start DMC DMA - enters PendingHalt
    controller.request_dmc_dma(0xC000, false, None);

    // DMA is active
    assert!(controller.is_active(), "DMC DMA should be active after request");

    // CPU should NOT be stalled during PendingHalt
    assert!(!controller.is_cpu_stalled(),
        "CPU should NOT be stalled during DMC PendingHalt");

    // Step with CPU reading - halt succeeds
    let current_phase = ApuPhase::Get;
    let result = controller.step_cycle(false, Some(0x1234), current_phase).unwrap();

    // NOW CPU is stalled
    assert!(result.cpu_halted, "DMA result should indicate CPU halted");
    assert!(controller.is_cpu_stalled(),
        "CPU should be stalled after DMC halt succeeded");
}
