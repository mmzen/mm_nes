use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Default, Debug, Clone)]
pub enum APUType {
    #[default]
    NESAPU,
    DUMMY
}

impl Display for APUType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            APUType::NESAPU => write!(f, "apu type: NESAPU"),
            APUType::DUMMY => write!(f, "apu type: DUMMY")
        }
    }
}

impl PartialEq for APUType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (APUType::NESAPU, APUType::NESAPU) => true,
            (APUType::DUMMY, APUType::DUMMY) => true,
            _ => false,
        }
    }
}

pub trait APU {}