//! App frame shared by every suite window: settings, wheel zoom, toasts,
//! top bar, navigation rail, status bar, screenshots.

use crate::theme::{self, semibold, Tone};
use crate::widgets;
use egui::{Context, CornerRadius, FontFamily, Key, Modifiers, RichText, Sense, Stroke, Ui};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

pub const ZOOM_MIN: f32 = 0.5;
pub const ZOOM_MAX: f32 = 3.0;

/// Per-user window settings, stored as JSON next to the app's database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub dark: bool,
    pub zoom: f32,
    pub lang: String,
}

impl Default for UiSettings {
    fn default() -> Self {
        UiSettings {
            dark: false,
            zoom: 1.0,
            lang: "de".into(),
        }
    }
}

impl UiSettings {
    pub fn load(path: &Path) -> UiSettings {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .map(UiSettings::sanitized)
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| e.to_string())
    }
    fn sanitized(mut self) -> UiSettings {
        if !self.zoom.is_finite() {
            self.zoom = 1.0;
        }
        self.zoom = self.zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        self
    }
}

/// Settings file for an app: `<dir of db>/ui.json`.
pub fn settings_path(db: &Path) -> PathBuf {
    db.parent()
        .map(|p| p.join("ui.json"))
        .unwrap_or_else(|| PathBuf::from("ui.json"))
}

/// One-time setup: fonts, theme, zoom.
pub fn install(ctx: &Context, s: &UiSettings) {
    theme::install_fonts(ctx);
    theme::apply(ctx, s.dark);
    ctx.set_zoom_factor(s.zoom);
    ctx.options_mut(|o| o.zoom_with_keyboard = true);
}

/// Ctrl+mouse wheel (and pinch) zoom, Ctrl+0 reset. Ctrl+Plus/Minus are handled
/// by egui itself. Returns the new zoom when it changed this frame.
pub fn handle_zoom(ctx: &Context, s: &mut UiSettings) -> Option<f32> {
    let delta = ctx.input(|i| i.zoom_delta());
    let reset = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Num0));
    let current = ctx.zoom_factor();
    let target = if reset {
        1.0
    } else if (delta - 1.0).abs() > f32::EPSILON {
        (current * delta).clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        current
    };
    if (target - current).abs() > 1e-4 {
        ctx.set_zoom_factor(target);
    }
    if (target - s.zoom).abs() > 1e-4 {
        s.zoom = target;
        Some(target)
    } else {
        None
    }
}

/// Toggle dark mode and re-apply the theme.
pub fn set_dark(ctx: &Context, s: &mut UiSettings, dark: bool) {
    s.dark = dark;
    theme::apply(ctx, dark);
}

struct Toast {
    text: String,
    tone: Tone,
    born: Instant,
}

/// Short messages in the bottom-right corner. Errors stay longer.
#[derive(Default)]
pub struct Toasts {
    items: Vec<Toast>,
}

impl Toasts {
    pub fn push(&mut self, text: impl Into<String>, tone: Tone) {
        self.items.push(Toast {
            text: text.into(),
            tone,
            born: Instant::now(),
        });
        if self.items.len() > 5 {
            self.items.remove(0);
        }
    }
    pub fn ok(&mut self, text: impl Into<String>) {
        self.push(text, Tone::Ok)
    }
    pub fn err(&mut self, text: impl Into<String>) {
        self.push(text, Tone::Danger)
    }
    pub fn latest(&self) -> Option<(&str, Tone)> {
        self.items.last().map(|t| (t.text.as_str(), t.tone))
    }
    pub fn show(&mut self, ctx: &Context) {
        self.items.retain(|t| {
            t.born.elapsed()
                < if t.tone == Tone::Danger {
                    Duration::from_secs(9)
                } else {
                    Duration::from_secs(4)
                }
        });
        if self.items.is_empty() {
            return;
        }
        let dark = ctx.global_style().visuals.dark_mode;
        let mut close = None;
        egui::Area::new(egui::Id::unique("craft_toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -40.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                for (i, t) in self.items.iter().enumerate() {
                    let (fg, bg) = theme::tone_colors(t.tone, dark);
                    egui::Frame::new()
                        .fill(bg)
                        .stroke(Stroke::new(1.0, theme::tone_solid(t.tone, dark)))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 12,
                            spread: 0,
                            color: theme::tokens(dark).shadow,
                        })
                        .show(ui, |ui| {
                            ui.set_max_width(420.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&t.text).color(fg).family(semibold()));
                                if ui.small_button("x").on_hover_text("close").clicked() {
                                    close = Some(i);
                                }
                            });
                        });
                    ui.add_space(6.0);
                }
            });
        if let Some(i) = close {
            self.items.remove(i);
        }
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

