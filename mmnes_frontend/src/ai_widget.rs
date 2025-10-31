use std::cell::RefCell;
use std::rc::Rc;
use eframe::egui;
use eframe::egui::{pos2, Align, Color32, ColorImage, Context, CornerRadius, Frame, Image, Key, Label, Layout, Margin, RichText, Stroke, TextureHandle, Ui};
use egui_extras::{Size, StripBuilder};
use log::{error, warn};
use mmnes_core::nes_console::NesConsoleError;
use crate::ai_worker::{AiWorkMessage, AiWorker, AiWorkerError};
use crate::helpers_ui::HelpersUI;
use crate::llm_client::Prompt;
use crate::nes_front_ui::{NesButton, NesButtonId};
use crate::nes_mediator::NesMediator;
use crate::nes_ui_widget::NesUiWidget;

const WINDOW_NAME: &str = "NES Coach";
const ASSISTANT_ERROR_MESSAGE: &str = "Sorry, I couldn't get a response...";
const ASSISTANT_WELCOME_MESSAGE: &str = "Hi! Click one of the action buttons to get some help.";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PendingAction {
    None,
    AskCoach,
    TellMe
}

pub struct AiWidget {
    visible: bool,
    nes_mediator: Rc<RefCell<NesMediator>>,
    error: Option<NesConsoleError>,
    buttons: Vec<NesButton>,
    messages: Vec<ChatMessage>,
    is_sending: bool,
    ai_worker: AiWorker,
    is_waiting_frame: bool,
    pending_action: PendingAction,
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
    image: Option<ColorImage>,
    texture: Option<TextureHandle>,
}

impl ChatMessage {
    fn new(role: ChatRole, text: String) -> ChatMessage {
        ChatMessage {
            role,
            text,
            image: None,
            texture: None,
        }
    }

