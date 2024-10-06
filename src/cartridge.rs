use std::cell::RefCell;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::bus_device::BusDevice;
use crate::ppu::PpuNameTableMirroring;

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