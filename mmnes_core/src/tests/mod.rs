use log::LevelFilter;
use simplelog::{Config, TestLogger};
use std::sync::Once;
use crate::memory_bank::MemoryBank;

mod nes_bus;
mod memory_bank;
mod ppu_2c02;
mod ppu_dma;
mod cpu_6502;
mod memory_mirror;
mod input_external;
mod key_events;
mod sound_playback_passive;
mod nes_samples;

static START: Once = Once::new();

fn init_logger_for_test() {
    START.call_once(|| TestLogger::init(LevelFilter::Trace, Config::default()).unwrap());
}

pub fn init() {
    init_logger_for_test();
}

fn create_memory_bank(size: usize, address_range: (u16, u16)) -> MemoryBank {
    MemoryBank::new(size, address_range)
}




