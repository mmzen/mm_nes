use log::{info, LevelFilter};
use simplelog::{Config, SimpleLogger};
use crate::cpu::CPU;
use crate::cpu_6502::Cpu6502;
use crate::memory_64k::Memory64k;

mod cpu;
mod cpu_6502;
mod memory;
mod memory_64k;

fn logger_init(debug: bool) {
    let log_level = if debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    SimpleLogger::init(log_level,   Config::default()).unwrap();
}

fn main() {
    logger_init(true);
    info!("emulator bootstrapping...");

    let memory = Box::new(Memory64k::default());
    let mut cpu = Cpu6502::new(memory);

    cpu.initialize().expect("cpu initialization failed");
}
