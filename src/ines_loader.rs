use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::rc::Rc;
use log::debug;
use crate::cartridge::Cartridge;
use crate::loader::{Loader, LoaderError};
use crate::nrom128_cartridge::NROM128Cartridge;

const HEADER_SIZE: usize = 16;
const TRAINER_BIT_MASK: u8 = 0b00000100;
const PRG_ROM_BLOCK_UNIT: usize = 16384;
const CHR_ROM_BLOCK_UNIT: usize = 8192;

#[derive(Debug)]
pub struct INesLoader;

impl Loader for INesLoader {
    fn load(&mut self, path: &PathBuf) -> Result<Rc<RefCell<dyn Cartridge>>, LoaderError> {
        let mut file = File::open(path)?;
        let header = self.load_header(&mut file)?;

        let prg_addr= if header.flags_6 & TRAINER_BIT_MASK != 0 {
            HEADER_SIZE + 512
        } else {
            HEADER_SIZE
        };

        let prg_rom_size = header.prg_rom as usize * PRG_ROM_BLOCK_UNIT;
        let chr_rom_size = header.chr_rom as usize * CHR_ROM_BLOCK_UNIT;

        let chr_addr = prg_addr + prg_rom_size;

        debug!("loader: prg rom data starting at offset 0x{:04x}, {} bytes", prg_addr, prg_rom_size);
        debug!("loader: chr rom data starting at offset 0x{:04x}, {} bytes", chr_addr, chr_rom_size);
        debug!("loader: mapper: {}", header.flags_6);

        File::seek(&mut file, SeekFrom::Start(prg_addr as u64))?;
        let cartridge = self.build_cartridge(file, prg_rom_size, chr_rom_size)?;

        Ok(cartridge)
    }
}

impl  INesLoader  {

    fn load_header(&mut self, file: &mut File) -> Result<INesRomHeader, LoaderError> {
        let mut buffer = vec![0u8; HEADER_SIZE];
        file.read_exact(&mut buffer)?;

        let header = INesRomHeader::from_bytes(&buffer);

        if header.preamble != [0x4E, 0x45, 0x53, 0x1A] {
            Err(LoaderError::InvalidRomFormat)
        } else {
            debug!("iNES ROM detected");

            if header.flags_7 & 0x0C == 0x08 {
                debug!("ROM format v2.0 detected: 0x{:02X}", header.flags_7 & 0x0C);
            } else {
                debug!("ROM format v1.0 detected: 0x{:02X}", header.flags_7 & 0x0C);
            }

            debug!("ROM header: {:?}", header);
            Ok(header)
        }
    }

    fn build_cartridge(&self, file: File, prg_rom_size: usize, chr_rom_size: usize) -> Result<Rc<RefCell<dyn Cartridge>>, LoaderError> {
        debug!("creating cartridge");

        let cartridge = NROM128Cartridge::new(file.bytes(), prg_rom_size, chr_rom_size)?;
        Ok(Rc::new(RefCell::new(cartridge)))
    }

    pub fn new() -> Box<INesLoader> {
        let loader = INesLoader {
        };

        Box::new(loader)
    }
}

#[repr(C, packed)]
#[derive(Debug)]
struct INesRomHeader {
    preamble: [u8; 4],
    prg_rom: u8,
    chr_rom: u8,
    flags_6: u8,
    flags_7: u8,
    flags_8: u8,
    flags_9: u8,
    flags_10: u8,
    _reserved: [u8; 5],
}

impl INesRomHeader {
    fn from_bytes(bytes: &[u8]) -> INesRomHeader {

        INesRomHeader {
            preamble: [bytes[0], bytes[1], bytes[2], bytes[3]],
            prg_rom: bytes[4],
            chr_rom: bytes[5],
            flags_6: bytes[6],
            flags_7: bytes[7],
            flags_8: bytes[8],
            flags_9: bytes[9],
            flags_10: bytes[10],
            _reserved: [bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]]
        }
    }
}
