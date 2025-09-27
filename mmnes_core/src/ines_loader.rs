use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::rc::Rc;
use log::info;
use crate::cartridge::Cartridge;
use crate::loader::{Loader, LoaderError};
use crate::mapper::NesMapper;
use crate::memory_ciram::PpuNameTableMirroring;
use crate::mmc1_cartridge::Mmc1Cartridge;
use crate::nrom_cartridge::NromCartridge;
use crate::unrom_cartridge::UnromCartridge;

const HEADER_SIZE: usize = 16;

pub trait FromINes: Debug {
    fn from_ines(file: File, header: INesRomHeader) -> Result<impl Cartridge, LoaderError>
    where
        Self: Sized;
}

#[derive(Debug)]
pub struct INesLoader {
    header: INesRomHeader,
    file: File
}

impl Loader for INesLoader {

    fn from_file(path: PathBuf) -> Result<INesLoader, LoaderError> {
        let mut file = File::open(path)?;
        let header = INesLoader::load_header(&mut file)?;

        let loader = INesLoader {
            header,
            file
        };

        Ok(loader)
    }

    fn build_cartridge(self) -> Result<Rc<RefCell<dyn Cartridge>>, LoaderError> {
        info!("building cartridge...");

        let cartridge: Rc<RefCell<dyn Cartridge>> = match self.header.mapper {
            NesMapper::NROM => Rc::new(RefCell::new(NromCartridge::from_ines(self.file, self.header)?)),
            NesMapper::UxROM => Rc::new(RefCell::new(UnromCartridge::from_ines(self.file, self.header)?)),
            NesMapper::MMC1 => Rc::new(RefCell::new(Mmc1Cartridge::from_ines(self.file, self.header)?)),
            _ => Err(LoaderError::UnsupportedMapper(self.header.mapper.name().to_string()))?
        };

        Ok(cartridge)
    }
}

impl INesLoader {

