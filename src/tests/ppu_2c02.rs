use std::cell::RefCell;
use std::rc::Rc;
use log::debug;
use crate::bus::MockBusStub;
use crate::bus_device::{BusDevice, MockBusDeviceStub};
use crate::memory::{Memory, MemoryError};
use crate::ppu::PpuNameTableMirroring;
use crate::ppu_2c02::Ppu2c02;
use crate::tests::init;

const CHR_MEMORY_RANGE: (u16, u16) = (0x0000, 0x1FFF);
const CHR_MEMORY_SIZE: usize = 8192;
const CHR_NAME: &str = "Test CHR-ROM";
const PPU_EXTERNAL_MEMORY_RANGE: (u16, u16) = (0x2000, 0x3FFF);
const PPU_EXTERNAL_MEMORY_SIZE: usize = 8;
const VALID_DATA_ADDRESS: u16 = 0x1000;
const VALID_DATA_VALUE: u8 = 0x14;

fn check_memory(_: Ppu2c02) {
}

fn create_bus() -> MockBusStub {
    let bus = MockBusStub::new();
    bus
}

fn create_ppu() -> Ppu2c02 {
    let mut chr_rom = MockBusDeviceStub::new();

    chr_rom.expect_size().returning(|| CHR_MEMORY_SIZE);
    chr_rom.expect_get_address_range().returning(|| CHR_MEMORY_RANGE);
    chr_rom.expect_get_name().returning(|| CHR_NAME.to_string());
    chr_rom.expect_is_addr_in_address_space().returning(|addr| addr == VALID_DATA_ADDRESS);
    chr_rom.expect_read_byte().returning(move |addr|
        if addr == VALID_DATA_ADDRESS { Ok(VALID_DATA_VALUE) } else { Err(MemoryError::OutOfRange(addr)) }
    );

    Ppu2c02::new(Rc::new(RefCell::new(chr_rom)), PpuNameTableMirroring::Horizontal).unwrap()
}

#[test]
fn test_initialize_ppu() {
    init();

    let mut ppu = create_ppu();
    assert_eq!(ppu.initialize().unwrap(), PPU_EXTERNAL_MEMORY_SIZE);

    check_memory(ppu)
}

#[test]
fn is_in_boundary_works() {
    init();

    let ppu = create_ppu();

    assert_eq!(ppu.is_addr_in_address_space(0x0000), false);
    assert_eq!(ppu.is_addr_in_address_space(0x4000), false);
    assert_eq!(ppu.is_addr_in_address_space(0x2008), true);
}

#[test]
fn read_write_byte_works() {
    init();

    let mut ppu = create_ppu();
    let address = 0x2000;
    let value = 0xAB;

    ppu.write_byte(address, value).unwrap();
    assert_eq!(ppu.read_byte(address).unwrap(), value);
}

#[test]
fn read_write_word_raise_error() {
    init();

    let mut ppu = create_ppu();
    let address = 0x2000;
    let value = 0xAB;

    assert_eq!(
        ppu.write_word(address, value),
        Err(MemoryError::OutOfRange(address))
    );
}

#[test]
fn read_write_to_registers_works() {
    init();

    let mut ppu = create_ppu();
    let registers = [
        (0x2000, "controller"), (0x2001, "mask"), (0x2002, "status"), (0x2003, "oam_addr"),
        (0x2005, "scroll")
    ];
    let value = 0xAB;

    for register in &registers {
        ppu.write_byte(register.0, value).unwrap();
        let result = ppu.get_register_value(register.1);

        debug!("0x{:04X} - {}: expected: 0x{:04X}, result: 0x{:04X}", register.0, register.1, value, result);
        assert_eq!(result, value);
    }
}

#[test]
fn read_write_to_addr_register_works() {
    let mut ppu = create_ppu();
    let address = 0x2006;
    let value = (0xAB, 0xCD);
    let expected = 0xABCD;

    ppu.write_byte(address, value.0).unwrap();
    ppu.write_byte(address, value.1).unwrap();

    assert_eq!(ppu.get_v_value(), expected);
}

#[test]
fn write_to_addr_and_read_to_data_registers_works() {
    init();

    let mut ppu = create_ppu();
    let addr = 0x2006;
    let data = 0x2007;

    let addr_value = (((VALID_DATA_ADDRESS & 0xFF00) >> 8) as u8, (VALID_DATA_ADDRESS & 0x00FF) as u8);

    ppu.write_byte(addr, addr_value.0).unwrap();
    ppu.write_byte(addr, addr_value.1).unwrap();
    let _ = ppu.read_byte(data).unwrap();
    let result = ppu.read_byte(data).unwrap();

    assert_eq!(result, VALID_DATA_VALUE);
}



