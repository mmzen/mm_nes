use std::collections::VecDeque;

pub const NES_CONTROLLER_KEY_A: usize = 0x00;
pub const NES_CONTROLLER_KEY_B: usize = 0x01;
pub const NES_CONTROLLER_KEY_SELECT: usize = 0x02;
pub const NES_CONTROLLER_KEY_START: usize = 0x03;
pub const NES_CONTROLLER_KEY_UP: usize = 0x04;
pub const NES_CONTROLLER_KEY_DOWN: usize = 0x05;
pub const NES_CONTROLLER_KEY_LEFT: usize = 0x06;
pub const NES_CONTROLLER_KEY_RIGHT: usize = 0x07;

#[derive(Debug, Clone, PartialEq)]
pub struct KeyEvent {
    pub key: usize,
    pub pressed: bool
}

#[derive(Debug, Clone)]
pub struct KeyEvents { events: VecDeque<KeyEvent> }

impl KeyEvents {
    pub fn new() -> Self {
        KeyEvents { events: VecDeque::new() }
    }
    
    pub fn push_back(&mut self, key_event: KeyEvent) {
        self.events.push_back(key_event);
    }

    pub fn pop_front(&mut self) -> Option<KeyEvent> {
        self.events.pop_front()
    }
    
    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Iterator for KeyEvents {
    type Item = KeyEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.pop_front()
    }
}

impl FromIterator<KeyEvent> for KeyEvents {
    fn from_iter<T: IntoIterator<Item = KeyEvent>>(iter: T) -> Self {
        KeyEvents { events: iter.into_iter().collect() }
    }
}