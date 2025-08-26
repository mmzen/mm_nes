use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
#[cfg(test)]
use mockall::automock;
use crate::bus::BusError;

#[derive(Default, Debug, Clone)]
pub enum MemoryType {
    #[default]
    NESMemory,
}

impl Display for MemoryType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MemoryType::NESMemory => write!(f, "memory type: NESMemory"),
        }
    }
}

impl PartialEq for MemoryType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MemoryType::NESMemory, MemoryType::NESMemory) => true
        }
    }
}

#[cfg_attr(test, automock)]
pub trait Memory: Debug {
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



