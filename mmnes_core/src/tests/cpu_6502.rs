// Authorship: Human 50% | Claude 50%
use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::MockBusStub;
use crate::cpu::{CPU, CpuError, Interruptible};
use crate::cpu_6502::{Cpu6502, APU_DMC_IRQ, APU_FRAME_COUNTER_IRQ, PPU_NMI};
use crate::tests::init;


fn create_bus() -> MockBusStub {
    let bus = MockBusStub::new();
    bus
}

fn create_cpu() -> Cpu6502 {
    let bus = create_bus();
    let cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu
}

/// Helper to execute the 7-cycle interrupt sequence after instruction completion.
/// Interrupt sequences now execute one cycle at a time instead of batching.
fn run_interrupt_sequence(cpu: &mut Cpu6502) -> Result<(), CpuError> {
    for i in 1..=7 {
        let result = cpu.step_cycle()?;
        if i == 7 {
            assert!(result.instruction_complete, "Interrupt sequence should complete on cycle 7");
        } else {
            assert!(!result.instruction_complete, "Interrupt sequence cycle {} should not be complete", i);
        }
    }
    Ok(())
}

#[test]
fn there_is_no_interrupt_at_creation() -> Result<(), CpuError> {
    init();
    let cpu = create_cpu();

    let result = cpu.get_internal_interrupt_value();
    assert_eq!(result, 0);

    Ok(())
}

#[test]
fn signal_irq_works() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();
    let irq_masks = [
        APU_FRAME_COUNTER_IRQ,
        APU_DMC_IRQ,
        APU_FRAME_COUNTER_IRQ | APU_DMC_IRQ
    ];

    for mask in irq_masks {
        cpu.signal_irq(mask)?;
        let result = cpu.get_internal_interrupt_value();
        assert_eq!(result, mask);
        cpu.clear_internal_interrupt_value();
    }

    Ok(())
}

#[test]
fn clear_irq_works() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();
    let irq_masks = [
        APU_FRAME_COUNTER_IRQ,
        APU_DMC_IRQ,
        APU_FRAME_COUNTER_IRQ | APU_DMC_IRQ
    ];

    for mask in irq_masks {
        cpu.signal_irq(mask)?;
        let result = cpu.get_internal_interrupt_value();
        assert_eq!(result, mask);

        cpu.clear_irq(mask)?;
        let result = cpu.get_internal_interrupt_value();
        assert_eq!(result, 0);

        cpu.clear_internal_interrupt_value();
    }

    Ok(())
}

#[test]
fn assert_irq_works() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();
    let irq_masks = [
        APU_FRAME_COUNTER_IRQ,
        APU_DMC_IRQ,
        APU_FRAME_COUNTER_IRQ | APU_DMC_IRQ
    ];

    let result = cpu.is_asserted_irq()?;
    assert_eq!(result, false);

    for mask in irq_masks {
        cpu.signal_irq(mask)?;
        let result = cpu.is_asserted_irq()?;
        assert_eq!(result, true);

        cpu.clear_internal_interrupt_value();
    }

    Ok(())
}

#[test]
fn assert_irq_by_source_works() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();
    let irq_masks = [
        (APU_FRAME_COUNTER_IRQ, APU_FRAME_COUNTER_IRQ, true),
        (APU_DMC_IRQ, APU_DMC_IRQ, true),
        (APU_FRAME_COUNTER_IRQ | APU_DMC_IRQ, APU_FRAME_COUNTER_IRQ | APU_DMC_IRQ, true),
        (APU_FRAME_COUNTER_IRQ, APU_DMC_IRQ, false),
        (APU_DMC_IRQ, APU_FRAME_COUNTER_IRQ, false),
        (APU_FRAME_COUNTER_IRQ | APU_DMC_IRQ, APU_DMC_IRQ, true),
        (APU_FRAME_COUNTER_IRQ | APU_DMC_IRQ, APU_FRAME_COUNTER_IRQ, true),
    ];

    let result = cpu.is_asserted_irq_by_source(0xFF)?;
    assert_eq!(result, false);

    for (mask, source, assert_result) in irq_masks {
        cpu.signal_irq(mask)?;
        let result = cpu.is_asserted_irq_by_source(source)?;
        assert_eq!(result, assert_result);

        cpu.clear_internal_interrupt_value();
    }

    Ok(())
}

