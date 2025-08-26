use std::path::PathBuf;
use mmnes_core::key_event::KeyEvents;
use mmnes_core::nes_console::NesConsoleError;
use mmnes_core::nes_frame::NesFrame;

#[derive(Debug, Clone)]
pub enum NesMessage {
    Frame(NesFrame),
    LoadRom(PathBuf),
    Keys(KeyEvents),
    Start,
    Pause,
    Reset,
    Error(NesConsoleError),
}