// Authorship: Human 95% | Claude 5%
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
    device.expect_get_device_type().returning(|| BusDeviceType::WRAM(MemoryType::StandardMemory));

    device.expect_size().returning(move || memory_size);
    device.expect_get_virtual_address_range().returning(move || memory_range);

    match (request, length) {
        (RequestType::Read, RequestData::Byte(value)) => {
            device.expect_read_byte().times(1).with(eq(expected_addr)).returning(move |_| Ok(value));
        },

        (RequestType::Write, RequestData::Byte(value)) => {
            device.expect_write_byte().times(1).with(eq(expected_addr), eq(value)).returning(|_, _| Ok(()));
        },

        (RequestType::Read, RequestData::Word(value)) => {
            // NESBus::read_word calls read_byte twice (low byte first, then high byte)
            let low = (value & 0xFF) as u8;
            let high = ((value >> 8) & 0xFF) as u8;
            device.expect_read_byte().times(1).with(eq(expected_addr)).returning(move |_| Ok(low));
            device.expect_read_byte().times(1).with(eq(expected_addr + 1)).returning(move |_| Ok(high));
        },

        (RequestType::Write, RequestData::Word(value)) => {
            // NESBus::write_word calls write_byte twice (low byte first, then high byte)
            let low = (value & 0xFF) as u8;
            let high = ((value >> 8) & 0xFF) as u8;
            device.expect_write_byte().times(1).with(eq(expected_addr), eq(low)).returning(|_, _| Ok(()));
            device.expect_write_byte().times(1).with(eq(expected_addr + 1), eq(high)).returning(|_, _| Ok(()));
        },

        (RequestType::ReadWrite, RequestData::Byte(value)) => {
            device.expect_read_byte().times(1).with(eq(expected_addr)).returning(move |_| Ok(value));
            device.expect_write_byte().times(1).with(eq(expected_addr), eq(value)).returning(|_, _| Ok(()));
        },

        (RequestType::ReadWrite, RequestData::Word(value)) => {
            // NESBus word operations call byte operations
            let low = (value & 0xFF) as u8;
            let high = ((value >> 8) & 0xFF) as u8;
            device.expect_write_byte().times(1).with(eq(expected_addr), eq(low)).returning(|_, _| Ok(()));
            device.expect_write_byte().times(1).with(eq(expected_addr + 1), eq(high)).returning(|_, _| Ok(()));
            device.expect_read_byte().times(1).with(eq(expected_addr)).returning(move |_| Ok(low));
            device.expect_read_byte().times(1).with(eq(expected_addr + 1)).returning(move |_| Ok(high));
        },

        (RequestType::Unmapped, _) => {
            device.expect_read_byte().times(0);
            device.expect_write_byte().times(0);
        },

        (RequestType::None, _) => {
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
fn open_bus_returns_last_data_bus_value() {
    init();

    let unmapped_addr = 0x2000;
    let expected_value = 0xAB;

    let mut nes_bus = create_nes_bus_with_bus_device(unmapped_addr, RequestType::Unmapped, RequestData::None);

    // Write to unmapped address - this updates the data bus value
    let result0 = nes_bus.write_byte(unmapped_addr, expected_value);
    // Read from unmapped address - should return the last data bus value (0xAB)
    let result1 = nes_bus.read_byte(unmapped_addr);

    assert_eq!(result0, Ok(()));
    assert_eq!(result1, Ok(expected_value));  // Open bus returns last data bus value
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

#[test]
fn data_bus_is_shared_with_external_reference() {
    // This test verifies that the data_bus returned by get_data_bus() is the same
    // Cell that NESBus uses internally. This is critical for DMC DMA + open bus behavior.
    init();

    let nes_bus = create_nes_bus();
    let data_bus = nes_bus.get_data_bus();

    // Initially data_bus should be 0
    assert_eq!(data_bus.get(), 0);

    // Set data_bus externally (simulating DMA setting it to 0x00)
    data_bus.set(0x42);

    // Read from unmapped address - should return data_bus value
    let result = nes_bus.read_byte(0x5000);  // Unmapped address

    assert_eq!(result, Ok(0x42), "Open bus should return data_bus value");

    // After read, data_bus should still be 0x42 (open bus returns same value)
    assert_eq!(data_bus.get(), 0x42);
}

#[test]
fn read_from_prg_rom_area_updates_data_bus() {
    // Simulates DMC DMA reading from PRG ROM and verifying data_bus is updated
    init();

    let prg_value = 0x00;  // DMC sample value (often 0x00 for silence)

    // Create a mock PRG ROM device at $8000-$BFFF
    let prg_device = create_bus_device_with_expectations(
        16384, (0x8000, 0xBFFF), 0x0000, RequestType::Read, RequestData::Byte(prg_value));
    let prg_device = Rc::new(RefCell::new(prg_device));

    let mut nes_bus = create_nes_bus();
    nes_bus.add_device(prg_device.clone()).expect("failed to add PRG ROM");

    let data_bus = nes_bus.get_data_bus();

    // Set data_bus to a different value first (simulating CPU fetch of $40)
    data_bus.set(0x40);
    assert_eq!(data_bus.get(), 0x40);

    // Read from PRG ROM (simulating DMC DMA) - should update data_bus to 0x00
    let result = nes_bus.read_byte(0x8000);
    assert_eq!(result, Ok(prg_value));

    // Verify data_bus was updated by the PRG ROM read
    assert_eq!(data_bus.get(), prg_value, "data_bus should be updated by PRG ROM read");

    // Now read from unmapped address (simulating open bus read from $4000)
    // Should return the data_bus value (0x00)
    let open_bus_result = nes_bus.read_byte(0x5000);  // Unmapped address
    assert_eq!(open_bus_result, Ok(prg_value), "Open bus should return updated data_bus");
}