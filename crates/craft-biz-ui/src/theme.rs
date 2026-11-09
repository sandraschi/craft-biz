//! High-contrast light and dark themes, fonts and text sizes.
//!
//! Rule of the suite: body text is near-black on white (light) or near-white
//! on near-black (dark). "Weak" text is still at least 7:1 against its
//! background; nothing is light grey on white.

use egui::epaint::Shadow;
use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle, Visuals,
};
use std::sync::Arc;

/// Semantic colour role for pills, card accents, toasts and charts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Tone {
    #[default]
    Neutral,
    Accent,
    Info,
    Ok,
    Warn,
    Danger,
    Violet,
    Teal,
}

impl Tone {
    pub const ALL: [Tone; 8] = [
        Tone::Neutral,
        Tone::Accent,
        Tone::Info,
        Tone::Ok,
        Tone::Warn,
        Tone::Danger,
        Tone::Violet,
        Tone::Teal,
    ];
}

/// Every colour the kit paints with. Derived from the current dark/light mode.
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub dark: bool,
    /// Window background behind panels.
    pub canvas: Color32,
    /// Panels, cards, table rows.
    pub surface: Color32,
    /// Slightly raised areas: column backgrounds, table header, stripes.
    pub sunken: Color32,
    pub text: Color32,
    /// Secondary text. Still >= 7:1 on `surface`.
    pub text_weak: Color32,
    /// Hairlines between rows, card edges.
    pub border: Color32,
    /// Input and focus borders (>= 3:1 non-text contrast).
    pub border_strong: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub accent_soft: Color32,
    pub shadow: Color32,
}

pub fn tokens(dark: bool) -> Tokens {
    if dark {
        Tokens {
            dark,
            canvas: Color32::from_rgb(0x0B, 0x10, 0x17),
            surface: Color32::from_rgb(0x15, 0x1B, 0x25),
            sunken: Color32::from_rgb(0x1C, 0x24, 0x31),
            text: Color32::from_rgb(0xF3, 0xF6, 0xFA),
            text_weak: Color32::from_rgb(0xC6, 0xCE, 0xD9),
            border: Color32::from_rgb(0x33, 0x3D, 0x4C),
            border_strong: Color32::from_rgb(0x6A, 0x78, 0x8C),
            accent: Color32::from_rgb(0x74, 0xA7, 0xFF),
            on_accent: Color32::from_rgb(0x07, 0x14, 0x2B),
            accent_soft: Color32::from_rgb(0x1B, 0x33, 0x5C),
            shadow: Color32::from_black_alpha(140),
        }
    } else {
        Tokens {
            dark,
            canvas: Color32::from_rgb(0xEE, 0xF1, 0xF5),
            surface: Color32::WHITE,
            sunken: Color32::from_rgb(0xF4, 0xF6, 0xF9),
            text: Color32::from_rgb(0x0B, 0x12, 0x20),
            text_weak: Color32::from_rgb(0x36, 0x41, 0x52),
            border: Color32::from_rgb(0xC9, 0xD1, 0xDC),
            border_strong: Color32::from_rgb(0x6B, 0x76, 0x86),
            accent: Color32::from_rgb(0x1A, 0x55, 0xD6),
            on_accent: Color32::WHITE,
            accent_soft: Color32::from_rgb(0xDC, 0xE7, 0xFD),
            shadow: Color32::from_black_alpha(38),
        }
    }
}

/// Current tokens for a context.
pub fn current(ctx: &Context) -> Tokens {
    tokens(ctx.global_style().visuals.dark_mode)
}

/// (foreground, background) of a tone. Foreground on background is >= 7:1.
pub fn tone_colors(tone: Tone, dark: bool) -> (Color32, Color32) {
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    if dark {
        match tone {
            Tone::Neutral => (c(0xDD, 0xE3, 0xEC), c(0x2A, 0x33, 0x42)),
            Tone::Accent => (c(0xB7, 0xD0, 0xFF), c(0x1B, 0x33, 0x5C)),
            Tone::Info => (c(0xA6, 0xE1, 0xFF), c(0x10, 0x34, 0x4A)),
            Tone::Ok => (c(0x95, 0xEB, 0xB6), c(0x10, 0x36, 0x21)),
            Tone::Warn => (c(0xFF, 0xD5, 0x8F), c(0x3D, 0x2B, 0x0B)),
            Tone::Danger => (c(0xFF, 0xAC, 0xAC), c(0x43, 0x16, 0x16)),
            Tone::Violet => (c(0xD4, 0xBF, 0xFF), c(0x2E, 0x1F, 0x50)),
            Tone::Teal => (c(0x8F, 0xEB, 0xDF), c(0x0E, 0x36, 0x33)),
        }
    } else {
        match tone {
            Tone::Neutral => (c(0x1F, 0x29, 0x37), c(0xE4, 0xE9, 0xF0)),
            Tone::Accent => (c(0x0E, 0x3A, 0x9C), c(0xDC, 0xE7, 0xFD)),
            Tone::Info => (c(0x07, 0x4A, 0x6E), c(0xD6, 0xF0, 0xFC)),
            Tone::Ok => (c(0x0B, 0x55, 0x2A), c(0xD4, 0xF3, 0xE0)),
            Tone::Warn => (c(0x6E, 0x3A, 0x00), c(0xFD, 0xE8, 0xC4)),
            Tone::Danger => (c(0x8A, 0x10, 0x10), c(0xFB, 0xDB, 0xDB)),
            Tone::Violet => (c(0x45, 0x1A, 0x8F), c(0xEA, 0xE0, 0xFD)),
            Tone::Teal => (c(0x0A, 0x4D, 0x47), c(0xD2, 0xF4, 0xEF)),
        }
    }
}

