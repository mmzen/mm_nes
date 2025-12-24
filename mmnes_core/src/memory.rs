// Authorship: Human 95% | Claude 5%
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
    Mmc1SwitchableMemory,
    PpuCiramMemory,
    Mmc2SwitchableMemory,
}

impl Display for MemoryType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MemoryType::StandardMemory => write!(f, "memory type: standard Memory"),
            MemoryType::Mmc1SwitchableMemory => write!(f, "memory type: MMC1 switchable Memory"),
            MemoryType::PpuCiramMemory => write!(f, "memory type: ciram Memory"),
            MemoryType::Mmc2SwitchableMemory => write!(f, "memory type: MMC2 switchable Memory"),
        }
    }
}

#[cfg_attr(test, automock)]
pub trait Memory: Debug {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(0)
    }
    
    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError>;

    /// Peek at memory without side effects (does NOT update data bus).
    ///
    /// WARNING: This method must NEVER be used for timing-accurate code paths.
    /// Use only for debugging, disassembly, tracing, and test verification.
    /// For actual emulation, always use `read_byte()` which properly updates
    /// the data bus and triggers device side effects.
    fn peek_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, _addr: u16, _value: u8) -> Result<(), MemoryError> {
        unreachable!()
    }

    /// Read a 16-bit word as two consecutive byte reads.
    ///
    /// WARNING: NOT for cycle-accurate execution paths!
    /// This method performs two separate `read_byte()` calls which:
    /// - Triggers side effects twice (e.g., $2002 VBlank clear, $4016 controller clock)
    /// - Updates data bus twice (high byte ends up on bus)
    /// - Does not model 6502 microcycle timing or page-wrap quirks
    ///
    /// For cycle-accurate emulation, perform individual byte reads per CPU cycle.
    /// This method is only safe for: initialization, debugging, tests.
    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let lo = self.read_byte(addr)?;
        let hi = self.read_byte(addr.wrapping_add(1))?;
        Ok(u16::from_le_bytes([lo, hi]))
    }

    /// Write a 16-bit word as two consecutive byte writes.
    ///
    /// WARNING: NOT for cycle-accurate execution paths!
    /// Same caveats as `read_word()` - performs two separate bus operations.
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



