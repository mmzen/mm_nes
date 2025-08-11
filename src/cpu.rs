use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use crate::memory::MemoryError;
#[cfg(test)]
use mockall::mock;

#[derive(Default, Debug, Clone)]
pub enum CpuType {
    #[default]
    NES6502
}

pub trait CPU: Interruptible + Debug {
    fn reset(&mut self) -> Result<(), CpuError>;
    fn initialize(&mut self) -> Result<(), CpuError>;
    fn panic(&self, error: &CpuError);
    fn dump_registers(&self);
    fn dump_flags(&self);
    #[allow(dead_code)]
    fn dump_memory(&self);
    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<u32, CpuError>;
    fn set_pc_immediate(&mut self, address: u16) -> Result<(), CpuError>;
    fn set_pc_indirect(&mut self, address: u16) -> Result<(), CpuError>;
}

#[derive(Debug)]
pub enum CpuError {
    MemoryError(MemoryError),
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
            CpuError::StackOverflow(addr) => { write!(f, "stack overflow 0x{:04X}", addr) },
            CpuError::StackUnderflow(addr) => { write!(f, "stack underflow 0x{:04X}", addr) },
            CpuError::InvalidOperand(s) => { write!(f, "missing or invalid operand: {}", s) },
            CpuError::ConfigurationError(s) => { write!(f, "configuration error: {}", s) },
            CpuError::Unimplemented(s) => { write!(f, "unimplemented: {}", s) },
            CpuError::Halted(addr) => { write!(f, "cpu halted 0x{:04X}", addr) }
        }
    }
}

pub trait Interruptible {
    #[allow(dead_code)]
    fn signal_irq(&mut self) -> Result<(), CpuError>;
    fn signal_nmi(&mut self) -> Result<(), CpuError>;
}

#[cfg(test)]
mock! {
    #[derive(Debug)]
    pub CpuStub {}

    impl CPU for CpuStub {
        fn reset(&mut self) -> Result<(), CpuError>;
        fn initialize(&mut self) -> Result<(), CpuError>;
        fn panic(&self, error: &CpuError);
        fn dump_registers(&self);
        fn dump_flags(&self);
        #[allow(dead_code)]
        fn dump_memory(&self);
        fn run(&mut self, start_cycle: u32, credits: u32) -> Result<u32, CpuError>;
        fn set_pc_immediate(&mut self, address: u16) -> Result<(), CpuError>;
        fn set_pc_indirect(&mut self, address: u16) -> Result<(), CpuError>;
    }

    impl Interruptible for CpuStub {
        fn signal_irq(&mut self) -> Result<(), CpuError>;
        fn signal_nmi(&mut self) -> Result<(), CpuError>;
    }
}