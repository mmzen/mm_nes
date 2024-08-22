use std::fs::File;
use std::process::exit;
use log::{debug, info, LevelFilter, trace};
use simplelog::{Config, SimpleLogger};
use clap::Parser;
use clap_num::maybe_hex;
use crate::apu::ApuType::RP2A03;
use crate::bus::BusType;
use crate::bus_device::BusDeviceType::{APU, CARTRIDGE, DMA, PPU, WRAM};
use crate::cartridge::CartridgeType::NROM128;
use crate::cpu::CpuType::NES6502;
use crate::dma::DmaType::PpuDma;
use crate::dma::PpuDmaType::NESPPUDMA;
use crate::loader::LoaderType::INESV1;
use crate::memory::MemoryType::NESMemory;
use crate::nes_console::{NESConsoleBuilder, NESConsoleError};
use crate::ppu::PpuType::NES2C02;

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
mod util;
mod ppu_2c02;
mod palette;
mod palette_2c02;
mod dma_device;
mod dma;
mod ppu_dma;
mod frame;
mod apu_rp2a03;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
        short = 'd',
        long = "debug",
        help = "debug mode",
        default_value_t = 0
    )]
    debug: u8,

    #[arg(
        short = 'x',
        long = "pc-addr",
        help = "set PC immediate address at startup",
        value_parser=maybe_hex::<u16>
    )]
    pc: Option<u16>,

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


fn logger_init(debug: u8) {

    let log_level = match debug {
        1 => LevelFilter::Debug,
        2 => LevelFilter::Trace,
        _ => LevelFilter::Info,
    };

    SimpleLogger::init(log_level, Config::default()).unwrap();
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
        .with_cpu_options(NES6502, trace_file)
        .with_bus_type(BusType::NESBus)
        .with_bus_device_type(WRAM(NESMemory))
        .with_bus_device_type(CARTRIDGE(NROM128))
        .with_bus_device_type(PPU(NES2C02))
        .with_bus_device_type(DMA(PpuDma(NESPPUDMA)))
        .with_bus_device_type(APU(RP2A03))
        .with_loader_type(INESV1)
        .with_rom_file(args.rom_file)
        .with_entry_point(args.pc)
        .build()?;

    info!("emulator starting...");
    console.power_on()?;

    Ok(())
}
