use std::fmt;
use std::fmt::{Display, Formatter};
use crate::cpu::CpuError;
use crate::memory::MemoryError;

#[derive(Default, Debug, Clone)]
pub enum ApuType {
    #[default]
    RP2A03
}

impl Display for ApuType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ApuType::RP2A03 => write!(f, "apu type: NESAPU"),
        }
    }
}

impl PartialEq for ApuType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ApuType::RP2A03, ApuType::RP2A03) => true
        }
    }
}

#[derive(Debug)]
pub enum ApuError {
    CpuError(CpuError),
    MemoryError(MemoryError)
}

impl From<CpuError> for ApuError { 
    fn from(error: CpuError) -> Self {
        ApuError::CpuError(error)
    }
}

impl Display for ApuError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ApuError::MemoryError(e) => { write!(f, "memory error: {}", e) },
            ApuError::CpuError(e) => { write!(f, "cpu error: {}", e) }
        }
    }
}

#[allow(dead_code)]
pub trait APU {
    fn reset(&mut self) -> Result<(), ApuError>;
    fn panic(&self, error: &ApuError);
    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<u32, ApuError>;
}