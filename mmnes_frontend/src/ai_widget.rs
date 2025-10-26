use std::cell::RefCell;
use std::rc::Rc;
use eframe::egui;
use eframe::egui::{pos2, Align, Color32, Context, CornerRadius, Frame, Key, Label, Layout, Margin, RichText, Stroke, Ui};
use egui_extras::{Size, StripBuilder};
use mmnes_core::nes_console::NesConsoleError;
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
    pub fn new(cc: &eframe::CreationContext<'_>, nes_mediator: Rc<RefCell<NesMediator>>) -> Result<AiWidget, NesConsoleError> {
        let button = NesButton::new(cc, NesButtonId(0), "AI", "AI Coach", include_bytes!("assets/cupid.png"))?;
        let buttons = vec![button];

        let widget = AiWidget {
            visible: false,
            nes_mediator,
            error: None,
            buttons,
            messages: vec![
                ChatMessage { role: ChatRole::Assistant, text: "Hi! Ask me for a hint anytime. (Enter to send, Shift+Enter for newline)".to_string() },
            ],
            input: String::new(),
            is_sending: false,
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

    fn ai_window_inner(&mut self, ui: &mut Ui) -> Result<(), NesConsoleError> {
        StripBuilder::new(ui)
            .size(Size::exact(28.0))     // header
            .size(Size::remainder())            // chat list (fills)
            .size(Size::exact(8.0))      // separator space
            .size(Size::exact(84.0))     // input bar
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
                            if ui.button("Clear").clicked() {
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
                                // Align left for assistant, right for user:
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
                                // Tiny inline "typing" bubble:
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

                strip.cell(|ui| {
                    ui.separator();
                });

                // Input bar
                strip.cell(|ui| {
                    let send_enabled = !self.is_sending && !self.input.trim().is_empty();
                    let mut send_now = false;

                    ui.horizontal(|ui| {
                        // Growing text box
                        let te = egui::TextEdit::multiline(&mut self.input)
                            .hint_text("Ask anything… (enter to send, shift + enter for newline)")
                            .desired_rows(3)
                            .lock_focus(true)
                            .desired_width(f32::INFINITY);
                        let response = ui.add_sized([ui.available_width() - 92.0, 64.0], te);

                        // Enter to send (unless Shift is held)
                        let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift);
                        if enter_pressed && send_enabled && response.has_focus() {
                            send_now = true;
                        }

                        // Send button
                        let label = if self.is_sending { "Sending…" } else { "Send" };
                        if ui.add_enabled(send_enabled, egui::Button::new(label)).clicked() {
                            send_now = true;
                        }
                    });

                    // Actually send after the UI frame is built
                    if send_now {
                        let text = std::mem::take(&mut self.input);

                        // Push user message to the UI immediately:
                        self.messages.push(ChatMessage { role: ChatRole::Player, text: text.clone() });
                        self.is_sending = true;

                        // TODO: start async LLM call here.
                        // When the response arrives, push:
                        // self.messages.push(ChatMsg { role: ChatRole::Assistant, text: reply });
                        // self.is_sending = false;
                        //
                        // For now fake a reply:
                        #[allow(unused_variables)]
                        let _ = {
                            let reply = "Try waiting for the fireball, then jump twice while holding run.";
                            self.messages.push(ChatMessage { role: ChatRole::Assistant, text: reply.into() });
                            self.is_sending = false;
                        };
                    }
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