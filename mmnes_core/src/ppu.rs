use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use crate::bus::BusError;
use crate::bus_device::BusDevice;
use crate::cpu::CpuError;
use crate::dma_device::DmaDevice;
use crate::nes_frame::NesFrame;
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

pub trait PPU: BusDevice + DmaDevice {
    fn reset(&mut self) -> Result<(), PpuError>;
    fn panic(&self, error: &PpuError);

    /// Run the PPU for 114 cycles (completing 1 scanline), returning the new cycle count after execution and a full frame if available (after having rendered 240 scanlines).
    /// The current implementation ignore the credits input, and will always render a full scanline, updating
    /// the current cycle count by 114.
    /// ```start_cycle```: current cycle of execution,
    /// ```credits```: the number of cycles available to execute instructions (ignored)
    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<(u32, Option<NesFrame>), PpuError>;
    fn frame(&self) -> NesFrame;
}

#[derive(Debug, Clone)]
pub enum PpuError {
    BusError(BusError),
    MemoryError(MemoryError),
    CpuError(CpuError),
    UnsupportedConfiguration(String)
}

impl Error for PpuError {}

impl Display for PpuError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            PpuError::BusError(e) => { write!(f, "-> bus error: {}", e) }
            PpuError::MemoryError(e) => { write!(f, "-> memory error: {}", e) }
            PpuError::CpuError(e) => { write!(f, "-> cpu error: {}", e) }
            PpuError::UnsupportedConfiguration(s) => { write!(f, "unsupported configuration: {}", s) }
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

impl From<CpuError> for PpuError {
    fn from(error: CpuError) -> Self {
        PpuError::CpuError(error)
    }
}