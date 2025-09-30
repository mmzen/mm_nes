use eframe::egui::{vec2, CornerRadius, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, Widget};
use eframe::epaint::StrokeKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Play, Pause, Reset, Power, Debugger, LoadRom,
    // fallback
    Generic,
}

fn paint_icon(p: &Painter, rect: Rect, stroke: Stroke, icon: Icon) {
    // Draw within a 24x24 logical box centered in `rect`
    let side = rect.size().min_elem().min(24.0);
    let scale = side / 24.0;
    let offset = rect.center() - 0.5 * Vec2::splat(side);
    let to = |x: f32, y: f32| Pos2::new(offset.x + x * scale, offset.y + y * scale);
    let fill = stroke.color;

    match icon {
        Icon::Play => {
            p.add(Shape::convex_polygon(vec![to(8.0,5.0), to(19.0,12.0), to(8.0,19.0)], fill, Stroke::NONE));
        }
        Icon::Pause => {
            let r1 = Rect::from_min_max(to(7.0,5.0),  to(11.0,19.0));
            let r2 = Rect::from_min_max(to(13.0,5.0), to(17.0,19.0));
            p.rect_filled(r1, CornerRadius::same((2.0 * scale) as u8), fill);
            p.rect_filled(r2, CornerRadius::same((2.0 * scale) as u8), fill);
        }
        Icon::Reset => {
            p.circle_stroke(to(12.0,12.0), 8.0*scale, stroke);
            p.line_segment([to(12.0,6.0), to(12.0,12.0)], stroke);
            p.add(Shape::convex_polygon(vec![to(16.0,10.0), to(14.8,9.3), to(15.5,11.2)], fill, Stroke::NONE));
        }
        Icon::Power => {
            p.line_segment([to(12.0,3.0), to(12.0,10.0)], stroke);
            p.circle_stroke(to(12.0,13.0), 7.0*scale, stroke);
        }
        Icon::Debugger => {
            p.circle_stroke(to(12.0,7.0), 2.0*scale, stroke);
            let body = Rect::from_min_max(to(8.0,9.0), to(16.0,17.0));
            p.rect_stroke(body, CornerRadius::same((4.0 * scale) as u8), stroke, StrokeKind::Inside);
            p.line_segment([to(4.0,9.0),  to(8.0,9.0)], stroke);
            p.line_segment([to(16.0,9.0), to(20.0,9.0)], stroke);
            p.line_segment([to(5.0,14.0), to(8.0,14.0)], stroke);
            p.line_segment([to(16.0,14.0), to(19.0,14.0)], stroke);
        }
        Icon::LoadRom => {
            p.line_segment([to(12.0,3.0), to(12.0,12.0)], stroke);
            p.add(Shape::convex_polygon(vec![to(12.0,14.0), to(10.8,12.8), to(13.2,12.8)], fill, Stroke::NONE));
            let tray = Rect::from_min_max(to(4.0,17.0), to(20.0,21.0));
            p.rect_stroke(tray, CornerRadius::same((2.0 * scale) as u8), stroke, StrokeKind::Inside);
        }
        Icon::Generic => {
            // simple square
            let r = Rect::from_min_max(to(6.0,6.0), to(18.0,18.0));
            p.rect_stroke(r, CornerRadius::same((3.0 * scale) as u8), stroke, StrokeKind::Inside);
        }
    }
}


/***
 * XXX
 *  icons should be in buttons in widgets
 ***/
pub fn icon_for_label(label: &str) -> Icon {
    let s = label.trim();

    if s.eq_ignore_ascii_case("LOAD ROM") { return Icon::LoadRom; }
    if s.eq_ignore_ascii_case("PLAY") { return Icon::Play; }
    if s.eq_ignore_ascii_case("PAUSE") { return Icon::Pause; }
    if s.eq_ignore_ascii_case("RESET") { return Icon::Reset; }
    if s.eq_ignore_ascii_case("POWER") { return Icon::Power; }
    if s.eq_ignore_ascii_case("DEBUGGER") { return Icon::Debugger; }
    Icon::Generic
}

pub struct SquareIconButton {
    pub icon: Icon,
    pub size: f32,
    pub selected: bool,
    pub tooltip: Option<&'static str>,
}

impl SquareIconButton {
    pub fn new(icon: Icon) -> SquareIconButton {
        Self { icon, size: 36.0, selected: false, tooltip: None }
    }

    pub fn size(mut self, side: f32) -> SquareIconButton {
        self.size = side;
        self
    }

    pub fn selected(mut self, v: bool) -> SquareIconButton {
        self.selected = v;
        self
    }

    pub fn tooltip(mut self, t: &'static str) -> SquareIconButton {
        self.tooltip = Some(t);
        self
    }
}

impl Widget for SquareIconButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let padding = vec2(10.0, 10.0);
        let desired = vec2(self.size, self.size) + 2.0 * padding;
        let (rect, mut resp) = ui.allocate_exact_size(desired, Sense::click());

        // Visuals that match egui buttons
        let visuals = ui.style().interact_selectable(&resp, self.selected);
        let rounding = visuals.corner_radius.at_least(6);
        ui.painter().rect_filled(rect, rounding, visuals.bg_fill);
        ui.painter().rect_stroke(rect, rounding, visuals.bg_stroke, StrokeKind::Inside);

        // Icon
        let inner = rect.shrink2(padding);
        let stroke = Stroke { width: 1.8, color: visuals.fg_stroke.color };
        paint_icon(ui.painter(), inner, stroke, self.icon);

        if let Some(t) = self.tooltip {
            resp = resp.on_hover_text(t);
        }
        resp
    }
}