use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use crate::bus::BusError;
use crate::cpu::CpuError;
use crate::memory::MemoryError;

#[derive(Default, Debug, Clone)]
pub enum PpuType {
    #[default]
    NES2C02
}

impl Display for PpuType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PpuType::NES2C02 => write!(f, "ppu type: NES2C02")
        }
    }
}

impl PartialEq for PpuType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PpuType::NES2C02, PpuType::NES2C02) => true
        }
    }
}

pub trait PPU {
    fn reset(&mut self) -> Result<(), PpuError>;
    fn initialize(&mut self) -> Result<(), PpuError>;
    fn panic(&self, error: &PpuError);
}

#[derive(Debug)]
pub enum PpuError {
    BusError(BusError),
    MemoryError(MemoryError)
}

impl Error for PpuError {}

impl Display for PpuError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            PpuError::BusError(e) => { write!(f, "bus error: {}", e) }
            PpuError::MemoryError(e) => { write!(f, "memory error: {}", e) }
        }
    }
}

impl From<MemoryError> for PpuError {
    fn from(error: MemoryError) -> Self {
        PpuError::MemoryError(error)
    }
}

impl From<BusError> for PpuError {
    fn from(error: BusError) -> Self {
        PpuError::BusError(error)
    }
}