/// A saturated colour for accent bars, chart lines and avatars (white or dark text readable on it).
pub fn tone_solid(tone: Tone, dark: bool) -> Color32 {
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    match (tone, dark) {
        (Tone::Neutral, false) => c(0x5B, 0x67, 0x78),
        (Tone::Neutral, true) => c(0x8D, 0x99, 0xAB),
        (Tone::Accent, false) => c(0x1A, 0x55, 0xD6),
        (Tone::Accent, true) => c(0x74, 0xA7, 0xFF),
        (Tone::Info, false) => c(0x08, 0x7C, 0xB4),
        (Tone::Info, true) => c(0x4F, 0xC3, 0xF7),
        (Tone::Ok, false) => c(0x15, 0x80, 0x3D),
        (Tone::Ok, true) => c(0x4A, 0xDE, 0x80),
        (Tone::Warn, false) => c(0xC2, 0x6A, 0x00),
        (Tone::Warn, true) => c(0xFB, 0xBF, 0x24),
        (Tone::Danger, false) => c(0xC6, 0x22, 0x22),
        (Tone::Danger, true) => c(0xF8, 0x71, 0x71),
        (Tone::Violet, false) => c(0x6D, 0x28, 0xD9),
        (Tone::Violet, true) => c(0xA7, 0x8B, 0xFA),
        (Tone::Teal, false) => c(0x0F, 0x76, 0x6E),
        (Tone::Teal, true) => c(0x2D, 0xD4, 0xBF),
    }
}

/// Named font families registered by [`install_fonts`].
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

fn system_font(names: &[&str]) -> Option<Vec<u8>> {
    let dirs: Vec<std::path::PathBuf> = [
        std::env::var("WINDIR")
            .ok()
            .map(|w| std::path::Path::new(&w).join("Fonts")),
        Some(std::path::PathBuf::from("C:\\Windows\\Fonts")),
        Some(std::path::PathBuf::from("/usr/share/fonts/truetype/dejavu")),
        Some(std::path::PathBuf::from("/System/Library/Fonts")),
    ]
    .into_iter()
    .flatten()
    .collect();
    names
        .iter()
        .find_map(|n| dirs.iter().find_map(|d| std::fs::read(d.join(n)).ok()))
}

/// Fonts: Segoe UI (regular, semibold, bold) and Consolas when present, egui's
/// built-in faces as fallback. The thin default UI face is never the primary.
pub fn install_fonts(ctx: &Context) {
    let mut defs = FontDefinitions::default();
    let fallback_prop = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let fallback_mono = defs
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    let mut add = |key: &str, files: &[&str]| -> bool {
        match system_font(files) {
            Some(bytes) => {
                defs.font_data
                    .insert(key.to_string(), Arc::new(FontData::from_owned(bytes)));
                true
            }
            None => false,
        }
    };
    let regular = add("ui-regular", &["segoeui.ttf", "DejaVuSans.ttf", "SFNS.ttf"]);
    let semi = add("ui-semibold", &["seguisb.ttf", "DejaVuSans-Bold.ttf"]);
    let boldf = add("ui-bold", &["segoeuib.ttf", "DejaVuSans-Bold.ttf"]);
    let mono = add("ui-mono", &["consola.ttf", "DejaVuSansMono.ttf"]);
    let sym = add("ui-symbol", &["seguisym.ttf"]);
    let chain = |first: Option<&str>| -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        if let Some(f) = first {
            v.push(f.to_string());
        }
        if regular {
            v.push("ui-regular".into());
        }
        if sym {
            v.push("ui-symbol".into());
        }
        for f in &fallback_prop {
            if !v.contains(f) {
                v.push(f.clone());
            }
        }
        v
    };
    defs.families.insert(FontFamily::Proportional, chain(None));
    defs.families
        .insert(semibold(), chain(semi.then_some("ui-semibold")));
    defs.families.insert(
        bold(),
        chain(boldf.then_some("ui-bold").or(semi.then_some("ui-semibold"))),
    );
    let mut m: Vec<String> = Vec::new();
    if mono {
        m.push("ui-mono".into());
    }
    m.extend(fallback_mono);
    if sym {
        m.push("ui-symbol".into());
    }
    defs.families.insert(FontFamily::Monospace, m);
    ctx.set_fonts(defs);
}

