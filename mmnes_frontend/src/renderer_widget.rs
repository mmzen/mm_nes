use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;
use eframe::egui;
use eframe::egui::{pos2, vec2, Color32, ColorImage, Context, Image, TextureHandle, TextureOptions, Ui};
use log::warn;
use mmnes_core::nes_console::NesConsoleError;
use mmnes_core::util::measure_exec_time;
use crate::nes_front_ui::{NesButton, NesButtonId};
use crate::nes_mediator::NesMediator;
use crate::nes_message::NesMessage;
use crate::nes_message::NesMessage::{Pause, Play, PowerOff, Reset};
use crate::nes_ui_widget::NesUiWidget;
use crate::text_8x8_generator::Test8x8Generator;

const WINDOW_NAME: &str = "NES Emulator";
const RENDERER_PLAY_BUTTON: NesButtonId = NesButtonId(0);
const RENDERER_PAUSE_BUTTON: NesButtonId = NesButtonId(1);
const RENDERER_RESET_BUTTON: NesButtonId = NesButtonId(2);
const RENDERER_POWER_OFF_BUTTON: NesButtonId = NesButtonId(3);
const RENDERER_BUTTONS: [NesButton; 4] = [
    NesButton { id: RENDERER_PLAY_BUTTON, label: "PLAY", tooltip: "Run emulator" },
    NesButton { id: RENDERER_PAUSE_BUTTON, label: "PAUSE", tooltip: "Pause/Run emulator" },
    NesButton { id: RENDERER_RESET_BUTTON, label: "RESET", tooltip: "Reset emulator" },
    NesButton { id: RENDERER_POWER_OFF_BUTTON, label: "POWER OFF", tooltip: "Power off emulator" },
];

pub struct RendererWidget {
    visible: bool,
    rom_file: Option<PathBuf>,
    error: Option<NesConsoleError>,
    height: usize,
    width: usize,
    texture: TextureHandle,
    texture_options: TextureOptions,
    last_tick: Instant,
    last_frame_counter: u32,
    frame_counter: u32,
    rendering_duration_ms: f64,
    ui_fps: f32,
    emulator_fps: f32,
    nes_frame: Option<ColorImage>,
    nes_mediator: Rc<RefCell<NesMediator>>,
}

impl NesUiWidget for RendererWidget {
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_rom_file(&mut self, rom_file: Option<PathBuf>) {
        self.rom_file = rom_file;
    }

    fn set_error(&mut self, error: Option<NesConsoleError>) {
        self.error = error;
    }

    fn menu_buttons(&self) -> &[NesButton] {
        &RENDERER_BUTTONS
    }

    fn on_button(&mut self, id: NesButtonId) -> Result<(), NesConsoleError> {
        let _ = match id {
            RENDERER_PLAY_BUTTON => self.nes_mediator.borrow_mut().send_message(Play),
            RENDERER_PAUSE_BUTTON => self.nes_mediator.borrow_mut().send_message(Pause),
            RENDERER_RESET_BUTTON => self.nes_mediator.borrow_mut().send_message(Reset),
            RENDERER_POWER_OFF_BUTTON => self.nes_mediator.borrow_mut().send_message(PowerOff),
            _ => return Err(NesConsoleError::InternalError("unknown button".to_string())),
        };

        Ok(())
    }

    fn footer(&self) -> Vec<String> {
        let mut fields = Vec::<String>::new();

        fields.push(format!("rendering: {:.3} ms", self.rendering_duration_ms));
        fields.push(format!("UI: {:>5.1} fps", self.ui_fps));
        fields.push(format!("emulator: {:>5.1} fps", self.emulator_fps));

        fields
    }

    fn draw(&mut self, ctx: &Context) -> Result<(), NesConsoleError> {
        self.renderer_window(ctx)
    }
}

