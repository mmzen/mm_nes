use eframe::egui::Context;
use mmnes_core::nes_console::NesConsoleError;
use crate::nes_front_ui::{NesButton, NesButtonId};
use crate::nes_ui_widget::NesUiWidget;

pub struct NesRomMetaDataWidget {

}

impl NesUiWidget for NesRomMetaDataWidget {
    fn set_visible(&mut self, _: bool) {
    }

    fn visible(&self) -> bool {
        false
    }

    fn set_error(&mut self, _: Option<NesConsoleError>) {
    }

    fn menu_buttons(&self) -> &[NesButton] {
        &[]
    }

    fn on_button(&mut self, _: NesButtonId) -> Result<(), NesConsoleError> {
        Ok(())
    }

    fn footer(&self) -> Vec<String> {
        [].to_vec()
    }

    fn draw(&mut self, ctx: &Context) -> Result<(), NesConsoleError> {
        Ok(())
    }
}