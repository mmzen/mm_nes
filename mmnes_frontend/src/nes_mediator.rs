// Authorship: Human 100% | Claude 0%
use std::path::PathBuf;
use log::warn;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use eframe::egui::ColorImage;
use mmnes_core::nes_console::NesConsoleError;
use mmretrodb::nes_rom_metadata::NesRomMetadata;
use crate::nes_message::NesMessage;

#[derive(Debug)]
enum Event<A, R> {
    Idle,
    Requested(R),
    Answered(A)
}

impl<A, R> Default for Event<A, R> {
    fn default() -> Event<A, R> {
        Event::Idle
    }
}

impl<A, R> Event<A, R> {
    fn request(&mut self, request: R) {
        if !matches!(self, Event::Answered(_)) {
            *self = Event::Requested(request);
        }
    }

    fn set(&mut self, payload: A) {
        *self = Event::Answered(payload)
    }

    fn is_requested(&self) -> bool {
        matches!(self, Event::Requested(_))
    }

    fn is_answered(&self) -> bool {
        matches!(self, Event::Answered(_))
    }

    fn take_answer(&mut self) -> Option<A> {
        match std::mem::take(self) {
            Event::Answered(payload) => Some(payload),
            _ => None,
        }
    }

    pub fn take_request(&mut self) -> Option<R> {
        match std::mem::take(self) {
            Event::Requested(payload) => Some(payload),
            _ => None,
        }
    }
}

#[derive(Default, Debug)]
struct NesMediatorEvents {
    frame: Event<ColorImage, ()>,
    rom_metadata: Event<NesRomMetadata, u32>,
}

pub struct Common {
    rom_file: Option<PathBuf>,
    rom_metadata: Option<NesRomMetadata>,
}

impl Default for Common {
    fn default() -> Common {
        Common {
            rom_file: None,
            rom_metadata: None,
        }
    }
}

impl Common {
    pub fn rom_file(&self) -> Option<&PathBuf> {
        self.rom_file.as_ref()
    }

    pub fn set_rom_file(&mut self, rom_file: Option<PathBuf>) {
        self.rom_file = rom_file;
    }

    pub fn rom_metadata(&self) -> Option<&NesRomMetadata> {
        self.rom_metadata.as_ref()
    }

    pub fn set_rom_metadata(&mut self, rom_file: Option<NesRomMetadata>) {
        self.rom_metadata = rom_file;
    }
}

pub struct NesMediator {
    frame_rx: Receiver<NesMessage>,
    command_tx: SyncSender<NesMessage>,
    debug_rx: Receiver<NesMessage>,
    error_rx: Receiver<NesMessage>,
    events: NesMediatorEvents,
    common: Common,
}

impl NesMediator {

    pub fn new(frame_rx: Receiver<NesMessage>, command_tx: SyncSender<NesMessage>, debug_rx: Receiver<NesMessage>, error_rx: Receiver<NesMessage>) -> NesMediator {
        NesMediator {
            frame_rx,
            command_tx,
            debug_rx,
            error_rx,
            common: Common::default(),
            events: NesMediatorEvents::default(),
        }
    }
    
    pub fn common_mut(&mut self) -> &mut Common { 
        &mut self.common
    }

    pub fn common(&self) -> &Common {
        &self.common
    }
    
    pub fn request_frame(&mut self) { self.events.frame.request(()); }
    pub fn is_frame_requested(&self) -> bool { self.events.frame.is_requested() }
    pub fn set_frame(&mut self, image: ColorImage) { self.events.frame.set(image); }
    pub fn is_frame_available(&self) -> bool { self.events.frame.is_answered() }
    pub fn frame(&mut self) -> Option<ColorImage> { self.events.frame.take_answer() }

    pub fn request_rom_metadata(&mut self, crc: u32) { self.events.rom_metadata.request(crc); }
    pub fn is_rom_metadata_requested(&self) -> bool { self.events.rom_metadata.is_requested() }
    pub fn set_rom_metadata(&mut self, metadata: NesRomMetadata) { self.events.rom_metadata.set(metadata); }
    pub fn is_rom_metadata_available(&self) -> bool { self.events.rom_metadata.is_answered() }
    pub fn rom_metadata(&mut self) -> Option<NesRomMetadata> { self.events.rom_metadata.take_answer() }
    pub fn rom_metadata_request(&mut self) -> Option<u32> { self.events.rom_metadata.take_request() }


    pub fn read_messages(&self) -> Result<Vec<NesMessage>, NesConsoleError> {
        let mut messages = Vec::new();
        
        loop {
            match self.frame_rx.try_recv() {
                Ok(message) => match message {
                    NesMessage::Error(_) |
                    NesMessage::Frame(_) => {
                        messages.push(message);
                    },

                    other => warn!("unexpected frame message: {:?}", other),
                },
                
                Err(TryRecvError::Empty) => break,
                
                Err(TryRecvError::Disconnected) => {
                    return Err(NesConsoleError::ChannelCommunication("NES backend is gone ...".to_string()));
                }
            }
        }

        Ok(messages)
    }

    pub fn read_debug_messages(&self) -> Result<Vec<NesMessage>, NesConsoleError> {
        let mut messages = Vec::new();

        loop {
            match self.debug_rx.try_recv() {
                Ok(message) => match message {
                    NesMessage::CpuSnapshot(_) |
                    NesMessage::CpuSnapshotSet(_) => {
                        messages.push(message);
                    },

                    other => warn!("unexpected debug message: {:?}", other),
                },

                Err(TryRecvError::Empty) => break,

                Err(TryRecvError::Disconnected) => {
                    return Err(NesConsoleError::ChannelCommunication("NES backend is gone ...".to_string()));
                }
            }
        }

        Ok(messages)
    }

    pub fn read_error_messages(&self) -> Result<Vec<NesMessage>, NesConsoleError> {
        let mut messages = Vec::new();

        loop {
            match self.error_rx.try_recv() {
                Ok(message) => match message {
                    NesMessage::Error(_) => messages.push(message),
                    other => warn!("unexpected debug message: {:?}", other),
                },

                Err(TryRecvError::Empty) => break,

                Err(TryRecvError::Disconnected) => {
                    return Err(NesConsoleError::ChannelCommunication("NES backend is gone ...".to_string()));
                }
            }
        }

        Ok(messages)
    }

    pub fn send_message(&mut self, message: NesMessage) -> Result<(), NesConsoleError> {
        match self.command_tx.try_send(message) {
            Ok(()) => Ok(()),

            Err(TrySendError::Full(_frame)) => {
                warn!("NES UI channel is full, dropping message ...");
                Ok(())
            },

            Err(TrySendError::Disconnected(_)) => {
                Err(NesConsoleError::ChannelCommunication("NES backend is gone ...".to_string()))
            }
        }
    }
}