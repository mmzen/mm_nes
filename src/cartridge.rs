use std::fmt;
use std::fmt::{Display, Formatter};

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

pub trait Cartridge {}