use std::sync::Once;
use log::LevelFilter;
use simplelog::{Config, TestLogger};

mod rdb;

static START: Once = Once::new();

fn init_logger_for_test() {
    START.call_once(|| TestLogger::init(LevelFilter::Trace, Config::default()).unwrap());
}

pub fn init() {
    init_logger_for_test();
}