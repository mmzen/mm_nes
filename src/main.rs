use std::fs::File;
use log::{debug, info, LevelFilter};
use simplelog::{Config, SimpleLogger};
use clap::Parser;
use clap_num::maybe_hex;
use crate::bus::BusType;
use crate::bus_device::BusDeviceType;
use crate::cartridge::CartridgeType;
use crate::cpu::{CpuType};
use crate::loader::LoaderType;
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
mod nrom128_cartridge;

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

    let trace_file = if let Some(trace_file) = args.trace_file {
        debug!("output for traces: {}", trace_file);
        Some(File::create(trace_file)?)
    } else {
        debug!("output for traces: stdout");
        None
    };

    info!("emulator bootstrapping...");

    let mut console = builder
        .with_cpu_options(CpuType::NES6502, trace_file)
        .with_bus_type(BusType::NESBus)
        .with_bus_device_type(BusDeviceType::WRAM(MemoryType::NESMemory))
        .with_bus_device_type(BusDeviceType::CARTRIDGE(CartridgeType::NROM128))
        .with_loader_type(LoaderType::INESV1)
        .with_rom_file(args.rom_file)
        .with_entry_point(args.pc)
        .build()?;

    info!("emulator starting...");
    console.power_on()?;

    Ok(())
}