    fn load_header(file: &mut File) -> Result<INesRomHeader, LoaderError> {
        let mut buffer = vec![0u8; HEADER_SIZE];
        file.read_exact(&mut buffer)?;

        INesRomHeader::from_bytes(&buffer)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum ConsoleType {
    NesFamicom,
    VsSystem,
    PlayChoice10,
    Famiclone,
    NesFamicomWithEPSM,
    VrTechVT01,
    VrTechVT02,
    VrTechVT03,
    VrTechVT09,
    VrTechVT32,
    VrTechVT369,
    UmcUm6578,
    FamicomNetWorkSystem,
    Unknown,
}

impl Display for ConsoleType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsoleType::NesFamicom => write!(f, "NES Family Computer"),
            ConsoleType::VsSystem => write!(f, "Visual Studio System"),
            ConsoleType::PlayChoice10 => write!(f, "NES Family Computer PlayChoice 10"),
            ConsoleType::Famiclone => write!(f, "Famicom Clone"),
            ConsoleType::NesFamicomWithEPSM => write!(f, "NES Family Computer with EPSM"),
            ConsoleType::VrTechVT01 => write!(f, "NES Family Computer (VR-Tech VT01)"),
            ConsoleType::VrTechVT02 => write!(f, "NES Family Computer (VR-Tech VT02)"),
            ConsoleType::VrTechVT03 => write!(f, "NES Family Computer (VR-Tech VT03)"),
            ConsoleType::VrTechVT09 => write!(f, "NES Family Computer (VR-Tech VT09)"),
            ConsoleType::VrTechVT32 => write!(f, "NES Family Computer (VR-Tech VT32)"),
            ConsoleType::VrTechVT369 => write!(f, "NES Family Computer (VR-Tech VT369)"),
            ConsoleType::UmcUm6578 => write!(f, "NES Family Computer (Umc Um6578)"),
            ConsoleType::FamicomNetWorkSystem => write!(f, "Famicom Network System"),
            ConsoleType::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Region {
    NTSC,
    PAL,
    Multiple,
    Dendy,
}

impl Display for Region {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Region::NTSC => write!(f, "NTSC"),
            Region::PAL => write!(f, "PAL"),
            Region::Multiple => write!(f, "Multiple"),
            Region::Dendy => write!(f, "Dendy"),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ExpansionDevice {
    Unspecified,
    StandardController,
    Zapper,
    TwoZapper,
    Other,
}

impl Display for ExpansionDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpansionDevice::Unspecified => write!(f, "Unspecified"),
            ExpansionDevice::StandardController => write!(f, "Standard controller"),
            ExpansionDevice::Zapper => write!(f, "Zapper"),
            ExpansionDevice::TwoZapper => write!(f, "Two Zapper"),
            ExpansionDevice::Other => write!(f, "Other"),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum VsPpuType {
    Rx2C03Variant,
    RP2C04_0001,
    RP2C04_0002,
    RP2C04_0003,
    RP2C04_0004,
    RC2C05_0001,
    RC2C05_0002,
    RC2C05_0003,
    RC2C05_0004,
    Unknown,
    None,
}

impl Display for VsPpuType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VsPpuType::Rx2C03Variant => write!(f, "Rx2C03 variant"),
            VsPpuType::RP2C04_0001 => write!(f, "RP2C04_0001"),
            VsPpuType::RP2C04_0002 => write!(f, "RP2C04_0002"),
            VsPpuType::RP2C04_0003 => write!(f, "RP2C04_0003"),
            VsPpuType::RP2C04_0004 => write!(f, "RP2C04_0004"),
            VsPpuType::RC2C05_0001 => write!(f, "RC2C05_0001"),
            VsPpuType::RC2C05_0002 => write!(f, "RC2C05_0002"),
            VsPpuType::RC2C05_0003 => write!(f, "RC2C05_0003"),
            VsPpuType::RC2C05_0004 => write!(f, "RC2C05_0004"),
            VsPpuType::Unknown => write!(f, "Unknown"),
            VsPpuType::None => write!(f, "None"),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum VsHardwareType {
    VsUnisystem,
    VsUnisystemRbiBaseballProtection,
    VsUnisystemTKOBoxingProtection,
    VsUnisystemSuperXeviousProtection,
    VsUnisystemIceClimberProtection,
    VsDualSystem,
    VsDualSystemRaidOnBungelingBayProtection,
    Unknown,
    None,
}

impl Display for VsHardwareType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VsHardwareType::VsUnisystem => write!(f, "Vs Unisystem"),
            VsHardwareType::VsUnisystemRbiBaseballProtection => write!(f, "Vs Unisystem RBI Baseball Protection"),
            VsHardwareType::VsUnisystemTKOBoxingProtection => write!(f, "Vs Unisystem TKO Boxing Protection"),
            VsHardwareType::VsUnisystemSuperXeviousProtection => write!(f, "Vs Unisystem Super Xevious Protection"),
            VsHardwareType::VsUnisystemIceClimberProtection => write!(f, "Vs Unisystem Ice Climber Protection"),
            VsHardwareType::VsDualSystem => write!(f, "Vs Dual System"),
            VsHardwareType::VsDualSystemRaidOnBungelingBayProtection => write!(f, "Vs Dual System Raid on Bungeling Bay Protection"),
            VsHardwareType::Unknown => write!(f, "Unknown"),
            VsHardwareType::None => write!(f, "None"),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum RomArea {
    PrgRom,
    ChrRom,
    PrgRam,
    PrgNvRam,
    ChrRam,
    ChrNvRam,
}

impl Display for RomArea {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RomArea::PrgRom => write!(f, "prg_rom"),
            RomArea::ChrRom => write!(f, "chr_rom"),
            RomArea::PrgRam => write!(f, "prg_ram"),
            RomArea::PrgNvRam => write!(f, "prg_nvram"),
            RomArea::ChrRam => write!(f, "chr_ram"),
            RomArea::ChrNvRam => write!(f, "chr_nvram"),
        }
    }
}

#[derive(Debug)]
pub struct INesRomHeader {
    pub prg_rom_size: usize,
    pub chr_rom_size: usize,
    pub prg_ram_size: usize,
    pub prg_nvram_size: usize,
    pub chr_ram_size: usize,
    pub chr_nvram_size: usize,
    pub nametables_layout: PpuNameTableMirroring,
    pub battery: bool,
    pub trainer: bool,
    pub alternative_nametables: bool,
    pub console_type: ConsoleType,
    pub ines2: bool,
    pub mapper: NesMapper,
    pub sub_mapper: u8,
    pub region: Region,
    pub vs_ppu_type: VsPpuType,
    pub vs_hardware_type: VsHardwareType,
    pub misc_rom: u16,
    pub expansion_device: ExpansionDevice
}

impl INesRomHeader {
    pub fn prg_offset(&self) -> u64 {
        if self.trainer == true {
            (HEADER_SIZE + 512) as u64
        } else {
            HEADER_SIZE as u64
        }
    }

