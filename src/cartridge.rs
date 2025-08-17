use std::cell::RefCell;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{BufReader, Error, Read};
use std::rc::Rc;
use crate::bus_device::BusDevice;
use crate::memory::{Memory, MemoryError};
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
    UNROM
}

impl Display for CartridgeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CartridgeType::NESCARTRIDGE => { write!(f, "cartridge type: NESCARTRIDGE") },
            CartridgeType::NROM => { write!(f, "cartridge type: NROM") }
            CartridgeType::UNROM => { write!(f, "cartridge type: UNROM") }
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
    fn get_mirroring(&self) -> PpuNameTableMirroring;
}

/***
 * helper functions
 ***/
pub fn write_rom_data(rom: &mut dyn Memory, size: usize, data: &mut BufReader<File>) -> Result<(), CartridgeError> {
    let mut buf = vec![0u8; size];
    data.read_exact(&mut buf)?;

    for (i, &byte) in buf.iter().enumerate() {
        rom.write_byte(i as u16, byte)?;
    }

    Ok(())
}