use std::cell::RefCell;
use std::fmt::Debug;
use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;
use log::{debug, info};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge;
use crate::cartridge::{Cartridge, CartridgeError, CPU_ADDRESS_SPACE, PPU_ADDRESS_SPACE};
use crate::cartridge::CartridgeType::NROM;
use crate::ines_loader::{FromINes, INesRomHeader};
use crate::loader::LoaderError;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::ppu::PpuNameTableMirroring;

const NROM_PRG_MEMORY_BANK_SIZE: usize = 32 * 1024;
const NROM_CHR_MEMORY_BANK_SIZE: usize = 8 * 1024;
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

    pub fn new(mut data: BufReader<File>,
               prg_rom_offset: u64, prg_rom_size: usize,
               chr_rom_offset: u64, chr_rom_size: usize,
               chr_ram_size: usize, mirroring: PpuNameTableMirroring) -> Result<NromCartridge, CartridgeError> {

        if chr_rom_size > 0 && chr_ram_size > 0 {
            Err(CartridgeError::Unsupported(
                format!("NROM cartridge does not support both CHR-ROM (detected: {} bytes) and CHR-RAM (detected: {} bytes)", chr_rom_size, chr_ram_size)))?
        }

        let prg_memory_banks = cartridge::create_prg_rom_memory(&mut data, prg_rom_offset, prg_rom_size, NROM_PRG_MEMORY_BANK_SIZE, CPU_ADDRESS_SPACE)?;
        let prg_rom = cartridge::get_first_bank_or_fail(prg_memory_banks, prg_rom_size, NROM_PRG_MEMORY_BANK_SIZE, true)?;
        debug!("NROM: prg rom size: {}, number of bank: {}", prg_rom_size, 1);

        let (chr_memory_size, is_chr_rom) = cartridge::get_chr_memory_size_and_type(chr_rom_size, chr_ram_size);
        let rom_data = if is_chr_rom { Some(&mut data) } else { None };

        let chr_memory_banks = cartridge::create_chr_memory(rom_data, chr_rom_offset, chr_memory_size, NROM_CHR_MEMORY_BANK_SIZE, is_chr_rom, PPU_ADDRESS_SPACE)?;
        let chr_mem = cartridge::get_first_bank_or_fail(chr_memory_banks, chr_rom_size, NROM_CHR_MEMORY_BANK_SIZE, is_chr_rom)?;

        debug!("NROM: chr memory size: {}, number of bank: {}, ram: {}", chr_memory_size, 1, !is_chr_rom);

        let cartridge = NromCartridge {
            prg_rom: Rc::new(RefCell::new(prg_rom)),
            chr_rom: Rc::new(RefCell::new(chr_mem)),
            device_type: BusDeviceType::CARTRIDGE(NROM),
            mirroring,
            prg_rom_size,
        };

        Ok(cartridge)
    }

    fn build(file: File,
             prg_rom_offset: u64, prg_rom_size: usize,
             chr_rom_offset: Option<u64>, chr_rom_size: usize,
             chr_ram_size: usize, mirroring: PpuNameTableMirroring) -> Result<NromCartridge, LoaderError> {
        info!("creating NROM cartridge");

        let reader = BufReader::new(file);
        let chr_rom_offset = if let Some(chr_rom_offset_unwrapped) = chr_rom_offset { chr_rom_offset_unwrapped } else { 0 };

        let cartridge = NromCartridge::new(reader, prg_rom_offset, prg_rom_size, chr_rom_offset, chr_rom_size, chr_ram_size, mirroring)?;
        Ok(cartridge)
    }
}

impl FromINes for NromCartridge {
    #[allow(refining_impl_trait)]
    fn from_ines(file: File, header: INesRomHeader) -> Result<NromCartridge, LoaderError>
    where
        Self: Sized
    {
        let cartridge = NromCartridge::build(file,
                                             header.prg_offset(), header.prg_rom_size,
                                             header.chr_offset(), header.chr_rom_size,
                                             header.chr_ram_size,
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
        format!("{}-{}", MAPPER_NAME, self.prg_rom_size)
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

    fn get_mirroring(&self) -> PpuNameTableMirroring {
        self.mirroring.clone()
    }
}