    pub fn chr_offset(&self) -> Option<u64> {
        if self.chr_rom_size == 0 {
            None
        } else {
            Some(self.prg_offset() + self.prg_rom_size as u64)
        }
    }

    fn verify_raw_header_size(bytes: &[u8]) -> Result<(), LoaderError> {
        if bytes.len() < HEADER_SIZE {
            Err(LoaderError::InvalidRomFormat)
        } else {
            Ok(())
        }
    }

    fn build_preamble(bytes: &[u8]) -> [u8; 4] {
        let mut preamble = [0u8; 4];
        preamble.copy_from_slice(&bytes[0..4]);
        preamble
    }

    fn verify_preamble(preamble: &[u8; 4]) -> Result<(), LoaderError> {
        if *preamble != [0x4E, 0x45, 0x53, 0x1A] {
            Err(LoaderError::InvalidRomFormat)
        } else {
            Ok(())
        }
    }

    fn build_rom_from_msb_and_lsb_ines2(bytes: &[u8], area: RomArea) -> usize {
        let (byte0, byte1, mask, shift, unit) = match area {
            RomArea::PrgRom => (bytes[4], bytes[9], 0x0F, 0, 16 * 1024),
            RomArea::ChrRom => (bytes[5], bytes[9], 0xF0, 4, 8 * 1024),
            _=> { panic!("invalid rom area: {:?}", area) }
        };

        let result: usize = if byte1 == 0x0F {
            let mul = (byte0 & 0x03) * 2 + 1;
            let exp = (byte0 & !0x03) >> 3;
            (2u16.pow(exp as u32) * (mul as u16)) as usize // XXX padding should be added if non-aligned
        } else {
            let lsb = byte0 as u16;
            let msb = ((byte1 & mask) as u16) << (8 - shift);
            (msb | lsb) as usize * unit
        };

        info!("area: {}, size: {} bytes", area, result);
        result
    }

    fn build_rom_from_msb_and_lsb_ines1(bytes: &[u8], area: RomArea) -> usize {
        let result = match area {
            RomArea::PrgRom => (bytes[4] as usize) * 16 * 1024,
            RomArea::ChrRom => (bytes[5] as usize) * 8 * 1024,
            _=> { panic!("invalid rom area: {:?}", area) }
        };

        info!("area: {}, size: {} bytes", area, result);
        result
    }

    fn build_prg_rom_size(bytes: &[u8], ines2: bool) -> usize {
        if ines2 == true {
            INesRomHeader::build_rom_from_msb_and_lsb_ines2(bytes, RomArea::PrgRom)
        } else {
            INesRomHeader::build_rom_from_msb_and_lsb_ines1(bytes, RomArea::PrgRom)
        }
    }

    fn build_chr_rom_size(bytes: &[u8], ines2: bool) -> usize {
        if ines2 == true {
            INesRomHeader::build_rom_from_msb_and_lsb_ines2(bytes, RomArea::ChrRom)
        } else {
            INesRomHeader::build_rom_from_msb_and_lsb_ines1(bytes, RomArea::ChrRom)
        }
    }

    /***
     * XXX
     * specific cases to handle between ines1 and ines2
     */
    fn build_ram_size(bytes: &[u8], area: RomArea, ines2: bool) -> usize {
        let (byte, mask, shift) = match area {
            RomArea::PrgRam => (bytes[10], 0x0F, 0),
            RomArea::PrgNvRam => (bytes[10], 0xF0, 4),
            RomArea::ChrRam => {
                if ines2 == true {
                    (bytes[11], 0x0F, 0)
                } else {
                    if bytes[5] == 0 {
                        (0x07, 0xFF, 0) // will eventually compute as 8192
                    } else {
                        (0, 0, 0)
                    }
                }
            },
            RomArea::ChrNvRam => (bytes[11], 0xF0, 4),
            _=> { panic!("invalid rom area: {:?}", area) }
        };

        let shift_count = (byte & mask) >> shift;

        let result = if shift_count > 0 {
            0x40 << shift_count
        } else {
            0
        };

        info!("area: {}, size: {} bytes", area, result);
        result
    }

    fn build_prg_ram_size(bytes: &[u8], ines2: bool) -> usize {
        INesRomHeader::build_ram_size(bytes, RomArea::PrgRam, ines2)
    }

