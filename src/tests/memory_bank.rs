use crate::memory::{Memory, MemoryError};
use crate::memory_bank::{MEMORY_DEFAULT_SIZE, MemoryBank};
use crate::tests::init;


fn check_memory(mut memory: MemoryBank) {
    for byte in memory.as_slice().iter() {
        assert_eq!(*byte, 0xFF);
    }
}

#[test]
fn test_initialize_memory_with_default_size() {
    init();

    let mut memory_bank = MemoryBank::default();
    let expected_size = MEMORY_DEFAULT_SIZE;

    assert_eq!(memory_bank.initialize().unwrap(), expected_size);
    assert_eq!(memory_bank.size(), expected_size);

    check_memory(memory_bank)
}

#[test]
fn test_initialize_memory_with_specified_size() {
    init();

    let mut memory_bank = MemoryBank::new_with_size(512);
    assert_eq!(memory_bank.initialize().unwrap(), 512);

    check_memory(memory_bank)
}

#[test]
fn test_is_in_boundary_works() {
    init();

    let mut memory_bank = MemoryBank::new_with_size(512);
    assert_eq!(memory_bank.is_addr_in_boundary(1024), false);
    assert_eq!(memory_bank.is_addr_in_boundary(256), true);
}

#[test]
fn read_byte_out_of_range_error() {
    init();

    let mut memory_bank = MemoryBank::new_with_size(64);
    let out_of_range_addr = memory_bank.size() as u16 + 1;

    assert_eq!(
        memory_bank.read_byte(out_of_range_addr),
        Err(MemoryError::OutOfRange(out_of_range_addr))
    );
}

#[test]
fn write_byte_out_of_range_returns_error() {
    init();

    let mut memory_bank = MemoryBank::new_with_size(64);
    let out_of_range_addr = (memory_bank.size() + 1) as u16;
    let value = 0xAB;

    assert_eq!(
        memory_bank.write_byte(out_of_range_addr, value),
        Err(MemoryError::OutOfRange(out_of_range_addr))
    );
}

#[test]
fn test_read_write_byte() {
    init();

    let mut memory = MemoryBank::default();
    let test_address = 0x1000;
    let test_value = 0xAB;

    memory.write_byte(test_address, test_value).unwrap();
    assert_eq!(memory.read_byte(test_address).unwrap(), test_value);
}

#[test]
fn should_read_and_write_word_correctly_at_specific_address() {
    init();

    let mut memory_bank = MemoryBank::new_with_size(0x1000);
    let test_addr = 0x0200;
    let test_value = 0xABCD;

    // Write a word to the memory
    memory_bank.write_word(test_addr, test_value).unwrap();

    // Read the word from the memory
    let read_value = memory_bank.read_word(test_addr).unwrap();

    // Assert that the written and read values are equal
    assert_eq!(read_value, test_value);
}

#[test]
fn test_word_read_write_across_boundaries() {
    init();

    let mut memory = MemoryBank::new_with_size(0x1000);
    let addr = 0x0FFF;
    let value = 0xABCD;

    memory.write_word(addr, value).unwrap();

    assert_eq!(memory.read_word(addr).unwrap(), value);
}

#[test]
fn test_read_write_unaligned_word() {
    init();

    let mut memory = MemoryBank::new_with_size(256);

    // Write unaligned word
    assert_eq!(memory.write_word(1, 0xABCD), Ok(()));

    // Read unaligned word
    assert_eq!(memory.read_word(1), Ok(0xABCD));
}