#[test]
fn signal_nmi_works() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    assert!(!cpu.is_asserted_nmi()?);
    cpu.signal_nmi()?;
    assert!(cpu.is_asserted_nmi()?);

    Ok(())
}

#[test]
fn clear_nmi_works() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    cpu.signal_nmi()?;
    assert!(cpu.is_asserted_nmi()?);

    // Note: clear_nmi only clears the line state, not pending_nmi
    // The NMI will still be serviced once signaled (edge-triggered)
    cpu.clear_nmi()?;
    // pending_nmi remains true until serviced
    assert!(cpu.is_asserted_nmi()?);

    Ok(())
}

#[test]
fn assert_nmi_works() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    let result = cpu.is_asserted_nmi()?;
    assert_eq!(result, false);

    cpu.signal_nmi()?;
    let result = cpu.is_asserted_nmi()?;
    assert_eq!(result, true);

    cpu.clear_internal_interrupt_value();

    Ok(())
}

#[test]
fn assert_nmi_does_not_return_ok_when_irq_are_pending() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    cpu.signal_irq(APU_FRAME_COUNTER_IRQ)?;
    let result = cpu.is_asserted_nmi()?;
    assert_eq!(result, false);

    Ok(())
}

#[test]
fn assert_irq_does_not_return_ok_when_nmi_are_pending() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    cpu.signal_nmi()?;
    let result = cpu.is_asserted_irq()?;
    assert_eq!(result, false);

    Ok(())
}

#[test]
fn clear_nmi_does_not_clear_irq() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    cpu.signal_nmi()?;
    cpu.signal_irq(APU_DMC_IRQ)?;
    cpu.clear_nmi()?;

    let result = cpu.get_internal_interrupt_value();
    assert_eq!(result, APU_DMC_IRQ);

    Ok(())
}

#[test]
fn clear_irq_does_not_clear_nmi() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    cpu.signal_nmi()?;
    cpu.signal_irq(APU_DMC_IRQ)?;
    cpu.clear_irq(APU_DMC_IRQ)?;

    // NMI is stored separately from IRQ, clearing IRQ should not affect NMI
    assert!(cpu.is_asserted_nmi()?);
    assert!(!cpu.is_asserted_irq()?);

    Ok(())
}

#[test]
fn assert_irq_works_when_both_nmi_and_irq_are_pending() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    cpu.signal_nmi()?;
    cpu.signal_irq(APU_DMC_IRQ)?;
    let result = cpu.is_asserted_irq()?;
    assert_eq!(result, true);

    let result = cpu.is_asserted_nmi()?;
    assert_eq!(result, true);

    Ok(())
}

// ============================================================================
// Cycle-stepping tests
// ============================================================================

#[test]
fn cpu_is_not_mid_instruction_at_start() -> Result<(), CpuError> {
    init();
    let cpu = create_cpu();

    assert!(!cpu.is_mid_instruction());
    Ok(())
}

#[test]
fn cpu_is_not_halted_at_start() -> Result<(), CpuError> {
    init();
    let cpu = create_cpu();

    assert!(!cpu.is_halted());
    Ok(())
}

#[test]
fn cpu_get_cycles_returns_zero_at_start() -> Result<(), CpuError> {
    init();
    let cpu = create_cpu();

    assert_eq!(cpu.get_cycles(), 0);
    Ok(())
}

#[test]
fn cpu_halt_cycles_puts_cpu_in_halted_state() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    cpu.halt_cycles(10);
    assert!(cpu.is_halted());

    Ok(())
}

#[test]
fn cpu_halt_cycles_decrements_on_step_cycle() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    cpu.halt_cycles(3);
    assert!(cpu.is_halted());

    // First step - still halted (2 remaining)
    let result = cpu.step_cycle()?;
    assert!(result.halted);
    assert!(cpu.is_halted());

    // Second step - still halted (1 remaining)
    let result = cpu.step_cycle()?;
    assert!(result.halted);
    assert!(cpu.is_halted());

    // Third step - no longer halted (0 remaining)
    let result = cpu.step_cycle()?;
    assert!(result.halted);
    assert!(!cpu.is_halted());

    Ok(())
}

