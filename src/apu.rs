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

pub trait APU {}