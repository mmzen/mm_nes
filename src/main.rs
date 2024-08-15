use std::fs::File;
use log::{debug, info, LevelFilter};
use simplelog::{Config, SimpleLogger};
use clap::Parser;
use clap_num::maybe_hex;
use crate::bus::BusType;
use crate::bus_device::BusDeviceType;
use crate::cpu::{CpuType};
use crate::memory::MemoryType;
use crate::nes_console::{NESConsoleBuilder, NESConsoleError};

mod cpu;
mod cpu_6502;
mod memory;
mod memory_bank;
mod loader;
mod ines_loader;
mod nes_console;
mod nes_bus;
mod ppu;
#[cfg(test)]
mod tests;
mod bus;
mod apu;
mod bus_device;
mod dummy_device;
mod cartridge;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
        short = 'd',
        long = "debug",
        help = "debug mode",
        default_value_t = false
    )]
    debug: bool,

    #[arg(
        short = 'x',
        long = "addr",
        help = "set PC address at startup",
        value_parser=maybe_hex::<u16>,
        default_value_t = 0xc000
    )]
    pc: u16,

    #[arg(
        short = 't',
        long = "trace-file",
        help = "output for CPU tracing"
    )]
    trace_file: Option<String>,

    #[arg(
        short = 'f',
        long = "rom-file",
        help = "rom file to load",
        required = true,
    )]
    rom_file: String
}


fn logger_init(debug: bool) {
    let log_level = if debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    SimpleLogger::init(log_level,   Config::default()).unwrap();
}

fn main() -> Result<(), NESConsoleError> {
    let args: Args = Args::parse();

    logger_init(args.debug);

    let builder = NESConsoleBuilder::new();

    let file = if let Some(trace_file) = args.trace_file {
        debug!("output for traces: {}", trace_file);
        Some(File::create(trace_file)?)
    } else {
        debug!("output for traces: stdout");
        None
    };

    info!("emulator bootstrapping...");

    let console = builder
        .with_cpu_options(CpuType::NES6502, file)
        .with_bus_type(BusType::NESBus)
        .with_bus_device_type(BusDeviceType::WRAM(MemoryType::NESMemory))
        .build();

    if let Err(error) = console {
        return Err(error);
    }

    info!("emulator starting...");

    //let mut cpu;
    //let mut memory : Box<dyn Memory> = Box::new(MemoryBank::default());
    //let status;
    //let mut loader;

    //populate_memory(&mut memory)?;

    //loader = Box::new(INesLoader::new_with_memory(&mut memory));
    //loader.load_rom(&args.rom_file).expect("fuck you");



    //cpu = Cpu6502::new(memory, file);
    //cpu.initialize().expect("cpu initialization failed");
    //status = cpu.run_start_at(args.pc);

    //if let Err(error) = status {
    //    cpu.panic(&error);
    //    Err(error)
    //} else {
    //    Ok(())
    //}
    Ok(())
}
