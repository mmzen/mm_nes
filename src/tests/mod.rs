use log::LevelFilter;
use simplelog::{Config, TestLogger};
use std::sync::Once;

mod nes_bus;
mod memory_bank;
mod ppu_2c02;
mod ppu_dma;

static START: Once = Once::new();

fn init_logger_for_test() {
    START.call_once(|| TestLogger::init(LevelFilter::Trace, Config::default()).unwrap());
}

pub fn init() {
    init_logger_for_test();
}



