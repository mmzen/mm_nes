use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
#[cfg(test)]
use mockall::automock;
use crate::bus::BusError;

#[derive(Default, Debug, Clone, PartialEq)]
pub enum MemoryType {
    #[default]
    StandardMemory,
    SwitchableMemory,
    PpuCiramMemory,
}

impl Display for MemoryType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MemoryType::StandardMemory => write!(f, "memory type: standard Memory"),
            MemoryType::SwitchableMemory => write!(f, "memory type: switchable Memory"),
            MemoryType::PpuCiramMemory => write!(f, "memory type: ciram Memory"),
        }
    }
}

#[cfg_attr(test, automock)]
pub trait Memory: Debug {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(0)
    }
    
    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError>;
    
    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    /***
     * XXX
     * should be implemented by default (double call to read/write byte)
     ***/
    fn write_byte(&mut self, _addr: u16, _value: u8) -> Result<(), MemoryError> {
        unreachable!()
    }
    
    fn read_word(&self, addr: u16) -> Result<u16, MemoryError>;
    
    fn write_word(&mut self, _addr: u16, _value: u16) -> Result<(), MemoryError> {
        unreachable!()
    }
    
    #[allow(dead_code)]
    fn dump(&self) {
        unimplemented!()
    }
    
    fn size(&self) -> usize;
}

#[derive(Debug, PartialEq, Clone)]
pub enum MemoryError {
    OutOfRange(u16),
    BusError(u16),
    IllegalState(String),
    InvalidAddressSpace(String)
}

impl Error for MemoryError {}

impl Display for MemoryError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            MemoryError::OutOfRange(addr) => write!(f, "memory access out of bounds: 0x{:04X}", addr),
            MemoryError::BusError(addr) => { write!(f, "bus error: 0x{:04X}", addr) },
            MemoryError::IllegalState(s) => { write!(f, "illegal state: {}", s) }
            MemoryError::InvalidAddressSpace(s) => { write!(f, "invalid address space: {}", s) }
        }
    }
}

impl From<BusError> for MemoryError {
    fn from(error: BusError) -> Self {
        match error {
            BusError::Unmapped(address) => MemoryError::BusError(address)
        }
    }
}



