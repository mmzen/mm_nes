use std::cell::RefCell;
use std::rc::Rc;
use crate::memory::{Memory, MemoryType};
use mockall::predicate::eq;
use crate::bus::Bus;
use crate::bus_device::{BusDeviceType, MockBusDeviceStub};
use crate::nes_bus::{BUS_ADDRESSABLE_SIZE, NESBus};
use crate::tests::init;

const DEFAULT_MEMORY_SIZE: usize = 2048;
const DEFAULT_MEMORY_RANGE: (u16, u16) = (0x0000, 0x1FFF);
const DEFAULT_DEVICE_NAME: &str = "test device";

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

fn create_bus_device_with_expectations(memory_size: usize, memory_range: (u16, u16), expected_addr: u16, request: RequestType, length: RequestData) -> MockBusDeviceStub {
    let mut device = MockBusDeviceStub::new();

    device.expect_get_name().returning(|| DEFAULT_DEVICE_NAME.to_string());
    device.expect_get_device_type().returning(|| BusDeviceType::WRAM(MemoryType::NESMemory));

    device.expect_size().returning(move || memory_size);
    device.expect_get_address_range().returning(move || memory_range);

    match (request, length) {
        (RequestType::Read, RequestData::Byte(value)) => {
            device.expect_is_addr_in_address_space().returning(|_| true);
            device.expect_read_byte().times(1).with(eq(expected_addr)).returning(move |_| Ok(value));
        },

        (RequestType::Write, RequestData::Byte(value)) => {
            device.expect_is_addr_in_address_space().returning(|_| true);
            device.expect_write_byte().times(1).with(eq(expected_addr), eq(value)).returning(|_, _| Ok(()));
        },

        (RequestType::Read, RequestData::Word(value)) => {
            device.expect_is_addr_in_address_space().returning(|_| true);
            device.expect_read_word().times(1).with(eq(expected_addr)).returning(move |_| Ok(value));
        },

        (RequestType::Write, RequestData::Word(value)) => {
            device.expect_is_addr_in_address_space().returning(|_| true);
            device.expect_write_word().times(1).with(eq(expected_addr), eq(value)).returning(|_, _| Ok(()));
        },

        (RequestType::ReadWrite, RequestData::Byte(value)) => {
            device.expect_is_addr_in_address_space().returning(|_| true);
            device.expect_read_byte().times(1).with(eq(expected_addr)).returning(move |_| Ok(value));
            device.expect_write_byte().times(1).with(eq(expected_addr), eq(value)).returning(|_, _| Ok(()));
        },

        (RequestType::ReadWrite, RequestData::Word(value)) => {
            device.expect_is_addr_in_address_space().returning(|_| true);
            device.expect_read_word().times(1).with(eq(expected_addr)).returning(move |_| Ok(value));
            device.expect_write_word().times(1).with(eq(expected_addr), eq(value)).returning(|_, _| Ok(()));
        },

        (RequestType::Unmapped, _) => {
            device.expect_is_addr_in_address_space().returning(|_| false);
            device.expect_read_byte().times(0);
            device.expect_write_byte().times(0);
        },

        (RequestType::None, _) => {
            device.expect_is_addr_in_address_space().returning(|_| false);
        },

        _ => {}
    }

    device
}

fn create_nes_bus() -> NESBus {
    NESBus::new()
}

fn create_nes_bus_with_bus_device(expected_addr: u16, request: RequestType, length: RequestData) -> NESBus {
    let device = create_bus_device_with_expectations(DEFAULT_MEMORY_SIZE, DEFAULT_MEMORY_RANGE, expected_addr, request, length);
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

    let nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::Read, RequestData::Byte(expected_value));

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

    let nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::Read, RequestData::Word(expected_value));

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

    let nes_bus = create_nes_bus_with_bus_device(0, RequestType::None, RequestData::None);

    let result = nes_bus.size();

    assert_eq!(result, BUS_ADDRESSABLE_SIZE);
}

#[test]
fn returns_bus_error_on_unmapped_access() {
    init();

    let expected_addr = 0x2000;
    let expected_value = 0xAB;

    let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::Unmapped, RequestData::None);

    let result0 = nes_bus.write_byte(expected_addr, expected_value);
    let result1 = nes_bus.read_byte(expected_addr);

    assert_eq!(result0, Ok(()));
    assert_eq!(result1, Ok(0x00));
}

#[test]
fn mirrors_content_when_address_space_si_larger_than_size() {
    init();

    let addresses = [(0x0000, 0x0000), (0x0800, 0x0000), (0x07FF, 0x07FF), (0x1000, 0x0000), (0x1800, 0x0000), (0x1FFF, 0x07FF)];
    let expected_value = 0xAB;

    for (virtual_addr, expected_addr) in addresses {
        let mut nes_bus = create_nes_bus_with_bus_device(expected_addr, RequestType::ReadWrite, RequestData::Byte(expected_value));

        let result0 = nes_bus.write_byte(virtual_addr, expected_value);
        let result1 = nes_bus.read_byte(virtual_addr);

        assert_eq!(result0, Ok(()));
        assert_eq!(result1, Ok(expected_value));
    }
}

#[test]
fn read_is_routed_to_right_device() {
    init();

    let expected_value = 0xAB;
    let virtual_addr = 0x8000;
    let expected_addr = 0x0000;

    let device0 = create_bus_device_with_expectations(
        2048, (0x0000, 0x07FF), 0, RequestType::None, RequestData::None);
    let device1 = create_bus_device_with_expectations(
        16384, (0x8000, 0xBFFF), expected_addr, RequestType::Read, RequestData::Byte(expected_value));

    let device0 = Rc::new(RefCell::new(device0));
    let device1 = Rc::new(RefCell::new(device1));

    let mut nes_bus = create_nes_bus();

    nes_bus.add_device(device0.clone()).expect("failed to add bus device");
    nes_bus.add_device(device1.clone()).expect("failed to add bus device");
    let result = nes_bus.read_byte(virtual_addr);

    assert_eq!(result, Ok(expected_value));
}