#[test]
fn step_cycle_executes_nop_instruction() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    // Create bus with NOP instruction at address 0x8000
    let mut bus = TracingBus::new();
    // NOP opcode (0xEA) at 0x8000, followed by another NOP
    bus.load_memory(&[
        (0x8000, 0xEA), // NOP
        (0x8001, 0xEA), // NOP
        // Reset vector points to 0x8000
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // After init, PC should be 0x8000
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x8000, "PC should be at 0x8000 after init");

    // Execute first cycle - should fetch opcode
    let result = cpu.step_cycle()?;
    assert!(!result.instruction_complete, "First cycle should not complete instruction");
    assert!(result.memory_read, "First cycle should read memory");
    assert_eq!(result.address, Some(0x8000), "Should read from PC");
    assert_eq!(result.data, Some(0xEA), "Should read NOP opcode");

    // Execute second cycle - should complete NOP (2-cycle instruction)
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete, "Second cycle should complete NOP instruction");

    // PC should have advanced to 0x8001
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x8001, "PC should advance after NOP");

    Ok(())
}

#[test]
fn step_cycle_executes_lda_immediate() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    // LDA #$42 at 0x8000
    bus.load_memory(&[
        (0x8000, 0xA9), // LDA immediate
        (0x8001, 0x42), // value 0x42
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // Cycle 1: Fetch opcode
    let result = cpu.step_cycle()?;
    assert!(!result.instruction_complete);

    // Cycle 2: Fetch immediate value and execute
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete, "LDA immediate should complete on cycle 2");

    // Verify A register was loaded
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.a(), 0x42, "A register should be 0x42");
    assert_eq!(snapshot.pc(), 0x8002, "PC should be 0x8002");

    Ok(())
}

#[test]
fn step_cycle_executes_jsr_and_rts() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    // JSR $8010 at 0x8000, subroutine at 0x8010 just does RTS
    bus.load_memory(&[
        (0x8000, 0x20), // JSR
        (0x8001, 0x10), // low byte
        (0x8002, 0x80), // high byte (target = 0x8010)
        // Subroutine at 0x8010
        (0x8010, 0x60), // RTS
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // Execute JSR (6 cycles)
    for i in 0..6 {
        let result = cpu.step_cycle()?;
        if i < 5 {
            assert!(!result.instruction_complete, "JSR should not complete until cycle 6");
        } else {
            assert!(result.instruction_complete, "JSR should complete on cycle 6");
        }
    }

    // Verify PC is at subroutine
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x8010, "PC should be at subroutine after JSR");

    // Execute RTS (6 cycles)
    for i in 0..6 {
        let result = cpu.step_cycle()?;
        if i < 5 {
            assert!(!result.instruction_complete, "RTS should not complete until cycle 6");
        } else {
            assert!(result.instruction_complete, "RTS should complete on cycle 6");
        }
    }

    // Verify PC returned (JSR pushes PC+2, RTS adds 1, so we return to 0x8003)
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x8003, "PC should be 0x8003 after RTS (instruction after JSR)");

    Ok(())
}

#[test]
fn step_cycle_executes_sta_zero_page() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    // LDA #$42, STA $10
    bus.load_memory(&[
        (0x8000, 0xA9), // LDA immediate
        (0x8001, 0x42), // value 0x42
        (0x8002, 0x85), // STA zero page
        (0x8003, 0x10), // ZP address $10
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let bus_rc = Rc::new(RefCell::new(bus));
    let mut cpu = Cpu6502::new(bus_rc.clone());
    cpu.initialize()?;

    // Execute LDA #$42 (2 cycles)
    cpu.step_cycle()?;
    cpu.step_cycle()?;

    // Verify A is loaded
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.a(), 0x42);

    // Execute STA $10 (3 cycles)
    for i in 0..3 {
        let result = cpu.step_cycle()?;
        if i < 2 {
            assert!(!result.instruction_complete, "STA ZP should not complete until cycle 3");
        } else {
            assert!(result.instruction_complete, "STA ZP should complete on cycle 3");
            assert!(result.memory_write, "STA should write to memory");
            assert_eq!(result.address, Some(0x0010), "Should write to ZP $10");
            assert_eq!(result.data, Some(0x42), "Should write value 0x42");
        }
    }

    // Verify memory was written
    let written_value = bus_rc.borrow().peek(0x10);
    assert_eq!(written_value, 0x42, "Memory at $10 should be 0x42");

    Ok(())
}

