use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::Instant;
use eframe::{egui, App, Frame};
use eframe::egui::{vec2, CentralPanel, Color32, ColorImage, Context, Event, Grid, Image, Key, Margin, RawInput, TextureHandle, TextureOptions, TopBottomPanel};
use egui_file_dialog::FileDialog;
use log::{error, warn};
use mmnes_core::nes_frame::NesFrame;
use mmnes_core::util::measure_exec_time;
use mmnes_core::key_event::{KeyEvent, KeyEvents, NES_CONTROLLER_KEY_A, NES_CONTROLLER_KEY_B, NES_CONTROLLER_KEY_DOWN, NES_CONTROLLER_KEY_LEFT, NES_CONTROLLER_KEY_RIGHT, NES_CONTROLLER_KEY_SELECT, NES_CONTROLLER_KEY_START, NES_CONTROLLER_KEY_UP};
use mmnes_core::nes_console::NesConsoleError;
use crate::nes_front_end::NesFrontEnd;
use crate::nes_message::NesMessage;
use crate::nes_message::NesMessage::{Keys, LoadRom, Pause, Reset};
use crate::text_8x8_generator::Test8x8Generator;

pub struct NesFrontUI {
    rx: Receiver<NesMessage>,
    tx: SyncSender<NesMessage>,
    texture: TextureHandle,
    texture_options: TextureOptions,
    height: usize,
    width: usize,
    last_tick: Instant,
    last_frame_counter: u32,
    frame_counter: u32,
    rendering_duration_ms: f64,
    ui_fps: f32,
    emulator_fps: f32,
    emulator_viewport_frame: egui::containers::Frame,
    input: KeyEvents,
    rom_file_dialog: FileDialog,
    rom_file: Option<PathBuf>,
    error: Option<NesConsoleError>,
    nes_frame: Option<ColorImage>,
}

impl NesFrontUI {

    fn create_default_texture(width: usize, height: usize, color: Color32) -> Vec<Color32> {
        let mut vec = Vec::<Color32>::with_capacity(width * height);

        for _ in 0..width * height {
            vec.push(color);
        }

        vec
    }


    pub fn new(cc: &eframe::CreationContext<'_>, tx: SyncSender<NesMessage>, rx: Receiver<NesMessage>, width: usize, height: usize) -> NesFrontUI {
        let vec = NesFrontUI::create_default_texture(width, height, Color32::DARK_GRAY);

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

        let frame = egui::containers::Frame {
            inner_margin: Default::default(),
            outer_margin: Default::default(),
            fill: Color32::from_hex("#727370").unwrap(),
            stroke: Default::default(),
            corner_radius: Default::default(),
            shadow: Default::default(),
        };

        NesFrontUI {
            rx,
            tx,
            texture,
            texture_options,
            height,
            width,
            last_tick: Instant::now(),
            last_frame_counter: 0,
            frame_counter: 0,
            rendering_duration_ms: 0.0,
            ui_fps: 0.0,
            emulator_fps: 0.0,
            emulator_viewport_frame: frame,
            input: KeyEvents::new(),
            rom_file_dialog: FileDialog::new(),
            rom_file: None,
            error: None,
            nes_frame: None,
        }
    }

    fn compute_fps(&mut self) {
        const ALPHA: f32 = 0.1;

        let now = Instant::now();
        let duration = (now - self.last_tick).as_secs_f32();

        if duration > 0.0 {
            // UI FPS: 1/duration (EMA-smoothed)
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

    fn read_and_process_messages(&mut self) -> Result<(), NesConsoleError> {

        while let Ok(message) = self.rx.try_recv() {
            match message {
                NesMessage::Error(e) => {
                    error!("received error from NES backend: {}", e);
                    let background = Color32::DARK_GRAY;
                    let foreground = Color32::DARK_RED;

                    let mut image = ColorImage::new([self.width, self.height], NesFrontUI::create_default_texture(self.width, self.height, background));
                    let _ = Test8x8Generator::draw_text_wrapped_centered(&mut image, &format!("{}", e), foreground);
                    self.error = Some(e);
                    println!("{:?}", image);
                    self.nes_frame = Some(image);
                },
                NesMessage::Frame(nes_frame) => {
                    self.frame_counter = nes_frame.count();
                    self.nes_frame = Some(ColorImage::from_rgba_unmultiplied([nes_frame.width(), nes_frame.height()], nes_frame.pixels()))
                }
                _ => { warn!("unexpected message: {:?}", message);  }
            }
        }

        Ok(())
    }

    fn send_message(&mut self, message: NesMessage) -> Result<(), NesConsoleError> {
        match self.tx.try_send(message) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_frame)) => Ok(()), // drop input
            Err(TrySendError::Disconnected(message)) => {
                Err(NesConsoleError::ChannelCommunication(format!("NES backend is gone ... {:?}", message)))
            }
        }
    }

    fn send_input_to_emulator(&mut self) {
        if let Ok(()) = self.tx.try_send(Keys(self.input.clone())) {
            self.input.clear();
        }
    }

    fn load_rom_file(&mut self) {
        if let Some(path) = self.rom_file_dialog.take_picked() {
            self.rom_file = Some(path.clone());
            let _ = self.send_message(LoadRom(path));
        }
    }
}

impl App for NesFrontUI {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {

        self.send_input_to_emulator();
        let _ = self.read_and_process_messages();

        if let Some(image) = self.nes_frame.take() {
            self.texture.set(image, self.texture_options);
        }

        ctx.request_repaint();

        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            Grid::new("edit_grid").num_columns(1).spacing([10.0, 4.0]).show(ui, |ui| {
                if ui.button("Load ROM").clicked() {
                    self.rom_file_dialog.pick_file();
                }

                self.load_rom_file();
                self.rom_file_dialog.update(ctx);

                if ui.button("Reset").clicked() {
                    let _ = self.send_message(Reset);
                }
                let _ = ui.button("Power Off");
                
                if ui.button("Pause").clicked() {
                    let _ = self.send_message(Pause);
                }
                ui.end_row();
            });
        });

        TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(format!("rendering: {:.3} ms | UI: {:>5.1} fps | Emulator: {:>5.1} fps",
                self.rendering_duration_ms, self.ui_fps, self.emulator_fps
            ));
        });

        CentralPanel::default().frame(self.emulator_viewport_frame).show(ctx, |ui| {
            let img_px = vec2(self.width as f32, self.height as f32);
            let available_size = ui.available_size();
            let scale = (available_size.x / img_px.x).min(available_size.y / img_px.y);
            let size  = img_px * scale;

            egui::Frame::new()
                .inner_margin(Margin::same(16.0 as i8))
                .show(ui, |ui| {
                    ui.vertical_centered_justified(|ui| {
                        let (_, duration) = measure_exec_time(|| {
                            ui.add(Image::new((self.texture.id(), size)));
                        });
                        self.compute_fps();
                        self.rendering_duration_ms = duration.as_secs_f64() * 1000.0;
                    });
                });
        });
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
                return !handled; // drop handled events so egui won’t react
            }
            true
        });
    }
}