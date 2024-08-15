use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Default, Debug, Clone)]
pub enum PPUType {
    #[default]
    NESPPU,
    DUMMY
}

impl Display for PPUType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PPUType::NESPPU => write!(f, "ppu type: NESPPU"),
            PPUType::DUMMY => write!(f, "ppu type: DUMMY")
        }
    }
}

pub trait PPU {}