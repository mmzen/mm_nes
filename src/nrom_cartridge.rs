use std::cell::RefCell;
use std::fmt::Debug;
use std::io;
use std::rc::Rc;
use log::debug;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge::Cartridge;
use crate::cartridge::CartridgeType::NROM;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::ppu::PpuNameTableMirroring;

const CPU_ADDRESS_SPACE: (u16, u16) = (0x8000, 0xFFFF);
const PPU_ADDRESS_SPACE: (u16, u16) = (0x0000, 0x1FFF);
const MAPPER_NAME: &str = "NROM";

#[derive(Debug)]
pub struct NROMCartridge {
    prg_rom: Rc<RefCell<MemoryBank>>,
    chr_rom: Rc<RefCell<MemoryBank>>,
    device_type: BusDeviceType,
    mirroring: PpuNameTableMirroring,
    prg_rom_size: usize,
}

impl NROMCartridge {

    fn write_rom_data(rom: &mut dyn Memory, size: usize, data: impl Iterator<Item = io::Result<u8>>) -> Result<(), MemoryError> {
        let mut total = 0;

        for (i, byte_result) in data.take(size).enumerate() {
            match byte_result {
                Ok(byte) => {
                    rom.write_byte(i as u16, byte)?;
                },
                Err(e) => eprintln!("Error reading byte: {}", e),
            }
            total += 1;
        }
        debug!("CARTRIDGE: total bytes read: {}", total);

        Ok(())
    }

    pub fn new<I>(mut data: I, prg_rom_size: usize, chr_rom_size: usize, mirroring: PpuNameTableMirroring) -> Result<Self, MemoryError>
    where
        I: Iterator<Item = io::Result<u8>>,{

        let mut prg_rom = MemoryBank::new(prg_rom_size, CPU_ADDRESS_SPACE);
        let mut chr_rom = MemoryBank::new(chr_rom_size, PPU_ADDRESS_SPACE);

        debug!("CARTRIDGE: loading prg_rom data ({} KB)...", prg_rom_size / 1024);
        NROMCartridge::write_rom_data(&mut prg_rom, prg_rom_size, &mut data)?;

        debug!("CARTRIDGE: loading chr_rom data ({} KB)...", chr_rom_size / 1024);
        NROMCartridge::write_rom_data(&mut chr_rom, chr_rom_size, &mut data)?;

        let cartridge = NROMCartridge {
            prg_rom: Rc::new(RefCell::new(prg_rom)),
            chr_rom: Rc::new(RefCell::new(chr_rom)),
            device_type: BusDeviceType::CARTRIDGE(NROM),
            mirroring,
            prg_rom_size,
        };

        Ok(cartridge)
    }
}

impl Memory for NROMCartridge {
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

impl BusDevice for NROMCartridge {
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

impl Cartridge for NROMCartridge {
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