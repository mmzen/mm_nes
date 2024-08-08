use std::error::Error;
use std::fmt::{Debug, Display, Formatter};


pub trait Memory: Debug {
    fn initialize(&mut self) -> Result<usize, MemoryError>;
    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError>;
    fn write_byte(&mut self, addr: u16, value: u8) -> Result<u8, MemoryError>;
    fn read_word(&self, addr: u16) -> Result<u16, MemoryError>;
    fn write_word(&mut self, addr: u16, value: u16) -> Result<u16, MemoryError>;
    fn dump(&self);
    fn is_addr_in_boundary(&self, addr: u16) -> bool;
}

#[derive(Debug, PartialEq)]
pub enum MemoryError {
    OutOfBounds(u16),
    ReadWriteError,
    Other(String)
}

impl Error for MemoryError {}

impl Display for MemoryError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            MemoryError::OutOfBounds(addr) => write!(f, "memory access out of bounds: 0x{:04X}", addr),
            MemoryError::ReadWriteError => write!(f, "error reading or writing memory"),
            MemoryError::Other(msg) => write!(f, "unexpected error: {}", msg)
        }
    }
}

