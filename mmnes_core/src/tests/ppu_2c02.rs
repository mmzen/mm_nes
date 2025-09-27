use std::cell::RefCell;
use std::rc::Rc;
use log::debug;
use crate::bus::MockBusStub;
use crate::bus_device::{BusDeviceType, MockBusDeviceStub};
use crate::cpu::MockCpuStub;
use crate::memory::{Memory, MemoryError, MemoryType};
use crate::memory_ciram::PpuNameTableMirroring;
use crate::ppu_2c02::Ppu2c02;
use crate::tests::init;

const CHR_MEMORY_RANGE: (u16, u16) = (0x0000, 0x1FFF);
const CHR_MEMORY_SIZE: usize = 8192;
const CHR_NAME: &str = "Test CHR-ROM";
const PPU_EXTERNAL_MEMORY_SIZE: usize = 8;
const VALID_CH_ROM_ADDRESS: u16 = 0x1000;
const VALID_PALETTE_ADDRESS: u16 = 0x3FAB;
const VALID_NAME_TABLE_ADDRESS: u16 = 0x2100;
const VALID_DATA_VALUE: u8 = 0x14;
const CONTROL_REGISTER_INCR_1: u8 = 0x00;
const CONTROL_REGISTER_INCR_32: u8 = 0x04;

fn create_cpu() -> MockCpuStub {
    let cpu = MockCpuStub::new();
    cpu
}

#[allow(dead_code)]
fn create_bus() -> MockBusStub {
    let bus = MockBusStub::new();
    bus
}

fn create_ppu() -> Ppu2c02 {
    create_ppu_with_nametable_mirroring(PpuNameTableMirroring::Horizontal)
}

fn create_ppu_with_nametable_mirroring(mirroring: PpuNameTableMirroring) -> Ppu2c02 {
    let mut chr_rom = MockBusDeviceStub::new();
    let cpu = create_cpu();

    chr_rom.expect_size().returning(|| CHR_MEMORY_SIZE);
    chr_rom.expect_get_virtual_address_range().returning(|| CHR_MEMORY_RANGE);
    chr_rom.expect_get_device_type().returning(|| BusDeviceType::WRAM(MemoryType::StandardMemory));
    chr_rom.expect_get_name().returning(|| CHR_NAME.to_string());
    chr_rom.expect_read_byte().returning(move |addr| {
        if addr == VALID_CH_ROM_ADDRESS || addr == VALID_CH_ROM_ADDRESS + 1 {
            Ok(VALID_DATA_VALUE) }
        else {
            Err(MemoryError::OutOfRange(addr))
        }
    });

    Ppu2c02::new(
        Rc::new(RefCell::new(chr_rom)),
        mirroring,
        Rc::new(RefCell::new(cpu))
    ).unwrap()
}

fn write_address_to_addr_register(ppu: &mut Ppu2c02, value: u16) -> Result<(), MemoryError> {
    let high_byte = ((value & 0xFF00) >> 8) as u8;
    let low_byte = (value & 0x00FF) as u8;
    ppu.write_byte(0x06, high_byte)?;
    ppu.write_byte(0x06, low_byte)?;
    Ok(())
}

fn write_data_to_data_register(ppu: &mut Ppu2c02, value: u8) -> Result<(), MemoryError> {
    ppu.write_byte(0x07, value)?;
    Ok(())
}

fn set_v_increment(ppu: &mut Ppu2c02, value: u8) {
    match value {
        1 => ppu.write_byte(0x00, CONTROL_REGISTER_INCR_1).unwrap(),
        32 => ppu.write_byte(0x00, CONTROL_REGISTER_INCR_32).unwrap(),
        _ => panic!("invalid v increment value: {}", value)
    }
}

#[test]
fn test_initialize_ppu() {
    init();

    let mut ppu = create_ppu();
    assert_eq!(ppu.initialize().unwrap(), PPU_EXTERNAL_MEMORY_SIZE);
}

#[test]
fn read_write_byte_works() {
    init();

    let mut ppu = create_ppu();
    let address = 0x00;
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
        (0x00, "controller"), (0x01, "mask"), (0x02, "status"), (0x03, "oam_addr"),
        (0x05, "scroll")
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
    let address = 0x06;
    let value = (0xAB, 0xCD);
    let expected = 0xABCD & 0x3FFF;

    ppu.write_byte(address, value.0).unwrap();
    ppu.write_byte(address, value.1).unwrap();

    assert_eq!(ppu.get_v_value(), expected);
}

#[test]
fn write_to_addr_and_read_to_data_registers_to_chr_rom_works() {
    init();

    let mut ppu = create_ppu();
    let data = 0x07;

    set_v_increment(&mut ppu, 1);
    write_address_to_addr_register(&mut ppu, VALID_CH_ROM_ADDRESS).unwrap();

    let _ = ppu.read_byte(data).unwrap();
    let result = ppu.read_byte(data).unwrap();

    assert_eq!(result, VALID_DATA_VALUE);
}

#[test]
fn write_to_addr_and_data_and_read_to_data_registers_to_palette_works() {
    init();

    let mut ppu = create_ppu();
    let data = 0x07;

    write_address_to_addr_register(&mut ppu, VALID_PALETTE_ADDRESS).unwrap();
    ppu.write_byte(data, VALID_DATA_VALUE).unwrap();

    write_address_to_addr_register(&mut ppu, VALID_PALETTE_ADDRESS).unwrap();
    let result = ppu.read_byte(data).unwrap();

    assert_eq!(result, VALID_DATA_VALUE);
}

