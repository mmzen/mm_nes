// Authorship: Human 75% | Claude 25%
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