use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::{spawn, JoinHandle};
use std::time::{Duration};
use log::{error, LevelFilter};
use simplelog::{Config, SimpleLogger};
use clap::{Parser};
use clap_num::maybe_hex;
use eframe::egui::{vec2, ViewportBuilder};
use eframe::NativeOptions;
use mmnes_core::nes_console::NesConsoleError;
use crate::nes_front_end::NesFrontEnd;
use crate::nes_front_ui::NesFrontUI;
use crate::nes_message::NesMessage;

mod nes_front_ui;
mod sound_player;
mod nes_message;
mod nes_front_end;
mod text_8x8_generator;
mod tooltip_6502;
mod debugger_widget;
mod helpers_ui;
mod nes_ui_widget;
mod nes_mediator;
mod renderer_widget;
mod image_text_button;
mod llm_orchestrator;
mod llm_client;
mod openai_llm;
mod ai_widget;
#[cfg(test)]
pub mod tests;
mod ai_worker;

const APP_NAME: &str = "MMNES";

const FRAME_BUFFER_WIDTH: usize = 256;
const FRAME_BUFFER_HEIGHT: usize = 240;
const CHANNEL_BOUND_SIZE: usize = 10;
const DEBUG_CHANNEL_BOUND_SIZE: usize = 100;
const ERROR_BOUND_SIZE: usize = 10;
const FRAMES_PER_SECOND: f64 = 60.098_8;
const SPIN_BEFORE: Duration = Duration::from_micros(500);

const VIEWPORT_HEIGHT: f32 = 600.0;
const VIEWPORT_WIDTH: f32 = 900.0;

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
        short = 'f',
        long = "rom-file",
        help = "rom file to immediately load",
    )]
    rom_file: Option<PathBuf>
}


fn logger_init(debug: u8) {

    let log_level = match debug {
        1 => LevelFilter::Debug,
        2 => LevelFilter::Trace,
        _ => LevelFilter::Info,
    };

    SimpleLogger::init(log_level, Config::default()).unwrap();
}

fn spawn_emulator_thread(_args: &Args, frame_tx: SyncSender<NesMessage>, command_rx: Receiver<NesMessage>, debug_tx: SyncSender<NesMessage>, error_tx: SyncSender<NesMessage>) -> Result<JoinHandle<Result<(), NesConsoleError>>, NesConsoleError> {

    let handle = spawn(move || -> Result<(), NesConsoleError> {
        let mut front = NesFrontEnd::new(frame_tx, command_rx, debug_tx, error_tx).map_err(|e| {
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

    let native_options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size(vec2(VIEWPORT_WIDTH, VIEWPORT_HEIGHT))
            .with_resizable(true),
        ..Default::default()
    };

    let (frame_tx, frame_rx) = sync_channel::<NesMessage>(CHANNEL_BOUND_SIZE);
    let (command_tx, command_rx) = sync_channel::<NesMessage>(CHANNEL_BOUND_SIZE);
    let (debug_tx, debug_rx) = sync_channel::<NesMessage>(DEBUG_CHANNEL_BOUND_SIZE);
    let (error_tx, error_rx) = sync_channel::<NesMessage>(ERROR_BOUND_SIZE);

    let _ = spawn_emulator_thread(&args, frame_tx, command_rx, debug_tx, error_tx)?;

    let _ = eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            let nes_front_ui = NesFrontUI::new(args, cc, command_tx, frame_rx, debug_rx, error_rx, FRAME_BUFFER_WIDTH, FRAME_BUFFER_HEIGHT);
            if let Err(error) = nes_front_ui {
                panic!("failed to initialize NES front UI: {}", error);
            }

            Ok(Box::new(nes_front_ui.unwrap()))
        }),
    );

    Ok(())
}
