use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use crate::memory::MemoryError;

pub trait CPU: Debug {
    fn reset(&mut self) -> Result<(), CpuError>;
    fn initialize(&mut self) -> Result<(), CpuError>;
    fn dump_registers(&self);
    fn run(&mut self) -> Result<(), CpuError>;
}

#[derive(Debug)]
pub enum CpuError {
    MemoryError(MemoryError)
}

impl From<MemoryError> for CpuError {
    fn from(error: MemoryError) -> Self {
        CpuError::MemoryError(error)
    }
}

impl Error for CpuError {}

impl Display for CpuError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            CpuError::MemoryError(error) => write!(f, "memory error: {}", error),
            _ => write!(f, "error details not available")
        }
    }
}