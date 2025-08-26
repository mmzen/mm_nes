use std::fs::File;
use std::hint::spin_loop;
use std::sync::mpsc::TrySendError;
use std::sync::mpsc::{Receiver, SyncSender};
use std::thread::sleep;
use std::time::{Duration, Instant};
use log::{debug, error, info, warn};
use mmnes_core::apu::ApuType::RP2A03;
use mmnes_core::bus::BusType;
use mmnes_core::bus_device::BusDeviceType::{APU, CARTRIDGE, CONTROLLER, PPU, WRAM};
use mmnes_core::cartridge::CartridgeType::NROM;
use mmnes_core::controller::ControllerType::StandardController;
use mmnes_core::cpu::CpuType::NES6502;
use mmnes_core::loader::LoaderType::INESV2;
use mmnes_core::memory::MemoryType::NESMemory;
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
    Paused,
    Idle,
}

pub struct NesFrontEnd {
    rx: Receiver<NesMessage>,
    tx: SyncSender<NesMessage>,
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
            .with_bus_device_type(WRAM(NESMemory))
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

    pub fn new(args: Args, tx: SyncSender<NesMessage>, rx: Receiver<NesMessage>) -> Result<NesFrontEnd, NesConsoleError> {
        let nes = NesFrontEnd::create_emulator(&args)?;

        let front = NesFrontEnd {
            nes,
            tx,
            rx,
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
    fn send_message(&self, message: NesMessage) -> Result<(), NesConsoleError> {
        match self.tx.try_send(message) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_message)) => {
                warn!("NES frontend channel is full, dropping message ...");
                Ok(())
            },
            Err(TrySendError::Disconnected(message)) => {
                Err(NesConsoleError::ChannelCommunication(format!("UI is gone ... {:?}", message)))
            }
        }
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

    fn read_and_process_messages(&mut self) -> Result<NesFrontEndState, NesConsoleError> {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                NesMessage::Keys(key_events) => {
                    self.nes.set_input(key_events)?;
                },
                NesMessage::Reset => {
                    self.nes.reset()?;
                    return Ok(self.state.clone());
                },
                NesMessage::Pause => {
                    match self.state {
                        NesFrontEndState::Running => {
                            return Ok(NesFrontEndState::Paused);
                        },
                        NesFrontEndState::Paused => {
                            return Ok(NesFrontEndState::Running);
                        },
                        _ => {
                            continue;
                        }
                    }
                },
                NesMessage::LoadRom(rom_file) => {
                    self.args.rom_file = rom_file;
                    let result = NesFrontEnd::create_emulator(&self.args);

                    return match result {
                        Ok(nes) => {
                            self.nes = nes;
                            Ok(NesFrontEndState::Running)
                        },
                        Err(e) => {
                            self.send_error(e);
                            Ok(NesFrontEndState::Idle)
                        },
                    }
                },
                _ => {
                    warn!("unexpected message: {:?}", message);
                }
            };
        }

        Ok(self.state.clone())
    }

    pub fn run(&mut self) -> Result<(), NesConsoleError> {
        let frame_duration = Duration::from_secs_f64(1.0 / FRAMES_PER_SECOND);
        let mut next_frame = Instant::now() + frame_duration;
        let mut sound_player = SoundPlayer::new().map_err(|e| NesConsoleError::ControllerError(e.to_string()))?;

        loop {
            self.state = self.read_and_process_messages()?;

            if self.state == NesFrontEndState::Running {
                let (frame, samples) = self.nes.step_frame()?;
                self.process_frame(frame)?;
                self.process_samples(samples, &mut sound_player)?;

                next_frame = NesFrontEnd::sleep_until_next_frame(next_frame, frame_duration);
            }
        }
    }
}