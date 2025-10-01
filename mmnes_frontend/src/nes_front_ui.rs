use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, SyncSender};
use eframe::{egui, App, Frame};
use eframe::egui::{vec2, Align, Align2, Button, CentralPanel, Color32, Context, Event, Grid, Image, ImageButton, Key, Layout, Margin, RawInput, Response, RichText, Stroke, TopBottomPanel, Ui, Vec2};
use egui_file_dialog::FileDialog;
use log::warn;
use mmnes_core::key_event::{KeyEvent, KeyEvents, NES_CONTROLLER_KEY_A, NES_CONTROLLER_KEY_B, NES_CONTROLLER_KEY_DOWN, NES_CONTROLLER_KEY_LEFT, NES_CONTROLLER_KEY_RIGHT, NES_CONTROLLER_KEY_SELECT, NES_CONTROLLER_KEY_START, NES_CONTROLLER_KEY_UP};
use mmnes_core::nes_console::NesConsoleError;
use crate::debugger_widget::DebuggerWidget;
use crate::nes_mediator::NesMediator;
use crate::nes_message::NesMessage;
use crate::nes_message::NesMessage::{Keys, LoadRom};
use crate::nes_ui_widget::NesUiWidget;
use crate::renderer_widget::RendererWidget;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct NesButtonId(pub u16);

#[derive(Copy, Clone)]
pub enum ButtonKind { Primary, Secondary, Danger }

#[derive(Debug, Copy, Clone)]
pub struct NesButton {
    pub id: NesButtonId,
    pub label: &'static str,
    pub tooltip: &'static str
}

pub struct NesFrontUI {
    emulator_viewport_frame: egui::containers::Frame,
    input: KeyEvents,
    rom_file_dialog: FileDialog,
    rom_file: Option<PathBuf>,
    error: Option<NesConsoleError>,
    widgets: Vec<Box<dyn NesUiWidget>>,
    nes_mediator: Rc<RefCell<NesMediator>>
}

impl NesFrontUI {

    pub fn new(cc: &eframe::CreationContext<'_>, command_tx: SyncSender<NesMessage>, frame_rx: Receiver<NesMessage>, debug_rx: Receiver<NesMessage>, error_rx: Receiver<NesMessage>, width: usize, height: usize) -> NesFrontUI {

        let frame = egui::containers::Frame {
            inner_margin: Default::default(),
            outer_margin: Default::default(),
            fill: Color32::from_rgb(65, 33, 150),
            stroke: Default::default(),
            corner_radius: Default::default(),
            shadow: Default::default(),
        };

        let nes_mediator = Rc::new(RefCell::new(NesMediator::new(frame_rx, command_tx, debug_rx, error_rx)));

        let mut widgets = Vec::<Box<dyn NesUiWidget>>::new();

        let renderer_ui =  RendererWidget::new(height, width, cc, nes_mediator.clone());
        let debugger_ui = DebuggerWidget::new(nes_mediator.clone());

        widgets.push(Box::new(renderer_ui));
        widgets.push(Box::new(debugger_ui));

        NesFrontUI {
            emulator_viewport_frame: frame,
            input: KeyEvents::new(),
            rom_file_dialog: FileDialog::new(),
            rom_file: None,
            error: None,
            nes_mediator,
            widgets
        }
    }

    fn send_input_to_emulator(&mut self) -> Result<(), NesConsoleError> {
        if self.input.is_empty() {
            return Ok(());
        }

        let inputs = std::mem::take(&mut self.input);
        self.nes_mediator.borrow_mut().send_message(Keys(inputs))
    }

    pub fn read_error_messages(&mut self) -> Result<(), NesConsoleError> {
        let messages = self.nes_mediator.borrow().read_error_messages()?;

        for message in messages {
            match message {
                NesMessage::Error(error) => self.error = Some(error),
                _ => warn!("unexpected message: {:?}", message),
            };
        }

        for widget in &mut self.widgets {
            widget.set_error(self.error.clone());
        }

        Ok(())
    }

