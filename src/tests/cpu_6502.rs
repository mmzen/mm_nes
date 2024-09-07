use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::MockBusStub;
use crate::cpu::Interruptible;
use crate::cpu_6502::Cpu6502;
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
fn signal_irq_works() {
    init();
    let mut cpu = create_cpu();

    let result = cpu.is_irq_pending();
    assert_eq!(result, false);

    cpu.signal_irq().unwrap();
    let result = cpu.is_irq_pending();
    assert_eq!(result, true);
}

#[test]
fn signal_nmi_works() {
    init();
    let mut cpu = create_cpu();

    let result = cpu.is_nmi_pending();
    assert_eq!(result, false);

    cpu.signal_nmi().unwrap();
    let result = cpu.is_nmi_pending();
    assert_eq!(result, true);
}