use std::fs::File;
use std::hint::spin_loop;
use std::ops::ControlFlow;
use std::ops::ControlFlow::{Break, Continue};
use std::sync::mpsc::TrySendError;
use std::sync::mpsc::{Receiver, SyncSender};
use std::thread::sleep;
use std::time::{Duration, Instant};
use log::{debug, info, warn};
use mmnes_core::apu::ApuType::RP2A03;
use mmnes_core::bus::BusType;
use mmnes_core::bus_device::BusDeviceType::{APU, CARTRIDGE, CONTROLLER, PPU, WRAM};
use mmnes_core::cartridge::CartridgeType::NROM;
use mmnes_core::controller::ControllerType::StandardController;
use mmnes_core::cpu::CpuType::NES6502;
use mmnes_core::cpu_debugger::DebugCommand;
use mmnes_core::loader::LoaderType::INESV2;
use mmnes_core::memory::MemoryType::StandardMemory;
use mmnes_core::nes_console::{NesConsole, NesConsoleBuilder, NesConsoleError};
use mmnes_core::nes_frame::NesFrame;
use mmnes_core::nes_samples::NesSamples;
use mmnes_core::ppu::PpuType::NES2C02;
use crate::{Args, FRAMES_PER_SECOND, SPIN_BEFORE};
use crate::nes_message::NesMessage;
use crate::sound_player::SoundPlayer;

#[derive(Debug, Clone, PartialEq)]
enum NesFrontEndState {
    Running,
    Debug(DebugCommand),
    Paused,
    Idle,
}

impl NesFrontEndState {
    fn toggle_pause(&self) -> Option<Self> {
        match self {
            NesFrontEndState::Running => Some(NesFrontEndState::Paused),
            NesFrontEndState::Paused  => Some(NesFrontEndState::Running),
            _ => None,
        }
    }
}

pub struct NesFrontEnd {
    command_rx: Receiver<NesMessage>,
    frame_tx: SyncSender<NesMessage>,
    debug_tx: SyncSender<NesMessage>,
    nes: NesConsole,
    args: Args,
    state: NesFrontEndState
}

impl NesFrontEnd {

    fn create_emulator(args: &Args) -> Result<NesConsole, NesConsoleError> {
        let builder = NesConsoleBuilder::new();

        let trace_file = if let Some(trace_file) = args.trace_file.clone() {
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
            .with_bus_device_type(WRAM(StandardMemory))
            .with_bus_device_type(CARTRIDGE(NROM))
            .with_bus_device_type(APU(RP2A03))
            .with_bus_device_type(PPU(NES2C02))
            .with_bus_device_type(CONTROLLER(StandardController))
            .with_loader_type(INESV2)
            .with_rom_file(args.rom_file.clone())
            .with_entry_point(args.pc)
            .build()?;

        console.power_on()?;
        info!("emulator ready");

        Ok(console)
    }

    pub fn new(args: Args, frame_tx: SyncSender<NesMessage>, command_rx: Receiver<NesMessage>, debug_tx: SyncSender<NesMessage>) -> Result<NesFrontEnd, NesConsoleError> {
        let nes = NesFrontEnd::create_emulator(&args)?;

        let front = NesFrontEnd {
            nes,
            frame_tx,
            command_rx,
            debug_tx,
            args,
            state: NesFrontEndState::Running
        };

        Ok(front)
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
    fn try_send_common(tx: &SyncSender<NesMessage>, label: &str, message: NesMessage) -> Result<(), NesConsoleError> {
        match tx.try_send(message) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                warn!("NES frontend {} channel is full, dropping message ...", label);
                Ok(())
            }
            Err(TrySendError::Disconnected(msg)) => Err(
                NesConsoleError::ChannelCommunication(format!("UI is gone ... {:?}", msg))
            ),
        }
    }

    fn send_message(&self, message: NesMessage) -> Result<(), NesConsoleError> {
        NesFrontEnd::try_send_common(&self.frame_tx, "frame", message)
    }

    fn send_debug_message(&self, message: NesMessage) -> Result<(), NesConsoleError> {
        NesFrontEnd::try_send_common(&self.debug_tx, "debug", message)
    }

