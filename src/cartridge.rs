use std::fmt;
use std::fmt::{Display, Formatter};
use crate::bus_device::BusDevice;

#[derive(Default, Debug, Clone)]
pub enum CartridgeType {
    #[default]
    NESCARTRIDGE,
    NROM128,
}

impl Display for CartridgeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CartridgeType::NESCARTRIDGE => { write!(f, "cartridge type: NESCARTRIDGE") },
            CartridgeType::NROM128 => { write!(f, "cartridge type: NROM128") }
        }
    }
}

impl PartialEq for CartridgeType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (CartridgeType::NESCARTRIDGE, CartridgeType::NESCARTRIDGE) => true,
            (CartridgeType::NROM128, CartridgeType::NROM128) => true,
            _ => false,
        }
    }
}

pub trait Cartridge: BusDevice {}