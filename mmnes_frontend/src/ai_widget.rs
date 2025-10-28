use std::cell::RefCell;
use std::rc::Rc;
use eframe::egui;
use eframe::egui::{pos2, Align, Color32, ColorImage, Context, CornerRadius, Frame, Image, Key, Label, Layout, Margin, RichText, Stroke, TextureHandle, Ui};
use egui_extras::{Size, StripBuilder};
use mmnes_core::nes_console::NesConsoleError;
use crate::ai_worker::{AiWorkMessage, AiWorker, AiWorkerError};
use crate::helpers_ui::HelpersUI;
use crate::llm_client::Prompt;
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
    is_sending: bool,
    ai_worker: AiWorker,
    is_waiting_frame: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ChatRole {
    Player,
    Assistant
}

#[derive(Clone)]
struct ChatMessage {
    role: ChatRole,
    text: String,
    preview: Option<ColorImage>,
    texture: Option<TextureHandle>,
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
                ChatMessage {
                    role: ChatRole::Assistant,
                    text: "Hi! Click one of the action buttons to get some help.".to_string(),
                    preview: None,
                    texture: None,
                },
            ],
            is_sending: false,
            ai_worker,
            is_waiting_frame: false,
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

    fn draw_bubble(ui: &mut Ui, message: &mut ChatMessage) {
        let visuals = ui.visuals();
        let is_user = message.role == ChatRole::Player;

        let (fill, stroke) = if is_user {
            (visuals.widgets.inactive.bg_fill.gamma_multiply(0.9),
             Stroke::new(1.0, visuals.widgets.inactive.fg_stroke.color.gamma_multiply(0.3)))
        } else {
            (visuals.faint_bg_color,
             Stroke::new(1.0, visuals.widgets.noninteractive.fg_stroke.color.gamma_multiply(0.2)))
        };

        Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.add(Label::new(RichText::new(&message.text)).wrap());

                if let Some(texture) = &message.texture {
                    let mut size = texture.size_vec2();
                    let max_w = 256.0_f32.min(ui.available_width());
                    if size.x > max_w {
                        let scale = max_w / size.x;
                        size.x *= scale;
                        size.y *= scale;
                    }
                    ui.add(Image::new((texture.id(), size)));
                }
            });
    }

    fn fetch_ai_response(&mut self) {
        while let Some(message) = self.ai_worker.try_recv() {
            match message {
                AiWorkMessage::Reply { text, .. } => self.messages.push(ChatMessage {
                    role: ChatRole::Assistant, text, preview: None, texture: None
                }),

                AiWorkMessage::Error { text, .. } => self.messages.push(ChatMessage {
                    role: ChatRole::Assistant, text: format!("error: {text}"), preview: None, texture: None
                }),
            }

            self.is_sending = false;
            self.is_waiting_frame = false;
        }
    }

    fn try_finish_pending_capture_and_send(&mut self) {
        if !self.is_waiting_frame {
            return;
        }

        if !self.nes_mediator.borrow().is_frame_available() {
            return;
        }

        let frame = self.nes_mediator.borrow_mut().frame();

        if let Some(image) = frame {
            match self.build_prompt_coach(&image) {
                Ok(prompt) => {
                    self.add_prompt_with_preview("Ask Coach", Self::build_prompt_coach_text(), prompt, Some(image));
                    self.is_waiting_frame = false;
                },

                Err(e) => {
                    self.messages.push(ChatMessage {
                        role: ChatRole::Assistant,
                        text: format!("error: {e}"),
                        preview: None,
                        texture: None,
                    });
                    self.is_waiting_frame = false;
                    self.is_sending = false;
                }
            }
        }
    }

    fn build_prompt_coach(&self, image: &ColorImage) -> Result<Prompt, AiWorkerError> {
        let jpg_bytes = HelpersUI::color_image_to_jpeg_bytes(&image, 100, [0, 0, 0]);

        let result = match jpg_bytes {
            Ok(bytes) => {
                let prompt = Prompt::new("Give one short, gentle hint for the current NES scene.".to_string(), Some(bytes));
                Ok(prompt)
            },

            Err(err) => Err(AiWorkerError::InternalError(format!("Failed to convert color image to JPEG bytes: {}", err)))
        };

        result
    }

    fn build_prompt_cheat(&self) -> Prompt {
        Prompt::new(Self::build_prompt_cheat_text().to_string(), None)
    }

    fn build_prompt_tell_me(&self) -> Prompt {
        Prompt::new(Self::build_prompt_tell_me_text().to_string(), None)
    }

    fn on_click_ask_coach(&mut self) {
        if self.is_sending {
            return;
        }

        if self.nes_mediator.borrow().is_frame_available() {
            let frame = self.nes_mediator.borrow_mut().frame();

            if let Some(image) = frame {
                match self.build_prompt_coach(&image) {
                    Ok(prompt) => {
                        self.add_prompt_with_preview("Ask Coach", AiWidget::build_prompt_coach_text(), prompt, Some(image));
                    },

                    Err(e) => {
                        self.messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            text: format!("error: {}", e),
                            preview: None,
                            texture: None,
                        });
                    }
                }
            }
        } else {
            self.nes_mediator.borrow_mut().request_frame();
            self.is_waiting_frame = true;
        }
    }

    fn build_prompt_coach_text() -> &'static str {
        "Give one short, gentle hint for the current NES scene."
    }

    fn build_prompt_cheat_text() -> &'static str {
        "Suggest one simple NES cheat for this scene."
    }

    fn build_prompt_tell_me_text() -> &'static str {
        "Tell me something interesting about this game: super mario bros"
    }


    fn add_prompt(&mut self, user_label: &str, prompt: Prompt) {
        if self.is_sending {
            return;
        }

        self.messages.push(ChatMessage {
            role: ChatRole::Player,
            text: user_label.to_string(),
            preview: None,
            texture: None,
        });

        self.is_sending = true;
        self.is_waiting_frame = false;

        if let Err(e) = self.ai_worker.request(prompt) {
            self.messages.push(ChatMessage {
                role: ChatRole::Assistant,
                text: format!("error: {e}"),
                preview: None,
                texture: None,
            });
            self.is_sending = false;
        }
    }

    fn add_prompt_with_preview(&mut self, user_label: &str, debug_text: &str, prompt: Prompt, preview: Option<ColorImage>) {
        if self.is_sending {
            return;
        }

        self.messages.push(ChatMessage {
            role: ChatRole::Player,
            text: format!("{}\n\nPrompt:\n{}", user_label, debug_text),
            preview,
            texture: None,
        });

        self.is_sending = true;

        if let Err(error) = self.ai_worker.request(prompt) {
            self.messages.push(ChatMessage {
                role: ChatRole::Assistant,
                text: format!("error: {}", error),
                preview: None,
                texture: None,
            });
            self.is_sending = false;
        }
    }

    fn ai_window_inner(&mut self, ui: &mut Ui) -> Result<(), NesConsoleError> {
        self.fetch_ai_response();

        self.try_finish_pending_capture_and_send();

        if ui.input(|i| i.key_pressed(Key::F1)) { self.on_click_ask_coach(); }
        if ui.input(|i| i.key_pressed(Key::F2)) { self.add_prompt("Cheat!", self.build_prompt_cheat()); }
        if ui.input(|i| i.key_pressed(Key::F3)) { self.add_prompt("Tell me something", self.build_prompt_tell_me()); }

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
                        ui.label(
                            if self.is_waiting_frame { "capturing…" }
                            else if self.is_sending { "thinking…" }
                            else { "ready" }
                        );

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Clear").on_hover_text("Clear chat").clicked() {
                                self.messages.clear();
                                self.is_sending = false;
                                self.is_waiting_frame = false;
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

                            for (idx, message) in self.messages.iter_mut().enumerate() {
                                let is_user = message.role == ChatRole::Player;
                                let layout = if is_user {
                                    Layout::right_to_left(Align::Min)
                                } else {
                                    Layout::left_to_right(Align::Min)
                                };

                                ui.with_layout(layout, |ui| {
                                    ui.scope(|ui| {
                                        ui.set_max_width(max_bubble_width);

                                        // Upload texture once if we have a preview but no handle yet
                                        if message.texture.is_none() {
                                            if let Some(image) = message.preview.take() {
                                                let tex = ui.ctx().load_texture(
                                                    format!("ai_prompt_preview_{idx}"),
                                                    image,
                                                    Default::default(),
                                                );

                                                message.texture = Some(tex);
                                            }
                                        }

                                        // Draw bubble with text + optional image
                                        Self::draw_bubble(ui, message);
                                    });
                                });

                                ui.add_space(6.0);
                            }

                            if self.is_waiting_frame || self.is_sending {
                                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                                    Frame::new()
                                        .fill(ui.visuals().faint_bg_color)
                                        .corner_radius(CornerRadius::same(8))
                                        .inner_margin(Margin::symmetric(10, 8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.add(egui::Spinner::new().size(12.0));
                                                ui.label(if self.is_waiting_frame { "Capturing…" } else { "Thinking…" });
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
                            self.on_click_ask_coach();
                        }

                        ui.add_space(gap);

                        // Cheat!
                        let cheat_label = if self.is_sending { "Thinking…" } else { "Cheat!" };
                        let cheat = ui.add_sized([width, height], egui::Button::new(cheat_label))
                            .on_hover_text("Quick power-up or cheat (F2)");
                        if !self.is_sending && cheat.clicked() {
                            let prompt = self.build_prompt_cheat();
                            self.add_prompt_with_preview("Cheat!", Self::build_prompt_cheat_text(), prompt, None);
                        }

                        ui.add_space(gap);

                        // Placeholder (Action 3)
                        let a3_label = if self.is_sending { "Thinking…" } else { "Tell me something" };
                        let a3 = ui.add_sized([width, height], egui::Button::new(a3_label))
                            .on_hover_text("Tell me something about this game (F3)");
                        if !self.is_sending && a3.clicked() {
                            let prompt = self.build_prompt_tell_me();
                            self.add_prompt_with_preview("Tell me something", Self::build_prompt_tell_me_text(), prompt, None);
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