    fn build_prg_nvram_size(bytes: &[u8], ines2: bool) -> usize {
        INesRomHeader::build_ram_size(bytes, RomArea::PrgNvRam, ines2)
    }

    fn build_chr_ram_size(bytes: &[u8], ines2: bool) -> usize {
        INesRomHeader::build_ram_size(bytes, RomArea::ChrRam, ines2)
    }

    fn build_chr_nvram_size(bytes: &[u8], ines2: bool) -> usize {
        INesRomHeader::build_ram_size(bytes, RomArea::ChrNvRam, ines2)
    }

    fn build_nametables_layout(bytes: &[u8]) -> PpuNameTableMirroring {
        let result = if bytes[6] & 0x01 == 0 {
            PpuNameTableMirroring::Horizontal
        } else {
            PpuNameTableMirroring::Vertical
        };

        info!("nametables_layout: {}", result);
        result
    }

    fn build_battery(bytes: &[u8]) -> bool {
        let result= bytes[6] & 0x02 != 0;
        info!("battery: {}", result);
        result
    }

    fn build_trainer(bytes: &[u8]) -> bool {
        let result = bytes[6] & 0x04 != 0;
        info!("trainer: {}", result);
        result
    }

    fn build_alternative_nametables(bytes: &[u8]) -> bool {
        let result = bytes[6] & 0x08 != 0;
        info!("alternative_nametables: {}", result);
        result
    }

    fn build_console_type(bytes: &[u8]) -> ConsoleType {
        let value = if (bytes[7] & 0x03) == 0x03 {
            bytes[13] & 0x0F
        } else {
            bytes[7] & 0x03
        } ;

        let result = match value {
            0 => ConsoleType::NesFamicom,
            1 => ConsoleType::VsSystem,
            2 => ConsoleType::PlayChoice10,
            3 => ConsoleType::Famiclone,
            4 => ConsoleType::NesFamicomWithEPSM,
            5 => ConsoleType::VrTechVT01,
            6 => ConsoleType::VrTechVT02,
            7 => ConsoleType::VrTechVT03,
            8 => ConsoleType::VrTechVT09,
            9 => ConsoleType::VrTechVT32,
            10 => ConsoleType::VrTechVT369,
            11 => ConsoleType::UmcUm6578,
            12 => ConsoleType::FamicomNetWorkSystem,
            _ => ConsoleType::Unknown,
        };

        info!("console_type: {}", result);
        result
    }

    fn build_ines2_identifier(bytes: &[u8]) -> bool {
        let result = bytes[7] & 0x08 != 0;
        info!("ines2: {}", result);
        result
    }

    fn build_mapper(bytes: &[u8], ines2: bool) -> NesMapper {
        let d3_d0: u16 = (bytes[6] & 0xF0) as u16;
        let d7_d4: u16 = (bytes[7] & 0xF0) as u16;
        let d11_d8: u16 = (bytes[8] & 0x0F) as u16;

        let result0 = if ines2 == true  {
            (d3_d0 >> 4) | (d7_d4 << 4) | (d11_d8 << 8)
        } else {
            (d3_d0 >> 4) | (d7_d4 << 4)
        };

        let result = NesMapper::from_id(result0);
        info!("mapper: {} ({})", result.name(), result0);

        result
    }

    fn build_sub_mapper(bytes: &[u8]) -> u8 {
        let result = (bytes[8] & 0xF0) >> 4;
        info!("sub_mapper: {}", result);
        result
    }

    fn build_region(bytes: &[u8]) -> Region {
        let result = match bytes[12] & 0x03 {
            0 => Region::NTSC,
            1 => Region::PAL,
            2 => Region::Multiple,
            3 => Region::Dendy,
            _ => unreachable!()
        };

        info!("region: {}", result);
        result
    }

    fn build_vs_ppu_type(bytes: &[u8]) -> VsPpuType {
        let result = if bytes[3] & 0x03 != 0x01 {
            VsPpuType::None
        } else {
            match bytes[13] & 0x0F {
                0x00 => VsPpuType::Rx2C03Variant,
                0x02 => VsPpuType::RP2C04_0001,
                0x03 => VsPpuType::RP2C04_0002,
                0x04 => VsPpuType::RP2C04_0003,
                0x05 => VsPpuType::RP2C04_0004,
                0x08 => VsPpuType::RC2C05_0001,
                0x09 => VsPpuType::RC2C05_0002,
                0x0A => VsPpuType::RC2C05_0003,
                0x0B => VsPpuType::RC2C05_0004,
                _ => VsPpuType::Unknown,
            }
        };

        info!("vs_ppu_type: {}", result);
        result
    }

