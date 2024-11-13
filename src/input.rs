use std::fmt::Debug;

pub trait Input: Debug {
    fn get_input_state(&mut self, control_states: &mut [u8; 8]);
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