/// Top bar: app mark + title, then `middle` (search, palette hint), then `right`.
pub fn top_bar(
    ui: &mut Ui,
    app: &str,
    tone: Tone,
    middle: impl FnOnce(&mut Ui),
    right: impl FnOnce(&mut Ui),
) {
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.set_min_height(40.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(7), theme::tone_solid(tone, t.dark));
        let mark: String = app
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            mark,
            egui::FontId::new(16.0, theme::bold()),
            if t.dark {
                t.canvas
            } else {
                egui::Color32::WHITE
            },
        );
        ui.label(RichText::new(app).family(theme::bold()).size(18.0));
        ui.add_space(16.0);
        middle(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    });
}

/// Item in the left navigation rail.
pub fn nav_item(
    ui: &mut Ui,
    icon: &str,
    label: &str,
    selected: bool,
    badge: Option<(usize, Tone)>,
) -> egui::Response {
    let t = theme::current(ui.ctx());
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 34.0), Sense::click());
    let mut truncated = false;
    if ui.is_rect_visible(rect) {
        let bg = if selected {
            Some(t.accent_soft)
        } else if resp.hovered() {
            Some(t.sunken)
        } else {
            None
        };
        if let Some(bg) = bg {
            ui.painter().rect_filled(rect, CornerRadius::same(7), bg);
        }
        if selected {
            let bar = egui::Rect::from_min_size(
                rect.left_top() + egui::vec2(0.0, 6.0),
                egui::vec2(3.0, rect.height() - 12.0),
            );
            ui.painter()
                .rect_filled(bar, CornerRadius::same(2), t.accent);
        }
        let fg = if selected && !t.dark {
            theme::tone_colors(Tone::Accent, false).0
        } else {
            t.text
        };
        ui.painter().text(
            rect.left_center() + egui::vec2(14.0, 0.0),
            egui::Align2::LEFT_CENTER,
            icon,
            egui::FontId::proportional(16.0),
            fg,
        );
        let fam = if selected {
            semibold()
        } else {
            FontFamily::Proportional
        };
        let badge_w = badge
            .filter(|(n, _)| *n > 0)
            .map(|(n, _)| {
                widgets::pill_size(
                    ui.painter(),
                    &if n > 99 {
                        "99+".to_string()
                    } else {
                        n.to_string()
                    },
                )
                .x + 12.0
            })
            .unwrap_or(0.0);
        let mut job = egui::text::LayoutJob::simple_singleline(
            label.to_string(),
            egui::FontId::new(14.5, fam),
            fg,
        );
        job.wrap =
            egui::text::TextWrapping::truncate_at_width((rect.width() - 48.0 - badge_w).max(30.0));
        let galley = ui.painter().layout_job(job);
        let pos = rect.left_center() + egui::vec2(40.0, -galley.size().y / 2.0);
        truncated = galley.elided;
        ui.painter().galley(pos, galley, fg);
        if let Some((n, tone)) = badge.filter(|(n, _)| *n > 0) {
            let s = if n > 99 {
                "99+".to_string()
            } else {
                n.to_string()
            };
            let size = widgets::pill_size(ui.painter(), &s);
            widgets::paint_pill(
                ui.painter(),
                egui::pos2(rect.right() - size.x - 8.0, rect.center().y - size.y / 2.0),
                &s,
                tone,
                t.dark,
            );
        }
    }
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if truncated {
        resp.on_hover_text(label)
    } else {
        resp
    }
}

