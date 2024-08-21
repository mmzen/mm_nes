use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use crate::memory::MemoryError;

#[derive(Debug, Clone)]
pub enum DmaDeviceType {
    PpuDmaDevice
}

impl Display for DmaDeviceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DmaDeviceType::PpuDmaDevice => { write!(f, "dma device type: PPU DMA") }
        }
    }
}

pub trait DmaDevice: Debug {
    fn dma_write(&mut self, offset: u8, value: u8) -> Result<(), MemoryError>;
}