use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::MockBusStub;
use crate::cpu::{CpuError, Interruptible};
use crate::cpu_6502::{Cpu6502, APU_DMC_IRQ, APU_FRAME_COUNTER_IRQ, PPU_NMI};
use crate::tests::init;


fn create_bus() -> MockBusStub {
    let bus = MockBusStub::new();
    bus
}

fn create_cpu() -> Cpu6502 {
    let bus = create_bus();
    let cpu = Cpu6502::new(Rc::new(RefCell::new(bus)), false, None);
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

    cpu.signal_nmi()?;
    let result = cpu.get_internal_interrupt_value();
    assert_eq!(result, PPU_NMI);
    cpu.clear_internal_interrupt_value();

    Ok(())
}

#[test]
fn clear_nmi_works() -> Result<(), CpuError> {
    init();
    let mut cpu = create_cpu();

    cpu.signal_nmi()?;
    let result = cpu.get_internal_interrupt_value();
    assert_eq!(result, PPU_NMI);

    cpu.clear_irq(PPU_NMI)?;
    let result = cpu.get_internal_interrupt_value();
    assert_eq!(result, 0);


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

    let result = cpu.get_internal_interrupt_value();
    assert_eq!(result, PPU_NMI);

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