use std::io::Write;
use tempfile::NamedTempFile;
use crate::ines_loader::INesLoader;
use crate::loader::Loader;
use crate::mapper::NesMapper;
use crate::memory_ciram::PpuNameTableMirroring;
use crate::tests::init;

#[test]
fn test_load_header_success() {
    init();

    // Create a valid iNES header with correct magic bytes and basic data
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A, // "NES\x1A" magic
        0x02, // 2 * 16KB PRG-ROM
        0x01, // 1 * 8KB CHR-ROM
        0x00, // Horizontal mirroring, no battery, no trainer
        0x00, // Mapper 0 (NROM)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 // Padding
    ];

    // Create a temporary file with the header
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(&header_bytes).unwrap();
    temp_file.flush().unwrap();

    // Test load_header
    let result = INesLoader::from_file(temp_file.path().into());

    assert!(result.is_ok());
    let loader = result.unwrap();
    assert_eq!(loader.header().prg_rom_size, 2 * 16 * 1024);
    assert_eq!(loader.header().chr_rom_size, 1 * 8 * 1024);
    assert_eq!(loader.header().nametables_layout, PpuNameTableMirroring::Horizontal);
}

#[test]
fn test_load_header_invalid_size() {
    init();

    // Create a file with fewer than 16 bytes (incomplete header)
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A, // "NES\x1A" magic
        0x02, 0x01, 0x00, 0x00  // Only 8 bytes total
    ];

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(&header_bytes).unwrap();
    temp_file.flush().unwrap();

    let result = INesLoader::from_file(temp_file.path().into());

    assert!(result.is_err());
}

#[test]
fn test_load_header_invalid_magic() {
    init();

    // Create a header with invalid magic bytes
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x00, // Invalid magic (NES\x00 instead of NES\x1A)
        0x02, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
    ];

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(&header_bytes).unwrap();
    temp_file.flush().unwrap();

    let result = INesLoader::from_file(temp_file.path().into());

    assert!(result.is_err());
}

#[test]
fn test_load_header_with_prg_rom_size() {
    init();

    // Create a header with PRG ROM size = 4 * 16KB = 64KB
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A, // "NES\x1A" magic
        0x04, // 4 * 16KB PRG-ROM
        0x00, // 0 CHR-ROM
        0x00, // Horizontal mirroring, no battery, no trainer
        0x00, // Mapper 0 (NROM)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 // Padding
    ];

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(&header_bytes).unwrap();
    temp_file.flush().unwrap();

    let result = INesLoader::from_file(temp_file.path().into());

    assert!(result.is_ok());
    let loader = result.unwrap();
    assert_eq!(loader.header().prg_rom_size, 4 * 16 * 1024);
}

#[test]
fn test_load_header_with_chr_rom_size() {
    init();

    // Create a header with CHR ROM size = 2 * 8KB = 16KB
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A, // "NES\x1A" magic
        0x02, // 2 * 16KB PRG-ROM
        0x02, // 2 * 8KB CHR-ROM
        0x00, // Horizontal mirroring, no battery, no trainer
        0x00, // Mapper 0 (NROM)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 // Padding
    ];

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(&header_bytes).unwrap();
    temp_file.flush().unwrap();

    let result = INesLoader::from_file(temp_file.path().into());

    assert!(result.is_ok());
    let loader = result.unwrap();
    assert_eq!(loader.header().chr_rom_size, 2 * 8 * 1024);
}

#[test]
fn test_load_header_with_various_mapper_numbers_ines1() {
    init();

    // Test different mapper numbers encoded in bytes 6 and 7
    let test_cases = vec![
        (0x00, 0x00, NesMapper::NROM),
        (0x10, 0x00, NesMapper::MMC1),
        (0x20, 0x00, NesMapper::UxROM),
        (0x70, 0x70, NesMapper::TQROM),
    ];

    for (byte6, byte7, expected_mapper_id) in test_cases {
        let header_bytes: Vec<u8> = vec![
            0x4E, 0x45, 0x53, 0x1A, // "NES\x1A" magic
            0x02, // 2 * 16KB PRG-ROM
            0x01, // 1 * 8KB CHR-ROM
            byte6, // Mapper lower nibble + flags
            byte7, // Mapper upper nibble + flags
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 // Padding
        ];

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&header_bytes).unwrap();
        temp_file.flush().unwrap();

        let result = INesLoader::from_file(temp_file.path().into());

        assert!(result.is_ok());
        let loader = result.unwrap();
        assert_eq!(loader.header().mapper, expected_mapper_id);
    }
}