#[test]
fn read_to_data_registers_with_increments_to_name_tables_works() {
    init();

    let data = 0x07;
    let iterations: usize = 20;
    let increments: [usize; 2] = [1, 32];

    for inc in increments {
        let mut ppu = create_ppu();
        set_v_increment(&mut ppu, inc as u8);

        for (index, value) in (VALID_NAME_TABLE_ADDRESS..).step_by(inc).take(iterations).enumerate() {
            write_address_to_addr_register(&mut ppu, value).unwrap();
            ppu.write_byte(data, VALID_DATA_VALUE + index as u8).unwrap();
        }

        write_address_to_addr_register(&mut ppu, VALID_NAME_TABLE_ADDRESS).unwrap();
        let _ = ppu.read_byte(data).unwrap();

        for (index, _) in (VALID_NAME_TABLE_ADDRESS..).step_by(inc).take(iterations).enumerate() {
            let result = ppu.read_byte(data).unwrap();
            assert_eq!(result, VALID_DATA_VALUE + index as u8);
        }
    }
}

#[test]
fn write_to_data_registers_with_increments_to_name_tables_works() {
    init();

    let data = 0x07;
    let iterations: usize = 20;
    let increments: [usize; 2] = [1, 32];

    for inc in increments {
        let mut ppu = create_ppu();
        set_v_increment(&mut ppu, inc as u8);

        write_address_to_addr_register(&mut ppu, VALID_NAME_TABLE_ADDRESS).unwrap();

        for (index, _) in (VALID_NAME_TABLE_ADDRESS..).step_by(inc).take(iterations).enumerate() {
            ppu.write_byte(data, VALID_DATA_VALUE + index as u8).unwrap();
        }

        for (index, value) in (VALID_NAME_TABLE_ADDRESS..).step_by(inc).take(iterations).enumerate() {
            write_address_to_addr_register(&mut ppu, value).unwrap();
            let _ = ppu.read_byte(data).unwrap();
            let result = ppu.read_byte(data).unwrap();
            assert_eq!(result, VALID_DATA_VALUE + index as u8);
        }
    }
}

fn test_for_values_at_addresses(ppu: &mut Ppu2c02, value: u8, addresses: &[(u16, u16)]) {
    for addr in addresses {
        println!("writing at 0x{:04X}, expected value (0x{:02X}) should be at 0x{:04X} and 0x{:04X}",
                 addr.0, value, addr.0, addr.1);

        write_address_to_addr_register(ppu, addr.0).unwrap();
        write_data_to_data_register(ppu, value).unwrap();

        write_address_to_addr_register(ppu, addr.0).unwrap();
        ppu.read_byte(0x07).unwrap();
        assert_eq!(ppu.read_byte(0x07).unwrap(), value);

        write_address_to_addr_register(ppu, addr.1).unwrap();
        ppu.read_byte(0x07).unwrap();
        assert_eq!(ppu.read_byte(0x07).unwrap(), value);
    }
}

#[test]
fn test_horizontal_nametable_read_write() {
    init();

    let mut ppu = create_ppu_with_nametable_mirroring(PpuNameTableMirroring::Horizontal);

    let nametable_addresses = [(0x2000, 0x2400), (0x23FF, 0x27FF), (0x2800, 0x2C00), (0x2BFF, 0x2FFF)];
    let value = 0xAB;

    set_v_increment(&mut ppu, 1);
    test_for_values_at_addresses(&mut ppu, value, &nametable_addresses);
}

#[test]
fn test_vertical_nametable_read_write() {
    init();

    let mut ppu = create_ppu_with_nametable_mirroring(PpuNameTableMirroring::Vertical);

    let nametable_addresses = [(0x2000, 0x2800), (0x23FF, 0x2BFF), (0x2400, 0x2C00), (0x27FF, 0x2FFF)];
    let value = 0xAB;

    set_v_increment(&mut ppu, 1);
    test_for_values_at_addresses(&mut ppu, value, &nametable_addresses);
}

#[test]
fn test_read_to_status_clears_vblank_and_reset_latch() {
    init();

    let mut ppu = create_ppu();
    let status = 0x02;
    let addr = 0x06;
    let status_value = 0xFF;
    let addr_value = 0xAB;

    ppu.write_byte(status, status_value).unwrap();
    ppu.write_byte(addr, addr_value).unwrap();

    let result0 = ppu.read_byte(status).unwrap();
    let result1 = ppu.read_byte(status).unwrap();

    assert_eq!(result0, status_value);
    assert_eq!(result1, status_value & 0x7F );

    ppu.write_byte(addr, addr_value).unwrap();
    let result2 = ppu.read_byte(addr).unwrap();
    assert_eq!(result2, 0x00);
}

#[test]
fn v_wraps_to_0x0000_when_incrementing_from_0x3fff() {
    init();

    let mut ppu = create_ppu();
    let data = 0xAB;

    set_v_increment(&mut ppu, 1);

    write_address_to_addr_register(&mut ppu, 0x3FFF).unwrap();
    write_data_to_data_register(&mut ppu, data).unwrap();
    let v = ppu.get_v_value();
    println!("V: 0x{:04X}", v);
    assert_eq!(ppu.get_v_value(), 0x0000);
}