    fn load_rom_file(&mut self) -> Result<(), NesConsoleError> {
        if let Some(path) = self.rom_file_dialog.take_picked() {
            self.rom_file = Some(path.clone());

            for widget in &mut self.widgets {
                widget.set_rom_file(self.rom_file.clone());
            }
            
            self.nes_mediator.borrow_mut().send_message(LoadRom(path))?;
        }

        Ok(())
    }

    fn get_window_title(&self) -> String {
        let mut title = "MMNES".to_string();

        let rom_name = if let Some(rom_file) = &self.rom_file {
            if let Some (rom_file_str) = rom_file.file_name() {
                " - ".to_string() + &*rom_file_str.to_string_lossy()
            } else {
                " - (invalid filename)".to_string()
            }
        } else {
            " - (idle)".to_string()
        };

        title += &rom_name;

        if let Some(_) = &self.error {
            title += " - (error)";
        }

        title
    }

    fn show_error_modal(&mut self, ctx: &Context, error: &NesConsoleError) {
        let mut close_requested = false;

        egui::Window::new("error_modal_window")
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);

                ui.horizontal(|ui| {
                    let title = RichText::new("Fatalistic Error ☝").heading().color(Color32::from_rgb(230, 75, 75));
                    ui.label(title);
                });

                ui.add_space(4.0);
                ui.label(RichText::new(error.to_string()).strong());

                ui.add_space(8.0);
                ui.separator();

                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(Button::new("OK").min_size(vec2(80.0, 0.0))).clicked() {
                            close_requested = true;
                        }
                    });
                });

                if ui.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter)) {
                    close_requested = true;
                }
            });

        if close_requested {
            self.error = None;
            for widget in &mut self.widgets {
                widget.set_error(None);
            }
        }
    }

    fn main_menu_text_button(ui: &mut Ui, label: &str, tooltip: &str, kind: ButtonKind, selected: bool) -> Response {
        let resp = ui.scope(|ui| {
            let style = ui.style_mut();
            style.spacing.button_padding = vec2(8.0, 4.0);

            let vis = ui.style().visuals.clone();
            //let accent = Color32::from_rgb(78, 201, 176);
            let red    = Color32::from_rgb(200, 80, 80);
            let base_bg = vis.widgets.inactive.bg_fill;
            let base_stroke = vis.widgets.inactive.bg_stroke;

            let (fill, stroke) = match kind {
                ButtonKind::Primary   |
                ButtonKind::Secondary => (base_bg, base_stroke),
                ButtonKind::Danger    => (red.linear_multiply(if selected { 0.90 } else { 0.75 }),
                                       Stroke::new(1.0, red.linear_multiply(0.90))),
            };

            let text = RichText::new(label).size(11.0).strong();

            let mut b = Button::new(text).min_size(vec2(80.0, 24.0)).stroke(stroke);

            if matches!(kind, ButtonKind::Primary | ButtonKind::Danger) {
                b = b.fill(fill);
            }

            let r = ui.add(b).on_hover_text(tooltip);

            if selected && r.hovered() == false {
                // simulate emphasis by re-adding a stroke via a small frame around it
                // (kept minimal—no custom vector icons/draw calls)
            }

            r
        }).inner;

        resp
    }

    fn install_theme(ctx: &Context) {
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = vec2(8.0, 8.0);
        style.spacing.window_margin = Margin::same(12);
        style.visuals = egui::Visuals::dark();

        let bg0 = Color32::from_rgb(20, 12, 43);
        let bg1 = Color32::from_rgb(252, 108, 42);
        let bg2 = Color32::from_rgb(40, 40, 40);
        let fg  = Color32::from_rgb(230, 234, 238);
        let acc = Color32::from_rgb(78, 201, 176);

        style.visuals.widgets.inactive.bg_fill = bg1;
        style.visuals.widgets.hovered.bg_fill  = bg2;
        style.visuals.widgets.active.bg_fill   = bg2;

        style.visuals.override_text_color = Some(fg);
        style.visuals.window_fill = bg0;
        style.visuals.panel_fill  = bg0;

        ctx.set_style(style);

        let mut visuals = ctx.style().visuals.clone();
        //visuals.selection.bg_fill = acc.linear_multiply(0.35);
        visuals.hyperlink_color = acc;
        ctx.set_visuals(visuals);
    }
}

