// Authorship: Human 0% | Claude 100%
//! Tests for AxROM (Mapper 7) cartridge implementation.

use std::io::{BufReader, Seek, SeekFrom, Write};
use tempfile::NamedTempFile;
use crate::cartridge::Cartridge;
use crate::ines_loader::Region;
use crate::mapper::axrom_cartridge::AxromCartridge;
use crate::memory::Memory;
use crate::memory_ciram::PpuNameTableMirroring;
use crate::tests::init;

/// Create a test AxROM cartridge with specified number of 32KB PRG banks.
/// Each bank is filled with its bank number for easy identification.
fn create_test_cartridge(num_banks: usize) -> AxromCartridge {
    let prg_rom_size = num_banks * 32 * 1024; // 32KB per bank
    let mut test_data = Vec::with_capacity(prg_rom_size);

    // Fill each bank with its bank number
    for bank in 0..num_banks {
        for _ in 0..(32 * 1024) {
            test_data.push(bank as u8);
        }
    }

    let mut temp_file = NamedTempFile::new().expect("failed to create temp file");
    temp_file.write_all(&test_data).expect("failed to write test data");
    temp_file.seek(SeekFrom::Start(0)).expect("failed to seek to start");

    let file = temp_file.reopen().expect("failed to reopen temp file");
    let reader = BufReader::new(file);

    AxromCartridge::new(
        reader,
        0,                  // prg_rom_offset
        prg_rom_size,       // prg_rom_size
        0,                  // chr_rom_size (must be 0 for AxROM)
        8 * 1024,           // chr_ram_size (8KB)
        0,                  // prg_ram_size (none)
        PpuNameTableMirroring::Horizontal, // initial mirroring (will be converted to SingleScreen)
        Region::NTSC,
    ).expect("failed to create AxROM cartridge")
}

#[test]
fn test_initial_bank_is_zero() {
    init();

    let cartridge = create_test_cartridge(4);
    assert_eq!(cartridge.get_current_bank(), 0);
}

#[test]
fn test_bank_selection_basic() {
    init();

    let mut cartridge = create_test_cartridge(8);

    // Select bank 3
    cartridge.write_byte(0x8000, 0x03).unwrap();
    assert_eq!(cartridge.get_current_bank(), 3);

    // Select bank 7
    cartridge.write_byte(0x8000, 0x07).unwrap();
    assert_eq!(cartridge.get_current_bank(), 7);

    // Select bank 0
    cartridge.write_byte(0x8000, 0x00).unwrap();
    assert_eq!(cartridge.get_current_bank(), 0);
}

#[test]
fn test_bank_selection_masks_upper_bits() {
    init();

    let mut cartridge = create_test_cartridge(16);

    // Write with upper bits set - only bits 0-3 should be used for bank
    cartridge.write_byte(0x8000, 0xF5).unwrap(); // 0xF5 & 0x0F = 5
    assert_eq!(cartridge.get_current_bank(), 5);

    cartridge.write_byte(0x8000, 0xE9).unwrap(); // 0xE9 & 0x0F = 9
    assert_eq!(cartridge.get_current_bank(), 9);
}

#[test]
fn test_bank_selection_wraps_modulo_num_banks() {
    init();

    // Create cartridge with 4 banks (banks 0-3)
    let mut cartridge = create_test_cartridge(4);
    assert_eq!(cartridge.get_num_banks(), 4);

    // Select bank 5 -> should wrap to 5 % 4 = 1
    cartridge.write_byte(0x8000, 0x05).unwrap();
    assert_eq!(cartridge.get_current_bank(), 1);

    // Select bank 15 -> should wrap to 15 % 4 = 3
    cartridge.write_byte(0x8000, 0x0F).unwrap();
    assert_eq!(cartridge.get_current_bank(), 3);

    // Select bank 8 -> should wrap to 8 % 4 = 0
    cartridge.write_byte(0x8000, 0x08).unwrap();
    assert_eq!(cartridge.get_current_bank(), 0);
}

