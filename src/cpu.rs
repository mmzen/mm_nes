use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use crate::memory::MemoryError;

pub trait CPU {
    fn reset(&mut self) -> Result<(), CpuError>;
    fn initialize(&mut self) -> Result<(), CpuError>;
    fn panic(&self, error: &CpuError);
    fn dump_registers(&self);
    fn dump_flags(&self);
    #[allow(dead_code)]
    fn dump_memory(&self);
    fn run(&mut self) -> Result<(), CpuError>;
    fn run_start_at(&mut self, address: u16) -> Result<(), CpuError>;
}

#[derive(Debug)]
pub enum CpuError {
    MemoryError(MemoryError),
    IllegalInstruction(u8),
    Unimplemented(String),
    InvalidOperand(String),
    StackOverflow(u16),
    StackUnderflow(u16),
    ConfigurationError(String),
    Halted(u16),
}

impl From<MemoryError> for CpuError {
    fn from(error: MemoryError) -> Self {
        CpuError::MemoryError(error)
    }
}

impl From<std::io::Error> for CpuError {
    fn from(error: std::io::Error) -> Self {
        CpuError::ConfigurationError(error.to_string())
    }
}

impl Error for CpuError {}

impl Display for CpuError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            CpuError::MemoryError(error) => write!(f, "memory error: {}", error),
            CpuError::IllegalInstruction(op) => write!(f, "illegal instruction 0x{:02X}", op),
            CpuError::StackOverflow(addr) => { write!(f, "stack overflow 0x{:04X}", addr) },
            CpuError::StackUnderflow(addr) => { write!(f, "stack underflow 0x{:04X}", addr) },
            CpuError::InvalidOperand(s) => { write!(f, "missing or invalid operand: {}", s) },
            CpuError::ConfigurationError(s) => { write!(f, "configuration error: {}", s) },
            CpuError::Unimplemented(s) => { write!(f, "unimplemented: {}", s) },
            CpuError::Halted(addr) => { write!(f, "cpu halted 0x{:04X}", addr) }
        }
    }
}