use std::path::PathBuf;
use log::warn;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use mmnes_core::nes_console::NesConsoleError;
use crate::nes_message::NesMessage;

pub struct NesMediator {
    frame_rx: Receiver<NesMessage>,
    command_tx: SyncSender<NesMessage>,
    debug_rx: Receiver<NesMessage>,
    error_rx: Receiver<NesMessage>,
    rom_file: Option<PathBuf>,
}

impl NesMediator {

    pub fn new(frame_rx: Receiver<NesMessage>, command_tx: SyncSender<NesMessage>, debug_rx: Receiver<NesMessage>, error_rx: Receiver<NesMessage>) -> NesMediator {
        NesMediator {
            frame_rx,
            command_tx,
            debug_rx,
            error_rx,
            rom_file: None,
        }
    }

    pub fn rom_file(&self) -> Option<&PathBuf> {
        self.rom_file.as_ref()
    }

    pub fn set_rom_file(&mut self, rom_file: Option<PathBuf>) {
        self.rom_file = rom_file;
    }

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