use eframe::egui::RichText;

pub struct HelpersUI;

impl HelpersUI {
    pub fn monospace(s: &str) -> RichText {
        RichText::new(s).monospace()
    }

    pub fn header(s: &str) -> RichText {
        RichText::new(s).monospace().strong()
    }

}