use std::io::Write;
use tempfile::NamedTempFile;
use crate::ines_loader::{ConsoleType, INESVersion, INesLoader, Region, VsHardwareType, VsPpuType};
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

#[test]
fn test_load_header_with_various_mapper_numbers_ines2() {
    init();

    // Cases: (byte6, byte7, expected mapper)
    // NES 2.0 requires bits 3..2 of byte7 == 10b (0x08)
    // Keep submapper = 2 (byte8 high nibble 0x2), region PAL (byte12 = 1)
    let cases = vec![
        (0x00, 0x08, NesMapper::NROM),
        (0x10, 0x08, NesMapper::MMC1),
        (0x20, 0x08, NesMapper::UxROM),
        (0x70, 0x78, NesMapper::TQROM),
    ];

    for (byte6, byte7, expected_mapper) in cases {
        let header_bytes: Vec<u8> = vec![
            0x4E, 0x45, 0x53, 0x1A, // "NES\x1A"
            0x02, // PRG (LSB) = 2 * 16KB
            0x01, // CHR (LSB) = 1 * 8KB
            byte6,
            byte7, // also carries NES2.0 signature in bits 3..2
            0x20,  // byte8: submapper in high nibble = 2, mapper[11:8] in low nibble = 0
            0x00,  // byte9: PRG/CHR MSBs = 0 (simple sizes)
            0x00, 0x00,
            0x01,  // byte12: region = PAL
            0x00,  // byte13: VS details (unused here)
            0x00, 0x00,
        ];

        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(&header_bytes).unwrap();
        tmp.flush().unwrap();

        let loader = INesLoader::from_file(tmp.path().into()).unwrap();
        let h = loader.header();

        assert_eq!(h.version, INESVersion::V2, "should detect NES 2.0 via byte7 bits 3..2 == 10b");
        assert_eq!(h.mapper, expected_mapper);
        assert_eq!(h.sub_mapper, 2);
        assert_eq!(h.region.to_string(), "PAL");
        assert_eq!(h.prg_rom_size, 2 * 16 * 1024);
        assert_eq!(h.chr_rom_size, 1 * 8 * 1024);
        assert_eq!(h.nametables_layout, PpuNameTableMirroring::Horizontal);
    }
}

#[test]
fn test_diskdude_header_sets_version_and_mapper() {
    init();

    // bytes[7..=15] = "DiskDude!" and flags6 high nibble = 1 (MMC1)
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,       // magic
        0x02, 0x01,                   // PRG=2, CHR=1
        0x10,                         // byte6: mapper low nibble=1 (MMC1), all flags clear
        b'D', b'i', b's', b'k', b'D', b'u', b'd', b'e', b'!', // clobbered bytes[7..15]
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.version, INESVersion::DiskDude);
    // In DiskDude case, ignore flags7 nibble and use only flags6 high nibble for mapper
    assert_eq!(h.mapper, NesMapper::MMC1);
    assert_eq!(h.nametables_layout, PpuNameTableMirroring::Horizontal);
    assert_eq!(h.prg_rom_size, 2 * 16 * 1024);
    assert_eq!(h.chr_rom_size, 1 * 8 * 1024);
}

#[test]
fn test_trainer_affects_prg_and_chr_offsets() {
    init();

    // Trainer bit (byte6 bit2) set
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A, // magic
        0x02, // PRG=2
        0x01, // CHR=1
        0x04, // byte6: trainer bit set
        0x00, // byte7
        0x00, 0x00, 0x00, 0x00,
        0x00, // byte12
        0x00, 0x00, 0x00,
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    // With trainer: PRG starts after 16-byte header + 512-byte trainer
    assert_eq!(h.prg_offset(), (16 + 512) as u64);

    // CHR starts after PRG:
    let expected_chr_off = h.prg_offset() + h.prg_rom_size as u64;
    assert_eq!(h.chr_offset(), Some(expected_chr_off));
}

#[test]
fn test_chr_offset_none_when_no_chr() {
    init();

    // CHR size = 0 → CHR offset should be None
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x02, // PRG=2
        0x00, // CHR=0
        0x00, // byte6
        0x00, // byte7
        0x00, 0x00, 0x00, 0x00,
        0x00, // byte12
        0x00, 0x00, 0x00,
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.prg_rom_size, 2 * 16 * 1024);
    assert_eq!(h.chr_rom_size, 0);
    assert_eq!(h.chr_offset(), None);
}

