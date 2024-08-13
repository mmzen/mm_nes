use std::fs::File;
use log::{debug, info, LevelFilter};
use simplelog::{Config, SimpleLogger};
use clap::Parser;
use clap_num::maybe_hex;

use crate::cpu::{CPU, CpuError};
use crate::cpu_6502::Cpu6502;
use crate::ines_loader::INesLoader;
use crate::loader::Loader;
use crate::memory::{Memory, MemoryError};
use crate::memory_64k::Memory64k;

mod cpu;
mod cpu_6502;
mod memory;
mod memory_64k;
mod loader;
mod ines_loader;

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

fn populate_memory(memory: &mut Box<dyn Memory>) -> Result<(), MemoryError> {
    debug!("populating memory with ROM data...");

    memory.initialize()?;
    Ok(())
}

fn main() -> Result<(), CpuError> {
    let args: Args = Args::parse();

    logger_init(args.debug);
    info!("emulator bootstrapping...");

    let mut cpu;
    let mut memory : Box<dyn Memory> = Box::new(Memory64k::default());
    let status;
    let mut loader;

    populate_memory(&mut memory)?;

    loader = Box::new(INesLoader::new_with_memory(&mut memory));
    loader.load_rom(&args.rom_file).expect("fuck you");

    let file = if let Some(trace_file) = args.trace_file {
        debug!("output for traces: {}", trace_file);
        Some(File::create(trace_file)?)
    } else {
        debug!("output for traces: stdout");
        None
    };

    cpu = Cpu6502::new(memory, file);
    cpu.initialize().expect("cpu initialization failed");
    status = cpu.run_start_at(args.pc);

    if let Err(error) = status {
        cpu.panic(&error);
        Err(error)
    } else {
        Ok(())
    }
}
