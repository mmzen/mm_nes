use std::sync::Once;
use log::LevelFilter;
use simplelog::{Config, TestLogger};
use mmnes_core::memory_bank::MemoryBank;

mod llm_client;
mod nes_rom_metadata_worker;

static START: Once = Once::new();

fn init_logger_for_test() {
    START.call_once(|| TestLogger::init(LevelFilter::Trace, Config::default()).unwrap());
}

pub fn init() {
    init_logger_for_test();
}