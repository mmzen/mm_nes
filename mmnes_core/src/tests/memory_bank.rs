use crate::bus::MockBusStub;
use crate::bus_device::BusDevice;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::tests::{create_memory_bank, init};

const DEFAULT_MEMORY_RANGE: (u16, u16) = (0x1000, 0x1FFF);
const DEFAULT_MEMORY_SIZE: usize = 4096;

fn check_memory(memory: MemoryBank) {
    for i in 0..memory.size() {
        let byte = memory.read_byte(i as u16).unwrap();
        assert_eq!(byte, 0x00);
    }
}

#[allow(dead_code)]
fn create_bus() -> MockBusStub {
    let bus = MockBusStub::new();
    bus
}

#[test]
fn test_initialize_memory_with_specified_size() {
    init();

    let mut memory_bank = create_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    assert_eq!(memory_bank.initialize().unwrap(), DEFAULT_MEMORY_SIZE);

    check_memory(memory_bank)
}

#[test]
fn read_byte_out_of_range_error() {
    init();

    let memory_bank = create_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    let out_of_range_addr = DEFAULT_MEMORY_RANGE.1 + 1;

    assert_eq!(
        memory_bank.read_byte(out_of_range_addr),
        Err(MemoryError::OutOfRange(out_of_range_addr))
    );
}

#[test]
fn write_byte_out_of_range_returns_error() {
    init();

    let mut memory_bank = create_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    let out_of_range_addr = DEFAULT_MEMORY_RANGE.1 + 1;
    let value = 0xAB;

    assert_eq!(
        memory_bank.write_byte(out_of_range_addr, value),
        Err(MemoryError::OutOfRange(out_of_range_addr))
    );
}

#[test]
fn write_byte() {
    init();

    let mut memory_bank = create_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    let test_address = 0x00FF;
    let test_value = 0xAB;

    memory_bank.write_byte(test_address, test_value).unwrap();
    assert_eq!(memory_bank.read_byte(test_address).unwrap(), test_value);
}

#[test]
fn read_and_write_word_correctly_at_specific_address() {
    init();

    let mut memory_bank = create_memory_bank(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE);
    let test_address = 0x00FF;
    let test_value = 0xAB;

    memory_bank.write_word(test_address, test_value).unwrap();
    let read_value = memory_bank.read_word(test_address).unwrap();

    assert_eq!(read_value, test_value);
}