    fn new_with_image(role: ChatRole, text: String, image: ColorImage) -> ChatMessage {
        ChatMessage {
            role,
            text,
            image: Some(image),
            texture: None,
        }
    }
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
                ChatMessage::new(ChatRole::Assistant, ASSISTANT_WELCOME_MESSAGE.to_string())
            ],
            is_sending: false,
            ai_worker,
            is_waiting_frame: false,
            pending_action: PendingAction::None,
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

    fn fetch_ai_response(&mut self) -> Result<(), AiWorkerError> {
        while let Some(message) = self.ai_worker.try_recv() {
            match message {
                AiWorkMessage::Reply { text, .. } => {
                    self.messages.push(ChatMessage::new(ChatRole::Assistant, text))
                },

                AiWorkMessage::Error { text, .. } => {
                    self.messages.push(ChatMessage::new(ChatRole::Assistant, ASSISTANT_ERROR_MESSAGE.to_string()));
                    error!("error in fetching AI response: {}", text);
                },
            }

            self.is_sending = false;
        }

        Ok(())
    }

    fn build_prompt_with_image(text: &'static str, image: &ColorImage) -> Result<Prompt, AiWorkerError> {
        let jpg_bytes = HelpersUI::color_image_to_jpeg_bytes(&image, 90, [0, 0, 0]);

        let result = match jpg_bytes {
            Ok(bytes) => Ok(Prompt::new(text.to_string(), Some(bytes))),
            Err(err) => Err(AiWorkerError::InternalError(format!("Failed to convert color image to jpeg: {}", err)))
        };

        result
    }

    fn build_prompt_coach_text() -> &'static str {
        "Give one short, accurate and helpful hint for the current NES game scene."
    }

    fn build_prompt_cheat_text() -> &'static str {
        "Suggest one simple NES cheat for this scene."
    }

    fn build_prompt_tell_me_text() -> &'static str {
        "Tell me something interesting about this game"
    }

    fn build_prompt_coach(image: &ColorImage) -> Result<Prompt, AiWorkerError> {
        AiWidget::build_prompt_with_image(AiWidget::build_prompt_coach_text(), image)
    }

    fn build_prompt_cheat() -> Result<Prompt, AiWorkerError> {
        Ok(Prompt::new(AiWidget::build_prompt_cheat_text().to_string(), None))
    }

    fn build_prompt_tell_me(image: &ColorImage) -> Result<Prompt, AiWorkerError> {
        AiWidget::build_prompt_with_image(AiWidget::build_prompt_tell_me_text(), image)
    }

    fn try_finish_pending_capture(&mut self) -> Result<(), AiWorkerError> {
        if !self.is_waiting_frame {
            return Ok(());
        }

        if !self.nes_mediator.borrow().is_frame_available() {
            return Ok(());
        }

        let screenshot_opt = self.nes_mediator.borrow_mut().frame();

        if let Some(screenshot) = screenshot_opt {
            let result = match self.pending_action {
                PendingAction::AskCoach => {
                    match AiWidget::build_prompt_coach(&screenshot) {
                        Ok(prompt) => self.chat_verbose("Help me Coach!", prompt, Some(screenshot)),
                        Err(e) => Err(e),
                    }
                },

                PendingAction::TellMe => {
                    match AiWidget::build_prompt_tell_me(&screenshot) {
                        Ok(prompt) => self.chat_verbose("Tell me something!", prompt, Some(screenshot)),
                        Err(e) => Err(e),
                    }
                },

                _ => Err(AiWorkerError::InternalError("Unexpected state for screenshot capture".to_string())),
            };

            if let Err(e) = result {
                self.messages.push(ChatMessage::new(ChatRole::Assistant, ASSISTANT_ERROR_MESSAGE.to_string()));
                error!("failed to send prompt: {}", e);
                self.is_sending = false;
            }
        } else {
            warn!("frame was pending but could not capture it");
        }

        self.is_waiting_frame = false;
        self.pending_action = PendingAction::None;

        Ok(())
    }

    fn send_prompt(&mut self, prompt: Prompt) -> Result<(), AiWorkerError> {
        self.is_sending = true;

        if let Err(error) = self.ai_worker.request(prompt) {
            self.messages.push(ChatMessage::new(ChatRole::Assistant, ASSISTANT_ERROR_MESSAGE.to_string()));
            self.is_sending = false;
            Err(error)
        } else {
            Ok(())
        }
    }

    fn chat(&mut self, user_label: &str, prompt: Prompt) -> Result<(), AiWorkerError> {
        if self.is_sending {
            return Ok(());
        }

        self.messages.push(ChatMessage::new(ChatRole::Player, user_label.to_string()));
        self.send_prompt(prompt)
    }

    fn chat_verbose(&mut self, user_label: &str, prompt: Prompt, image: Option<ColorImage>) -> Result<(), AiWorkerError> {
        if self.is_sending {
            return Ok(());
        }

        let verbose_message = format!("{}:\nprompt:\n{}", user_label, prompt.text);

        if let Some(image) = image {
            self.messages.push(ChatMessage::new_with_image(ChatRole::Player, verbose_message, image));
        } else {
            self.messages.push(ChatMessage::new(ChatRole::Player, verbose_message));
        }

        self.send_prompt(prompt)
    }

    fn ask_coach(&mut self) -> Result<(), AiWorkerError> {
        if self.is_sending {
            return Ok(());
        }

        if self.is_waiting_frame == false {
            self.nes_mediator.borrow_mut().request_frame();
            self.is_waiting_frame = true;
            self.pending_action = PendingAction::AskCoach;
        }

        Ok(())
    }

    fn tell_me(&mut self) -> Result<(), AiWorkerError> {
        if self.is_sending {
            return Ok(());
        }

        if self.is_waiting_frame == false {
            self.nes_mediator.borrow_mut().request_frame();
            self.is_waiting_frame = true;
            self.pending_action = PendingAction::TellMe;
        }

        Ok(())
    }

    fn cheat(&mut self) -> Result<(), AiWorkerError> {
        self.chat("Cheat!", AiWidget::build_prompt_cheat()?)
    }


    fn ai_window_inner(&mut self, ui: &mut Ui) -> Result<(), AiWorkerError> {
        self.fetch_ai_response()?;
        self.try_finish_pending_capture()?;

        if ui.input(|i| i.key_pressed(Key::F1)) { self.ask_coach()?; }
        if ui.input(|i| i.key_pressed(Key::F2)) {  self.cheat()?;  }
        if ui.input(|i| i.key_pressed(Key::F3)) { self.tell_me()?; }

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
                            if self.is_sending { "Thinking…" } else { "Ready" }
                        );

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

                                        // Upload texture once if we have a screenshot
                                        if message.texture.is_none() {
                                            if let Some(image) = message.image.take() {
                                                let tex = ui.ctx().load_texture(
                                                    format!("ai_prompt_preview_{idx}"),
                                                    image,
                                                    Default::default(),
                                                );

                                                message.texture = Some(tex);
                                            }
                                        }

                                        Self::draw_bubble(ui, message);
                                    });
                                });

                                ui.add_space(6.0);
                            }

                            if self.is_sending {
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
                        let ask_label = if self.is_sending { "Thinking…" } else { "Ask Coach\n(F1)" };
                        let ask = ui.add_sized([width, height], egui::Button::new(ask_label))
                            .on_hover_text("General guidance (F1)");
                        if !self.is_sending && ask.clicked() {
                            self.ask_coach();
                        }

                        ui.add_space(gap);

                        // Cheat!
                        let cheat_label = if self.is_sending { "Thinking…" } else { "Cheat!\n(F2)" };
                        let cheat = ui.add_sized([width, height], egui::Button::new(cheat_label))
                            .on_hover_text("Quick power-up or cheat (F2)");
                        if !self.is_sending && cheat.clicked() {
                           self.cheat();
                        }

                        ui.add_space(gap);

                        // Placeholder (Action 3)
                        let a3_label = if self.is_sending { "Thinking…" } else { "Tell me something\n(F3)" };
                        let a3 = ui.add_sized([width, height], egui::Button::new(a3_label))
                            .on_hover_text("Tell me something about this game (F3)");
                        if !self.is_sending && a3.clicked() {
                            self.tell_me();
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