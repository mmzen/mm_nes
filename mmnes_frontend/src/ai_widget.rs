use std::cell::RefCell;
use std::rc::Rc;
use eframe::egui;
use eframe::egui::{pos2, Align, Color32, Context, CornerRadius, Frame, Key, Label, Layout, Margin, RichText, Stroke, Ui};
use egui_extras::{Size, StripBuilder};
use mmnes_core::nes_console::NesConsoleError;
use crate::ai_worker::{AiWorkMessage, AiWorker};
use crate::nes_front_ui::{NesButton, NesButtonId};
use crate::nes_mediator::NesMediator;
use crate::nes_ui_widget::NesUiWidget;

const WINDOW_NAME: &str = "NES Coach";

pub struct AiWidget {
    visible: bool,
    nes_mediator: Rc<RefCell<NesMediator>>,
    error: Option<NesConsoleError>,
    buttons: Vec<NesButton>,
    messages: Vec<ChatMessage>,
    input: String,
    is_sending: bool,
    ai_worker: AiWorker,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ChatRole {
    Player,
    Assistant
}

#[derive(Clone, Debug)]
struct ChatMessage {
    role: ChatRole,
    text: String,
}

impl NesUiWidget for AiWidget {
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_error(&mut self, error: Option<NesConsoleError>) {
        self.error = error;
    }

    fn menu_buttons(&self) -> &[NesButton] {
        &self.buttons
    }

    fn on_button(&mut self, _: NesButtonId) -> Result<(), NesConsoleError> {
        self.switch_visible();
        Ok(())
    }

    fn footer(&self) -> Vec<String> {
        [].to_vec()
    }

    fn draw(&mut self, ctx: &Context) -> Result<(), NesConsoleError> {
        self.ai_window(ctx)
    }
}

impl AiWidget {
    pub fn new(cc: &eframe::CreationContext<'_>, nes_mediator: Rc<RefCell<NesMediator>>, ai_worker: AiWorker) -> Result<AiWidget, NesConsoleError> {
        let button = NesButton::new(cc, NesButtonId(0), "AI", "AI Coach", include_bytes!("assets/cupid.png"))?;
        let buttons = vec![button];

        let widget = AiWidget {
            visible: false,
            nes_mediator,
            error: None,
            buttons,
            messages: vec![
                ChatMessage { role: ChatRole::Assistant, text: "Hi! Click one of the action buttons to get some help.".to_string() },
            ],
            input: String::new(),
            is_sending: false,
            ai_worker,
        };

        Ok(widget)
    }

    fn switch_visible(&mut self) {
        self.visible = !self.visible;
    }

    fn status_dot(ui: &mut Ui, color: Color32) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);

