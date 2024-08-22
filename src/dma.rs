use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use crate::memory::MemoryError;

#[derive(Debug, Clone)]
pub enum DmaType {
    PpuDma(PpuDmaType)
}

#[derive(Default, Debug, Clone)]
pub enum PpuDmaType {
    #[default]
    NESPPUDMA
}

impl Display for DmaType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DmaType::PpuDma(ppu_dma_type) => write!(f, "dma type: PPU - {}", ppu_dma_type)
        }
    }
}

impl Display for PpuDmaType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PpuDmaType::NESPPUDMA => write!(f, "NESPPUDMA")
        }
    }
}


#[derive(Debug)]
pub enum DmaError {
}

impl Display for DmaError {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Error for DmaError {}

pub trait Dma {
    fn transfer_memory(&mut self, value: u8) -> Result<u16, MemoryError>;
}