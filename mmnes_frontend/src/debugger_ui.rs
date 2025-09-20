use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use eframe::egui;
use eframe::egui::{vec2, Button, Color32, Grid, Key, Response, RichText, Shadow, Stroke, TextStyle, Ui};
use egui_extras::{Column, TableBody, TableBuilder, TableRow};
use log::warn;
use mmnes_core::cpu_debugger::{CpuSnapshot, DebugCommand};
use mmnes_core::nes_console::NesConsoleError;
use crate::helpers_ui::HelpersUI;
use crate::nes_mediator::NesMediator;
use crate::nes_message::NesMessage;
use crate::nes_message::NesMessage::Debug;
use crate::nes_ui::NesUI;
use crate::tooltip_6502::ToolTip6502;

const MAX_CPU_SNAPSHOTS: usize = 256;

pub struct DebuggerUI {
    visible: bool,
    is_debugger_running: bool,
    is_debugger_attached: bool,
    rom_file: Option<PathBuf>,
    nes_mediator: Rc<RefCell<NesMediator>>,
    cpu_snapshots: Vec<Box<dyn CpuSnapshot>>,
}

impl NesUI for DebuggerUI {
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn footer(&self) -> String {
        let debugger_state = if self.is_debugger_attached { "attached" } else { "disabled" };
        format!("debugger: {}", debugger_state)
    }

    fn draw(&mut self, ui: &mut Ui) -> Result<(), NesConsoleError> {
        self.debugger_window(ui)
    }
}

impl DebuggerUI {

    pub fn new(nes_mediator: Rc<RefCell<NesMediator>>) -> DebuggerUI {
        DebuggerUI {
            visible: false,
            is_debugger_running: false,
            is_debugger_attached: false,
            rom_file: None,
            nes_mediator,
            cpu_snapshots: Vec::new(),
        }
    }
    
    pub fn set_rom_file(&mut self, rom_file: Option<PathBuf>) {
        self.rom_file = rom_file;
    }