#[test]
fn test_mirroring_initial_state_is_single_screen_lower() {
    init();

    let cartridge = create_test_cartridge(4);

    // AxROM should convert any initial mirroring to SingleScreenLower
    assert_eq!(
        cartridge.get_current_mirroring(),
        PpuNameTableMirroring::SingleScreenLower
    );
}

#[test]
fn test_mirroring_changes_with_bit_4() {
    init();

    let mut cartridge = create_test_cartridge(4);

    // Initial state: SingleScreenLower (bit 4 = 0)
    assert_eq!(
        cartridge.get_current_mirroring(),
        PpuNameTableMirroring::SingleScreenLower
    );

    // Set bit 4 = 1 -> SingleScreenUpper
    cartridge.write_byte(0x8000, 0x10).unwrap();
    assert_eq!(
        cartridge.get_current_mirroring(),
        PpuNameTableMirroring::SingleScreenUpper
    );

    // Clear bit 4 = 0 -> SingleScreenLower
    cartridge.write_byte(0x8000, 0x00).unwrap();
    assert_eq!(
        cartridge.get_current_mirroring(),
        PpuNameTableMirroring::SingleScreenLower
    );

    // Set bit 4 = 1 with bank select -> both should work
    cartridge.write_byte(0x8000, 0x13).unwrap(); // bank 3, upper screen
    assert_eq!(cartridge.get_current_bank(), 3);
    assert_eq!(
        cartridge.get_current_mirroring(),
        PpuNameTableMirroring::SingleScreenUpper
    );
}

#[test]
fn test_mirroring_shared_with_ppu() {
    init();

    let mut cartridge = create_test_cartridge(4);

    // Get the shared mirroring reference (same one PPU would use)
    let mirroring_ref = cartridge.get_mirroring();

    // Initial state
    assert_eq!(*mirroring_ref.borrow(), PpuNameTableMirroring::SingleScreenLower);

    // Change mirroring via cartridge write
    cartridge.write_byte(0x8000, 0x10).unwrap();

    // Verify the shared reference sees the change
    assert_eq!(*mirroring_ref.borrow(), PpuNameTableMirroring::SingleScreenUpper);
}

#[test]
fn test_read_from_correct_bank() {
    init();

    let mut cartridge = create_test_cartridge(4);

    // Bank 0 is filled with 0x00
    cartridge.write_byte(0x8000, 0x00).unwrap();
    assert_eq!(cartridge.read_byte(0x8000).unwrap(), 0x00);
    assert_eq!(cartridge.read_byte(0xFFFF).unwrap(), 0x00);

    // Bank 1 is filled with 0x01
    cartridge.write_byte(0x8000, 0x01).unwrap();
    assert_eq!(cartridge.read_byte(0x8000).unwrap(), 0x01);
    assert_eq!(cartridge.read_byte(0xFFFF).unwrap(), 0x01);

    // Bank 2 is filled with 0x02
    cartridge.write_byte(0x8000, 0x02).unwrap();
    assert_eq!(cartridge.read_byte(0x8000).unwrap(), 0x02);
    assert_eq!(cartridge.read_byte(0xFFFF).unwrap(), 0x02);

    // Bank 3 is filled with 0x03
    cartridge.write_byte(0x8000, 0x03).unwrap();
    assert_eq!(cartridge.read_byte(0x8000).unwrap(), 0x03);
    assert_eq!(cartridge.read_byte(0xFFFF).unwrap(), 0x03);
}

#[test]
fn test_chr_ram_is_readable_and_writable() {
    init();

    let cartridge = create_test_cartridge(4);
    let chr_ram = cartridge.get_chr_rom();

    // Write to CHR-RAM
    chr_ram.borrow_mut().write_byte(0x0000, 0xAB).unwrap();
    chr_ram.borrow_mut().write_byte(0x1000, 0xCD).unwrap();
    chr_ram.borrow_mut().write_byte(0x1FFF, 0xEF).unwrap();

    // Read back
    assert_eq!(chr_ram.borrow().read_byte(0x0000).unwrap(), 0xAB);
    assert_eq!(chr_ram.borrow().read_byte(0x1000).unwrap(), 0xCD);
    assert_eq!(chr_ram.borrow().read_byte(0x1FFF).unwrap(), 0xEF);
}

