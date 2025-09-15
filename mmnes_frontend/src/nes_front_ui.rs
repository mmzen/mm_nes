use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::time::Instant;
use eframe::{egui, App, Frame};
use eframe::egui::{pos2, vec2, Align, Align2, Button, CentralPanel, Color32, ColorImage, Context, Event, Grid, Image, Key, RawInput, Response, RichText, TextureHandle, TextureOptions, TopBottomPanel, Ui, UiKind};
use egui_file_dialog::FileDialog;
use egui_extras::{Column, TableBody, TableBuilder, TableRow};
use log::{error, warn};
use mmnes_core::cpu_debugger::{CpuSnapshot, DebugCommand};
use mmnes_core::util::measure_exec_time;
use mmnes_core::key_event::{KeyEvent, KeyEvents, NES_CONTROLLER_KEY_A, NES_CONTROLLER_KEY_B, NES_CONTROLLER_KEY_DOWN, NES_CONTROLLER_KEY_LEFT, NES_CONTROLLER_KEY_RIGHT, NES_CONTROLLER_KEY_SELECT, NES_CONTROLLER_KEY_START, NES_CONTROLLER_KEY_UP};
use mmnes_core::nes_console::NesConsoleError;
use crate::nes_message::NesMessage;
use crate::nes_message::NesMessage::{Debug, Keys, LoadRom, Pause, Reset};
use crate::text_8x8_generator::Test8x8Generator;

const MAX_CPU_SNAPSHOTS: usize = 256;

pub struct NesFrontUI {
    frame_rx: Receiver<NesMessage>,
    command_tx: SyncSender<NesMessage>,
    debug_rx: Receiver<NesMessage>,
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
    debug_window: bool,
    is_debug_running: bool,
    is_debugger_attached: bool,
    cpu_snapshots: Vec<Box<dyn CpuSnapshot>>
}

impl NesFrontUI {

    fn create_default_texture(width: usize, height: usize, color: Color32) -> Vec<Color32> {
        let mut vec = Vec::<Color32>::with_capacity(width * height);

        for _ in 0..width * height {
            vec.push(color);
        }

        vec
    }