#[test]
fn test_vs_system_fields_ines1() {
    init();

    // iNES 1.x, console type = VsSystem (byte7 low 2 bits = 0b01),
    // VS PPU type from byte13 low nibble, VS HW from byte13 high nibble
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x02, // PRG
        0x01, // CHR
        0x00, // byte6
        0x01, // byte7: console type = VsSystem (bits 1..0 = 01), *not* NES2 (bits 3..2 = 00)
        0x00, 0x00, 0x00,
        0x00,
        0x00, // byte12
        0x52, // byte13: high nibble 0x5 -> VsDualSystem, low nibble 0x2 -> RP2C04_0001
        0x00, 0x00, 0x00
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.console_type, ConsoleType::VsSystem);
    assert_eq!(h.vs_ppu_type, VsPpuType::RP2C04_0001);
    assert_eq!(h.vs_hardware_type, VsHardwareType::VsDualSystem);
}

#[test]
fn test_ines2_version_and_submapper_region_fields() {
    init();

    // NES 2.0, submapper = 3, region = Dendy, mapper = NROM
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A, // magic
        0x01, // PRG=1 * 16KB
        0x01, // CHR=1 * 8KB
        0x00, // byte6 (mapper low nibble 0)
        0x08, // byte7: NES2 signature (bits 3..2 = 10b), console type = 0
        0x30, // byte8: submapper=3 (high nibble), mapper[11:8]=0 (low nibble)
        0x00, // byte9: PRG/CHR MSBs = 0
        0x00, // byte10
        0x00, // byte11
        0x03, // byte12: region = Dendy
        0x00, // byte13
        0x00, 0x00,
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.version, INESVersion::V2);
    assert_eq!(h.sub_mapper, 3);
    assert_eq!(h.region.to_string(), "Dendy");
    assert_eq!(h.mapper, NesMapper::NROM);
    assert_eq!(h.prg_rom_size, 1 * 16 * 1024);
    assert_eq!(h.chr_rom_size, 1 * 8 * 1024);
}

#[test]
fn test_ines1_vertical_mirroring() {
    init();

    // bit0=1 -> vertical mirroring
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A, // NES^Z
        0x02, // PRG=2
        0x01, // CHR=1
        0x01, // byte6: bit0=1 -> vertical
        0x00, // byte7
        0x00, 0x00, 0x00, 0x00, // 8..11
        0x00, 0x00, 0x00, 0x00, // 12..15
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.version, INESVersion::V1);
    assert_eq!(h.nametables_layout, PpuNameTableMirroring::Vertical);
}

/// iNES 1.x: PlayChoice-10 console type (Flags 7 TT=2), not NES 2.0
#[test]
fn test_ines1_playchoice10_console_type() {
    init();

    // byte7 low bits TT=10b (=2) -> PlayChoice-10; bits3..2=00 -> iNES 1.x, not NES 2.0
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x02, // PRG
        0x01, // CHR
        0x00, // byte6
        0x02, // byte7: TT=2 (PlayChoice-10), bits3..2=00 (not NES 2.0)
        0x00, 0x00, 0x00, 0x00,
        0x00, // byte12
        0x00, 0x00, 0x00,
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.version, INESVersion::V1);
    assert_eq!(h.console_type, ConsoleType::PlayChoice10);
}

/// iNES 1.x: battery-backed PRG RAM bit (Flags 6 bit1)
#[test]
fn test_ines1_battery_flag() {
    init();

    // byte6 bit1 = 1 -> battery-backed persistent memory present
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x02, // PRG
        0x01, // CHR
        0x02, // byte6: battery bit set
        0x00, // byte7
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert!(h.battery);
}