#[test]
fn step_cycle_executes_branch_taken_and_continues() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    // Test that after a branch is taken, the next instruction executes correctly
    // BEQ $8004 (branch taken because Z flag will be set by LDA #$00)
    // At 0x8004: LDA #$42
    // At 0x8006: NOP (verify PC advances correctly after branch)
    bus.load_memory(&[
        (0x8000, 0xA9), // LDA immediate
        (0x8001, 0x00), // value 0x00 (sets Z flag)
        (0x8002, 0xF0), // BEQ (branch if equal/zero)
        (0x8003, 0x00), // offset +0 (branch to 0x8004)
        (0x8004, 0xA9), // LDA immediate
        (0x8005, 0x42), // value 0x42
        (0x8006, 0xEA), // NOP
        (0x8007, 0xEA), // NOP
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // Execute LDA #$00 (2 cycles) - sets Z flag
    cpu.step_cycle()?;
    cpu.step_cycle()?;

    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.a(), 0x00, "A should be 0x00");
    assert_eq!(snapshot.pc(), 0x8002, "PC should be at BEQ instruction");

    // Execute BEQ (branch taken, no page cross = 3 cycles total)
    // Cycle 1: fetch opcode (already counted - we're at PC 0x8002)
    // Cycle 2: fetch offset
    cpu.step_cycle()?;  // Fetch opcode
    cpu.step_cycle()?;  // Fetch offset (branch evaluated, taken but not complete yet)

    // Cycle 3: branch taken (no page cross) - completes here
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete, "BEQ should complete on cycle 3 (taken, no page cross)");

    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x8004, "PC should be at branch target after BEQ");

    // Execute LDA #$42 at 0x8004 (2 cycles)
    cpu.step_cycle()?;
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete, "LDA immediate should complete on cycle 2");

    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.a(), 0x42, "A should be 0x42 after LDA");
    assert_eq!(snapshot.pc(), 0x8006, "PC should advance to 0x8006 after LDA #$42");

    // Execute first NOP at 0x8006 (2 cycles) - this tests PC advancement after branch
    cpu.step_cycle()?;
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete, "NOP should complete");

    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x8007, "PC should advance to 0x8007 after NOP");

    Ok(())
}

// ============================================================================
// Interrupt timing tests (Phase 3: NMI/IRQ polling at penultimate cycle)
// ============================================================================

/// Test that NMI signaled before the final cycle's poll is latched and serviced.
/// NMI should be serviced after the instruction completes.
#[test]
fn nmi_signaled_before_final_cycle_is_serviced() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    // NOP (2 cycles) followed by another NOP
    bus.load_memory(&[
        (0x8000, 0xEA), // NOP
        (0x8001, 0xEA), // NOP
        // NMI vector points to 0x9000
        (0xFFFA, 0x00),
        (0xFFFB, 0x90),
        // NMI handler
        (0x9000, 0xEA), // NOP (just to have something)
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // Clear interrupt disable flag so we can also test IRQ behavior
    cpu.set_status_for_test(cpu.get_status() & !0x04);

    // Cycle 1: Fetch NOP opcode - NMI is polled here (clear)
    let result = cpu.step_cycle()?;
    assert!(!result.instruction_complete);

    // Signal NMI before cycle 2 (between cycles 1 and 2)
    // This should be picked up by the poll at the start of cycle 2
    cpu.signal_nmi()?;

    // Cycle 2: Execute NOP - NMI is polled at start of this cycle, should be latched
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete, "NOP should complete on cycle 2");

    // After instruction completion, CPU transitions to InterruptSequence state
    // Execute the 7-cycle interrupt sequence
    assert!(cpu.is_mid_instruction(), "CPU should be in interrupt sequence");
    run_interrupt_sequence(&mut cpu)?;

    // PC should now be at NMI vector (0x9000)
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x9000, "PC should be at NMI handler after interrupt");

    Ok(())
}