    pub fn new(cc: &eframe::CreationContext<'_>, command_tx: SyncSender<NesMessage>, frame_rx: Receiver<NesMessage>, debug_rx: Receiver<NesMessage>, width: usize, height: usize) -> NesFrontUI {
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
            frame_rx,
            command_tx,
            debug_rx,
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
            debug_window: false,
            is_debug_running: false,
            is_debugger_attached: false,
            cpu_snapshots: Vec::new()
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

    fn clear_error(&mut self) {
        self.error = None;
    }

    fn read_and_process_messages(&mut self) -> Result<(), NesConsoleError> {

        while let Ok(message) = self.frame_rx.try_recv() {
            match message {
                NesMessage::Error(e) => {
                    error!("received error from NES backend: {}", e);
                    let background = Color32::DARK_GRAY;
                    let foreground = Color32::DARK_RED;

                    let mut image = ColorImage::new([self.width, self.height], NesFrontUI::create_default_texture(self.width, self.height, background));
                    let _ = Test8x8Generator::draw_text_wrapped_centered(&mut image, &format!("{}", e), foreground);
                    self.error = Some(e);
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

    fn read_debug_messages(&mut self) -> Result<(), NesConsoleError> {
        while let Ok(message) = self.debug_rx.try_recv() {
            match message {
                NesMessage::CpuSnapshot(snap) => self.cpu_snapshots.push(snap),
                NesMessage::CpuSnapshotSet(snaps) => self.cpu_snapshots.extend(snaps),
                _ => panic!("unexpected message: {:?}", message),
            };
        }

        let hard_cap = MAX_CPU_SNAPSHOTS;
        let len = self.cpu_snapshots.len();
        if len > hard_cap {
            let keep_from = len - hard_cap;
            self.cpu_snapshots.drain(0..keep_from);
        }

        Ok(())
    }

    fn send_message(&mut self, message: NesMessage) -> Result<(), NesConsoleError> {
        match self.command_tx.try_send(message) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_frame)) => {
                warn!("NES UI channel is full, dropping message ...");
                Ok(())
            },
            Err(TrySendError::Disconnected(message)) => {
                Err(NesConsoleError::ChannelCommunication(format!("NES backend is gone ... {:?}", message)))
            }
        }
    }

    fn send_input_to_emulator(&mut self) {
        if let Ok(()) = self.command_tx.try_send(Keys(self.input.clone())) {
            self.input.clear();
        }
    }

    fn load_rom_file(&mut self) {
        if let Some(path) = self.rom_file_dialog.take_picked() {
            self.clear_error();

            self.rom_file = Some(path.clone());
            let _ = self.send_message(LoadRom(path));
        }
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

    fn monospace(s: &str) -> RichText {
        RichText::new(s).monospace()
    }

    fn header(s: &str) -> RichText {
        RichText::new(s).monospace().strong()
    }

    fn disasm_line(field: &str, is_current: bool) -> RichText {
        let mut rt = NesFrontUI::monospace(field);
        if is_current {
            rt = rt.color(Color32::from_rgb(255, 128, 0));
        };

        rt
    }

    fn debugger_header_bar(&self, ui: &mut Ui) {
        let rom = self.rom_file
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("(no ROM)");

        let pc  = self.cpu_snapshots.last()
            .map(|s| format!("0x{:04X}", s.pc()))
            .unwrap_or_else(|| "------".to_string());

        ui.horizontal(|ui| {
             ui.label(RichText::new("  NES Debugger").strong());
            ui.separator();

            ui.label(RichText::new(format!("ROM: {}", rom)).monospace());

            ui.separator();
            ui.label(RichText::new(format!("PC: {}", pc)).monospace());

            ui.separator();
            ui.label(RichText::new(format!("ATTACHED: {}", self.is_debugger_attached.to_string().to_uppercase())).monospace());
        });
    }

    fn debugger_icon_button(&self, ui: &mut Ui, glyph: &str, tooltip: &str, fill: Color32) -> Response {
        let rt  = RichText::new(glyph).monospace().size(16.0);
        let button = Button::new(rt).fill(fill).min_size(vec2(28.0, 24.0));
        let response = ui.add(button);
        response.on_hover_text(tooltip)
    }

    fn debugger_toolbar(&mut self, ui: &mut Ui) {
        ui.input(|i| {
            if i.modifiers.shift && i.key_pressed(Key::F5) {
                self.is_debug_running = false;
                let _ = self.send_message(Debug(DebugCommand::Paused));
            } else if i.key_pressed(Key::F5) {
                self.is_debug_running = true;
                let _ = self.send_message(Debug(DebugCommand::Run));
            }
            if i.modifiers.shift && i.key_pressed(Key::F11) {
                let _ = self.send_message(Debug(DebugCommand::StepOut));
            } else if i.key_pressed(Key::F11) {
                let _ = self.send_message(Debug(DebugCommand::StepInto));
            }
            if i.key_pressed(Key::F10) {
                let _ = self.send_message(Debug(DebugCommand::StepOver));
            }
            if i.key_pressed(Key::F7) {
                let _ = self.send_message(Debug(DebugCommand::StepInstruction));
            }
        });

        let run_fill  = Color32::from_rgb( 34, 132,  76);
        let pause_fill = Color32::from_rgb(178, 54, 54);
        let default_fill = Color32::from_rgb(66, 66, 72);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);

            // Group 1: Run / Stop
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        let run_color = if self.is_debug_running { run_fill } else { Color32::from_rgb(42, 96, 66) };

                        if self.debugger_icon_button(ui, "▶", "Run / Continue (F5)", run_color).clicked() {
                            self.is_debug_running = true;
                            self.is_debugger_attached = true;
                            let _ = self.send_message(Debug(DebugCommand::Run));
                        }

                        if self.debugger_icon_button(ui, "⏸", "Pause (Shift+F5)", pause_fill).clicked() {
                            self.is_debug_running = false;
                            self.is_debugger_attached = true;
                            let _ = self.send_message(Debug(DebugCommand::Paused));
                        }
                    });
                });

            // Group 2: Stepping
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        if self.debugger_icon_button(ui, "↔", "Step Over (F10)", default_fill).clicked() {
                            let _ = self.send_message(Debug(DebugCommand::StepOver));
                        }
                        if self.debugger_icon_button(ui, "↘", "Step Into (F11)", default_fill).clicked() {
                            let _ = self.send_message(Debug(DebugCommand::StepInto));
                        }
                        if self.debugger_icon_button(ui, "↗", "Step Out (Shift+F11)", default_fill).clicked() {
                            let _ = self.send_message(Debug(DebugCommand::StepOut));
                        }
                        if self.debugger_icon_button(ui, "⏭", "Step Instruction (F7)", default_fill).clicked() {
                            self.is_debug_running = false;
                            self.is_debugger_attached = true;
                            let _ = self.send_message(Debug(DebugCommand::StepInstruction));
                        }
                    });
                });

            // Group 3: Others
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        if self.debugger_icon_button(ui, "🚫", "Clear Screen", default_fill).clicked() {
                            self.cpu_snapshots.clear();
                        }

                        if self.debugger_icon_button(ui, "🔌", "Attach / Detach", default_fill).clicked() {
                            self.is_debug_running = !self.is_debug_running;
                            self.is_debugger_attached = !self.is_debugger_attached;

                            let _ = if self.is_debugger_attached {
                                self.send_message(Debug(DebugCommand::Paused))
                            } else {
                                self.send_message(Debug(DebugCommand::Detach))
                            };
                        }
                    });
                });

            // Group 4: Quit
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        if self.debugger_icon_button(ui, "❌", "Quit", default_fill).clicked() {
                            self.is_debug_running = false;
                            self.is_debugger_attached = false;
                            self.debug_window = false;
                            let _ = self.send_message(Debug(DebugCommand::Detach));
                        }
                    });
                });
        });
    }

    fn debugger_table_header(&self, mut header: TableRow) {
        for item in ["", "PC", "BYTES", "OP", "OPERAND", "A", "X", "Y", "P", "SP", "CYCLES"] {
            header.col(|ui| {ui.label(NesFrontUI::header(item)); });
        }
    }

    fn debugger_table_body(&mut self, body: TableBody) {
        let total = self.cpu_snapshots.len();
        if total == 0 {
            return;
        }

        body.rows(20.0, total, |mut row| {
            let idx = row.index();
            let snapshot = &self.cpu_snapshots[row.index()];
            let is_current = (idx + 1) == total;

            let pc = format!("{:04X}", snapshot.pc());
            let mut bytes = String::new();

            for i in 0..snapshot.instruction().len() {
                let o = format!(" {:02X}", i);
                bytes.push_str(&o);
            }

            let mut op = snapshot.mnemonic();
            if snapshot.is_illegal() {
                op.push('*');
            }

            let operand = snapshot.operand();
            let a = format!("{:02X}", snapshot.a());
            let x = format!("{:02X}", snapshot.x());
            let y = format!("{:02X}", snapshot.y());
            let p = format!("{:02X}", snapshot.p());
            let sp = format!("{:02X}", snapshot.sp());
            let cycles = format!("{}", snapshot.cycles());

            row.col(|ui| {
                ui.label(Self::monospace(if is_current { "▶" } else { " " }));
            });

            for item in [pc, bytes, op, operand, a, x, y, p, sp, cycles].iter() {
                row.col(|ui| {
                    let rt = NesFrontUI::disasm_line(&item, is_current);
                    ui.label(rt);
                });
            }
        });
    }

    fn debugger_window(&mut self, ui: &mut Ui) {
        let _ = self.read_debug_messages();

        self.debugger_header_bar(ui);
        ui.separator();
        self.debugger_toolbar(ui);

        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("instructions_scroll")
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.scope(|ui| {
                    ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);

                    TableBuilder::new(ui)
                        .striped(true)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::initial(16.0).at_least(16.0))   // ▶
                        .column(Column::initial(60.0).at_least(60.0))   // PC
                        .column(Column::initial(120.0).at_least(90.0))  // BYTES
                        .column(Column::initial(72.0).at_least(60.0))   // MNEMONIC
                        .column(Column::remainder())                          // OPERAND
                        .column(Column::initial(38.0))                  // A
                        .column(Column::initial(38.0))                  // X
                        .column(Column::initial(38.0))                  // Y
                        .column(Column::initial(46.0))                  // P
                        .column(Column::initial(46.0))                  // SP
                        .column(Column::remainder())                          // CYCLES
                        .header(22.0, |header| {
                            self.debugger_table_header(header);
                        })
                        .body(|body| {
                            self.debugger_table_body(body);
                        });
                });
            });
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
                    self.clear_error();
                    let _ = self.send_message(Reset);
                }
                let _ = ui.button("Power Off");

                if ui.button("Pause").clicked() {
                    let _ = self.send_message(Pause);
                }

                if ui.button("Debugger").clicked() {
                    self.debug_window = !self.debug_window;
                }

                ui.end_row();
            });
        });

        TopBottomPanel::bottom("status").show(ctx, |ui| {
            let debugger = if self.is_debugger_attached { "attached" } else { "disabled" };

            ui.label(format!("debugger: {} | rendering: {:.3} ms | UI: {:>5.1} fps | Emulator: {:>5.1} fps",
                             debugger, self.rendering_duration_ms, self.ui_fps, self.emulator_fps
            ));
        });


        CentralPanel::default().frame(self.emulator_viewport_frame).show(ctx, |_| {
            egui::Window::new("emulator")
                .default_pos(pos2(0.0, 22.0))
                .show(ctx, |ui| {
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
                });

            egui::Window::new("NES Debugger")
                .title_bar(false)
                .default_pos(pos2(300.0, 22.0))
                .resizable(true)
                .open(&mut self.debug_window.clone())
                .show(ctx, |ui| {
                   self.debugger_window(ui);
                });
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