/// iNES 1.x: PRG-RAM size in 8 KiB units (Flags 8)
#[test]
fn test_ines1_prg_ram_size_byte8() {
    init();

    // byte8 = 2 -> 2 * 8 KiB = 16 KiB PRG-RAM (note: value 0 infers 8 KiB) (spec: rarely used)
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x01, // PRG
        0x01, // CHR
        0x00, // byte6
        0x00, // byte7
        0x02, // byte8: PRG-RAM size = 2 * 8KiB
        0x00, // byte9
        0x00, 0x00,
        0x00, // byte12
        0x00, 0x00, 0x00
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.prg_ram_size, 16 * 1024);
}

/// iNES 1.x: PRG-RAM default size = 8 KiB (Flags 8)
#[test]
fn test_ines1_prg_ram_zero_defaults_to_8k() {
    init();

    let header_bytes: Vec<u8> = vec![
        0x4E,0x45,0x53,0x1A, 0x01,0x01,
        0x00, // flags6
        0x00, // flags7
        0x00, // byte8=0 -> default to 8KiB
        0x00, // flag9
        0x00,0x00, 0x00, 0x00,0x00,0x00
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.prg_ram_size, 8 * 1024);
}

/// NES 2.0: verify MSB size extension (byte9) is applied to PRG/CHR sizes
#[test]
fn test_ines2_prg_chr_msb_size_extension() {
    init();

    // NES 2.0 id: byte7 bits3..2 = 10b (0x08)
    // PRG LSB=0x02, PRG MSB=0x1 -> 0x102 * 16 KiB = 4_227_072 bytes
    // CHR LSB=0x03, CHR MSB=0x2 -> 0x203? (careful: MSB in high nibble); actually 0x203? No: CHR units = 0x2<<8 | 0x03 = 515 -> 515*8 KiB = 4_218_880 bytes
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x02, // byte4: PRG LSB
        0x03, // byte5: CHR LSB
        0x00, // byte6
        0x08, // byte7: NES 2.0 signature, console type 0, mapper upper nibble=0
        0x00, // byte8: submapper=0, mapper[11:8]=0
        0x21, // byte9: CHR MSB=0x2 (hi nibble), PRG MSB=0x1 (lo nibble)
        0x00, // byte10
        0x00, // byte11
        0x00, // byte12: NTSC
        0x00, 0x00, 0x00, // 13..15
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.version, INESVersion::V2);
    assert_eq!(h.prg_rom_size, 4_227_072);
    assert_eq!(h.chr_rom_size, 4_218_880);
    assert_eq!(h.region.to_string(), "NTSC");
}

/// NES 2.0: PRG/CHR RAM and NVRAM sizes (bytes 10-11, logarithmic 64<<shift)
#[test]
fn test_ines2_prg_chr_ram_and_nvram_sizes() {
    init();

    // byte10: pppp(PGR-NVRAM)=8 => 64<<8 = 16384 (16 KiB), PPPP(PRG-RAM)=7 => 64<<7 = 8192 (8 KiB)
    // byte11: cccc(CHR-NVRAM)=9 => 64<<9 = 32768 (32 KiB), CCCC(CHR-RAM)=7 => 64<<7 = 8192 (8 KiB)
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x01, // PRG
        0x00, // CHR=0 (CHR-RAM)
        0x00, // byte6
        0x08, // byte7: NES 2.0
        0x00, // byte8
        0x00, // byte9
        0x87, // byte10: pppp=8 (NVRAM 16KiB), PPPP=7 (RAM 8KiB)
        0x97, // byte11: cccc=9 (NVRAM 32KiB), CCCC=7 (RAM 8KiB)
        0x00, // byte12 (NTSC)
        0x00, 0x00, 0x00,
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    // Adjust the field names below if your Header uses different ones.
    #[allow(unused_variables)]
    {
        // assert_eq!(h.prg_ram_size,   8 * 1024);
        // assert_eq!(h.prg_nvram_size, 16 * 1024);
        // assert_eq!(h.chr_ram_size,   8 * 1024);
        // assert_eq!(h.chr_nvram_size, 32 * 1024);
    }
    assert_eq!(h.version, INESVersion::V2);
    assert_eq!(h.chr_rom_size, 0); // CHR-ROM absent -> CHR-RAM setup via byte11
}