    fn build_vs_hardware_type(bytes: &[u8]) -> VsHardwareType {
        let result = if bytes[3] & 0x03 != 0x01 {
            VsHardwareType::None
        } else {
            match (bytes[13] & 0xF0) >> 4 {
                0x00 => VsHardwareType::VsUnisystem,
                0x01 => VsHardwareType::VsUnisystemRbiBaseballProtection,
                0x02 => VsHardwareType::VsUnisystemTKOBoxingProtection,
                0x03 => VsHardwareType::VsUnisystemSuperXeviousProtection,
                0x04 => VsHardwareType::VsUnisystemIceClimberProtection,
                0x05 => VsHardwareType::VsDualSystem,
                0x06 => VsHardwareType::VsDualSystemRaidOnBungelingBayProtection,
                _ => VsHardwareType::Unknown,
            }
        };

        info!("vs_hardware_type: {}", result);
        result
    }

    fn build_misc_rom(_: &[u8]) -> u16 {
        let result = 0;
        info!("misc_rom: {}", result);
        result
    }

    fn build_expansion_device(bytes: &[u8]) -> ExpansionDevice {
        let result = match bytes[15] & 0x3F {
            0x00 => ExpansionDevice::Unspecified,
            0x01 => ExpansionDevice::StandardController,
            0x08 => ExpansionDevice::Zapper,
            0x09 => ExpansionDevice::TwoZapper,
            _ => ExpansionDevice::Other
        };

        info!("expansion_device: {}", result);
        result
    }

    fn from_bytes(bytes: &[u8]) -> Result<INesRomHeader, LoaderError> {
        INesRomHeader::verify_raw_header_size(&bytes)?;

        let preamble = INesRomHeader::build_preamble(&bytes);
        INesRomHeader::verify_preamble(&preamble)?;

        let ines2 = INesRomHeader::build_ines2_identifier(&bytes);
        let prg_rom_size = INesRomHeader::build_prg_rom_size(&bytes, ines2);
        let chr_rom_size = INesRomHeader::build_chr_rom_size(&bytes, ines2);
        let prg_ram_size = INesRomHeader::build_prg_ram_size(&bytes, ines2);
        let prg_nvram_size = INesRomHeader::build_prg_nvram_size(&bytes, ines2);
        let chr_ram_size = INesRomHeader::build_chr_ram_size(&bytes, ines2);
        let chr_nvram_size = INesRomHeader::build_chr_nvram_size(&bytes, ines2);
        let nametables_layout = INesRomHeader::build_nametables_layout(&bytes);
        let battery = INesRomHeader::build_battery(&bytes);
        let trainer = INesRomHeader::build_trainer(&bytes);
        let alternative_nametables = INesRomHeader::build_alternative_nametables(&bytes);
        let console_type = INesRomHeader::build_console_type(&bytes);
        let mapper = INesRomHeader::build_mapper(&bytes, ines2);
        let sub_mapper = INesRomHeader::build_sub_mapper(&bytes);
        let region = INesRomHeader::build_region(&bytes);
        let vs_ppu_type = INesRomHeader::build_vs_ppu_type(&bytes);
        let vs_hardware_type = INesRomHeader::build_vs_hardware_type(&bytes);
        let misc_rom = INesRomHeader::build_misc_rom(&bytes);
        let expansion_device = INesRomHeader::build_expansion_device(&bytes);

        let headers = INesRomHeader {
            prg_rom_size,
            chr_rom_size,
            prg_ram_size,
            prg_nvram_size,
            chr_ram_size,
            chr_nvram_size,
            nametables_layout,
            battery,
            trainer,
            alternative_nametables,
            console_type,
            ines2,
            mapper,
            sub_mapper,
            region,
            vs_ppu_type,
            vs_hardware_type,
            misc_rom,
            expansion_device,
        };

        info!("prg offset: 0x{:04X} (+{} bytes)", headers.prg_offset(), headers.prg_offset());

        let chr_offset = headers.chr_offset();
        match chr_offset {
            Some(offset) => info!("chr offset: 0x{:04X} (+{} bytes)", offset, offset),
            None => info!("chr offset: no chr data"),
        }

        Ok(headers)
    }
}