    fn disasm_line(field: &str, is_current: bool) -> RichText {
        let mut rt = HelpersUI::monospace(field);

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

    fn debugger_trap_input(&mut self, ui: &mut Ui) -> Result<(), NesConsoleError> {
        ui.input(|i| -> Result<(), NesConsoleError> {
            if i.modifiers.shift && i.key_pressed(Key::F5) {
                self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::Paused))?;
            } else if i.key_pressed(Key::F5) {
                self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::Run))?;
            }
            if i.modifiers.shift && i.key_pressed(Key::F11) {
                self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::StepOut))?;
            } else if i.key_pressed(Key::F11) {
                self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::StepInto))?;
            }
            if i.key_pressed(Key::F10) {
                self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::StepOver))?;
            }
            if i.key_pressed(Key::F7) {
                self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::StepInstruction))?;
            }

            Ok(())
        })?;

        Ok(())
    }

    fn debugger_toolbar(&mut self, ui: &mut Ui) -> Result<(), NesConsoleError> {
        let default_fill = Color32::from_rgb(66, 66, 72);

        self.debugger_trap_input(ui)?;

        let resp = ui.horizontal(|ui| -> Result<(), NesConsoleError> {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);

            // Group 1: Run / Stop
            let resp0 = egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| -> Result<(), NesConsoleError> {
                    let resp = ui.horizontal_centered(|ui| -> Result<(), NesConsoleError> {

                        if self.debugger_icon_button(ui, "▶", "Run / Continue (F5)", default_fill).clicked() {
                            self.is_debugger_attached = true;
                            self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::Run))?;
                        }

                        if self.debugger_icon_button(ui, "⏸", "Pause (Shift+F5)", default_fill).clicked() {
                            self.is_debugger_attached = true;
                            self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::Paused))?;
                        }

                        Ok(())
                    });

                    resp.inner
                });

            // Group 2: Stepping
            let resp1 = egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| -> Result<(), NesConsoleError> {
                    let resp = ui.horizontal_centered(|ui| {
                        if self.debugger_icon_button(ui, "↔", "Step Over (F10)", default_fill).clicked() {
                            self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::StepOver))?;
                        }
                        if self.debugger_icon_button(ui, "↘", "Step Into (F11)", default_fill).clicked() {
                            self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::StepInto))?;
                        }
                        if self.debugger_icon_button(ui, "↗", "Step Out (Shift+F11)", default_fill).clicked() {
                            self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::StepOut))?;
                        }
                        if self.debugger_icon_button(ui, "⏭", "Step Instruction (F7)", default_fill).clicked() {
                            self.is_debugger_attached = true;
                            self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::StepInstruction))?;
                        }

                        Ok(())
                    });

                    resp.inner
                });

            // Group 3: Others
            let resp2 = egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| -> Result<(), NesConsoleError> {
                    let resp = ui.horizontal_centered(|ui| {
                        if self.debugger_icon_button(ui, "🚫", "Clear Screen", default_fill).clicked() {
                            self.cpu_snapshots.clear();
                        }

                        if self.debugger_icon_button(ui, "🔌", "Attach / Detach", default_fill).clicked() {
                            self.is_debugger_attached = !self.is_debugger_attached;

                            let _ = if self.is_debugger_attached {
                                self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::Paused))?;
                            } else {
                                self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::Detach))?;
                            };
                        }

                        if self.debugger_icon_button(ui, "✨", "Explain", default_fill).clicked() {
                        }

                        Ok(())
                    });

                    resp.inner
                });

            // Group 4: Quit
            let resp3 = egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    let resp  = ui.horizontal_centered(|ui| -> Result<(), NesConsoleError> {
                        if self.debugger_icon_button(ui, "❌", "Quit", default_fill).clicked() {
                            self.is_debugger_attached = false;
                            self.visible = false;
                            self.nes_mediator.borrow_mut().send_message(Debug(DebugCommand::Detach))?;
                        }

                        Ok(())
                    });

                    resp.inner
                });

            for response in [resp0, resp1, resp2, resp3] {
                if let Err(error) = response.inner {
                    return Err(error);
                }
            }

            Ok(())
        });

        resp.inner
    }

    fn debugger_table_header(&self, mut header: TableRow) {
        for item in ["", "PC", "BYTES", "OP", "OPERAND", "A", "X", "Y", "P", "SP", "CYCLES"] {
            header.col(|ui| {ui.label(HelpersUI::header(item)); });
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

            for i in snapshot.instruction().iter() {
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
                ui.label(HelpersUI::monospace(if is_current { "▶" } else { " " }));
            });

            for (i, item) in [pc, bytes, op, operand, a, x, y, p, sp, cycles].iter().enumerate() {
                row.col(|ui| {
                    let rt = DebuggerUI::disasm_line(&item, is_current);

                    if i == 2 && let Some(tooltip) = ToolTip6502::tooltip(&item) {
                        //ui.label(rt).on_hover_text(RichText::new(tooltip).monospace());
                        let resp = ui.label(rt);

                        resp.on_hover_ui_at_pointer(|ui| {
                            egui::Frame::popup(ui.style())
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .stroke(Stroke::new(0.0, Color32::PLACEHOLDER))
                                .shadow(Shadow::NONE)
                                .show(ui, |ui| {
                                    ui.style_mut().override_text_style = Some(TextStyle::Monospace);
                                    ui.set_min_width(420.0);

                                    ui.label(RichText::new(tooltip.title).strong().monospace());
                                    if let Some(summary) = tooltip.summary {
                                        ui.add_space(4.0);
                                        ui.label(RichText::new(summary).monospace());
                                    }

                                    if let Some(flags) = tooltip.flags_note {
                                        ui.add_space(4.0);
                                        ui.label(RichText::new(flags).monospace().monospace());
                                    }

                                    ui.add_space(8.0);
                                    Grid::new("instruction_tooltip_grid")
                                        .num_columns(5)
                                        .spacing([12.0, 4.0])
                                        .show(ui, |ui| {
                                            for h in ["addressing", "assembler", "opc", "bytes", "cycles"] {
                                                ui.label(RichText::new(h).monospace().strong());
                                            }
                                            ui.end_row();

                                            for row in &tooltip.rows {
                                                ui.label(RichText::new(row.addressing).monospace());
                                                ui.label(RichText::new(row.assembler).monospace());
                                                ui.label(RichText::new(row.opc).monospace());
                                                ui.label(RichText::new(row.bytes).monospace());
                                                ui.label(RichText::new(row.cycles).monospace());
                                                ui.end_row();
                                            }
                                        });

                                    if let Some(exception) = tooltip.exception {
                                        ui.add_space(8.0);
                                        ui.label(RichText::new(exception).monospace().monospace());
                                    }
                                });
                        });

                    } else {
                        ui.label(rt);
                    }
                });
            }
        });
    }

    pub fn read_debug_messages(&mut self) -> Result<(), NesConsoleError> {
        let messages = self.nes_mediator.borrow().read_debug_messages()?;

        for message in messages {
            match message {
                NesMessage::CpuSnapshot(snap) => self.cpu_snapshots.push(snap),
                NesMessage::CpuSnapshotSet(snaps) => self.cpu_snapshots.extend(snaps),
                _ => warn!("unexpected message: {:?}", message),
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

    fn debugger_window(&mut self, ui: &mut Ui) -> Result<(), NesConsoleError> {
        self.read_debug_messages()?;

        self.debugger_header_bar(ui);
        ui.separator();
        self.debugger_toolbar(ui)?;

        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("instructions_scroll")
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.scope(|ui| {
                    ui.style_mut().override_text_style = Some(TextStyle::Monospace);

                    TableBuilder::new(ui)
                        .striped(true)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::initial(16.0).at_least(16.0))    // ▶
                        .column(Column::initial(60.0).at_least(60.0))    // PC
                        .column(Column::initial(120.0).at_least(90.0))   // BYTES
                        .column(Column::initial(72.0).at_least(60.0))    // MNEMONIC
                        .column(Column::remainder())                                    // OPERAND
                        .column(Column::initial(38.0))                            // A
                        .column(Column::initial(38.0))                            // X
                        .column(Column::initial(38.0))                            // Y
                        .column(Column::initial(46.0))                            // P
                        .column(Column::initial(46.0))                            // SP
                        .column(Column::remainder())                                    // CYCLES
                        .header(22.0, |header| {
                            self.debugger_table_header(header);
                        })
                        .body(|body| {
                            self.debugger_table_body(body);
                        });
                });
            });

        Ok(())
    }
}