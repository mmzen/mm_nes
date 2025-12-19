// Authorship: Human 80% | Claude 20%
use std::cell::RefCell;
use std::rc::Rc;
use log::debug;
use crate::bus::MockBusStub;
use crate::bus_device::{BusDeviceType, MockBusDeviceStub};
use crate::config_spec::ConfigSpec;
use crate::cpu::MockCpuStub;
use crate::ines_loader::Region;
use crate::memory::{Memory, MemoryError, MemoryType};
use crate::memory_ciram::PpuNameTableMirroring;
use crate::ppu::PPU;
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

/// Create a CPU mock with expectations for NMI signaling (needed for PPU timing tests)
fn create_cpu_with_nmi_expectations() -> MockCpuStub {
    let mut cpu = MockCpuStub::new();
    cpu.expect_clear_nmi().returning(|| Ok(()));
    cpu.expect_signal_nmi().returning(|| Ok(()));
    cpu
}

fn create_ppu_for_timing_tests() -> Ppu2c02 {
    let mut chr_rom = MockBusDeviceStub::new();
    let cpu = create_cpu_with_nmi_expectations();

    chr_rom.expect_size().returning(|| CHR_MEMORY_SIZE);
    chr_rom.expect_get_virtual_address_range().returning(|| CHR_MEMORY_RANGE);
    chr_rom.expect_get_device_type().returning(|| BusDeviceType::WRAM(MemoryType::StandardMemory));
    chr_rom.expect_get_name().returning(|| CHR_NAME.to_string());
    chr_rom.expect_read_byte().returning(|_| Ok(0));

    let config = ConfigSpec::from_region(Region::NTSC);

    Ppu2c02::new(
        Rc::new(RefCell::new(chr_rom)),
        Rc::new(RefCell::new(PpuNameTableMirroring::Horizontal)),
        Rc::new(RefCell::new(cpu)),
        config,
    ).unwrap()
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

    let config = ConfigSpec::from_region(Region::NTSC);

    Ppu2c02::new(
        Rc::new(RefCell::new(chr_rom)),
        Rc::new(RefCell::new(mirroring)),
        Rc::new(RefCell::new(cpu)),
        config,
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
fn write_to_control_register_works() {
    init();

    let mut ppu = create_ppu();
    let address = 0x00; // PPU Control register ($2000)
    let value = 0xAB;

    // Write to control register
    ppu.write_byte(address, value).unwrap();

    // Note: PPU Control ($2000) is write-only on real hardware.
    // Reading it returns open bus, not the written value.
    // We use the internal inspection method to verify the write worked.
    assert_eq!(ppu.get_register_value("controller"), value);
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

    // First read of status returns:
    // - Bits 7,6,5 from status register (0xFF & 0xE0 = 0xE0)
    // - Bits 4-0 from open bus (last write was 0xAB, so 0xAB & 0x1F = 0x0B)
    // Result: 0xE0 | 0x0B = 0xEB
    let result0 = ppu.read_byte(status).unwrap();
    // Second read should have VBlank cleared (bit 7 = 0), same open bus
    // Open bus is now 0xEB (from previous read), so:
    // - Status bits: 0x7F & 0xE0 = 0x60
    // - Open bus bits: 0xEB & 0x1F = 0x0B
    // Result: 0x60 | 0x0B = 0x6B
    let result1 = ppu.read_byte(status).unwrap();

    let expected0 = (status_value & 0xE0) | (addr_value & 0x1F);  // 0xEB
    let expected1 = ((status_value & 0x7F) & 0xE0) | (expected0 & 0x1F);  // 0x6B
    assert_eq!(result0, expected0);
    assert_eq!(result1, expected1);

    // Reading status resets the address latch (write toggle)
    // To verify latch was reset: write two bytes to PPUADDR again
    // and verify the address was set correctly via V register
    ppu.write_byte(addr, 0x21).unwrap();  // High byte
    ppu.write_byte(addr, 0x00).unwrap();  // Low byte
    // V should be 0x2100 (masked to 14 bits)
    assert_eq!(ppu.get_v_value(), 0x2100);
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

// ============================================================================
// Dot-Accurate Timing Tests (Phase 6 - Cycle-Accurate Refactoring Validation)
// ============================================================================

/// Test that PPU starts at pre-render scanline (261 for NTSC)
/// This is correct NES behavior - the PPU starts in a known state
#[test]
fn test_ppu_initial_timing_state() {
    init();

    let ppu = create_ppu_for_timing_tests();

    // PPU starts at pre-render scanline (261) at dot 0
    assert_eq!(ppu.get_current_dot(), 0, "PPU should start at dot 0");
    assert_eq!(ppu.get_current_scanline(), 261, "PPU should start at pre-render scanline 261");
    assert!(!ppu.is_vblank_set(), "VBlank should not be set initially");
}

/// Test that PPU advances dots correctly
/// 1 CPU cycle = 3 PPU dots
#[test]
fn test_ppu_dot_advancement() {
    init();

    let mut ppu = create_ppu_for_timing_tests();
    ppu.initialize().unwrap();

    // After init, we're at scanline 261 (pre-render), dot 0
    let initial_dot = ppu.get_current_dot();

    // Run for 1 CPU cycle (3 PPU dots)
    let _ = ppu.run(0, 1).unwrap();
    let after_1_cycle = ppu.get_current_dot();
    assert_eq!(after_1_cycle, initial_dot + 3, "After 1 CPU cycle, should advance 3 dots");

    // Run for 10 more CPU cycles (30 PPU dots)
    let _ = ppu.run(1, 10).unwrap();
    let after_11_cycles = ppu.get_current_dot();
    assert_eq!(after_11_cycles, initial_dot + 33, "After 11 CPU cycles, should advance 33 dots");
}

/// Test that PPU wraps from dot 340 to dot 0 and increments scanline
#[test]
fn test_ppu_scanline_wrap() {
    init();

    let mut ppu = create_ppu_for_timing_tests();
    ppu.initialize().unwrap();

    // PPU starts at scanline 261, dot 0
    // Run for 114 CPU cycles (342 PPU dots = 1 full scanline + 1 dot)
    let _ = ppu.run(0, 114).unwrap();

    // After 342 dots from scanline 261: should wrap to scanline 0 (new frame)
    let scanline = ppu.get_current_scanline();
    let dot = ppu.get_current_dot();
    println!("After 114 cycles from pre-render: scanline={}, dot={}", scanline, dot);

    // Should have wrapped to scanline 0 (visible scanlines start)
    assert_eq!(scanline, 0, "Should have wrapped to scanline 0 (new frame)");
    assert_eq!(dot, 1, "Should be at dot 1 after wrap");
}

/// Test that VBlank is set at scanline 241 (NTSC)
#[test]
fn test_vblank_timing_at_scanline_241() {
    init();

    let mut ppu = create_ppu_for_timing_tests();
    ppu.initialize().unwrap();

    // From pre-render scanline 261, we need to go through:
    // - 1 scanline to finish pre-render (261 -> frame wrap)
    // - 242 scanlines to reach VBlank dot 1 (0-240 visible + scanline 241 dot 1)
    // Total: 243 scanlines * 341 dots / 3 dots per cycle
    // Use ceiling division to ensure we pass the VBlank trigger point
    let dots_to_vblank = 243u32 * 341;  // 243 scanlines to ensure we're past VBlank trigger
    let cycles_to_vblank = (dots_to_vblank + 2) / 3;  // Ceiling division

    let _ = ppu.run(0, cycles_to_vblank).unwrap();

    let scanline = ppu.get_current_scanline();
    println!("After {} CPU cycles: scanline={}, dot={}, vblank={}",
             cycles_to_vblank, scanline, ppu.get_current_dot(), ppu.is_vblank_set());

    // VBlank should be set once we've passed scanline 241, dot 1
    assert!(scanline >= 241, "Should have reached scanline 241 or beyond");
    assert!(ppu.is_vblank_set(), "VBlank should be set at scanline 241");
}

/// Test that VBlank is cleared on pre-render scanline (261 for NTSC)
#[test]
fn test_vblank_cleared_on_prerender_scanline() {
    init();

    let mut ppu = create_ppu_for_timing_tests();
    ppu.initialize().unwrap();

    // First, run to VBlank (scanline 241+)
    let dots_to_vblank = 243u32 * 341;
    let cycles_to_vblank = (dots_to_vblank + 2) / 3;
    let _ = ppu.run(0, cycles_to_vblank).unwrap();
    assert!(ppu.is_vblank_set(), "VBlank should be set");

    // Now run through rest of frame to pre-render scanline (261)
    // From ~scanline 243, need to reach scanline 261 + 1 dot to clear flags
    // That's about 20 more scanlines
    let dots_to_prerender = 21u32 * 341;
    let cycles_to_prerender = (dots_to_prerender + 2) / 3;
    let _ = ppu.run(cycles_to_vblank, cycles_to_prerender).unwrap();

    println!("After pre-render: scanline={}, dot={}, vblank={}",
             ppu.get_current_scanline(), ppu.get_current_dot(), ppu.is_vblank_set());

    // VBlank should be cleared at pre-render scanline
    assert!(!ppu.is_vblank_set(), "VBlank should be cleared on pre-render scanline");
}
