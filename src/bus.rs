use std::cell::RefCell;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
#[cfg(test)]
use mockall::mock;
use crate::bus_device::BusDevice;
use crate::memory::Memory;
#[cfg(test)]
use crate::memory::{MemoryError};

#[derive(Debug, Default, Clone)]
pub enum BusType {
    #[default]
    NESBus
}

pub trait Bus: Memory {
    fn add_device(&mut self, memory: Rc<RefCell<dyn BusDevice>>) -> Result<(), BusError>;
}

#[cfg(test)]
mock! {
    #[derive(Debug)]
    pub BusStub {}

    impl Bus for BusStub {
        fn add_device(&mut self, memory: Rc<RefCell<dyn BusDevice>>) -> Result<(), BusError>;
    }

    #[derive(Debug)]
    impl Memory for BusStub {
        fn initialize(&mut self) -> Result<usize, MemoryError>;
        fn read_byte(&self, addr: u16) -> Result<u8, MemoryError>;
        fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError>;
        fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError>;
        fn read_word(&self, addr: u16) -> Result<u16, MemoryError>;
        fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError>;
        #[allow(dead_code)]
        fn dump(&self);
        fn size(&self) -> usize;
    }
}

#[derive(Debug, PartialEq)]
pub enum BusError {
    Unmapped(u16)
}

impl Error for BusError {}

impl Display for BusError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            BusError::Unmapped(addr) => {
                write!(f, "unmapped memory location: 0x{:04X}", addr)
            }
        }
    }
}
