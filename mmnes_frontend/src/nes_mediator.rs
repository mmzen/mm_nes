use std::path::PathBuf;
use log::warn;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use eframe::egui::ColorImage;
use mmnes_core::nes_console::NesConsoleError;
use crate::nes_message::NesMessage;

#[derive(Debug, Clone)]
pub enum NesMediatorRequest {
    FrameRequest,
    FrameResponse(ColorImage),
}

pub struct NesMediator {
    frame_rx: Receiver<NesMessage>,
    command_tx: SyncSender<NesMessage>,
    debug_rx: Receiver<NesMessage>,
    error_rx: Receiver<NesMessage>,
    rom_file: Option<PathBuf>,
    request: Option<NesMediatorRequest>,
}

impl NesMediator {

    pub fn new(frame_rx: Receiver<NesMessage>, command_tx: SyncSender<NesMessage>, debug_rx: Receiver<NesMessage>, error_rx: Receiver<NesMessage>) -> NesMediator {
        NesMediator {
            frame_rx,
            command_tx,
            debug_rx,
            error_rx,
            rom_file: None,
            request: None,
        }
    }

    pub fn rom_file(&self) -> Option<&PathBuf> {
        self.rom_file.as_ref()
    }

    pub fn set_rom_file(&mut self, rom_file: Option<PathBuf>) {
        self.rom_file = rom_file;
    }

    pub fn request_frame(&mut self) {
        self.request = Some(NesMediatorRequest::FrameRequest);
    }

    pub fn is_frame_requested(&self) -> bool {
        matches!(self.request, Some(NesMediatorRequest::FrameRequest))
    }

    pub fn is_frame_available(&self) -> bool {
        matches!(self.request, Some(NesMediatorRequest::FrameResponse(_)))
    }

    pub fn frame(&mut self) -> Option<ColorImage> {
        if let Some(NesMediatorRequest::FrameResponse(image)) = self.request.take() {
            Some(image)
        } else {
            None
        }
    }

    pub fn set_frame(&mut self, frame: ColorImage) {
        self.request = Some(NesMediatorRequest::FrameResponse(frame));
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