#[test]
fn test_write_to_any_prg_address_updates_mapper() {
    init();

    let mut cartridge = create_test_cartridge(8);

    // Writes to any address in $8000-$FFFF should update mapper state
    cartridge.write_byte(0x8000, 0x01).unwrap();
    assert_eq!(cartridge.get_current_bank(), 1);

    cartridge.write_byte(0x9000, 0x02).unwrap();
    assert_eq!(cartridge.get_current_bank(), 2);

    cartridge.write_byte(0xA000, 0x03).unwrap();
    assert_eq!(cartridge.get_current_bank(), 3);

    cartridge.write_byte(0xFFFF, 0x04).unwrap();
    assert_eq!(cartridge.get_current_bank(), 4);
}

#[test]
fn test_size_returns_32kb() {
    init();

    let cartridge = create_test_cartridge(4);

    // size() should return 32KB (the window size, not total PRG-ROM size)
    assert_eq!(cartridge.size(), 32 * 1024);
}

#[test]
fn test_prg_ram_not_present_by_default() {
    init();

    let cartridge = create_test_cartridge(4);

    // Our test helper creates cartridge with prg_ram_size = 0
    assert!(cartridge.get_prg_ram().is_none());
}

#[test]
fn test_prg_ram_present_when_specified() {
    init();

    let prg_rom_size = 4 * 32 * 1024; // 4 banks
    let mut test_data = vec![0u8; prg_rom_size];
    for (i, byte) in test_data.iter_mut().enumerate() {
        *byte = (i / (32 * 1024)) as u8;
    }

    let mut temp_file = NamedTempFile::new().expect("failed to create temp file");
    temp_file.write_all(&test_data).expect("failed to write test data");
    temp_file.seek(SeekFrom::Start(0)).expect("failed to seek to start");

    let file = temp_file.reopen().expect("failed to reopen temp file");
    let reader = BufReader::new(file);

    let cartridge = AxromCartridge::new(
        reader,
        0,
        prg_rom_size,
        0,              // chr_rom_size
        8 * 1024,       // chr_ram_size
        8 * 1024,       // prg_ram_size (8KB)
        PpuNameTableMirroring::Horizontal,
        Region::NTSC,
    ).expect("failed to create AxROM cartridge");

    // PRG-RAM should be present
    let prg_ram = cartridge.get_prg_ram();
    assert!(prg_ram.is_some());

    // Test PRG-RAM is readable/writable
    // Note: MemoryBank uses masked addresses (0x0000-0x1FFF for 8KB)
    // The bus handles mapping $6000-$7FFF to these addresses
    let prg_ram = prg_ram.unwrap();
    prg_ram.borrow_mut().write_byte(0x0000, 0x42).unwrap();
    assert_eq!(prg_ram.borrow().read_byte(0x0000).unwrap(), 0x42);
    prg_ram.borrow_mut().write_byte(0x1FFF, 0xAB).unwrap();
    assert_eq!(prg_ram.borrow().read_byte(0x1FFF).unwrap(), 0xAB);
}

#[test]
fn test_chr_rom_rejected() {
    init();

    let prg_rom_size = 32 * 1024;
    let test_data = vec![0u8; prg_rom_size];

    let mut temp_file = NamedTempFile::new().expect("failed to create temp file");
    temp_file.write_all(&test_data).expect("failed to write test data");
    temp_file.seek(SeekFrom::Start(0)).expect("failed to seek to start");

    let file = temp_file.reopen().expect("failed to reopen temp file");
    let reader = BufReader::new(file);

    // Try to create AxROM with CHR-ROM (should fail)
    let result = AxromCartridge::new(
        reader,
        0,
        prg_rom_size,
        8 * 1024,       // chr_rom_size > 0 (NOT ALLOWED)
        0,              // chr_ram_size
        0,
        PpuNameTableMirroring::Horizontal,
        Region::NTSC,
    );

    assert!(result.is_err());
    match result {
        Err(crate::cartridge::CartridgeError::Unsupported(msg)) => {
            assert!(msg.contains("CHR-ROM"));
        }
        _ => panic!("Expected Unsupported error for CHR-ROM"),
    }
}
