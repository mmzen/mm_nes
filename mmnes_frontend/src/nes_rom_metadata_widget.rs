use std::cell::RefCell;
use std::rc::Rc;
use eframe::egui::Context;
use mmnes_core::nes_console::NesConsoleError;
use crate::nes_front_ui::{NesButton, NesButtonId};
use crate::nes_mediator::NesMediator;
use crate::nes_rom_metadata_worker::{NesRomMetadataWorker, NesRomMetadataWorkerError};
use crate::nes_rom_metadata_worker::NesRomMetadataMessage::ResponseMetadata;
use crate::nes_ui_widget::NesUiWidget;

pub struct NesRomMetaDataWidget {
    nes_mediator: Rc<RefCell<NesMediator>>,
    nes_rom_metadata_worker: NesRomMetadataWorker
}

impl NesRomMetaDataWidget {
    pub fn new(nes_mediator: Rc<RefCell<NesMediator>>, nes_rom_metadata_worker: NesRomMetadataWorker) -> Result<NesRomMetaDataWidget, NesConsoleError> {
        Ok(NesRomMetaDataWidget {
            nes_mediator,
            nes_rom_metadata_worker
        })
    }
}

impl NesUiWidget for NesRomMetaDataWidget {
    fn set_visible(&mut self, _: bool) {}

    fn visible(&self) -> bool {
        false
    }

    fn set_error(&mut self, _: Option<NesConsoleError>) {}

    fn menu_buttons(&self) -> &[NesButton] {
        &[]
    }

    fn on_button(&mut self, _: NesButtonId) -> Result<(), NesConsoleError> {
        Ok(())
    }

    fn footer(&self) -> Vec<String> {
        [].to_vec()
    }

    fn draw(&mut self, _: &Context) -> Result<(), NesConsoleError> {
        self.process_metadata_request().map_err(|_| NesConsoleError::InternalError("could not process metadata request".to_string()))?;
        self.process_worker_answer().map_err(|_| NesConsoleError::InternalError("could not process metadata request".to_string()))?;

        Ok(())
    }
}

impl NesRomMetaDataWidget {

    fn process_metadata_request(&mut self) -> Result<(), NesRomMetadataWorkerError> {
        let request_pending = self.nes_mediator.borrow().is_rom_metadata_requested();

        if request_pending == true {
            let crc = self.nes_mediator.borrow_mut()
                .rom_metadata_request()
                .ok_or_else(|| NesRomMetadataWorkerError::InternalError("could not get request payload".to_string()))?;

            self.nes_rom_metadata_worker.request(crc)?;
        }

        Ok(())
    }

    fn process_worker_answer(&mut self) -> Result<(), NesRomMetadataWorkerError> {

        match self.nes_rom_metadata_worker.try_recv() {
            Some(ResponseMetadata(Some(metadata))) => {
                self.nes_mediator.borrow_mut().set_rom_metadata(metadata);
            },

            _ => {}
        };

        Ok(())
    }
}