impl RendererWidget {
    pub fn new(height: usize, width: usize, cc: &eframe::CreationContext<'_>, nes_mediator: Rc<RefCell<NesMediator>>) -> RendererWidget {
        let vec = RendererWidget::create_default_texture(width, height, Color32::DARK_GRAY);

        let texture_options = TextureOptions {
            minification: egui::TextureFilter::Nearest,
            wrap_mode: Default::default(),
            magnification: egui::TextureFilter::Nearest,
            mipmap_mode: None,
        };

        let texture = cc.egui_ctx.load_texture(
            "nes-emulator-viewport",
            ColorImage::new([width, height], vec),
            texture_options
        );

        RendererWidget {
            visible: false,
            rom_file: None,
            error: None,
            height,
            width,
            texture,
            texture_options,
            last_tick: Instant::now(),
            last_frame_counter: 0,
            frame_counter: 0,
            rendering_duration_ms: 0.0,
            ui_fps: 0.0,
            emulator_fps: 0.0,
            nes_frame: None,
            nes_mediator,
        }
    }

    fn prepare_nes_frame(&mut self) -> Result<(), NesConsoleError> {
        if let Some(error) = &self.error {
            let image = self.error_frame(error);
            self.nes_frame = Some(image);
        } else {
            let messages = self.nes_mediator.borrow().read_messages()?;

            for message in messages {
                match message {
                    NesMessage::Frame(nes_frame) => {
                        self.frame_counter = nes_frame.count();
                        self.nes_frame = Some(ColorImage::from_rgba_unmultiplied([nes_frame.width(), nes_frame.height()], nes_frame.pixels()))
                    },

                    _ => { warn!("unexpected message: {:?}", message); }
                }
            }
        }

        Ok(())
    }

    fn create_default_texture(width: usize, height: usize, color: Color32) -> Vec<Color32> {
        let mut vec = Vec::<Color32>::with_capacity(width * height);

        for _ in 0..width * height {
            vec.push(color);
        }

        vec
    }

    fn compute_fps(&mut self) {
        const ALPHA: f32 = 0.1;

        let now = Instant::now();
        let duration = (now - self.last_tick).as_secs_f32();

        if duration > 0.0 {
            // UI FPS: 1 / duration (EMA-smoothed)
            let ui_fps = 1.0 / duration;
            self.ui_fps = if self.ui_fps == 0.0 {
                ui_fps
            } else {
                self.ui_fps + ALPHA * (ui_fps - self.ui_fps)
            };

            // Emulator FPS: delta frames / duration
            let delta_frames = if self.frame_counter >= self.last_frame_counter {
                (self.frame_counter - self.last_frame_counter) as f32
            } else {
                0.0
            };

            let emulator_fps = delta_frames / duration;
            self.emulator_fps = if self.emulator_fps == 0.0 {
                emulator_fps
            } else {
                self.emulator_fps + ALPHA * (emulator_fps - self.emulator_fps)
            };

            self.last_frame_counter = self.frame_counter;
            self.last_tick = now;
        }
    }

    fn error_frame(&self, error: &NesConsoleError) -> ColorImage {
        let background = Color32::DARK_GRAY;
        let foreground = Color32::DARK_RED;

        let mut image = ColorImage::new([self.width, self.height], RendererWidget::create_default_texture(self.width, self.height, background));
        let _ = Test8x8Generator::draw_text_wrapped_centered(&mut image, &format!("{}", error), foreground);

        image
    }

    fn renderer_window_inner(&mut self, ui: &mut Ui) -> Result<(), NesConsoleError> {
        let img_px = vec2(self.width as f32, self.height as f32);
        let available_size = ui.available_size();
        let scale = (available_size.x / img_px.x).min(available_size.y / img_px.y);
        let size  = img_px * scale;
        ui.vertical_centered_justified(|ui| {
            let (_, duration) = measure_exec_time(|| {
                ui.add(Image::new((self.texture.id(), size)));
            });
            self.compute_fps();
            self.rendering_duration_ms = duration.as_secs_f64() * 1000.0;
        });

        Ok(())
    }

    fn renderer_window(&mut self, ctx: &Context) -> Result<(), NesConsoleError> {
        let  _ = self.prepare_nes_frame();

        if let Some(image) = self.nes_frame.take() {
            self.texture.set(image, self.texture_options);
        }

        egui::Window::new(WINDOW_NAME)
            .default_pos(pos2(0.0, 28.0))
            .title_bar(false)
            .default_size([880.0, 526.0])
            .show(ctx, |ui| {
                self.renderer_window_inner(ui)
            });

        Ok(())
    }
}