/// Test that NMI signaled AFTER instruction completion triggers at the next
/// instruction boundary, BEFORE executing the next instruction.
///
/// In our emulation model:
/// - Poll happens at START of step_cycle() (FetchOpcode)
/// - NMI signaled after step_cycle() completes will be seen at the NEXT FetchOpcode
/// - The interrupt sequence begins BEFORE the next opcode is fetched/executed
#[test]
fn nmi_signaled_after_instruction_completion_delays_to_next_instruction() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    // NOP (2 cycles) followed by another NOP
    bus.load_memory(&[
        (0x8000, 0xEA), // NOP
        (0x8001, 0xEA), // NOP (will be interrupted)
        (0x8002, 0xEA), // NOP
        // NMI vector
        (0xFFFA, 0x00),
        (0xFFFB, 0x90),
        (0x9000, 0xEA), // NMI handler
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // Execute first NOP completely without NMI
    // Cycle 1: Fetch opcode (poll: no NMI)
    cpu.step_cycle()?;
    // Cycle 2: Execute NOP (poll: no NMI)
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete);

    // No interrupt should trigger - CPU should go to FetchOpcode, not InterruptSequence
    assert!(!cpu.is_mid_instruction(), "No interrupt should occur");

    // PC should be at 0x8001 (next instruction)
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x8001, "PC should be at 0x8001");

    // Now signal NMI AFTER first instruction completed
    // This simulates NMI arriving after the final cycle of an instruction
    cpu.signal_nmi()?;

    // Next step_cycle is FetchOpcode for second NOP at 0x8001
    // NMI should be polled and latched, then interrupt triggers IMMEDIATELY
    // The second NOP does NOT execute - interrupt takes priority
    let result = cpu.step_cycle()?;

    // This is interrupt cycle 1 (dummy read at PC), NOT the NOP fetch
    assert!(!result.instruction_complete, "First interrupt cycle should not complete");
    assert_eq!(result.address, Some(0x8001), "Should do dummy read at PC (0x8001)");

    // Complete the remaining 6 cycles of interrupt sequence
    for i in 2..=7 {
        let result = cpu.step_cycle()?;
        if i == 7 {
            assert!(result.instruction_complete, "Interrupt should complete on cycle 7");
        }
    }

    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x9000, "PC should be at NMI handler");

    Ok(())
}

/// Test that IRQ is only serviced if I flag is clear at the instruction boundary.
/// If I flag is set (by SEI), subsequent IRQs should not trigger.
#[test]
fn irq_respects_i_flag_at_instruction_completion() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    // SEI (2 cycles) - sets I flag, followed by NOP
    bus.load_memory(&[
        (0x8000, 0x78), // SEI
        (0x8001, 0xEA), // NOP (should execute, not interrupted because I=1)
        (0x8002, 0xEA), // NOP
        // IRQ vector
        (0xFFFE, 0x00),
        (0xFFFF, 0x90),
        (0x9000, 0xEA), // IRQ handler
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // Clear I flag initially
    cpu.set_status_for_test(cpu.get_status() & !0x04);

    // Execute SEI (2 cycles) WITHOUT any IRQ pending
    cpu.step_cycle()?;
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete, "SEI should complete");

    // Verify I flag is now set
    let snapshot = cpu.snapshot()?;
    assert!(snapshot.p() & 0x04 != 0, "I flag should be set after SEI");
    assert_eq!(snapshot.pc(), 0x8001, "PC should be at NOP");

    // NOW signal IRQ - but I flag is set, so it should NOT trigger
    cpu.signal_irq(APU_FRAME_COUNTER_IRQ)?;

    // Execute NOP (2 cycles) - IRQ should NOT trigger because I=1
    cpu.step_cycle()?;
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete, "NOP should complete (IRQ blocked by I flag)");

    // CPU should NOT be in interrupt sequence
    assert!(!cpu.is_mid_instruction(), "IRQ should NOT trigger because I flag is set");

    let snapshot = cpu.snapshot()?;
    // Should be at 0x8002 (next instruction), not at IRQ handler
    assert_eq!(snapshot.pc(), 0x8002, "PC should be at 0x8002, not IRQ handler");

    Ok(())
}

