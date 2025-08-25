use std::fs::File;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::{spawn, JoinHandle};
use std::time::{Duration};
use log::{debug, info, LevelFilter};
use simplelog::{Config, SimpleLogger};
use clap::{Parser};
use clap_num::maybe_hex;
use mmnes_core::apu::ApuType::RP2A03;
use mmnes_core::bus::BusType;
use mmnes_core::bus_device::BusDeviceType::{APU, CARTRIDGE, CONTROLLER, PPU, WRAM};
use mmnes_core::cartridge::CartridgeType::NROM;
use mmnes_core::controller::ControllerType::StandardController;
use mmnes_core::cpu::CpuType::NES6502;
use mmnes_core::key_event::KeyEvents;
use mmnes_core::nes_frame::NesFrame;
use mmnes_core::loader::LoaderType::INESV2;
use mmnes_core::memory::MemoryType::NESMemory;
use mmnes_core::nes_console::{NesConsole, NesConsoleBuilder, NesConsoleError};
use mmnes_core::ppu::PpuType::NES2C02;
use crate::nes_front_end::NesFrontEnd;
use crate::nes_front_ui::NesFrontUI;

mod nes_front_ui;
mod sound_player;
mod nes_message;
mod nes_front_end;

const APP_NAME: &str = "MMNES";

const FRAME_BUFFER_WIDTH: usize = 256;
const FRAME_BUFFER_HEIGHT: usize = 240;
const CHANNEL_BOUND_SIZE: usize = 10;
const FRAMES_PER_SECOND: f64 = 60.098_8;
const SPIN_BEFORE: Duration = Duration::from_micros(500);

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

fn create_emulator(args: Args) -> Result<NesConsole, NesConsoleError> {
    let builder = NesConsoleBuilder::new();

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

    console.power_on()?;
    info!("emulator ready");

    Ok(console)
}

fn spawn_emulator_thread(args: Args, tx: SyncSender<NesFrame>, rx: Receiver<KeyEvents>) -> Result<JoinHandle<Result<(), NesConsoleError>>, NesConsoleError> {

    let jh = spawn(move || -> Result<(), NesConsoleError>  {
        let nes = create_emulator(args)?;
        let mut front = NesFrontEnd::new(nes, tx, rx);
        front.run()?;
        Ok(())
    });

    Ok(jh)
}

fn main() -> Result<(), NesConsoleError> {
    let args: Args = Args::parse();

    logger_init(args.debug);

    let native_options = eframe::NativeOptions::default();
    let (tx0, rx0) = sync_channel::<NesFrame>(CHANNEL_BOUND_SIZE);
    let (tx1, rx1) = sync_channel::<KeyEvents>(CHANNEL_BOUND_SIZE);

    let _ = spawn_emulator_thread(args, tx0, rx1)?;

    let _ = eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            let nes_front_ui = NesFrontUI::new(cc, tx1, rx0, FRAME_BUFFER_WIDTH, FRAME_BUFFER_HEIGHT);
            Ok(Box::new(nes_front_ui))
        },),
    );

    Ok(())
}
