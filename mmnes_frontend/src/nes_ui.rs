use eframe::egui::Ui;
use mmnes_core::nes_console::NesConsoleError;

pub trait NesUI {
    fn set_visible(&mut self, visible: bool);
    fn visible(&self) -> bool;
    fn footer(&self) -> String;
    fn draw(&mut self, ui: &mut Ui) -> Result<(), NesConsoleError>;
}