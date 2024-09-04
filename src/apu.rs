use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Default, Debug, Clone)]
pub enum ApuType {
    #[default]
    RP2A03
}

impl Display for ApuType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ApuType::RP2A03 => write!(f, "apu type: NESAPU"),
        }
    }
}

impl PartialEq for ApuType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ApuType::RP2A03, ApuType::RP2A03) => true
        }
    }
}

#[allow(dead_code)]
pub trait APU {}