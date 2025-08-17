use std::cell::RefCell;
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::rc::Rc;
use log::{debug, info, warn};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge::{Cartridge, CartridgeError};
use crate::cartridge::CartridgeType::NROM;
use crate::ines_loader::{FromINes, INesRomHeader};
use crate::loader::LoaderError;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::ppu::PpuNameTableMirroring;

const CPU_ADDRESS_SPACE: (u16, u16) = (0x8000, 0xFFFF);
const PPU_ADDRESS_SPACE: (u16, u16) = (0x0000, 0x1FFF);
const MAPPER_NAME: &str = "NROM";

#[derive(Debug)]
pub struct NromCartridge {
    prg_rom: Rc<RefCell<MemoryBank>>,
    chr_rom: Rc<RefCell<MemoryBank>>,
    device_type: BusDeviceType,
    mirroring: PpuNameTableMirroring,
    prg_rom_size: usize,
}

impl NromCartridge {
    fn fix_chr_rom_size(chr_rom_size: usize) -> usize {
        if chr_rom_size == 0 {
            warn!("NROM has no 0 bytes chr rom: forcing to 8 KB");
            8192
        } else {
            chr_rom_size
        }
    }

    fn write_rom_data(rom: &mut dyn Memory, size: usize, data: &mut BufReader<File>) -> Result<(), CartridgeError> {
        let mut buf = vec![0u8; size];
        data.read_exact(&mut buf)?;

        for (i, &byte) in buf.iter().enumerate() {
            rom.write_byte(i as u16, byte)?;
        }

        Ok(())
    }

    pub fn new(mut data: BufReader<File>,
                  prg_rom_addr: u64, prg_rom_size: usize,
                  chr_rom_addr: u64, chr_rom_size: usize, mirroring: PpuNameTableMirroring) -> Result<NromCartridge, CartridgeError> {

        let mut prg_rom = MemoryBank::new(prg_rom_size, CPU_ADDRESS_SPACE);
        let mut chr_rom = MemoryBank::new(chr_rom_size, PPU_ADDRESS_SPACE);

        debug!("NROM: loading prg_rom data ({} KB)...", prg_rom_size / 1024);
        data.seek(SeekFrom::Start(prg_rom_addr))?;
        NromCartridge::write_rom_data(&mut prg_rom, prg_rom_size, &mut data)?;

        debug!("NROM: loading chr_rom data ({} KB)...", chr_rom_size / 1024);
        data.seek(SeekFrom::Start(chr_rom_addr))?;
        NromCartridge::write_rom_data(&mut chr_rom, chr_rom_size, &mut data)?;

        let cartridge = NromCartridge {
            prg_rom: Rc::new(RefCell::new(prg_rom)),
            chr_rom: Rc::new(RefCell::new(chr_rom)),
            device_type: BusDeviceType::CARTRIDGE(NROM),
            mirroring,
            prg_rom_size,
        };

        Ok(cartridge)
    }

    fn build(file: File,
             prg_rom_addr: u64, prg_rom_size: usize,
             chr_rom_addr: u64, chr_rom_size: usize, mirroring: PpuNameTableMirroring) -> Result<NromCartridge, LoaderError> {
        info!("creating NROM cartridge");

        let fixed_chr_rom_size = NromCartridge::fix_chr_rom_size(chr_rom_size);
        info!("fixed chr rom size: {} KB (was: {} KB)", fixed_chr_rom_size / 1024, chr_rom_size / 1024);

        let reader = BufReader::new(file);
        let cartridge = NromCartridge::new(reader, prg_rom_addr, prg_rom_size, chr_rom_addr, fixed_chr_rom_size, mirroring)?;
        Ok(cartridge)
    }
}

impl FromINes for NromCartridge {
    fn from_ines(file: File, header: INesRomHeader) -> Result<impl Cartridge, LoaderError>
    where
        Self: Sized
    {
        let cartridge = NromCartridge::build(file,
                                             header.prg_addr(), header.prg_rom_size,
                                             header.chr_addr(), header.chr_rom_size,
                                             header.nametables_layout)?;

        Ok(cartridge)
    }
}

impl Memory for NromCartridge {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        let mut result = 0;

        result += self.prg_rom.borrow_mut().initialize()?;
        result += self.chr_rom.borrow_mut().initialize()?;

        Ok(result)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.prg_rom.borrow().read_byte(addr)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        self.prg_rom.borrow_mut().write_byte(addr, value)
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        self.prg_rom.borrow().read_word(addr)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        self.prg_rom.borrow_mut().write_word(addr, value)
    }

    fn dump(&self) {
        self.prg_rom.borrow().dump();
    }

    fn size(&self) -> usize {
        self.prg_rom.borrow().size()
    }
}

impl BusDevice for NromCartridge {
    fn get_name(&self) -> String {
        format!("MAPPER_NAME-{}", self.prg_rom_size)
    }

    fn get_device_type(&self) -> BusDeviceType {
        self.device_type.clone()
    }

    fn get_address_range(&self) -> (u16, u16) {
        CPU_ADDRESS_SPACE
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        self.prg_rom.borrow().is_addr_in_address_space(addr)
    }
}

impl Cartridge for NromCartridge {
    fn get_chr_rom(&self) -> Rc<RefCell<dyn BusDevice>> {
        self.chr_rom.clone()
    }

    fn get_prg_rom(&self) -> Rc<RefCell<dyn BusDevice>> {
        self.prg_rom.clone()
    }

    fn get_mirroring(&self) -> PpuNameTableMirroring {
        self.mirroring.clone()
    }
}