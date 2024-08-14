use std::process::exit;
use log::{debug, LevelFilter};
use once_cell::sync::Lazy;
use simplelog::{Config, TestLogger};
use std::sync::Once;

mod nes_bus;
mod memory_bank;

static START: Once = Once::new();

fn init_logger_for_test() {
    START.call_once(|| TestLogger::init(LevelFilter::Debug, Config::default()).unwrap());
}

pub fn init() {
    init_logger_for_test();
}

