use std::path::PathBuf;
use mmnes_core::key_event::KeyEvents;
use mmnes_core::nes_console::NesConsoleError;
use mmnes_core::nes_frame::NesFrame;
use mmnes_core::cpu_debugger::CpuSnapshot;

#[derive(Debug)]
pub enum NesMessage {
    Frame(NesFrame),
    LoadRom(PathBuf),
    Keys(KeyEvents),
    Pause,
    Reset,
    DebugStepInstruction,
    Error(NesConsoleError),
    CpuSnapshot(Box<dyn CpuSnapshot>),
}