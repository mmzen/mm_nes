use std::cell::RefCell;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::io::Error;
use std::rc::Rc;
use crate::bus_device::BusDevice;
use crate::memory::MemoryError;
use crate::ppu::PpuNameTableMirroring;


#[derive(Debug)]
pub enum CartridgeError {
    LoadingError(String),
    MemoryError(MemoryError),
}

impl From<Error> for CartridgeError {
    fn from(error: Error) -> Self {
        CartridgeError::LoadingError(error.to_string())
    }
}

impl From<MemoryError> for CartridgeError {
    fn from(error: MemoryError) -> Self {
        CartridgeError::MemoryError(error)
    }
}

impl Display for CartridgeError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            CartridgeError::LoadingError(s) => { write!(f, "loading error {}", s) }
            CartridgeError::MemoryError(e) => { write!(f, "memory error {}", e) }
        }
    }
}

#[derive(Default, Debug, Clone)]
pub enum CartridgeType {
    #[default]
    NESCARTRIDGE,
    NROM,
}

impl Display for CartridgeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CartridgeType::NESCARTRIDGE => { write!(f, "cartridge type: NESCARTRIDGE") },
            CartridgeType::NROM => { write!(f, "cartridge type: NROM128") }
        }
    }
}

impl PartialEq for CartridgeType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (CartridgeType::NESCARTRIDGE, CartridgeType::NESCARTRIDGE) => true,
            (CartridgeType::NROM, CartridgeType::NROM) => true,
            _ => false,
        }
    }
}

pub trait Cartridge: BusDevice {
    fn get_chr_rom(&self) -> Rc<RefCell<dyn BusDevice>>;
    fn get_prg_rom(&self) -> Rc<RefCell<dyn BusDevice>>;
    fn get_mirroring(&self) -> PpuNameTableMirroring;
}