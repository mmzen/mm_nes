use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::dma_device::DmaDevice;

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
    fn link_device(&mut self, device: Rc<RefCell<dyn DmaDevice>>) -> Result<(), DmaError>;
}