impl App for NesFrontUI {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        let _ = self.read_error_messages();
        let _ = self.send_input_to_emulator();

        NesFrontUI::install_theme(ctx);
        ctx.request_repaint();

        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            Grid::new("edit_grid").num_columns(1).spacing([10.0, 4.0]).show(ui, |ui| {
                if NesFrontUI::main_menu_text_button(ui, "LOAD ROM", "Load ROM from file", ButtonKind::Primary, false).clicked() {
                    self.rom_file_dialog.pick_file();
                }

                let _ = self.load_rom_file();
                self.rom_file_dialog.update(ctx);

                for widget in &mut self.widgets {
                    let mut clicked: Option<NesButtonId> = None;
                    let buttons = widget.menu_buttons();

                    for button in buttons {
                        if NesFrontUI::main_menu_text_button(ui, button.label, button.tooltip, ButtonKind::Primary, false).clicked() {
                            clicked = Some(button.id);
                        };
                    }

                    if let Some(clicked_button_id) = clicked {
                        let _ = widget.on_button(clicked_button_id);
                    }
                }

                ui.end_row();
            });
        });

        CentralPanel::default().frame(self.emulator_viewport_frame).show(ctx, |ui| {
            Image::new(egui::include_image!("assets/bg.jpg")).paint_at(ui, ctx.screen_rect());

            for widget in &mut self.widgets {
                let _ = widget.draw(ctx);
            }

            let error = self.error.clone();

            if let Some(error) = error {
                self.show_error_modal(ctx, &error);
            }
        });
        
        TopBottomPanel::bottom("status").show(ctx, |ui| {
            let mut footer = String::new();

            for widget in &self.widgets {
                for field in widget.footer() {
                    footer.push_str(&format!("{} | ", field));
                }
            }
            
            ui.label(footer);
        });
        
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.get_window_title()));
    }

    fn raw_input_hook(&mut self, ctx: &Context, raw_input: &mut RawInput) {
        if ctx.wants_keyboard_input() { return; }

        raw_input.events.retain(|event| {
            if let Event::Key { key, pressed, .. } = event {
                let handled = match key {
                    Key::Z => { self.input.push_back( KeyEvent { key: NES_CONTROLLER_KEY_A, pressed: *pressed }); true }
                    Key::A => { self.input.push_back(KeyEvent { key: NES_CONTROLLER_KEY_B, pressed: *pressed }); true }
                    Key::Enter => { self.input.push_back(KeyEvent { key: NES_CONTROLLER_KEY_START, pressed: *pressed }); true }
                    Key::Escape => { self.input.push_back(KeyEvent { key: NES_CONTROLLER_KEY_SELECT, pressed: *pressed }); true }
                    Key::ArrowUp => { self.input.push_back(KeyEvent { key: NES_CONTROLLER_KEY_UP, pressed: *pressed }); true }
                    Key::ArrowDown => { self.input.push_back(KeyEvent { key: NES_CONTROLLER_KEY_DOWN, pressed: *pressed }); true }
                    Key::ArrowLeft => { self.input.push_back(KeyEvent { key: NES_CONTROLLER_KEY_LEFT, pressed: *pressed }); true }
                    Key::ArrowRight => { self.input.push_back(KeyEvent { key: NES_CONTROLLER_KEY_RIGHT, pressed: *pressed }); true }
                    _ => false,
                };
                return !handled;
            }
            true
        });
    }
}