/// Text sizes and spacing shared by all suite apps.
fn style_common(style: &mut egui::Style) {
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(12.5, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.5, FontFamily::Proportional),
        ),
        (TextStyle::Heading, FontId::new(21.0, semibold())),
        (
            TextStyle::Monospace,
            FontId::new(13.5, FontFamily::Monospace),
        ),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.window_margin = Margin::same(14);
    style.spacing.menu_margin = Margin::same(8);
}

pub fn visuals(dark: bool) -> Visuals {
    let t = tokens(dark);
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.override_text_color = Some(t.text);
    v.weak_text_color = Some(t.text_weak);
    v.panel_fill = t.canvas;
    v.window_fill = t.surface;
    v.window_stroke = Stroke::new(1.0, t.border);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: t.shadow,
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: t.shadow,
    };
    v.faint_bg_color = t.sunken;
    v.extreme_bg_color = t.surface;
    v.text_edit_bg_color = Some(t.surface);
    v.code_bg_color = t.sunken;
    v.hyperlink_color = t.accent;
    v.warn_fg_color = tone_solid(Tone::Warn, dark);
    v.error_fg_color = tone_solid(Tone::Danger, dark);
    v.selection.bg_fill = if dark {
        Color32::from_rgb(0x2A, 0x4C, 0x8A)
    } else {
        Color32::from_rgb(0xB9, 0xD0, 0xFB)
    };
    v.selection.stroke = Stroke::new(1.5, t.accent);
    let r = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = t.surface;
    w.noninteractive.weak_bg_fill = t.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
    w.noninteractive.corner_radius = r;
    let (btn, btn_hover, btn_active) = if dark {
        (
            Color32::from_rgb(0x26, 0x30, 0x3F),
            Color32::from_rgb(0x30, 0x3C, 0x4E),
            Color32::from_rgb(0x3A, 0x48, 0x5D),
        )
    } else {
        (
            Color32::from_rgb(0xE6, 0xEA, 0xF0),
            Color32::from_rgb(0xD9, 0xDF, 0xE8),
            Color32::from_rgb(0xC8, 0xD1, 0xDD),
        )
    };
    w.inactive.bg_fill = btn;
    w.inactive.weak_bg_fill = btn;
    w.inactive.bg_stroke = Stroke::new(1.0, t.border_strong);
    w.inactive.fg_stroke = Stroke::new(1.0, t.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = btn_hover;
    w.hovered.weak_bg_fill = btn_hover;
    w.hovered.bg_stroke = Stroke::new(1.5, t.accent);
    w.hovered.fg_stroke = Stroke::new(1.5, t.text);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = btn_active;
    w.active.weak_bg_fill = btn_active;
    w.active.bg_stroke = Stroke::new(1.5, t.accent);
    w.active.fg_stroke = Stroke::new(2.0, t.text);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;
    w.open.bg_fill = btn_hover;
    w.open.weak_bg_fill = btn_hover;
    w.open.bg_stroke = Stroke::new(1.0, t.accent);
    w.open.fg_stroke = Stroke::new(1.0, t.text);
    w.open.corner_radius = r;
    v.striped = true;
    v.indent_has_left_vline = false;
    v
}

/// Apply theme and type scale to a context (call when dark mode changes).
pub fn apply(ctx: &Context, dark: bool) {
    let theme = if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.all_styles_mut(style_common);
    ctx.set_visuals_of(theme, visuals(dark));
    ctx.set_theme(theme);
}

/// WCAG relative-luminance contrast ratio of two colours (1.0 to 21.0).
pub fn contrast(a: Color32, b: Color32) -> f32 {
    fn lum(c: Color32) -> f32 {
        let ch = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    }
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_contrast_is_high_in_both_modes() {
        for dark in [false, true] {
            let t = tokens(dark);
            for bg in [t.surface, t.canvas, t.sunken] {
                assert!(
                    contrast(t.text, bg) >= 12.0,
                    "text on bg dark={dark}: {}",
                    contrast(t.text, bg)
                );
                assert!(
                    contrast(t.text_weak, bg) >= 7.0,
                    "weak text dark={dark}: {}",
                    contrast(t.text_weak, bg)
                );
            }
            assert!(
                contrast(t.border_strong, t.surface) >= 3.0,
                "input borders visible dark={dark}"
            );
            assert!(
                contrast(t.on_accent, t.accent) >= 4.5,
                "button text dark={dark}"
            );
            assert!(contrast(t.accent, t.surface) >= 4.5, "links dark={dark}");
            for tone in Tone::ALL {
                let (fg, bg) = tone_colors(tone, dark);
                assert!(
                    contrast(fg, bg) >= 7.0,
                    "{tone:?} pill dark={dark}: {}",
                    contrast(fg, bg)
                );
                assert!(
                    contrast(fg, t.surface) >= 4.5,
                    "{tone:?} text on surface dark={dark}"
                );
            }
            let v = visuals(dark);
            assert!(
                contrast(
                    v.widgets.inactive.fg_stroke.color,
                    v.widgets.inactive.bg_fill
                ) >= 7.0,
                "button label dark={dark}"
            );
        }
    }
}
