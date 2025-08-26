use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use crate::bus::BusError;
use crate::bus_device::BusDevice;
use crate::cpu::CpuError;
use crate::dma_device::DmaDevice;
use crate::nes_frame::NesFrame;
use crate::memory::MemoryError;

#[derive(Debug, Clone, Copy)]
pub enum PpuNameTableMirroring {
    Vertical,
    Horizontal
}

impl Display for PpuNameTableMirroring {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PpuNameTableMirroring::Vertical => write!(f, "vertical mirroring"),
            PpuNameTableMirroring::Horizontal => write!(f, "horizontal mirroring")
        }
    }
}

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
    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<(u32, Option<NesFrame>), PpuError>;
    fn frame(&self) -> NesFrame;
}

#[derive(Debug, Clone)]
pub enum PpuError {
    BusError(BusError),
    MemoryError(MemoryError),
    CpuError(CpuError)
}

impl Error for PpuError {}

impl Display for PpuError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            PpuError::BusError(e) => { write!(f, "-> bus error: {}", e) }
            PpuError::MemoryError(e) => { write!(f, "-> memory error: {}", e) }
            PpuError::CpuError(e) => { write!(f, "-> cpu error: {}", e) }
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