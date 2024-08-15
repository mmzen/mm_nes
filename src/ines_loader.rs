use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;
use log::debug;
use crate::loader::{Loader, LoaderError};
use crate::memory::Memory;

const HEADER_SIZE: usize = 16;
const TRAINER_BIT_MASK: u8 = 0b00000100;
const ROM_BLOCK_UNIT: usize = 16384;

#[derive(Debug)]
pub struct INesLoader {
    memory: Option<Rc<RefCell<dyn Memory>>>
}

impl Loader for INesLoader {
    fn load_rom(&mut self, path: &str) -> Result<(), LoaderError> {
        let mut file = File::open(path)?;
        let header = self.load_header(&mut file)?;

        let prg_addr= if header.flags_6 & TRAINER_BIT_MASK != 0 {
            HEADER_SIZE + 512
        } else {
            HEADER_SIZE
        };

        let prg_size = header.rom_size as usize * ROM_BLOCK_UNIT;

        debug!("loader: ROM starting at address: 0x{}, {} bytes", prg_addr, prg_size);
        debug!("loader: mapper: {}", header.flags_6);

        self.load_prg_rom(&mut file, prg_addr, prg_size)?;

        Ok(())
    }

    fn set_target_memory(&mut self, memory: Rc<RefCell<dyn Memory>>) {
        self.memory = Some(memory.clone());
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

    fn load_prg_rom(&mut self, file: &mut File, start_addr: usize, size: usize) -> Result<(), LoaderError> {
        if let None = self.memory {
            Err(LoaderError::NotConfigured("missing target memory".to_string()))
        } else {
            let target_memory = self.memory.as_mut().unwrap();

            file.seek(SeekFrom::Start(start_addr as u64))?;
            file.read_exact(&mut target_memory.borrow_mut().as_slice()[0x8000..0x8000 + size])?;
            Ok(())
        }
    }

    pub fn new() -> Box<INesLoader> {
        let loader = INesLoader {
            memory: None,
        };

        Box::new(loader)
    }
}

#[repr(C, packed)]
#[derive(Debug)]
struct INesRomHeader {
    preamble: [u8; 4],
    rom_size: u8,
    ram_size: u8,
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
            rom_size: bytes[4],
            ram_size: bytes[5],
            flags_6: bytes[6],
            flags_7: bytes[7],
            flags_8: bytes[8],
            flags_9: bytes[9],
            flags_10: bytes[10],
            _reserved: [bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]]
        }
    }
}
