// Authorship: Human 70% | Claude 30%
use std::any::Any;
use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::bus::BusError;
use crate::bus_device::BusDevice;
use crate::cartridge::Cartridge;
use crate::config_spec::Configurable;
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

pub trait PPU: BusDevice + DmaDevice + Configurable {
    fn reset(&mut self) -> Result<(), PpuError>;
    fn panic(&self, error: &PpuError);

    /// Run the PPU for the specified number of CPU cycles.
    /// Converts CPU cycles to PPU dots and advances the PPU accordingly.
    /// Returns the new cycle count and an optional completed frame.
    /// ```start_cycle```: current cycle of execution,
    /// ```credits```: the number of CPU cycles to advance
    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<(u32, Option<NesFrame>), PpuError>;

    /// Advance PPU by the specified number of dots.
    /// This method provides cycle-accurate control for the master scheduler.
    /// NTSC: 3 dots per CPU cycle, PAL: ~3.2 dots per CPU cycle.
    /// Returns an optional completed frame.
    fn advance_dots(&mut self, dots: u32) -> Result<Option<NesFrame>, PpuError>;

    /// Get the current frame buffer.
    fn frame(&self) -> NesFrame;

    /// Get current dot position within scanline (0-340).
    fn get_dot(&self) -> u16;

    /// Get current scanline (0-261 for NTSC, 0-311 for PAL).
    fn get_scanline(&self) -> u16;

    /// Called at the end of each master cycle to clear per-cycle state.
    /// This clears latches like status_read_this_cycle used for boundary suppression.
    /// Must be called after all PPU advancement for the cycle is complete.
    fn end_master_cycle(&mut self);

    /// Set cartridge reference for mappers that need PPU address bus notifications (e.g., MMC3).
    /// The PPU will call `cartridge.notify_ppu_address(addr)` for all bus reads.
    fn set_cartridge(&mut self, cartridge: Rc<RefCell<dyn Cartridge>>);

    /// Downcast support for test helpers.
    fn as_any(&self) -> &dyn Any;
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