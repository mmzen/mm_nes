use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;

const DEVICE_NAME: &str = "Palette Memory";

#[derive(Debug)]
pub struct MemoryPalette {
    memory: MemoryBank,
}

impl Memory for MemoryPalette {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        self.memory.initialize()
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let mirrored_addr = self.get_mirrored_address(addr);
        self.memory.read_byte(mirrored_addr)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let mirrored_addr = self.get_mirrored_address(addr);
        self.memory.trace_read_byte(mirrored_addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        let mirrored_addr = self.get_mirrored_address(addr);
        self.memory.write_byte(mirrored_addr, value)
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let mirrored_addr = self.get_mirrored_address(addr);
        self.memory.read_word(mirrored_addr)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        let mirrored_addr = self.get_mirrored_address(addr);
        self.memory.write_word(mirrored_addr, value)
    }

    fn dump(&self) {
        self.memory.dump()
    }

    fn size(&self) -> usize {
        self.memory.size()
    }
}

impl BusDevice for MemoryPalette {
    fn get_name(&self) -> String {
        DEVICE_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        self.memory.get_device_type()
    }

    fn get_virtual_address_range(&self) -> (u16, u16) {
        self.memory.get_virtual_address_range()
    }
}

impl MemoryPalette {
    pub(crate) fn new(size: usize, address_range: (u16, u16)) -> Self {
        MemoryPalette {
            memory: MemoryBank::new(size, address_range)
        }
    }

    fn get_mirrored_address(&self, addr: u16) -> u16 {
        match addr {
            0x10 | 0x14 | 0x18 | 0x1C => addr - 0x10,
            _ => addr,
        }
    }
}