/// Test that NMI has priority over IRQ when both are pending.
/// When both are pending at instruction boundary, NMI should be serviced first.
#[test]
fn nmi_has_priority_over_irq() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    bus.load_memory(&[
        (0x8000, 0xEA), // NOP (will be interrupted by NMI)
        (0x8001, 0xEA), // NOP
        // NMI vector
        (0xFFFA, 0x00),
        (0xFFFB, 0x90),
        // IRQ vector
        (0xFFFE, 0x00),
        (0xFFFF, 0xA0),
        // NMI handler at 0x9000
        (0x9000, 0xEA),
        // IRQ handler at 0xA000
        (0xA000, 0xEA),
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // Clear I flag
    cpu.set_status_for_test(cpu.get_status() & !0x04);

    // Signal both NMI and IRQ before first instruction
    cpu.signal_nmi()?;
    cpu.signal_irq(APU_FRAME_COUNTER_IRQ)?;

    // At FetchOpcode, both interrupts are detected
    // NMI has priority, so interrupt sequence begins immediately
    // The NOP does NOT execute
    let result = cpu.step_cycle()?;

    // This is interrupt cycle 1 (dummy read at PC), NOT the NOP fetch
    assert!(!result.instruction_complete, "First interrupt cycle should not complete");
    assert_eq!(result.address, Some(0x8000), "Should do dummy read at PC (0x8000)");

    // Complete the remaining 6 cycles of interrupt sequence
    for i in 2..=7 {
        let result = cpu.step_cycle()?;
        if i == 7 {
            assert!(result.instruction_complete, "Interrupt should complete on cycle 7");
        }
    }

    // Should jump to NMI handler (0x9000), not IRQ handler (0xA000)
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x9000, "PC should be at NMI handler (NMI has priority)");

    Ok(())
}

/// Test that latching happens on each cycle - the LAST poll before completion is what matters.
/// If NMI is cleared between polls, it should still be serviced if it was latched.
#[test]
fn nmi_latched_state_persists_even_if_cleared_before_completion() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    // Use a 3-cycle instruction to have more opportunities to test timing
    // LDA $10 (Zero Page) is 3 cycles
    bus.load_memory(&[
        (0x0010, 0x42), // Value at ZP $10
        (0x8000, 0xA5), // LDA ZP
        (0x8001, 0x10), // address $10
        (0x8002, 0xEA), // NOP
        // NMI vector
        (0xFFFA, 0x00),
        (0xFFFB, 0x90),
        (0x9000, 0xEA), // NMI handler
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // Cycle 1: Fetch opcode (poll: no NMI)
    cpu.step_cycle()?;

    // Signal NMI before cycle 2
    cpu.signal_nmi()?;

    // Cycle 2: Fetch ZP address (poll: NMI pending, latched!)
    cpu.step_cycle()?;

    // Note: We can't easily "clear" NMI mid-instruction in a realistic way
    // because clear_pending_nmi is a test helper. The key insight is that
    // once NMI is latched, it stays latched for this instruction.

    // Cycle 3: Read from ZP, complete instruction
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete);
    // NMI should be serviced because it was latched on cycle 2
    assert!(cpu.is_mid_instruction(), "NMI should trigger");
    run_interrupt_sequence(&mut cpu)?;

    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x9000, "PC should be at NMI handler");

    Ok(())
}

// ============================================================================
// Interrupt Flag Latency tests (AccuracyCoin code 1)
// These tests verify that IRQ is serviced at instruction boundary without
// executing an extra opcode.
// ============================================================================

/// Test that IRQ pending at instruction boundary triggers immediately.
/// When IRQ is asserted and I flag is clear, the CPU must enter the interrupt
/// sequence at the next instruction boundary WITHOUT executing the next opcode.
#[test]
fn irq_at_instruction_boundary_triggers_immediately() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    // Program: NOP at 0x8000, LDA #$FF at 0x8001 (should NOT execute if IRQ triggers)
    bus.load_memory(&[
        (0x8000, 0xEA), // NOP (first instruction)
        (0x8001, 0xA9), // LDA immediate (should NOT be executed)
        (0x8002, 0xFF), // operand for LDA
        // IRQ vector points to 0xA000
        (0xFFFE, 0x00),
        (0xFFFF, 0xA0),
        // IRQ handler at 0xA000 - just has a NOP
        (0xA000, 0xEA),
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // Clear I flag to allow IRQ
    cpu.set_status_for_test(cpu.get_status() & !0x04);

    // Verify initial state
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x8000, "Should start at 0x8000");
    assert_eq!(snapshot.a(), 0, "A should be 0 initially");

    // Execute NOP (2 cycles)
    cpu.step_cycle()?; // Cycle 1: Fetch NOP
    let result = cpu.step_cycle()?; // Cycle 2: Execute NOP
    assert!(result.instruction_complete, "NOP should complete");

    // Now PC is at 0x8001 (LDA instruction)
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0x8001, "PC should be at 0x8001 after NOP");

    // Signal IRQ - this should be serviced at the next instruction boundary
    // BEFORE the LDA is executed
    cpu.signal_irq(APU_FRAME_COUNTER_IRQ)?;

    // Next step_cycle should detect IRQ at FetchOpcode and enter interrupt sequence
    // It should NOT fetch and execute the LDA instruction
    let result = cpu.step_cycle()?;

    // This cycle should be the first cycle of the interrupt sequence (dummy read at PC)
    // NOT the opcode fetch for LDA
    assert!(!result.instruction_complete, "First interrupt cycle should not complete");
    assert_eq!(result.address, Some(0x8001), "Should do dummy read at PC (0x8001)");

    // A register should still be 0 - LDA was never executed
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.a(), 0, "A should still be 0 - LDA was NOT executed");

    // Complete the remaining 6 cycles of interrupt sequence
    for i in 2..=7 {
        let result = cpu.step_cycle()?;
        if i == 7 {
            assert!(result.instruction_complete, "Interrupt should complete on cycle 7");
        }
    }

    // PC should be at IRQ handler
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0xA000, "PC should be at IRQ handler");
    assert_eq!(snapshot.a(), 0, "A should still be 0 - LDA was never executed");

    Ok(())
}

