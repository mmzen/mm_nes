use log::{debug, info, LevelFilter};
use simplelog::{Config, SimpleLogger};
use crate::cpu::{CPU, CpuError};
use crate::cpu_6502::Cpu6502;
use crate::memory::{Memory, MemoryError};
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

fn populate_memory(memory: &mut Box<dyn Memory>) -> Result<(), MemoryError> {
    debug!("populating memory with ROM data...");

    memory.write_byte(0xFFFC, 0x00)?;
    Ok(())
}

fn main() -> Result<(), CpuError> {
    logger_init(true);
    info!("emulator bootstrapping...");
    let mut cpu;
    let mut memory : Box<dyn Memory> = Box::new(Memory64k::default());
    let status;

    populate_memory(&mut memory)?;
    cpu = Cpu6502::new(memory);

    cpu.initialize().expect("cpu initialization failed");
    status = cpu.run();

    if let Err(error) = status {
        cpu.panic(&error);
        Err(error)
    } else {
        Ok(())
    }
}
