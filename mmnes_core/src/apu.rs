// Authorship: Human 70% | Claude 30%
use std::fmt;
use std::fmt::{Display, Formatter};
use crate::config_spec::Configurable;
use crate::cpu::CpuError;
use crate::irq_source::IrqError;
use crate::memory::MemoryError;
use crate::nes_samples::NesSamples;

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

#[derive(Debug, Clone)]
pub enum ApuError {
    CpuError(CpuError),
    MemoryError(MemoryError),
    IrqError(IrqError),
}

impl From<CpuError> for ApuError { 
    fn from(error: CpuError) -> Self {
        ApuError::CpuError(error)
    }
}

impl From<MemoryError> for ApuError {
    fn from(error: MemoryError) -> Self {
        ApuError::MemoryError(error)
    }
}

impl From<IrqError> for ApuError {
    fn from(error: IrqError) -> Self {
        ApuError::IrqError(error)
    }
}


impl Display for ApuError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ApuError::MemoryError(e) => { write!(f, "-> memory error: {}", e) },
            ApuError::CpuError(e) => { write!(f, "-> cpu error: {}", e) },
            ApuError::IrqError(e) => { write!(f, "-> irq error: {}", e) }
        }
    }
}

#[allow(dead_code)]
pub trait APU: Configurable {
    fn reset(&mut self) -> Result<(), ApuError>;
    fn panic(&self, error: &ApuError);

    /// Run the APU for exactly the specified number of cycles, returning the new cycle count after execution and an optional NesSample buffer containing samples.
    /// ```start_cycle```: current cycle of execution,
    /// ```credits```: the number of cycles available to execute instructions
    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<(u32, Option<NesSamples>), ApuError>;

    /// Execute a single APU cycle (for cycle-accurate mode).
    /// This advances all APU channels by one CPU cycle.
    /// Returns an optional sample if one was generated this cycle.
    fn step_cycle(&mut self) -> Result<Option<f32>, ApuError>;

    /// Check if DMC DMA is needed (sample buffer empty and bytes remaining).
    /// Returns the address to fetch from if DMA is needed, None otherwise.
    /// The scheduler should call this after each APU cycle to check for DMA requests.
    fn needs_dmc_dma(&self) -> Option<u16>;

    /// Provide the DMC with a fetched sample byte (called after DMC DMA completes).
    /// The scheduler calls this after performing the DMA read via the DmaController.
    fn provide_dmc_sample(&mut self, value: u8) -> Result<(), ApuError>;

    /// Debug: Get DMC timer period (for timing analysis).
    fn debug_get_dmc_timer_period(&self) -> u16;

    /// Debug: Get DMC bits remaining (for timing analysis).
    fn debug_get_dmc_bits_remaining(&self) -> u8;

    /// Debug: Get DMC timer counter (for timing analysis).
    fn debug_get_dmc_timer_counter(&self) -> u16;
}