use std::fmt::Debug;
use crate::key_event::KeyEvents;

pub trait Input: Debug {
    fn get_input_state(&mut self, control_states: &mut [u8; 8]);
    fn set_input_state(&mut self, _key_events: KeyEvents) {
        unreachable!()
    }
}

#[derive(Debug, PartialEq)]
pub enum InputError {
    InputFailure(String)
}

impl From<String> for InputError {
    fn from(error: String) -> Self {
        InputError::InputFailure(error)
    }
}