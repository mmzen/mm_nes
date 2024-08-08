use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use crate::memory::MemoryError;

pub trait CPU: Debug {
    fn reset(&mut self) -> Result<(), CpuError>;
    fn initialize(&mut self) -> Result<(), CpuError>;
    fn panic(&self, error: &CpuError);
    fn dump_registers(&self);
    fn dump_flags(&self);
    fn run(&mut self) -> Result<(), CpuError>;
}

#[derive(Debug)]
pub enum CpuError {
    MemoryError(MemoryError),
    InvalidOpcode(u8),
    InvalidOperand(String),
    StackOverflow,
    StackUnderflow,
    UnImplemented(String),
    FatalError,
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
            CpuError::InvalidOpcode(op) => write!(f, "invalid opcode 0x{:02X}", op),
            CpuError::StackOverflow => { write!(f, "stack overflow") },
            CpuError::StackUnderflow => { write!(f, "stack underflow") }
            CpuError::FatalError => { write!(f, "fatal error") }
            CpuError::InvalidOperand(s) => { write!(f, "missing or invalid operand: {}", s) }
            CpuError::UnImplemented(s) => { write!(f, "unimplemented operation: {}", s)  }
        }
    }
}