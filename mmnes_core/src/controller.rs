use std::fmt;
use std::fmt::{Display, Formatter};
use crate::bus_device::BusDevice;
use crate::key_event::KeyEvents;

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

pub trait Controller: BusDevice {
    fn set_input(&mut self, input: KeyEvents) -> Result<(), ControllerError>;
}

#[derive(Debug, PartialEq)]
pub enum ControllerError {
    IncorrectInput(String),
    Unexpected(String),
}

impl Display for ControllerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ControllerError::IncorrectInput(message) => write!(f, "incorrect input: {}", message),
            ControllerError::Unexpected(message) => write!(f, "unexpected error: {}", message),
        }
    }
}