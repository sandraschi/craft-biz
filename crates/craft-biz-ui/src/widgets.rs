//! Small building blocks: pills, avatars, buttons, KPI tiles, sparklines, empty states.

use crate::theme::{self, semibold, Tone};
use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Response, RichText, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};

/// Rounded label with tone colours. Returns the response (hover for tooltips).
pub fn pill(ui: &mut Ui, text: &str, tone: Tone) -> Response {
    let dark = ui.visuals().dark_mode;
    let (fg, bg) = theme::tone_colors(tone, dark);
    let font = FontId::new(12.5, semibold());
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, fg);
    let size = galley.size() + egui::vec2(16.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, CornerRadius::same(255), bg);
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, fg);
    }
    resp
}

/// Paint a pill at a position (for custom-painted widgets like kanban cards). Returns its width.
pub fn paint_pill(
    painter: &egui::Painter,
    left_top: Pos2,
    text: &str,
    tone: Tone,
    dark: bool,
) -> Rect {
    let (fg, bg) = theme::tone_colors(tone, dark);
    let galley = painter.layout_no_wrap(text.to_string(), FontId::new(12.0, semibold()), fg);
    let rect = Rect::from_min_size(left_top, galley.size() + egui::vec2(14.0, 5.0));
    painter.rect_filled(rect, CornerRadius::same(255), bg);
    painter.galley(rect.center() - galley.size() / 2.0, galley, fg);
    rect
}

/// Size a pill would take, for layout before painting.
pub fn pill_size(painter: &egui::Painter, text: &str) -> Vec2 {
    painter
        .layout_no_wrap(
            text.to_string(),
            FontId::new(12.0, semibold()),
            Color32::WHITE,
        )
        .size()
        + egui::vec2(14.0, 5.0)
}

const AVATAR_TONES: [Tone; 6] = [
    Tone::Accent,
    Tone::Violet,
    Tone::Teal,
    Tone::Ok,
    Tone::Warn,
    Tone::Info,
];

/// Stable colour for a name.
pub fn avatar_tone(name: &str) -> Tone {
    let h = name
        .bytes()
        .fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    AVATAR_TONES[(h as usize) % AVATAR_TONES.len()]
}

pub fn initials(name: &str) -> String {
    let parts: Vec<&str> = name
        .split(|c: char| c.is_whitespace() || c == '.' || c == '@')
        .filter(|p| !p.is_empty())
        .collect();
    let mut s: String = parts
        .iter()
        .take(2)
        .filter_map(|p| p.chars().next())
        .collect();
    if s.is_empty() {
        s.push('?');
    }
    s.to_uppercase()
}

/// Paint a round avatar with initials.
pub fn paint_avatar(painter: &egui::Painter, center: Pos2, radius: f32, name: &str, dark: bool) {
    let (fg, bg) = theme::tone_colors(avatar_tone(name), dark);
    painter.circle_filled(center, radius, bg);
    painter.circle_stroke(center, radius, Stroke::new(1.0, fg.gamma_multiply(0.4)));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        initials(name),
        FontId::new(radius * 0.9, semibold()),
        fg,
    );
}

pub fn avatar(ui: &mut Ui, name: &str, radius: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(radius * 2.0), Sense::hover());
    paint_avatar(
        ui.painter(),
        rect.center(),
        radius,
        name,
        ui.visuals().dark_mode,
    );
    resp.on_hover_text(name)
}

/// Filled accent button for the main action of a screen.
pub fn primary(ui: &mut Ui, text: &str) -> Response {
    let t = theme::current(ui.ctx());
    ui.add(
        egui::Button::new(RichText::new(text).color(t.on_accent).family(semibold()))
            .fill(t.accent)
            .stroke(Stroke::new(1.0, t.accent)),
    )
}

/// Button for destructive or irreversible actions.
pub fn danger(ui: &mut Ui, text: &str) -> Response {
    let dark = ui.visuals().dark_mode;
    let (fg, bg) = theme::tone_colors(Tone::Danger, dark);
    ui.add(
        egui::Button::new(RichText::new(text).color(fg).family(semibold()))
            .fill(bg)
            .stroke(Stroke::new(1.0, theme::tone_solid(Tone::Danger, dark))),
    )
}

/// Section title with optional count.
pub fn section(ui: &mut Ui, title: &str, count: Option<usize>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).family(semibold()).size(16.0));
        if let Some(n) = count {
            pill(ui, &n.to_string(), Tone::Neutral);
        }
    });
}

/// Page title row: big title, subtitle, actions on the right.
pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str, actions: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).family(semibold()).size(22.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions);
    });
    if !subtitle.is_empty() {
        ui.add(
            egui::Label::new(RichText::new(subtitle).color(theme::current(ui.ctx()).text_weak))
                .wrap(),
        );
    }
    ui.add_space(8.0);
}

/// White card with border and soft shadow.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let t = theme::current(ui.ctx());
    egui::Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(14))
        .shadow(egui::epaint::Shadow {
            offset: [0, 1],
            blur: 4,
            spread: 0,
            color: t.shadow.gamma_multiply(0.6),
        })
        .show(ui, |ui| ui.vertical(add).inner)
        .inner
}

