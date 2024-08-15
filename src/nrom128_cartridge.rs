use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use crate::bus::Bus;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge::CartridgeType::NROM128;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;

const MEMORY_RANGE: (u16, u16) = (0x8000, 0xBFFF);
const MEMORY_SIZE: usize = 16 * 1024;
const MAPPER_NAME: &str = "NROM-128";

#[derive(Debug)]
pub struct NROM128Cartridge {
    memory: MemoryBank,
    device_type: BusDeviceType,
}

impl NROM128Cartridge {
    pub fn new(bus: Rc<RefCell<dyn Bus>>) -> Self {
        NROM128Cartridge {
            memory: MemoryBank::new(MEMORY_SIZE, bus, MEMORY_RANGE),
            device_type: BusDeviceType::CARTRIDGE(NROM128),
        }
    }
}

impl Memory for NROM128Cartridge {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        self.memory.initialize()
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.memory.read_byte(addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        self.memory.write_byte(addr, value)
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        self.memory.read_word(addr)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        self.memory.write_word(addr, value)
    }

    fn dump(&self) {
        self.memory.dump();
    }

    fn size(&self) -> usize {
        self.memory.size()
    }

    fn as_slice(&mut self) -> &mut [u8] {
        self.memory.as_slice()
    }
}

impl BusDevice for NROM128Cartridge {
    fn get_name(&self) -> String {
        MAPPER_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        self.device_type.clone()
    }

    fn get_address_range(&self) -> (u16, u16) {
        MEMORY_RANGE
    }

    fn is_addr_in_boundary(&self, addr: u16) -> bool {
        self.memory.is_addr_in_boundary(addr)
    }
}