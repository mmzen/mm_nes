use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::{spawn, JoinHandle};
use std::time::{Duration};
use log::{error, LevelFilter};
use simplelog::{Config, SimpleLogger};
use clap::{Parser};
use clap_num::maybe_hex;
use mmnes_core::nes_console::NesConsoleError;
use crate::nes_front_end::NesFrontEnd;
use crate::nes_front_ui::NesFrontUI;
use crate::nes_message::NesMessage;

mod nes_front_ui;
mod sound_player;
mod nes_message;
mod nes_front_end;
mod text_8x8_generator;

const APP_NAME: &str = "MMNES";

const FRAME_BUFFER_WIDTH: usize = 256;
const FRAME_BUFFER_HEIGHT: usize = 240;
const CHANNEL_BOUND_SIZE: usize = 10;
const FRAMES_PER_SECOND: f64 = 60.098_8;
const SPIN_BEFORE: Duration = Duration::from_micros(500);

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
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
    rom_file: PathBuf
}


fn logger_init(debug: u8) {

    let log_level = match debug {
        1 => LevelFilter::Debug,
        2 => LevelFilter::Trace,
        _ => LevelFilter::Info,
    };

    SimpleLogger::init(log_level, Config::default()).unwrap();
}

fn spawn_emulator_thread(args: Args, tx: SyncSender<NesMessage>, rx: Receiver<NesMessage>) -> Result<JoinHandle<Result<(), NesConsoleError>>, NesConsoleError> {

    let handle = spawn(move || -> Result<(), NesConsoleError> {
        let mut front = NesFrontEnd::new(args, tx, rx).map_err(|e| {
            error!("fatal error while creating emulator: {}", e);
            e
        })?;

        front.run().map_err(|e| {
            error!("fatal error in emulator thread: {}", e);
            e
        })
    });

    Ok(handle)
}

fn main() -> Result<(), NesConsoleError> {
    let args: Args = Args::parse();

    logger_init(args.debug);

    let native_options = eframe::NativeOptions::default();
    let (tx0, rx0) = sync_channel::<NesMessage>(CHANNEL_BOUND_SIZE);
    let (tx1, rx1) = sync_channel::<NesMessage>(CHANNEL_BOUND_SIZE);

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