/// KPI tile: label, big value, optional note with tone. Prefer [`kpi_row`].
pub fn kpi(ui: &mut Ui, label: &str, value: &str, note: Option<(&str, Tone)>) {
    card(ui, |ui| {
        ui.set_min_width(170.0);
        kpi_body(ui, label, value, note);
    });
}

fn kpi_body(ui: &mut Ui, label: &str, value: &str, note: Option<(&str, Tone)>) {
    let t = theme::current(ui.ctx());
    ui.add(egui::Label::new(RichText::new(label).color(t.text_weak).size(13.0)).truncate());
    ui.add(egui::Label::new(RichText::new(value).family(semibold()).size(24.0)).truncate());
    match note {
        Some((n, tone)) => {
            pill(ui, n, tone);
        }
        None => {
            ui.add_space(22.0);
        }
    }
}

/// One KPI tile in a [`kpi_row`].
pub struct Kpi {
    pub label: String,
    pub value: String,
    pub note: Option<(String, Tone)>,
}

impl Kpi {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Kpi {
        Kpi {
            label: label.into(),
            value: value.into(),
            note: None,
        }
    }
    pub fn note(mut self, text: impl Into<String>, tone: Tone) -> Kpi {
        self.note = Some((text.into(), tone));
        self
    }
}

/// Equal-width KPI tiles that always fit the width (several rows if needed).
pub fn kpi_row(ui: &mut Ui, items: &[Kpi]) {
    if items.is_empty() {
        return;
    }
    let per_row = ((ui.available_width() / 172.0).floor() as usize).clamp(1, items.len());
    for chunk in items.chunks(per_row) {
        ui.columns(per_row, |cols| {
            for (i, k) in chunk.iter().enumerate() {
                card(&mut cols[i], |ui| {
                    ui.set_width(ui.available_width());
                    kpi_body(
                        ui,
                        &k.label,
                        &k.value,
                        k.note.as_ref().map(|(n, t)| (n.as_str(), *t)),
                    );
                });
            }
        });
        ui.add_space(6.0);
    }
}

/// Tiny line chart; last point marked.
pub fn sparkline(ui: &mut Ui, values: &[f32], size: Vec2, tone: Tone) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if values.len() < 2 || !ui.is_rect_visible(rect) {
        return resp;
    }
    let dark = ui.visuals().dark_mode;
    let col = theme::tone_solid(tone, dark);
    let (lo, hi) = values
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    let span = (hi - lo).max(1e-6);
    let pts: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = rect.left() + rect.width() * i as f32 / (values.len() - 1) as f32;
            let y = rect.bottom() - 2.0 - (rect.height() - 4.0) * (v - lo) / span;
            Pos2::new(x, y)
        })
        .collect();
    ui.painter()
        .add(egui::Shape::line(pts.clone(), Stroke::new(2.0, col)));
    if let Some(p) = pts.last() {
        ui.painter().circle_filled(*p, 3.0, col);
    }
    resp
}

/// Horizontal bar 0..1 with tone.
pub fn meter(ui: &mut Ui, fraction: f32, width: f32, tone: Tone) -> Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 8.0), Sense::hover());
    let t = theme::current(ui.ctx());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(4), t.sunken);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, t.border),
        StrokeKind::Inside,
    );
    let mut fill = rect;
    fill.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    ui.painter()
        .rect_filled(fill, CornerRadius::same(4), theme::tone_solid(tone, t.dark));
    resp
}

/// Centered message for empty lists, with a hint what to do.
pub fn empty_state(ui: &mut Ui, title: &str, hint: &str) {
    let t = theme::current(ui.ctx());
    ui.vertical_centered(|ui| {
        ui.add_space(30.0);
        ui.label(RichText::new(title).family(semibold()).size(17.0));
        ui.label(RichText::new(hint).color(t.text_weak));
        ui.add_space(30.0);
    });
}

/// Keyboard shortcut badge, e.g. "Ctrl+K".
pub fn key_hint(ui: &mut Ui, keys: &str) {
    let t = theme::current(ui.ctx());
    egui::Frame::new()
        .fill(t.sunken)
        .stroke(Stroke::new(1.0, t.border_strong))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(
                RichText::new(keys)
                    .family(FontFamily::Monospace)
                    .size(12.0)
                    .color(t.text_weak),
            );
        });
}

/// Label: value row for detail panels.
pub fn field(ui: &mut Ui, label: &str, value: impl FnOnce(&mut Ui)) {
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.add_sized(
            [120.0, 22.0],
            egui::Label::new(RichText::new(label).color(t.text_weak)).truncate(),
        );
        value(ui);
    });
}

/// Monospace amount, red when negative.
pub fn money(ui: &mut Ui, text: &str, negative: bool) -> Response {
    let dark = ui.visuals().dark_mode;
    let r = RichText::new(text).family(FontFamily::Monospace);
    ui.label(if negative {
        r.color(theme::tone_solid(Tone::Danger, dark))
    } else {
        r
    })
}
