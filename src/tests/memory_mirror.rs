use std::cell::RefCell;
use std::rc::Rc;
use crate::bus_device::BusDevice;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::memory_mirror::MemoryMirror;
use crate::tests::{create_memory_bank, init};

const VIRTUAL_MEMORY_RANGE: (u16, u16) = (0x8000, 0x9FFF);
const PHYSICAL_MEMORY_RANGE: (u16, u16) = (0x0000, 0x03FF);
const PHYSICAL_MEMORY_SIZE: usize = (PHYSICAL_MEMORY_RANGE.1 - PHYSICAL_MEMORY_RANGE.0 + 1)  as usize;

fn create_memory_mirror(memory_bank: Rc<RefCell<MemoryBank>>) -> MemoryMirror {
    MemoryMirror::new(memory_bank)
}

#[test]
fn test_create_memory_mirror_with_correct_address_space() -> Result<(), MemoryError> {
    init();

    let mut memory_bank = create_memory_bank(PHYSICAL_MEMORY_SIZE, VIRTUAL_MEMORY_RANGE);
    memory_bank.initialize()?;

    let memory_bank_rc = Rc::new(RefCell::new(memory_bank));
    let memory_mirror = create_memory_mirror(memory_bank_rc.clone());

    assert_eq!(memory_mirror.get_address_range(), memory_bank_rc.borrow().get_address_range());
    assert_eq!(memory_mirror.get_device_type(), memory_bank_rc.borrow().get_device_type());
    assert_eq!(memory_mirror.get_name(), memory_bank_rc.borrow().get_name());

    Ok(())
}

#[test]
fn test_read_byte_from_underlying_memory_bank() -> Result<(), MemoryError> {
    init();

    let mut memory_bank = create_memory_bank(PHYSICAL_MEMORY_SIZE, VIRTUAL_MEMORY_RANGE);
    memory_bank.initialize()?;

    let test_address = PHYSICAL_MEMORY_RANGE.0 + 128;
    let test_value = 0xAB;

    memory_bank.write_byte(test_address, test_value)?;

    let memory_bank_rc = Rc::new(RefCell::new(memory_bank));
    let memory_mirror = create_memory_mirror(memory_bank_rc.clone());

    let result = memory_mirror.read_byte(test_address)?;

    assert_eq!(result, test_value);

    Ok(())
}

#[test]
fn test_read_word_from_underlying_memory_bank() -> Result<(), MemoryError> {
    init();

    let mut memory_bank = create_memory_bank(PHYSICAL_MEMORY_SIZE, VIRTUAL_MEMORY_RANGE);
    memory_bank.initialize()?;

    let test_address = PHYSICAL_MEMORY_RANGE.0 + 256;
    let test_value = 0xABCD;

    memory_bank.write_word(test_address, test_value)?;

    let memory_bank_rc = Rc::new(RefCell::new(memory_bank));
    let memory_mirror = create_memory_mirror(memory_bank_rc.clone());

    let result = memory_mirror.read_word(test_address)?;

    assert_eq!(result, test_value);

    Ok(())
}

#[test]
fn test_write_byte_to_underlying_memory_bank() -> Result<(), MemoryError> {
    init();

    let mut memory_bank = create_memory_bank(PHYSICAL_MEMORY_SIZE, VIRTUAL_MEMORY_RANGE);
    memory_bank.initialize()?;

    let test_address = PHYSICAL_MEMORY_RANGE.0 + 512;
    let test_value = 0xCD;

    let memory_bank_rc = Rc::new(RefCell::new(memory_bank));
    let mut memory_mirror = create_memory_mirror(memory_bank_rc.clone());

    memory_mirror.write_byte(test_address, test_value)?;

    let result = memory_bank_rc.borrow().read_byte(test_address)?;

    assert_eq!(result, test_value);

    Ok(())
}

#[test]
fn test_write_word_to_underlying_memory_bank() -> Result<(), MemoryError> {
    init();

    let mut memory_bank = create_memory_bank(PHYSICAL_MEMORY_SIZE, VIRTUAL_MEMORY_RANGE);
    memory_bank.initialize()?;

    let test_address = PHYSICAL_MEMORY_RANGE.0 + 64;
    let test_value = 0xABCD;

    let memory_bank_rc = Rc::new(RefCell::new(memory_bank));
    let mut memory_mirror = create_memory_mirror(memory_bank_rc.clone());

    memory_mirror.write_word(test_address, test_value)?;

    let result = memory_bank_rc.borrow().read_word(test_address)?;

    assert_eq!(result, test_value);

    Ok(())
}

#[test]
fn test_size_returns_correct_memory_size_from_underlying_memory_bank() -> Result<(), MemoryError> {
    init();

    let mut memory_bank = create_memory_bank(PHYSICAL_MEMORY_SIZE, VIRTUAL_MEMORY_RANGE);
    memory_bank.initialize()?;

    let memory_bank_rc = Rc::new(RefCell::new(memory_bank));
    let memory_mirror = create_memory_mirror(memory_bank_rc.clone());

    let result = memory_mirror.size();

    assert_eq!(result, PHYSICAL_MEMORY_SIZE);

    Ok(())
}

#[test]
fn test_is_addr_in_address_space_returns_true_when_address_is_within_range() -> Result<(), MemoryError> {
    init();

    let mut memory_bank = create_memory_bank(PHYSICAL_MEMORY_SIZE, VIRTUAL_MEMORY_RANGE);
    memory_bank.initialize()?;

    let memory_bank_rc = Rc::new(RefCell::new(memory_bank));
    let memory_mirror = create_memory_mirror(memory_bank_rc);

    let test_address_start = VIRTUAL_MEMORY_RANGE.0;
    let test_address_middle = VIRTUAL_MEMORY_RANGE.0.wrapping_add(VIRTUAL_MEMORY_RANGE.1.wrapping_sub(VIRTUAL_MEMORY_RANGE.0) / 2);
    let test_address_end = VIRTUAL_MEMORY_RANGE.1;

    assert_eq!(memory_mirror.is_addr_in_address_space(test_address_start), true);
    assert_eq!(memory_mirror.is_addr_in_address_space(test_address_middle), true);
    assert_eq!(memory_mirror.is_addr_in_address_space(test_address_end), true);

    Ok(())
}

#[test]
fn test_is_addr_in_address_space_returns_false_when_address_is_outside_range() -> Result<(), MemoryError> {
    init();

    let mut memory_bank = create_memory_bank(PHYSICAL_MEMORY_SIZE, VIRTUAL_MEMORY_RANGE);
    memory_bank.initialize()?;

    let memory_bank_rc = Rc::new(RefCell::new(memory_bank));
    let memory_mirror = create_memory_mirror(memory_bank_rc.clone());

    assert_eq!(memory_mirror.is_addr_in_address_space(VIRTUAL_MEMORY_RANGE.1 + 1), false);
    assert_eq!(memory_mirror.is_addr_in_address_space(VIRTUAL_MEMORY_RANGE.0 - 1), false);

    Ok(())
}