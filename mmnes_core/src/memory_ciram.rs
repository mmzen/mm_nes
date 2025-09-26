use std::cell::RefCell;
use std::rc::Rc;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::bus_device::BusDeviceType::WRAM;
use crate::memory::{Memory, MemoryError};
use crate::memory::MemoryType::PpuCiramMemory;
use crate::memory_bank::MemoryBank;

const PPU_CIRAM_SIZE: usize = 2 * 1024;
const PPU_CIRAM_ADDRESS_RANGE: (u16, u16) = (0x2000, 0x3FFF);
const CIRAM_MEMORY_NAME: &str = "PPU CIRAM";

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PpuNameTableMirroring {
    Vertical,
    Horizontal,
    SingleScreenLower,
    SingleScreenUpper,
}

#[derive(Debug)]
pub struct CiramMemory {
    memory: Rc<RefCell<MemoryBank>>,
    mirroring: PpuNameTableMirroring,
}

impl CiramMemory {
    pub fn new(mirroring: PpuNameTableMirroring) -> CiramMemory {
        let memory = MemoryBank::new(PPU_CIRAM_SIZE, PPU_CIRAM_ADDRESS_RANGE);

        CiramMemory {
            memory: Rc::new(RefCell::new(memory)),
            mirroring,
        }
    }

    pub fn mirroring(&self) -> PpuNameTableMirroring {
        self.mirroring
    }

    fn remap_addr(&self, addr: u16) -> u16 {
        let nametable_offset = addr & 0x03FF;

        let a10 = match self.mirroring {
            PpuNameTableMirroring::Vertical => (addr >> 10) & 1,
            PpuNameTableMirroring::Horizontal => (addr >> 11) & 1,
            PpuNameTableMirroring::SingleScreenLower => 0,
            PpuNameTableMirroring::SingleScreenUpper => 1,
        };

        (a10 << 10) | nametable_offset
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
        PPU_CIRAM_SIZE
    }
}

impl BusDevice for CiramMemory {
    fn get_name(&self) -> String {
        PPU_CIRAM_SIZE.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        WRAM(PpuCiramMemory)
    }

    fn get_virtual_address_range(&self) -> (u16, u16) {
        PPU_CIRAM_ADDRESS_RANGE
    }
}