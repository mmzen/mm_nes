use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Default, Debug, Clone)]
pub enum CartridgeType {
    #[default]
    NESCARTRIDGE
}

impl Display for CartridgeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CartridgeType::NESCARTRIDGE => { write!(f, "cartridge type: NESCARTRIDGE") },
        }
    }
}

pub trait Cartridge {}