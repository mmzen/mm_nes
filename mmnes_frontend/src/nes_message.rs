use mmnes_core::key_event::KeyEvents;

#[derive(Debug, Clone)]
pub enum NesMessage {
    LoadRom(String),
    Keys(KeyEvents),
    Start,
    Pause,
    Reset
}