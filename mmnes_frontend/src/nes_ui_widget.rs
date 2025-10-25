use eframe::egui::Context;
use mmnes_core::nes_console::NesConsoleError;
use crate::nes_front_ui::{NesButton, NesButtonId};

pub trait NesUiWidget {
    #[allow(dead_code)]
    fn set_visible(&mut self, visible: bool);
    fn visible(&self) -> bool;
    fn set_error(&mut self, error: Option<NesConsoleError>);
    fn menu_buttons(&self) -> &[NesButton];
    fn on_button(&mut self, id: NesButtonId) -> Result<(), NesConsoleError>;
    fn footer(&self) -> Vec<String>;
    fn draw(&mut self, ctx: &Context) -> Result<(), NesConsoleError>;
}