        painter.circle_filled(rect.center(), 4.5, color);
    }

    fn draw_bubble(ui: &mut Ui, message: &ChatMessage) {
        let visuals = ui.visuals();
        let is_user = message.role == ChatRole::Player;

        let (fill, stroke) = if is_user {
            (visuals.widgets.inactive.bg_fill.gamma_multiply(0.9),
             Stroke::new(1.0, visuals.widgets.inactive.fg_stroke.color.gamma_multiply(0.3)))
        } else {
            (visuals.faint_bg_color,
             Stroke::new(1.0, visuals.widgets.noninteractive.fg_stroke.color.gamma_multiply(0.2)))
        };

        let rt = RichText::new(&message.text);

        Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.add(Label::new(rt).wrap());
            });
    }

    fn fetch_ai_response(&mut self) {
        while let Some(message) = self.ai_worker.try_recv() {
            match message {
                AiWorkMessage::Reply { text, .. } => self.messages.push(ChatMessage { role: ChatRole::Assistant, text }),
                AiWorkMessage::Error { text, .. } => self.messages.push(ChatMessage { role: ChatRole::Assistant, text: format!("error: {text}") })
            }

            self.is_sending = false;
        }
    }

    fn build_prompt_coach(&self) -> String {
        "Give one short, gentle hint for the current NES scene.".to_string()
    }

    fn build_prompt_cheat(&self) -> String {
        "Suggest one simple NES cheat for this scene.".to_string()
    }

    fn build_prompt_tell_me(&self) -> String {
        "Tell me something interesting about this game: super mario bros".to_string()
    }


    fn add_prompt(&mut self, user_label: &str, prompt: String) {
        if self.is_sending {
            return;
        }

        self.messages.push(ChatMessage { role: ChatRole::Player, text: user_label.to_string() });
        self.is_sending = true;

        if let Err(e) = self.ai_worker.request(prompt) {
            self.messages.push(ChatMessage { role: ChatRole::Assistant, text: format!("error: {e}") });
            self.is_sending = false;
        }
    }

    fn ai_window_inner(&mut self, ui: &mut Ui) -> Result<(), NesConsoleError> {
        self.fetch_ai_response();

        if ui.input(|i| i.key_pressed(Key::F1)) {
            self.add_prompt("Ask Coach", self.build_prompt_coach());
        }
        if ui.input(|i| i.key_pressed(Key::F2)) {
            self.add_prompt("Cheat!", self.build_prompt_cheat());
        }
        if ui.input(|i| i.key_pressed(Key::F3)) {
            self.add_prompt("Action 3", self.build_prompt_tell_me());
        }

        StripBuilder::new(ui)
            .size(Size::exact(28.0))   // header
            .size(Size::remainder())          // chat list
            .size(Size::exact(8.0))    // separator space
            .size(Size::exact(84.0))   // input bar
            .clip(true)
            .vertical(|mut strip| {
                // Header
                strip.cell(|ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        ui.label(RichText::new("NES Coach").strong());
                        ui.add_space(6.0);

                        AiWidget::status_dot(ui, if self.is_sending { Color32::ORANGE } else { Color32::GREEN });
                        ui.label(if self.is_sending { "thinking…" } else { "ready" });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Clear").on_hover_text("Clear chat").clicked() {
                                self.messages.clear();
                            }
                        });
                    });
                });

                // Chat area
                strip.cell(|ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("ai_chat_scroll")
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            let max_bubble_width = ui.available_width() * 0.82;

                            for message in &self.messages {
                                let is_user = message.role == ChatRole::Player;
                                let layout = if is_user {
                                    Layout::right_to_left(Align::Min)
                                } else {
                                    Layout::left_to_right(Align::Min)
                                };

                                ui.with_layout(layout, |ui| {
                                    ui.scope(|ui| {
                                        ui.set_max_width(max_bubble_width);
                                        AiWidget::draw_bubble(ui, message);
                                    });
                                });

                                ui.add_space(6.0);
                            }

                            if self.is_sending {
                                // inline "typing" bubble
                                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                                    Frame::new()
                                        .fill(ui.visuals().faint_bg_color)
                                        .corner_radius(CornerRadius::same(8))
                                        .inner_margin(Margin::symmetric(10, 8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.add(egui::Spinner::new().size(12.0));
                                                ui.label("Thinking…");
                                            });
                                        });
                                });
                            }
                        });
                });

                strip.cell(|ui| { ui.separator(); });

                // --- ACTION BUTTON BAR ---
                strip.cell(|ui| {
                    let width = 120.0;
                    let gap = 6.0;
                    let height = 72.0;

                    ui.horizontal(|ui| {
                        // Ask Coach
                        let ask_label = if self.is_sending { "Thinking…" } else { "Ask Coach" };
                        let ask = ui.add_sized([width, height], egui::Button::new(ask_label))
                            .on_hover_text("General guidance (F1)");
                        if !self.is_sending && ask.clicked() {
                            self.add_prompt("Ask Coach", self.build_prompt_coach());
                        }

                        ui.add_space(gap);

                        // Cheat!
                        let cheat_label = if self.is_sending { "Thinking…" } else { "Cheat!" };
                        let cheat = ui.add_sized([width, height], egui::Button::new(cheat_label))
                            .on_hover_text("Quick power-up or cheat (F2)");
                        if !self.is_sending && cheat.clicked() {
                            self.add_prompt("Cheat!", self.build_prompt_cheat());
                        }

                        ui.add_space(gap);

                        // Placeholder (Action 3)
                        let a3_label = if self.is_sending { "Thinking…" } else { "Tell me something" };
                        let a3 = ui.add_sized([width, height], egui::Button::new(a3_label))
                            .on_hover_text("Tell me something about this game (F3)");
                        if !self.is_sending && a3.clicked() {
                            self.add_prompt("Tell me something", self.build_prompt_tell_me());
                        }
                    });
                });
            });

        Ok(())
    }

    fn ai_window(&mut self, ctx: &Context) -> Result<(), NesConsoleError> {
        egui::Window::new(WINDOW_NAME)
            .title_bar(false)
            .default_pos(pos2(300.0, 22.0))
            .resizable(true)
            .open(&mut self.visible())
            .show(ctx, |ui| {
                let _ = self.ai_window_inner(ui);
            });

        Ok(())
    }
}