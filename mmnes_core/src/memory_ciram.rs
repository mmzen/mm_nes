use std::cell::RefCell;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::bus_device::BusDeviceType::WRAM;
use crate::memory::{Memory, MemoryError};
use crate::memory::MemoryType::PpuCiramMemory;
use crate::memory_bank::MemoryBank;

const PPU_CIRAM_PHYSICAL_SIZE: usize = 2 * 1024;
const PPU_CIRAM_VIRTUAL_SIZE: usize = 4 * 1024;
const PPU_CIRAM_VIRTUAL_ADDRESS_RANGE: (u16, u16) = (0x2000, 0x3EFF);
const PPU_CIRAM_MEMORY_NAME: &str = "PPU CIRAM";

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PpuNameTableMirroring {
    Vertical,
    Horizontal,
    SingleScreenLower,
    SingleScreenUpper,
}

impl Display for PpuNameTableMirroring {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PpuNameTableMirroring::Vertical => write!(f, "vertical mirroring"),
            PpuNameTableMirroring::Horizontal => write!(f, "horizontal mirroring"),
            PpuNameTableMirroring::SingleScreenLower => write!(f, "single screen lower"),
            PpuNameTableMirroring::SingleScreenUpper => write!(f, "single screen upper")
        }
    }
}

#[derive(Debug)]
pub struct CiramMemory {
    memory: Rc<RefCell<MemoryBank>>,
    mirroring: PpuNameTableMirroring,
}

impl CiramMemory {
    pub fn new(mirroring: PpuNameTableMirroring) -> CiramMemory {
        let memory = MemoryBank::new(PPU_CIRAM_PHYSICAL_SIZE, (0, (PPU_CIRAM_PHYSICAL_SIZE - 1) as u16));

        CiramMemory {
            memory: Rc::new(RefCell::new(memory)),
            mirroring,
        }
    }

    #[cfg(test)]
    pub fn mirroring(&self) -> PpuNameTableMirroring {
        self.mirroring
    }

    fn remap_addr(&self, addr: u16) -> u16 {
        let offset = addr & 0x03FF;

        let nametable = match self.mirroring {
            PpuNameTableMirroring::Vertical  => addr & 0x400,
            PpuNameTableMirroring::Horizontal => (addr & 0x800) >> 1,
            PpuNameTableMirroring::SingleScreenLower => 0x000,
            PpuNameTableMirroring::SingleScreenUpper => 0x400,
        };

        let remapped_addr = nametable | offset;
        //debug!("remapped address: 0x{:04X} -> 0x{:04X} ({})", addr, remapped_addr, self.mirroring);
        remapped_addr
    }
}

impl Memory for CiramMemory {
    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let remapped_addr = self.remap_addr(addr);
        self.memory.borrow().read_byte(remapped_addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        let remapped_addr = self.remap_addr(addr);
        self.memory.borrow_mut().write_byte(remapped_addr, value)
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let remapped_addr = self.remap_addr(addr);
        self.memory.borrow().read_word(remapped_addr)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        let remapped_addr = self.remap_addr(addr);
        self.memory.borrow_mut().write_word(remapped_addr, value)
    }

    fn size(&self) -> usize {
        PPU_CIRAM_VIRTUAL_SIZE
    }
}

impl BusDevice for CiramMemory {
    fn get_name(&self) -> String {
        PPU_CIRAM_MEMORY_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        WRAM(PpuCiramMemory)
    }

    fn get_virtual_address_range(&self) -> (u16, u16) {
        PPU_CIRAM_VIRTUAL_ADDRESS_RANGE
    }
}