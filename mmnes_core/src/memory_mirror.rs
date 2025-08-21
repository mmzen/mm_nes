use std::cell::RefCell;
use std::rc::Rc;
use log::debug;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;

#[derive(Debug)]
pub struct MemoryMirror {
    memory: Rc<RefCell<MemoryBank>>,
    address_space: (u16, u16)
}

impl MemoryMirror {

    fn is_address_space_valid(memory: Rc<RefCell<MemoryBank>>, address_space: (u16, u16)) -> bool {
        let real_virtual_size0 = memory.borrow().get_address_range().1 - memory.borrow().get_address_range().0;
        let mirror_virtual_size1 = address_space.1 - address_space.0;
        mirror_virtual_size1 <= real_virtual_size0
    }
    
    pub fn new(memory: Rc<RefCell<MemoryBank>>, address_space: (u16, u16)) -> Result<Self, MemoryError> {
        let result = if MemoryMirror::is_address_space_valid(memory.clone(), address_space) == false {
            Err(MemoryError::InvalidAddressSpace(
                format!("mirror virtual memory mirror can not be larger as the primary memory: {} versus {}.",
                        memory.borrow().get_address_range().1 - memory.borrow().get_address_range().0,
                        address_space.1 - address_space.0 + 1))
            )
        } else {
            let mirror = MemoryMirror {
                memory,
                address_space
            };

            Ok(mirror)
        };
        
        result
    }
}

impl Memory for MemoryMirror {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(0)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.memory.borrow().read_byte(addr)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        self.memory.borrow_mut().write_byte(addr, value)
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        self.memory.borrow().read_word(addr)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        self.memory.borrow_mut().write_word(addr, value)
    }

    fn dump(&self) {
        self.memory.borrow().dump();
    }

    fn size(&self) -> usize {
        self.memory.borrow().size()
    }
}


impl BusDevice for MemoryMirror {
    fn get_name(&self) -> String {
        self.memory.borrow().get_name()
    }

    fn get_device_type(&self) -> BusDeviceType {
        self.memory.borrow().get_device_type()
    }

    fn get_address_range(&self) -> (u16, u16) {
        self.address_space
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        debug!("==> 0x{:04X}: 0x{:04X} - 0x{:04X}", addr, self.address_space.0, self.address_space.1);
        self.address_space.0 <= addr && addr <= self.address_space.1
    }
}