    fn send_error(&self, error: NesConsoleError) {
        let _ = self.send_message(NesMessage::Error(error));
    }

    fn process_frame(&self, frame: NesFrame) -> Result<(), NesConsoleError> {
        self.send_message(NesMessage::Frame(frame))
    }

    fn process_samples(&self, samples: NesSamples, sound_player: &mut SoundPlayer) -> Result<(), NesConsoleError> {
        for sample in samples.samples() {
            sound_player.push_sample(*sample)
        }

        Ok(())
    }

    fn process_message(&mut self, message: NesMessage) -> Result<ControlFlow<NesFrontEndState, ()>, NesConsoleError> {
        match message {
            NesMessage::Keys(key_events) => {
                self.nes.set_input(key_events)?;
                Ok(Continue(()))
            },

            NesMessage::Reset => {
                self.nes.reset()?;
                Ok(Break(self.state.clone()))
            },

            NesMessage::Pause => {
                let state = self.state.toggle_pause();

                if let Some(s) = state {
                    Ok(Break(s))
                } else {
                    Ok(Continue(()))
                }
            },

            NesMessage::LoadRom(rom_file) => {
                self.args.rom_file = rom_file;

                match NesFrontEnd::create_emulator(&self.args) {
                    Ok(nes) => {
                        self.nes = nes;
                        Ok(Break(NesFrontEndState::Running))
                    }
                    Err(e) => {
                        self.send_error(e);
                        Ok(Break(NesFrontEndState::Idle))
                    }
                }
            },

            NesMessage::Debug(command) => {
                let state = NesFrontEndState::Debug(command);
                Ok(Break(state))
            },

            other => {
                warn!("unexpected message: {:?}", other);
                Ok(Continue(()))
            }
        }
    }

    fn read_and_process_messages(&mut self) -> Result<NesFrontEndState, NesConsoleError> {
        while let Ok(message) = self.command_rx.try_recv() {
            match self.process_message(message)? {
                Continue(()) => {}
                Break(next_state) => {
                    self.state = next_state.clone();
                    return Ok(next_state);
                }
            }
        }

        Ok(self.state.clone())
    }

    pub fn run(&mut self) -> Result<(), NesConsoleError> {
        let frame_duration = Duration::from_secs_f64(1.0 / FRAMES_PER_SECOND);
        let mut next_frame = Instant::now() + frame_duration;
        let mut sound_player = SoundPlayer::new().map_err(|e| NesConsoleError::ControllerError(e.to_string()))?;

        loop {
            self.state = self.read_and_process_messages()?;

            match self.state {
                NesFrontEndState::Running => {
                    let (frame, samples) = self.nes.step_frame()?;
                    self.process_frame(frame)?;
                    self.process_samples(samples, &mut sound_player)?;

                    next_frame = NesFrontEnd::sleep_until_next_frame(next_frame, frame_duration);
                },

                NesFrontEndState::Debug(DebugCommand::StepInstruction) => {
                    let (frame, samples, snapshot) = self.nes.step_instruction()?;

                    if let Some(frame) = frame {
                        self.process_frame(frame)?;
                        next_frame = NesFrontEnd::sleep_until_next_frame(next_frame, frame_duration);
                    }

                    if let Some(samples) = samples {
                        self.process_samples(samples, &mut sound_player)?;
                    }

                    self.send_debug_message(NesMessage::CpuSnapshot(snapshot))?;
                    self.state = NesFrontEndState::Debug(DebugCommand::Stop);
                },

                NesFrontEndState::Debug(DebugCommand::Stop) => {},

                NesFrontEndState::Debug(DebugCommand::Run) => {
                    self.state = NesFrontEndState::Running;
                },

                NesFrontEndState::Debug(command) => {
                    warn!("unsupported debug command: {:?}", command);
                    self.state = NesFrontEndState::Debug(DebugCommand::Stop);
                },

                NesFrontEndState::Paused => {},
                NesFrontEndState::Idle => {}
            }
        }
    }
}