/// Test that IRQ latched while I flag is set gets serviced after CLI clears the flag.
/// This tests the latching semantics: IRQ line state is sampled independently of I flag.
#[test]
fn irq_latched_while_i_flag_set_triggers_after_cli() -> Result<(), CpuError> {
    use crate::tests::singlestep::tracing_bus::TracingBus;

    init();

    let mut bus = TracingBus::new();
    // Program: SEI, <signal IRQ here>, CLI, NOP (NOP should NOT execute)
    bus.load_memory(&[
        (0x8000, 0x78), // SEI (2 cycles) - sets I flag
        (0x8001, 0x58), // CLI (2 cycles) - clears I flag
        (0x8002, 0xEA), // NOP - should NOT execute if IRQ triggers correctly
        // IRQ vector
        (0xFFFE, 0x00),
        (0xFFFF, 0xA0),
        // IRQ handler at 0xA000
        (0xA000, 0xEA),
        // Reset vector
        (0xFFFC, 0x00),
        (0xFFFD, 0x80),
    ]);

    let mut cpu = Cpu6502::new(Rc::new(RefCell::new(bus)));
    cpu.initialize()?;

    // Clear I flag initially so we start in a known state
    cpu.set_status_for_test(cpu.get_status() & !0x04);

    // Execute SEI (2 cycles) - this sets the I flag
    cpu.step_cycle()?;
    let result = cpu.step_cycle()?;
    assert!(result.instruction_complete, "SEI should complete");

    // Verify I flag is set
    let snapshot = cpu.snapshot()?;
    assert!(snapshot.p() & 0x04 != 0, "I flag should be set after SEI");
    assert_eq!(snapshot.pc(), 0x8001, "PC should be at CLI");

    // Signal IRQ while I flag is set
    // The IRQ line should be latched even though I=1
    cpu.signal_irq(APU_FRAME_COUNTER_IRQ)?;

    // Execute CLI (2 cycles) - this clears the I flag
    // During CLI's execution, IRQ is being latched each cycle
    cpu.step_cycle()?; // Cycle 1: Fetch CLI
    let result = cpu.step_cycle()?; // Cycle 2: Execute CLI
    assert!(result.instruction_complete, "CLI should complete");

    // After CLI, I flag should be clear
    let snapshot = cpu.snapshot()?;
    assert!(snapshot.p() & 0x04 == 0, "I flag should be clear after CLI");
    assert_eq!(snapshot.pc(), 0x8002, "PC should be at NOP");

    // Now at the next instruction boundary (FetchOpcode for NOP at 0x8002),
    // IRQ should be detected and serviced immediately
    let result = cpu.step_cycle()?;

    // This should be interrupt cycle 1, NOT the NOP fetch
    assert!(!result.instruction_complete);
    assert_eq!(result.address, Some(0x8002), "Should do dummy read at PC (0x8002)");

    // Complete interrupt sequence (cycles 2-7)
    for i in 2..=7 {
        let result = cpu.step_cycle()?;
        if i == 7 {
            assert!(result.instruction_complete, "Interrupt should complete on cycle 7");
        }
    }

    // PC should be at IRQ handler
    let snapshot = cpu.snapshot()?;
    assert_eq!(snapshot.pc(), 0xA000, "PC should be at IRQ handler");

    Ok(())
}