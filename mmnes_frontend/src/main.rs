mod nes_front_end;

use std::fs::File;
use std::hint::spin_loop;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::{sleep, spawn, JoinHandle};
use std::time::{Duration, Instant};
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
use mmnes_core::key_event::KeyEvents;
use mmnes_core::nes_frame::NesFrame;
use mmnes_core::loader::LoaderType::INESV2;
use mmnes_core::memory::MemoryType::NESMemory;
use mmnes_core::nes_console::{NESConsole, NESConsoleBuilder, NESConsoleError};
use mmnes_core::ppu::PpuType::NES2C02;
use crate::nes_front_end::NesFrontend;

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

pub fn start_emulator() -> Result<NESConsole, NESConsoleError> {
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

    Ok(console)
}

fn sleep_until_next_frame(next: Instant, frame: Duration) -> Instant {
    let now = Instant::now();
    let mut next = next;

    if next > now {
        let mut to_sleep = next - now;
        if to_sleep > SPIN_BEFORE {
            to_sleep -= SPIN_BEFORE;
            sleep(to_sleep);
        }

        while Instant::now() < next {
            spin_loop();
        }

        next + frame
    } else {
        while next <= now {
            next += frame;
        }

        next
    }
}

fn get_input(rx: &Receiver<KeyEvents>) -> Option<KeyEvents> {
    let mut key_events = KeyEvents::new();

    if let Ok(events) = rx.try_recv() {
        key_events = key_events.chain(events).collect();
    }

    if key_events.is_empty() {
        None
    } else {
        Some(key_events)
    }
}

fn run(nes: &mut NESConsole, tx: SyncSender<NesFrame>, rx: Receiver<KeyEvents>) -> Result<(), NESConsoleError> {
    let frame_duration = Duration::from_secs_f64(1.0 / FRAMES_PER_SECOND);
    let mut next_frame = Instant::now() + frame_duration;

    loop {
        let inputs = get_input(&rx);

        if let Some(inputs) = inputs {
            nes.set_input(inputs)?;
        }

        let frame =nes.step_frame()?;
        let result = tx.send(frame);

        next_frame = sleep_until_next_frame(next_frame, frame_duration);

        if let Err(error) = result {
            eprintln!("Error sending frame: {}", error);
        }
    }
}

fn spawn_emulator_thread(tx: SyncSender<NesFrame>, rx: Receiver<KeyEvents>) -> Result<JoinHandle<Result<(), NESConsoleError>>, NESConsoleError> {

    let jh = spawn(move || -> Result<(), NESConsoleError>  {
        let mut nes = start_emulator()?;
        run(&mut nes, tx, rx)?;
        unreachable!()
    });

    Ok(jh)
}

fn main() -> Result<(), NESConsoleError> {

    let native_options = eframe::NativeOptions::default();
    let (tx0, rx0) = sync_channel::<NesFrame>(CHANNEL_BOUND_SIZE);
    let (tx1, rx1) = sync_channel::<KeyEvents>(CHANNEL_BOUND_SIZE);

    let _ = spawn_emulator_thread(tx0, rx1)?;

    let _ = eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            let nes_frontend = NesFrontend::new(cc, tx1, rx0, FRAME_BUFFER_WIDTH, FRAME_BUFFER_HEIGHT);
            Ok(Box::new(nes_frontend))
        },),
    );

    Ok(())
}
