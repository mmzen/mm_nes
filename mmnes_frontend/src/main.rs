use std::fs::File;
use log::{debug, info, LevelFilter};
use simplelog::{Config, SimpleLogger};
use clap::Parser;
use clap_num::maybe_hex;
use mmnes_core::apu::ApuType::RP2A03;
use mmnes_core::bus::BusType;
use mmnes_core::bus_device::BusDeviceType::{APU, CARTRIDGE, CONTROLLER, PPU, WRAM};
use mmnes_core::cartridge::CartridgeType::NROM;
use mmnes_core::controller::ControllerType::StandardController;
use mmnes_core::cpu::CpuType::NES6502;
use mmnes_core::loader::LoaderType::INESV2;
use mmnes_core::memory::MemoryType::NESMemory;
use mmnes_core::nes_console::{NESConsoleBuilder, NESConsoleError};
use mmnes_core::ppu::PpuType::NES2C02;


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
        short = 'g',
        long = "cpu-tracing",
        help = "activate cpu tracing",
        default_value_t = false,
    )]
    cpu_tracing: bool,

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

    /***
     * XXX order of initialization is important:
     * 1. APU covers a single range from 0x4000 to 0x4017, because of the default bus implementation that does not support multiple ranges.
     * 2. PPU (OAM DMA) and CONTROLLER overwrite part of the APU range with their own memory spaces.
     * /!\ Changing the order will result in PPU and CONTROLLER having no mapping to the bus.
     ***/
    let mut console = builder
        .with_cpu_tracing_options(NES6502, args.cpu_tracing, trace_file)
        .with_bus_type(BusType::NESBus)
        .with_bus_device_type(WRAM(NESMemory))
        .with_bus_device_type(CARTRIDGE(NROM))
        .with_bus_device_type(APU(RP2A03))
        .with_bus_device_type(PPU(NES2C02))
        .with_bus_device_type(CONTROLLER(StandardController))
        .with_loader_type(INESV2)
        .with_rom_file(args.rom_file)
        .with_entry_point(args.pc)
        .build()?;

    info!("emulator starting...");
    let r = console.power_on();

    if let Err(e) = r {
        eprintln!("fatal error: {}", e);
    }

    Ok(())
}
