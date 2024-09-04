use std::fmt;
use std::fmt::{Debug, Display, Formatter};
#[cfg(test)]
use mockall::mock;
use crate::memory::MemoryError;

#[allow(dead_code)]
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

#[cfg(test)]
mock! {
    #[derive(Debug)]
    pub DmaDeviceStub {}

    impl DmaDevice for DmaDeviceStub {
        fn dma_write(&mut self, offset: u8, value: u8) -> Result<(), MemoryError>;
    }
}