use std::ops::Mul;
use eframe::egui::{vec2, Color32, Direction, Layout, Response, RichText, Sense, Stroke, TextureHandle, Ui, Vec2, Widget};
use eframe::epaint::StrokeKind;

#[derive(Copy, Clone)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Danger
}

pub struct ImageTextButton<'a> {
    pub icon: Option<&'a TextureHandle>,
    pub icon_size: Vec2,
    pub label: Option<&'a str>,
    pub kind: ButtonKind,
    pub selected: bool,
    pub min_size: Vec2,
    pub tooltip: Option<&'a str>,
}

impl<'a> ImageTextButton<'a> {
    pub fn new() -> Self {
        Self {
            icon: None,
            icon_size: vec2(16.0, 16.0),
            label: None,
            kind: ButtonKind::Secondary,
            selected: false,
            min_size: vec2(64.0, 48.0),
            tooltip: None,
        }
    }

    pub fn icon(mut self, tex: &'a TextureHandle) -> ImageTextButton<'a> {
        self.icon = Some(tex);
        self
    }

    pub fn label(mut self, s: &'a str) -> ImageTextButton<'a> {
        self.label = Some(s);
        self
    }

    pub fn kind(mut self, k: ButtonKind) -> ImageTextButton<'a> {
        self.kind = k;
        self
    }

    pub fn selected(mut self, s: bool) -> ImageTextButton<'a> {
        self.selected = s;
        self
    }

    pub fn min_size(mut self, s: Vec2) -> ImageTextButton<'a> {
        self.min_size = s;
        self
    }

    pub fn tooltip(mut self, t: &'a str) -> ImageTextButton<'a> {
        self.tooltip = Some(t);
        self
    }
}

impl<'a> Widget for ImageTextButton<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (mut rect, mut resp) = ui.allocate_exact_size(self.min_size, Sense::click());

        if resp.clicked() {
           rect = rect.shrink(1.8);
        } else if resp.hovered() {
            rect = rect.expand(1.1);
        }

        let vis = ui.style().visuals.clone();
        let visuals = ui.style().interact_selectable(&resp, self.selected);
        let rounding = visuals.corner_radius.at_least(0);

        let (fill, stroke) = match self.kind {
            ButtonKind::Secondary |
            ButtonKind::Primary   => {
                if resp.clicked() {
                    (vis.widgets.active.bg_fill, vis.widgets.active.bg_stroke)
                } else if resp.hovered() {
                    (vis.widgets.hovered.bg_fill, vis.widgets.hovered.bg_stroke)
                } else {
                    (vis.widgets.inactive.bg_fill, vis.widgets.inactive.bg_stroke)
                }
            },

            ButtonKind::Danger => {
                let red    = Color32::from_rgb(200, 80, 80);
                (red.linear_multiply(if self.selected { 0.90 } else { 0.75 }), Stroke::new(1.0, red.linear_multiply(0.90)))
            }
        };

        ui.painter().rect_filled(rect, rounding, fill);
        ui.painter().rect_stroke(rect, rounding, stroke, StrokeKind::Outside);

        let mut child = ui.child_ui(rect, Layout::centered_and_justified(Direction::LeftToRight), None);

        if let Some(texture) = self.icon {
            child.image((texture.id(), self.icon_size.mul(2.0)));
        }

        if self.icon.is_some() && let Some(text) = self.label {
            child.add_space(6.0);
            child.label(RichText::new(text).size(12.0).strong());
        }

        if let Some(tooltip) = self.tooltip {
            resp = resp.on_hover_text(tooltip);
        }

        resp
    }
}