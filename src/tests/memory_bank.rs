use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::MockBusStub;
use crate::bus_device::BusDevice;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::tests::init;

const DEFAULT_MEMORY_RANGE: (u16, u16) = (0x1000, 0x1FFF);
const DEFAULT_MEMORY_SIZE: usize = 4096;

fn check_memory(mut memory: MemoryBank) {
    for byte in memory.as_slice().iter() {
        assert_eq!(*byte, 0xFF);
    }
}

fn create_bus() -> MockBusStub {
    let bus = MockBusStub::new();
    bus
}

fn creat_memory_bank(size: usize, address_range: (u16, u16)) -> MemoryBank {
    let bus = Rc::new(RefCell::new(create_bus()));
    let mut memory_bank = MemoryBank::new(size, bus, address_range);


    memory_bank
}

#[test]
fn test_initialize_memory_with_specified_size() {
    init();

    let mut memory_bank = creat_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    assert_eq!(memory_bank.initialize().unwrap(), DEFAULT_MEMORY_SIZE);

    check_memory(memory_bank)
}

#[test]
fn is_in_boundary_works() {
    init();

    let memory_bank = creat_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);

    assert_eq!(memory_bank.is_addr_in_boundary(0x0000), false);
    assert_eq!(memory_bank.is_addr_in_boundary(0x2000), false);
    assert_eq!(memory_bank.is_addr_in_boundary(0x1ABC), true);
}

#[test]
fn read_byte_out_of_range_error() {
    init();

    let memory_bank = creat_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    let out_of_range_addr = DEFAULT_MEMORY_RANGE.0 + DEFAULT_MEMORY_SIZE as u16;

    assert_eq!(
        memory_bank.read_byte(out_of_range_addr),
        Err(MemoryError::OutOfRange(out_of_range_addr))
    );
}

#[test]
fn write_byte_out_of_range_returns_error() {
    init();

    let mut memory_bank = creat_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    let out_of_range_addr = DEFAULT_MEMORY_RANGE.0 + DEFAULT_MEMORY_SIZE as u16;
    let value = 0xAB;

    assert_eq!(
        memory_bank.write_byte(out_of_range_addr, value),
        Err(MemoryError::OutOfRange(out_of_range_addr))
    );
}

#[test]
fn write_byte() {
    init();

    let mut memory_bank = creat_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    let test_address = 0x1200;
    let test_value = 0xAB;

    memory_bank.write_byte(test_address, test_value).unwrap();
    assert_eq!(memory_bank.read_byte(test_address).unwrap(), test_value);
}

#[test]
fn read_and_write_word_correctly_at_specific_address() {
    init();

    let mut memory_bank = creat_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    let test_address = 0x1200;
    let test_value = 0xAB;

    memory_bank.write_word(test_address, test_value).unwrap();
    let read_value = memory_bank.read_word(test_address).unwrap();

    assert_eq!(read_value, test_value);
}

#[test]
fn word_read_write_across_boundaries() {
    init();

    let mut memory_bank = creat_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    let addr = DEFAULT_MEMORY_RANGE.1;
    let value = 0xABCD;

    memory_bank.write_word(addr, value).unwrap();

    assert_eq!(memory_bank.read_word(addr).unwrap(), value);
}

#[test]
fn read_write_unaligned_word() {
    init();

    let mut memory_bank = creat_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);

    assert_eq!(memory_bank.write_word(DEFAULT_MEMORY_RANGE.0 + 1, 0x1ABC), Ok(()));
    assert_eq!(memory_bank.read_word(DEFAULT_MEMORY_RANGE.0 + 1), Ok(0x1ABC));
}



