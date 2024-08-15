use std::cell::RefCell;
use std::rc::Rc;
use crate::memory::{Memory, MemoryError};
use mockall::predicate::eq;
use crate::bus::Bus;
use crate::bus_device::MockBusDeviceStub;
use crate::nes_bus::{BUS_ADDRESSABLE_SIZE, NESBus};
use crate::tests::init;

const DEFAULT_MEMORY_SIZE: usize = 256;
const DEFAULT_MEMORY_RANGE: (u16, u16) = (0x0000, 0x01FF);

enum RequestType {
    None,
    Read,
    Write,
    ReadWrite,
    Unmapped
}

enum RequestData {
    None,
    Byte(u8),
    Word(u16),
}

fn create_bus_device_with_expectations(addr: u16, request: RequestType, length: RequestData) -> MockBusDeviceStub {
    let mut device = MockBusDeviceStub::new();

    device.expect_size().returning(|| DEFAULT_MEMORY_SIZE);
    device.expect_get_address_range().times(1).returning(|| DEFAULT_MEMORY_RANGE);

    match (request, length) {
        (RequestType::Read, RequestData::Byte(value)) => {
            device.expect_is_addr_in_boundary().returning(|_| true);
            device.expect_read_byte().times(1).with(eq(addr)).returning(move |_| Ok(value));
        },

        (RequestType::Write, RequestData::Byte(value)) => {
            device.expect_is_addr_in_boundary().returning(|_| true);
            device.expect_write_byte().times(1).with(eq(addr), eq(value)).returning(|_, _| Ok(()));
        },

        (RequestType::Read, RequestData::Word(value)) => {
            device.expect_is_addr_in_boundary().returning(|_| true);
            device.expect_read_word().times(1).with(eq(addr)).returning(move |_| Ok(value));
        },

        (RequestType::Write, RequestData::Word(value)) => {
            device.expect_is_addr_in_boundary().returning(|_| true);
            device.expect_write_word().times(1).with(eq(addr), eq(value)).returning(|_, _| Ok(()));
        },

        (RequestType::ReadWrite, RequestData::Byte(value)) => {
            device.expect_is_addr_in_boundary().returning(|_| true);
            device.expect_read_byte().times(1).with(eq(addr)).returning(move |_| Ok(value));
            device.expect_write_byte().times(1).with(eq(addr), eq(value)).returning(|_, _| Ok(()));
        },

        (RequestType::ReadWrite, RequestData::Word(value)) => {
            device.expect_is_addr_in_boundary().returning(|_| true);
            device.expect_read_word().times(1).with(eq(addr)).returning(move |_| Ok(value));
            device.expect_write_word().times(1).with(eq(addr), eq(value)).returning(|_, _| Ok(()));
        },

        (RequestType::Unmapped, _) => {
            device.expect_is_addr_in_boundary().returning(|_| false);
            device.expect_read_byte().times(0);
            device.expect_write_byte().times(0);
        },

        _ => {}
    }

    device
}

fn create_nes_bus() -> NESBus {
    NESBus::new()
}

fn create_nes_bus_with_bus_device(expected_addr: u16, request: RequestType, length: RequestData) -> NESBus {
    let device = create_bus_device_with_expectations(expected_addr, request, length);
    let mut nes_bus = create_nes_bus();
    let err = nes_bus.add_device(Rc::new(RefCell::new(device)));

    if let Err(err) = err {
        panic!("failed to add bus device: {}", err);
    }

    nes_bus
}

#[test]
fn initialize_returns_ok() {
    init();

    let mut nes_bus = create_nes_bus();
    let result = nes_bus.initialize();
    assert_eq!(result, Ok(nes_bus.size()));
}

#[test]
fn read_byte_request_for_valid_address() {
    init();

    let expected_addr = 0x0000;
    let expected_value = 0xAB;

    let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::Read, RequestData::Byte(expected_value));

    let result = nes_bus.read_byte(expected_addr);

    assert_eq!(result, Ok(expected_value));
}

#[test]
fn write_byte_request_for_valid_address() {
    init();

    let expected_addr = 0x0000;
    let expected_value = 0xAB;

    let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::Write, RequestData::Byte(expected_value));
    let result = nes_bus.write_byte(expected_addr, expected_value);

    assert_eq!(result, Ok(()));
}

#[test]
fn read_write_byte_request_for_valid_address() {
    init();

    let expected_addr = 0x0000;
    let expected_value = 0xAB;

    let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::ReadWrite, RequestData::Byte(expected_value));

    let result0 = nes_bus.write_byte(expected_addr, expected_value);
    let result1 = nes_bus.read_byte(expected_addr);

    assert_eq!(result0, Ok(()));
    assert_eq!(result1, Ok(expected_value));
}

#[test]
fn read_word_request_for_valid_address() {
    init();

    let expected_addr = 0x0000;
    let expected_value = 0xABCD;

    let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::Read, RequestData::Word(expected_value));

    let result = nes_bus.read_word(expected_addr);
    assert_eq!(result, Ok(expected_value));
}

#[test]
fn write_word_request_for_valid_address() {
    init();

    let expected_addr = 0x0000;
    let expected_value = 0xABCD;

    let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::Write, RequestData::Word(expected_value));
    let result = nes_bus.write_word(expected_addr, expected_value);

    assert_eq!(result, Ok(()));
}

#[test]
fn read_write_word_request_for_valid_address() {
    init();

    let expected_addr = 0x0000;
    let expected_value = 0xABCD;

    let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::ReadWrite, RequestData::Word(expected_value));

    let result0 = nes_bus.write_word(expected_addr, expected_value);
    let result1 = nes_bus.read_word(expected_addr);

    assert_eq!(result0, Ok(()));
    assert_eq!(result1, Ok(expected_value));
}

#[test]
fn returns_size() {
    init();

    let expected_addr = 0x0000;

    let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::None, RequestData::None);

    let result = nes_bus.size();

    assert_eq!(result, BUS_ADDRESSABLE_SIZE);
}

#[test]
fn returns_bus_error_on_unmapped_access() {
    init();

    let expected_addr = 0x0000;
    let expected_value = 0xAB;

    let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::Unmapped, RequestData::None);

    let result0 = nes_bus.write_byte(expected_addr, expected_value);
    let result1 = nes_bus.read_byte(expected_addr);

    assert_eq!(result0, Err(MemoryError::BusError(expected_addr)));
    assert_eq!(result1, Err(MemoryError::BusError(expected_addr)));
}

#[test]
fn mirrors_content_when_address_space_si_larger_than_size() {
    init();

    let expected_addr = 0x0000;
    let virtual_addr = 0x0100;
    let expected_value = 0xAB;

    let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::ReadWrite, RequestData::Byte(expected_value));

    let result0 = nes_bus.write_byte(virtual_addr, expected_value);
    let result1 = nes_bus.read_byte(virtual_addr);

    assert_eq!(result0, Ok(()));
    assert_eq!(result1, Ok(expected_value));
}

/***
#[test]
fn should_translate_wram_address() {
    init();

    // Arrange
    let addresses = [(0x0800, 0x0000), (0x07FF, 0x07FF), (0x1000, 0x0000), (0x1800, 0x0000), (0x1FFF, 0x07FF)];
    let expected_value = 0xAB;

    for addr in addresses {
        let wram = create_memory_with_expectations(addr.1, expected_value);
        let mut nes_bus = create_nes_bus(addr.0, expected_value);

        // Act
        let result = nes_bus.read_byte(addr.0);

        // Assert
        assert_eq!(result, Ok(expected_value));
    }
}***/
