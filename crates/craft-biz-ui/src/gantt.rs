//! Gantt chart in the suite look: the standalone `egui_gantt` widget with
//! colours taken from [`crate::theme`].
//!
//! ```no_run
//! # fn demo(ui: &mut egui::Ui, model: &egui_gantt::GanttModel, state: &mut egui_gantt::GanttState) {
//! let style = craft_biz_ui::gantt::current(ui.ctx());
//! let out = egui_gantt::Gantt::new(model).style(&style).show(ui, state);
//! # let _ = out;
//! # }
//! ```

use crate::theme::{self, Tone};
use egui::Context;
pub use egui_gantt;
use egui_gantt::GanttStyle;

/// Gantt colours for the suite theme in light or dark mode.
///
/// Surfaces, text, grid, accent and danger come from the theme tokens; the
/// calendar tints (weekend, holiday, closure) and the today line keep the
/// widget's own high-contrast values, which are distinct from the theme's
/// stripes.
#[must_use]
pub fn style(dark: bool) -> GanttStyle {
    let t = theme::tokens(dark);
    let (accent_fg, _) = theme::tone_colors(Tone::Accent, dark);
    let mut s = if dark {
        GanttStyle::dark()
    } else {
        GanttStyle::light()
    };
    s.background = t.surface;
    s.row_alt = t.sunken;
    s.header_background = t.canvas;
    s.grid = t.border;
    s.text = t.text;
    s.text_weak = t.text_weak;
    s.bar = theme::tone_solid(Tone::Accent, dark);
    s.bar_progress = accent_fg;
    s.bar_border = accent_fg;
    s.critical = theme::tone_solid(Tone::Danger, dark);
    s.milestone = t.text;
    s.summary = t.text;
    s.link = t.text_weak;
    s.selection = t.accent_soft;
    s.selection_stroke = t.accent;
    s
}

/// Gantt colours for the context's current mode.
#[must_use]
pub fn current(ctx: &Context) -> GanttStyle {
    style(ctx.global_style().visuals.dark_mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_gantt::style::contrast_ratio;

    /// Same bar as the widget's own styles: text 7:1 on every background it
    /// can sit on, bars, arrows and markers 3:1.
    #[test]
    fn suite_gantt_style_keeps_contrast() {
        for dark in [false, true] {
            let s = style(dark);
            let backgrounds = [
                s.background,
                s.row_alt,
                s.header_background,
                s.weekend,
                s.holiday,
                s.closure,
                s.selection,
            ];
            for bg in backgrounds {
                assert!(
                    contrast_ratio(s.text, bg) >= 7.0,
                    "dark={dark} text on {bg:?}"
                );
                assert!(
                    contrast_ratio(s.text_weak, bg) >= 7.0,
                    "dark={dark} weak text on {bg:?}"
                );
            }
            for bg in [s.background, s.row_alt, s.weekend, s.holiday, s.closure] {
                for fg in [s.bar_border, s.critical, s.milestone, s.today, s.link] {
                    assert!(
                        contrast_ratio(fg, bg) >= 3.0,
                        "dark={dark} {fg:?} on {bg:?}"
                    );
                }
            }
        }
    }
}
