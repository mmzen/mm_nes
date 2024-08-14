use crate::memory::{Memory, MockMemory};
use std::cell::RefCell;
use std::rc::Rc;
use mockall::predicate::eq;
use crate::nes_bus::NESBus;
use crate::tests::init;

fn create_memory_with_expectations(addr: u16, value: u8) -> MockMemory {
    let mut memory = MockMemory::new();

    memory.expect_write_byte().with(eq(addr), eq(value)).returning(|_, _| Ok(()));
    memory.expect_read_byte().with(eq(addr)).returning(move |_| Ok(value));

    memory
}

fn create_nes_bus(expected_addr: u16, expected_value: u8,
                  wram: Option<MockMemory>, ppu: Option<MockMemory>, apu: Option<MockMemory>, cartridge: Option<MockMemory>) -> NESBus {
    let wram = wram.unwrap_or(MockMemory::new());
    let ppu = ppu.unwrap_or(MockMemory::new());
    let apu = apu.unwrap_or(MockMemory::new());
    let cartridge = cartridge.unwrap_or(MockMemory::new());

    let nes_bus = NESBus::new(Rc::new(RefCell::new(wram)), Rc::new(RefCell::new(ppu)), Rc::new(RefCell::new(apu)), Rc::new(RefCell::new(cartridge)));

    nes_bus
}

#[test]
fn initialize_returns_ok() {
    init();

    // Arrange
    let mut nes_bus = create_nes_bus(0, 0, None, None, None, None);

    // Act
    let result = nes_bus.initialize();

    // Assert
    assert_eq!(result, Ok(nes_bus.size()));
}

#[test]
fn should_handle_write_request_for_valid_wram_address() {
    init();

    // Arrange
    let expected_addr = 0x0000;
    let expected_value = 0xAB;
    let wram = create_memory_with_expectations(expected_addr, expected_value);
    let mut nes_bus = create_nes_bus(expected_addr, expected_value, Some(wram), None, None, None);

    // Act
    let result = nes_bus.write_byte(expected_addr, expected_value);

    // Assert
    assert_eq!(result, Ok(()));
}

#[test]
fn should_handle_read_request_for_valid_wram_address() {
    init();

    // Arrange
    let expected_addr = 0x0000;
    let expected_value = 0xAB;
    let wram = create_memory_with_expectations(expected_addr, expected_value);
    let mut nes_bus = create_nes_bus(expected_addr, expected_value, Some(wram), None, None, None);

    // Act
    let result = nes_bus.read_byte(expected_addr);

    // Assert
    assert_eq!(result, Ok(expected_value));
}

#[test]
fn should_translate_wram_address() {
    init();

    // Arrange
    let addresses = [(0x0800, 0x0000), (0x07FF, 0x07FF), (0x1000, 0x0000), (0x1800, 0x0000), (0x1FFF, 0x07FF)];
    let expected_value = 0xAB;

    for addr in addresses {
        let wram = create_memory_with_expectations(addr.1, expected_value);
        let mut nes_bus = create_nes_bus(addr.0, expected_value, Some(wram), None, None, None);

        // Act
        let result = nes_bus.read_byte(addr.0);

        // Assert
        assert_eq!(result, Ok(expected_value));
    }
}


#[test]
fn should_handle_read_request_for_valid_ppu_register_address() {
    init();

    // Arrange
    let expected_addr = 0x2000;
    let expected_value = 0xAB;
    let ppu = create_memory_with_expectations(expected_addr, expected_value);
    let mut nes_bus = create_nes_bus(expected_addr, expected_value, None, Some(ppu), None, None);

    // Act
    let result = nes_bus.read_byte(expected_addr);

    // Assert
    assert_eq!(result, Ok(expected_value));
}


#[test]
fn should_handle_write_request_for_valid_ppu_register_address() {
    init();

    // Arrange
    let expected_addr = 0x2000;
    let expected_value = 0xAB;
    let ppu = create_memory_with_expectations(expected_addr, expected_value);
    let mut nes_bus = create_nes_bus(expected_addr, expected_value, None, Some(ppu), None, None);

    // Act
    let result = nes_bus.write_byte(expected_addr, expected_value);

    // Assert
    assert_eq!(result, Ok(()));
}

#[test]
fn should_translate_ppu_address() {
    init();

    // Arrange
    let addresses = [(0x2000, 0x2000), (0x2007, 0x2007), (0x2008, 0x2000), (0x3008, 0x2000), (0x3732, 0x2002), (0x3FFF, 0x2007)];
    let expected_value = 0xAB;

    for addr in addresses {
        let ppu = create_memory_with_expectations(addr.1, expected_value);
        let mut nes_bus = create_nes_bus(addr.0, expected_value, None, Some(ppu), None, None);

        // Act
        let result = nes_bus.read_byte(addr.0);

        // Assert
        assert_eq!(result, Ok(expected_value));
    }
}


#[test]
fn should_handle_read_request_for_valid_apu_register_address() {
    init();

    // Arrange
    let expected_addr = 0x4000;
    let expected_value = 0xAB;
    let apu = create_memory_with_expectations(expected_addr, expected_value);
    let mut nes_bus = create_nes_bus(expected_addr, expected_value, None, None, Some(apu), None);

    // Act
    let result = nes_bus.read_byte(expected_addr);

    // Assert
    assert_eq!(result, Ok(expected_value));
}

#[test]
fn should_handle_write_request_for_valid_apu_register_address() {
    init();

    // Arrange
    let expected_addr = 0x4000;
    let expected_value = 0xAB;
    let apu = create_memory_with_expectations(expected_addr, expected_value);
    let mut nes_bus = create_nes_bus(expected_addr, expected_value, None, None, Some(apu), None);

    // Act
    let result = nes_bus.write_byte(expected_addr, expected_value);

    // Assert
    assert_eq!(result, Ok(()));
}

#[test]
fn should_handle_read_request_for_valid_cartridge_address() {
    init();

    // Arrange
    let expected_addr = 0x4200;
    let expected_value = 0xAB;
    let cartridge = create_memory_with_expectations(expected_addr, expected_value);
    let mut nes_bus = create_nes_bus(expected_addr, expected_value, None, None, None, Some(cartridge));

    // Act
    let result = nes_bus.read_byte(expected_addr);

    // Assert
    assert_eq!(result, Ok(expected_value));
}

#[test]
fn should_handle_write_request_for_valid_cartridge_address() {
    init();

    // Arrange
    let expected_addr = 0x4200;
    let expected_value = 0xAB;
    let cartridge = create_memory_with_expectations(expected_addr, expected_value);
    let mut nes_bus = create_nes_bus(expected_addr, expected_value, None, None, None, Some(cartridge));

    // Act
    let result = nes_bus.write_byte(expected_addr, expected_value);

    // Assert
    assert_eq!(result, Ok(()));
}

/***
#[test]
fn should_return_error_for_out_of_range_address() {
}

#[test]
fn should_return_last_effective_address_when_out_of_range_address_accessed() {
}
***/
