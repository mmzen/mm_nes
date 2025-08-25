use crate::input::Input;
use crate::key_event::{KeyEvents};

#[derive(Debug)]
pub struct InputExternal {
    key_events: KeyEvents,
}

impl Input for InputExternal {
    fn get_input_state(&mut self, control_states: &mut [u8; 8]) {
        while let Some(event) = self.key_events.next() {
            control_states[event.key] = event.pressed as u8;
        }
    }

    fn set_input_state(&mut self, key_events: KeyEvents) {
        for event in key_events {
            self.key_events.push_back(event);
        }
    }
}

impl InputExternal {
    pub fn new() -> Self {
        InputExternal {
            key_events: KeyEvents::new(),
        }
    }
}
