use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Default, Debug, Clone)]
pub enum ControllerType {
    #[default]
    StandardController,
}

impl Display for ControllerType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ControllerType::StandardController => write!(f, "controller type: Standard Controller"),
        }
    }
}

impl PartialEq for ControllerType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ControllerType::StandardController, ControllerType::StandardController) => true
        }
    }
}

pub trait Controller {}

#[derive(Debug, PartialEq)]
pub enum ControllerError {
    IncorrectInput(String)
}

