// Authorship: Human 65% | Claude 35%
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use crate::memory::MemoryError;
#[cfg(test)]
use mockall::mock;
use crate::cpu_debugger::CpuSnapshot;

#[derive(Default, Debug, Clone)]
pub enum CpuType {
    #[default]
    NES6502
}

/// Type of bus operation performed during a CPU cycle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BusOperation {
    /// No bus operation (internal cycle, though 6502 always reads something)
    #[default]
    None,
    /// Read from address
    Read,
    /// Write to address
    Write,
}

/// Intent for the bus operation the CPU will perform on the NEXT call to step_cycle().
/// This allows the DMA controller to know whether the CPU is about to read or write
/// before actually executing the cycle.
#[derive(Debug, Clone, Copy, Default)]
pub struct CpuBusIntent {
    /// The type of bus operation that will be performed
    pub op: BusOperation,
    /// The address that will be accessed (if known)
    pub address: Option<u16>,
    /// True if the CPU is about to perform a write (convenience method)
    pub is_write: bool,
}

/// Result of executing a single CPU cycle.
/// Provides information about what happened during the cycle for synchronization purposes.
#[derive(Debug, Clone, Default)]
pub struct CpuCycleResult {
    /// True if an instruction just completed this cycle.
    pub instruction_complete: bool,
    /// True if a memory read occurred this cycle.
    pub memory_read: bool,
    /// True if a memory write occurred this cycle.
    pub memory_write: bool,
    /// The address accessed this cycle (if any).
    pub address: Option<u16>,
    /// The data read or written this cycle (if any).
    pub data: Option<u8>,
    /// True if the CPU is halted (waiting for DMA).
    pub halted: bool,
    /// The type of bus operation performed.
    pub bus_op: BusOperation,
    /// Description of what this cycle did (for debugging).
    pub cycle_description: &'static str,
}

pub trait CPU: Interruptible + Debug {
    fn reset(&mut self) -> Result<(), CpuError>;
    fn initialize(&mut self) -> Result<(), CpuError>;
    fn panic(&self, error: &CpuError);
    fn dump_registers(&self);
    fn dump_flags(&self);
    #[allow(dead_code)]
    fn dump_memory(&self);

    /// Execute 1 single CPU cycle. Returns information about what happened during this cycle.
    /// This enables cycle-accurate emulation where PPU/APU can be advanced between CPU cycles.
    fn step_cycle(&mut self) -> Result<CpuCycleResult, CpuError>;

    /// Check if the CPU is currently mid-instruction (has pending cycles to complete).
    fn is_mid_instruction(&self) -> bool;

    /// Get the current instruction cycle number (0 = fetch opcode, 1+ = executing).
    /// Returns None if not in an executing state.
    fn get_instruction_cycle(&self) -> Option<u8>;

    /// Get the current opcode being executed (for debugging).
    /// Returns None if not executing an instruction.
    fn get_current_opcode(&self) -> Option<u8>;

    /// Check if the CPU is currently halted (e.g., waiting for DMA to complete).
    fn is_halted(&self) -> bool;

    /// Halt the CPU for the specified number of cycles (used by DMA).
    fn halt_cycles(&mut self, cycles: u32);

    /// Get the current total cycle count.
    fn get_cycles(&self) -> u32;

    /// Get the intended bus operation for the NEXT call to step_cycle().
    /// This allows the DMA controller to know whether the CPU will read or write
    /// before actually executing the cycle, which is critical for DMA halt timing.
    ///
    /// DMA can only halt the CPU on a read cycle - writes must complete.
    fn get_pending_bus_operation(&self) -> CpuBusIntent;

    fn set_pc_immediate(&mut self, address: u16) -> Result<(), CpuError>;
    fn set_pc_indirect(&mut self, address: u16) -> Result<(), CpuError>;
    fn snapshot(&self) -> Result<Box<dyn CpuSnapshot>, CpuError>;
}

#[derive(Debug, Clone)]
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
            CpuError::MemoryError(error) => write!(f, "-> memory error: {}", error),
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
    fn signal_irq(&mut self, irq_source: u8) -> Result<(), CpuError>;
    fn clear_irq(&mut self, irq_source: u8) -> Result<(), CpuError>;
    fn is_asserted_irq(&self) -> Result<bool, CpuError>;
    fn is_asserted_irq_by_source(&self, irq_source: u8) -> Result<bool, CpuError>;
    fn signal_nmi(&mut self) -> Result<(), CpuError>;
    fn clear_nmi(&mut self) -> Result<(), CpuError>;
    fn is_asserted_nmi(&self) -> Result<bool, CpuError>;
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
        fn set_pc_immediate(&mut self, address: u16) -> Result<(), CpuError>;
        fn set_pc_indirect(&mut self, address: u16) -> Result<(), CpuError>;
        fn snapshot(&self) -> Result<Box<dyn CpuSnapshot>, CpuError>;
        fn step_cycle(&mut self) -> Result<CpuCycleResult, CpuError>;
        fn is_mid_instruction(&self) -> bool;
        fn get_instruction_cycle(&self) -> Option<u8>;
        fn get_current_opcode(&self) -> Option<u8>;
        fn is_halted(&self) -> bool;
        fn halt_cycles(&mut self, cycles: u32);
        fn get_cycles(&self) -> u32;
        fn get_pending_bus_operation(&self) -> CpuBusIntent;
    }

    impl Interruptible for CpuStub {
        fn signal_irq(&mut self, irq_source: u8) -> Result<(), CpuError>;
        fn clear_irq(&mut self, irq_source: u8) -> Result<(), CpuError>;
        fn is_asserted_irq(&self) -> Result<bool, CpuError>;
        fn is_asserted_irq_by_source(&self, irq_source: u8) -> Result<bool, CpuError>;
        fn signal_nmi(&mut self) -> Result<(), CpuError>;
        fn clear_nmi(&mut self) -> Result<(), CpuError>;
        fn is_asserted_nmi(&self) -> Result<bool, CpuError>;
    }
}