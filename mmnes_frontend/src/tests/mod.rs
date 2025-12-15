// Authorship: Human 100% | Claude 0%
use std::sync::Once;
use log::LevelFilter;
use simplelog::{Config, TestLogger};

mod llm_client;
mod nes_rom_metadata_worker;

static START: Once = Once::new();

fn init_logger_for_test() {
    START.call_once(|| TestLogger::init(LevelFilter::Debug, Config::default()).unwrap());
}

pub fn init() {
    init_logger_for_test();
}