/// Group caption in the navigation rail.
pub fn nav_caption(ui: &mut Ui, text: &str) {
    let t = theme::current(ui.ctx());
    ui.add_space(10.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .size(11.5)
            .family(semibold())
            .color(t.text_weak),
    );
    ui.add_space(2.0);
}

/// Status bar: last message left, zoom / theme / version right. Returns true when the theme button was clicked.
pub fn status_bar(
    ui: &mut Ui,
    message: Option<(&str, Tone)>,
    zoom: f32,
    dark: bool,
    right_info: &str,
) -> StatusClick {
    let t = theme::current(ui.ctx());
    let mut out = StatusClick::default();
    ui.horizontal(|ui| {
        match message {
            Some((m, tone)) => {
                ui.label(RichText::new(m).color(theme::tone_colors(tone, t.dark).0));
            }
            None => {
                ui.label(RichText::new("Ready").color(t.text_weak));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(right_info)
                    .family(FontFamily::Monospace)
                    .size(12.0)
                    .color(t.text_weak),
            );
            ui.separator();
            if ui
                .button(if dark { "Light" } else { "Dark" })
                .on_hover_text("Switch light / dark")
                .clicked()
            {
                out.toggle_theme = true;
            }
            if ui
                .button(format!("{:.0}%", zoom * 100.0))
                .on_hover_text("Zoom: Ctrl + mouse wheel, Ctrl +/-; click to reset")
                .clicked()
            {
                out.reset_zoom = true;
            }
        });
    });
    out
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StatusClick {
    pub toggle_theme: bool,
    pub reset_zoom: bool,
}

/// Render N frames, save one PNG, close the window. For docs and review snapshots.
pub struct Shot {
    pub path: PathBuf,
    requested: bool,
    frames: u32,
    slot: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
}

impl Shot {
    pub fn new(path: PathBuf) -> Shot {
        Shot {
            path,
            requested: false,
            frames: 0,
            slot: Arc::new(Mutex::new(None)),
        }
    }

    /// Call once per frame. Returns true when the PNG was written.
    pub fn tick(&mut self, ctx: &Context) -> bool {
        self.frames += 1;
        ctx.request_repaint();
        if !self.requested && self.frames >= 10 {
            let slot = Arc::clone(&self.slot);
            ctx.request_screenshot(move |img| {
                *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(img);
            });
            self.requested = true;
        }
        let img = self
            .slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some(img) = img else { return false };
        let [w, h] = img.size;
        let mut rgba = Vec::with_capacity(w * h * 4);
        for p in &img.pixels {
            rgba.extend_from_slice(&p.to_array());
        }
        match image::save_buffer(
            &self.path,
            &rgba,
            w as u32,
            h as u32,
            image::ColorType::Rgba8,
        ) {
            Ok(()) => eprintln!("screenshot saved: {}", self.path.display()),
            Err(e) => eprintln!("screenshot failed: {e}"),
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_and_sanitize() {
        let dir =
            std::env::temp_dir().join(format!("craft-biz-ui-settings-{}", std::process::id()));
        let p = dir.join("ui.json");
        let s = UiSettings {
            dark: true,
            zoom: 1.25,
            lang: "en".into(),
        };
        s.save(&p).unwrap();
        assert_eq!(UiSettings::load(&p), s);
        std::fs::write(&p, br#"{"zoom": 99}"#).unwrap();
        assert_eq!(UiSettings::load(&p).zoom, ZOOM_MAX);
        std::fs::write(&p, b"junk").unwrap();
        assert_eq!(UiSettings::load(&p), UiSettings::default());
        let _ = std::fs::remove_dir_all(dir);
        assert_eq!(
            settings_path(Path::new("C:/x/app.db")),
            Path::new("C:/x").join("ui.json")
        );
    }
}