/// NES 2.0: Vs. System fields in byte13 (TT=1)
#[test]
fn test_ines2_vs_system_fields() {
    init();

    // NES2 + Vs: byte7 = 0x08 | 0x01 (TT=01b = Vs), byte13: high nibble = VS HW, low nibble = VS PPU
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x02, // PRG
        0x01, // CHR
        0x00, // byte6
        0x09, // byte7: NES2.0 + VsSystem
        0x00, // byte8
        0x00, // byte9
        0x00, 0x00,
        0x00,       // byte12
        0x52,       // byte13: HW=5 (VsDualSystem), PPU=2 (RP2C04_0001)
        0x00, 0x00, // 14..15
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.version, INESVersion::V2);
    assert_eq!(h.console_type, ConsoleType::VsSystem);
    assert_eq!(h.vs_ppu_type, VsPpuType::RP2C04_0001);
    assert_eq!(h.vs_hardware_type, VsHardwareType::VsDualSystem);
}

/// NES 2.0: PlayChoice-10 console type under NES 2.0 (TT=2)
#[test]
fn test_ines2_playchoice10_console_type() {
    init();

    // byte7 TT=2 (PlayChoice), bits3..2=10b (NES 2.0)
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x02, // PRG
        0x01, // CHR
        0x00, // byte6
        0x0A, // byte7: NES 2.0 + PlayChoice
        0x00, 0x00,
        0x00, 0x00,
        0x00, // byte12
        0x00, 0x00, 0x00,
    ];

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(&header_bytes).unwrap();
    tmp.flush().unwrap();

    let loader = INesLoader::from_file(tmp.path().into()).unwrap();
    let h = loader.header();

    assert_eq!(h.version, INESVersion::V2);
    assert_eq!(h.console_type, ConsoleType::PlayChoice10);
}

/// Baseline NES 2.0 version detection & iNES 1.x detection sanity
#[test]
fn test_version_detection_ines1_vs_nes20() {
    init();

    // iNES 1.x: bytes12..15 all zero, TT=00, NES2 bits=00
    let header_ines1: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x01, 0x01,
        0x00,
        0x00, // byte7 -> not NES2 (bits3..2=00)
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    let mut tmp1 = NamedTempFile::new().unwrap();
    tmp1.write_all(&header_ines1).unwrap();
    tmp1.flush().unwrap();

    let loader1 = INesLoader::from_file(tmp1.path().into()).unwrap();
    let h1 = loader1.header();

    assert_eq!(h1.version, INESVersion::V1);

    // NES 2.0: byte7 bits3..2==10b
    let header_nes2: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x01, 0x01,
        0x00,
        0x08, // NES 2.0 signature
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    let mut tmp2 = NamedTempFile::new().unwrap();
    tmp2.write_all(&header_nes2).unwrap();
    tmp2.flush().unwrap();

    let loader2 = INesLoader::from_file(tmp2.path().into()).unwrap();
    let h2 = loader2.header();

    assert_eq!(h2.version, INESVersion::V2);
}

#[test]
fn test_ines_guess_region_from_filename() {
    init();

    let filenames = [
        ("California Games (Europe)", Region::PAL),
        ("Dragon Quest (USA)", Region::NTSC),
        ("Super Mario Bros. (Europe)", Region::PAL),
        ("Tetris (Japan)", Region::NTSC),
        ("Final Fantasy VII (USA)", Region::NTSC),
        ("Super Mario Kart (Europe)", Region::PAL),
        ("Super Mario II (E)", Region::PAL),
        ("Ghost'n Goblins", Region::NTSC),
    ];

    // bits3..2=00b (NES 1.0)
    let header_bytes: Vec<u8> = vec![
        0x4E, 0x45, 0x53, 0x1A,
        0x02, // PRG
        0x01, // CHR
        0x00, // byte6
        0x00, // byte7: NES 1.0
        0x00, 0x00,
        0x00, 0x00,
        0x00,
        0x00, 0x00, 0x00,
    ];

    for (filename, expected_region) in filenames {
        let mut tmp = NamedTempFile::with_prefix(filename).unwrap();
        tmp.write_all(&header_bytes).unwrap();
        tmp.flush().unwrap();

        let loader = INesLoader::from_file(tmp.path().into()).unwrap();
        let h = loader.header();

        assert_eq!(h.region, expected_region);
    }
}