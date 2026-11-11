//! Data table: sortable columns, quick filter, virtual scrolling, selection,
//! pills and amounts in cells, CSV export of the current view.

use crate::theme::{self, semibold, Tone};
use crate::widgets;
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align2, CornerRadius, FontFamily, FontId, Id, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
};

#[derive(Debug, Clone)]
pub struct Col {
    pub title: String,
    /// Relative width; columns share the available width by weight.
    pub weight: f32,
    /// Right-aligned, sorted numerically (DE amounts understood).
    pub numeric: bool,
    /// Never narrower than this (px).
    pub min: f32,
    /// 0 = always shown. When the table is too narrow, the column with the
    /// highest priority number is hidden first.
    pub prio: u8,
}

fn auto_min(title: &str, weight: f32) -> f32 {
    (title.chars().count() as f32 * 7.6 + 34.0)
        .max(weight * 52.0)
        .max(52.0)
}

impl Col {
    pub fn new(title: impl Into<String>, weight: f32) -> Col {
        let title = title.into();
        let min = auto_min(&title, weight);
        Col {
            title,
            weight,
            numeric: false,
            min,
            prio: 0,
        }
    }
    pub fn num(title: impl Into<String>, weight: f32) -> Col {
        let title = title.into();
        let min = auto_min(&title, weight);
        Col {
            title,
            weight,
            numeric: true,
            min,
            prio: 0,
        }
    }
    pub fn min(mut self, px: f32) -> Col {
        self.min = px;
        self
    }
    /// Hide this column early when space is short (higher = earlier).
    pub fn prio(mut self, p: u8) -> Col {
        self.prio = p;
        self
    }
}

/// Column widths for `avail` pixels: hides low-priority columns until the
/// minimum widths fit, then shares the rest by weight. Hidden columns get 0.
pub fn fit_widths(cols: &[Col], avail: f32) -> Vec<f32> {
    let n = cols.len();
    let mut visible = vec![true; n];
    loop {
        let need: f32 = (0..n).filter(|i| visible[*i]).map(|i| cols[i].min).sum();
        if need <= avail {
            break;
        }
        match (0..n)
            .filter(|i| visible[*i] && cols[*i].prio > 0)
            .max_by_key(|i| (cols[*i].prio, *i))
        {
            Some(i) => visible[i] = false,
            None => break,
        }
    }
    let weight: f32 = (0..n)
        .filter(|i| visible[*i])
        .map(|i| cols[i].weight)
        .sum::<f32>()
        .max(0.001);
    let mut w: Vec<f32> = (0..n)
        .map(|i| {
            if visible[i] {
                avail * cols[i].weight / weight
            } else {
                0.0
            }
        })
        .collect();
    // Columns below their minimum take it; the others give back proportionally.
    for _ in 0..3 {
        let short: f32 = (0..n)
            .filter(|i| visible[*i] && w[*i] < cols[*i].min)
            .map(|i| cols[i].min - w[i])
            .sum();
        if short <= 0.5 {
            break;
        }
        let spare: f32 = (0..n)
            .filter(|i| visible[*i] && w[*i] > cols[*i].min)
            .map(|i| w[i] - cols[i].min)
            .sum();
        for i in 0..n {
            if !visible[i] {
                continue;
            }
            if w[i] < cols[i].min {
                w[i] = cols[i].min;
            } else if spare > 0.0 {
                w[i] -= (w[i] - cols[i].min) / spare * short.min(spare);
            }
        }
    }
    w
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Text(String),
    Strong(String),
    Weak(String),
    Mono(String),
    /// DE amount text; red when negative.
    Money(String),
    Pill(String, Tone),
    Pills(Vec<(String, Tone)>),
    Person(String),
    /// Values 0..1 or any range; drawn as a small line.
    Spark(Vec<f32>, Tone),
    /// A coloured status dot (tooltip-free, for a narrow first column).
    Dot(Tone),
    Empty,
}

impl Cell {
    pub fn text(&self) -> String {
        match self {
            Cell::Text(s)
            | Cell::Strong(s)
            | Cell::Weak(s)
            | Cell::Mono(s)
            | Cell::Money(s)
            | Cell::Pill(s, _)
            | Cell::Person(s) => s.clone(),
            Cell::Pills(v) => v
                .iter()
                .map(|(s, _)| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            Cell::Spark(v, _) => v.last().map(|x| format!("{x}")).unwrap_or_default(),
            Cell::Dot(_) => String::new(),
            Cell::Empty => String::new(),
        }
    }
}

/// Parse "1.234,56", "-12,5", "42", "81%" for numeric sorting.
pub fn sort_number(s: &str) -> Option<f64> {
    let t: String = s
        .trim()
        .trim_end_matches(['%', ' '])
        .trim_end_matches("EUR")
        .trim()
        .replace('.', "")
        .replace(',', ".");
    t.parse::<f64>().ok()
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableEvent {
    Select(String),
    Open(String),
}

#[derive(Clone, Default)]
struct Mem {
    sort: Option<(usize, bool)>,
    filter: String,
}

pub struct Table<'a> {
    id: Id,
    cols: &'a [Col],
    rows: &'a [(String, Vec<Cell>)],
    selected: Option<&'a str>,
    filter_box: bool,
    row_h: f32,
}

impl<'a> Table<'a> {
    /// `rows`: (row id, cells). Cells must match `cols` in length.
    pub fn new(
        id_salt: impl std::hash::Hash + std::fmt::Debug,
        cols: &'a [Col],
        rows: &'a [(String, Vec<Cell>)],
    ) -> Table<'a> {
        Table {
            id: Id::unique(("craft_table", id_salt)),
            cols,
            rows,
            selected: None,
            filter_box: true,
            row_h: 34.0,
        }
    }
    pub fn selected(mut self, id: Option<&'a str>) -> Self {
        self.selected = id;
        self
    }
    pub fn filter_box(mut self, on: bool) -> Self {
        self.filter_box = on;
        self
    }

    /// Rows in display order (after filter and sort), as indices into `rows`.
    fn order(&self, mem: &Mem) -> Vec<usize> {
        let f = mem.filter.trim().to_lowercase();
        let mut idx: Vec<usize> = (0..self.rows.len())
            .filter(|i| {
                f.is_empty()
                    || self.rows[*i]
                        .1
                        .iter()
                        .any(|c| c.text().to_lowercase().contains(&f))
            })
            .collect();
        if let Some((col, asc)) = mem.sort {
            let numeric = self.cols.get(col).is_some_and(|c| c.numeric);
            idx.sort_by(|a, b| {
                let ta = self.rows[*a].1.get(col).map(Cell::text).unwrap_or_default();
                let tb = self.rows[*b].1.get(col).map(Cell::text).unwrap_or_default();
                let o = if numeric {
                    sort_number(&ta)
                        .unwrap_or(f64::MIN)
                        .total_cmp(&sort_number(&tb).unwrap_or(f64::MIN))
                } else {
                    ta.to_lowercase().cmp(&tb.to_lowercase())
                };
                if asc {
                    o
                } else {
                    o.reverse()
                }
            });
        }
        idx
    }

    /// CSV (semicolon, Excel-DE friendly) of the current view.
    pub fn csv(&self) -> String {
        let mem: Mem = Mem::default();
        let esc = |s: &str| {
            if s.contains([';', '"', '\n']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        };
        let mut out = self
            .cols
            .iter()
            .map(|c| esc(&c.title))
            .collect::<Vec<_>>()
            .join(";");
        out.push('\n');
        for i in self.order(&mem) {
            out.push_str(
                &self.rows[i]
                    .1
                    .iter()
                    .map(|c| esc(&c.text()))
                    .collect::<Vec<_>>()
                    .join(";"),
            );
            out.push('\n');
        }
        out
    }

    pub fn show(self, ui: &mut Ui) -> Vec<TableEvent> {
        let mut events = Vec::new();
        let ctx = ui.ctx().clone();
        let t = theme::current(&ctx);
        let mut mem: Mem = ctx.data(|d| d.get_temp(self.id)).unwrap_or_default();
        if self.filter_box {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut mem.filter)
                        .hint_text("Filter...")
                        .desired_width(260.0),
                );
                if !mem.filter.is_empty() && ui.button("Clear").clicked() {
                    mem.filter.clear();
                }
                let n = self.order(&mem).len();
                ui.label(RichText::new(format!("{n} of {}", self.rows.len())).color(t.text_weak));
            });
            ui.add_space(4.0);
        }
        let order = self.order(&mem);
        let avail = ui.available_width().max(200.0);
        let widths = fit_widths(self.cols, avail);
        let total_w = widths.iter().sum::<f32>().max(avail);

        // Header
        let (hrect, _) = ui.allocate_exact_size(Vec2::new(total_w, 32.0), Sense::hover());
        ui.painter().rect_filled(
            hrect,
            CornerRadius {
                nw: 8,
                ne: 8,
                sw: 0,
                se: 0,
            },
            t.sunken,
        );
        ui.painter().line_segment(
            [hrect.left_bottom(), hrect.right_bottom()],
            Stroke::new(1.0, t.border_strong),
        );
        let mut x = hrect.left();
        for (i, c) in self.cols.iter().enumerate() {
            if widths[i] <= 0.0 {
                continue;
            }
            let r = Rect::from_min_size(
                Pos2::new(x, hrect.top()),
                Vec2::new(widths[i], hrect.height()),
            );
            let resp = ui
                .interact(r, self.id.with(("h", i)), Sense::click())
                .on_hover_text("Sort")
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                mem.sort = match mem.sort {
                    Some((k, true)) if k == i => Some((i, false)),
                    Some((k, false)) if k == i => None,
                    _ => Some((i, true)),
                };
            }
            let arrow = match mem.sort {
                Some((k, true)) if k == i => " \u{25B2}",
                Some((k, false)) if k == i => " \u{25BC}",
                _ => "",
            };
            let (pos, align) = if c.numeric {
                (
                    r.right_center() - Vec2::new(10.0, 0.0),
                    Align2::RIGHT_CENTER,
                )
            } else {
                (r.left_center() + Vec2::new(10.0, 0.0), Align2::LEFT_CENTER)
            };
            ui.painter().text(
                pos,
                align,
                format!("{}{arrow}", c.title),
                FontId::new(13.0, semibold()),
                t.text,
            );
            x += widths[i];
        }

        if order.is_empty() {
            widgets::empty_state(
                ui,
                "Nothing here",
                if mem.filter.is_empty() {
                    "No rows yet."
                } else {
                    "No row matches the filter."
                },
            );
        }
        let row_h = self.row_h;
        egui::ScrollArea::vertical()
            .id_salt(self.id.with("rows"))
            .auto_shrink([false, true])
            .show_rows(ui, row_h, order.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (n, &ri) in order.iter().enumerate().skip(range.start).take(range.len()) {
                    let (id, cells) = &self.rows[ri];
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(total_w, row_h), Sense::click());
                    let selected = self.selected == Some(id.as_str());
                    let bg = if selected {
                        t.accent_soft
                    } else if resp.hovered() {
                        if t.dark {
                            t.sunken
                        } else {
                            egui::Color32::from_rgb(0xEA, 0xF0, 0xFA)
                        }
                    } else if n % 2 == 1 {
                        t.sunken
                    } else {
                        t.surface
                    };
                    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
                    if selected {
                        ui.painter().rect_filled(
                            Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
                            CornerRadius::ZERO,
                            t.accent,
                        );
                    }
                    ui.painter().line_segment(
                        [rect.left_bottom(), rect.right_bottom()],
                        Stroke::new(1.0, t.border),
                    );
                    let mut x = rect.left();
                    for (i, cell) in cells.iter().enumerate().take(self.cols.len()) {
                        if widths[i] <= 0.0 {
                            continue;
                        }
                        let r = Rect::from_min_size(
                            Pos2::new(x, rect.top()),
                            Vec2::new(widths[i], row_h),
                        )
                        .shrink2(Vec2::new(10.0, 0.0));
                        paint_cell(ui, r, cell, self.cols[i].numeric);
                        x += widths[i];
                    }
                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.double_clicked() {
                        events.push(TableEvent::Open(id.clone()));
                    } else if resp.clicked() {
                        events.push(TableEvent::Select(id.clone()));
                    }
                }
            });
        ctx.data_mut(|d| d.insert_temp(self.id, mem));
        events
    }
}

fn paint_cell(ui: &Ui, r: Rect, cell: &Cell, numeric: bool) {
    let t = theme::current(ui.ctx());
    let painter = ui.painter_at(r);
    let text = |s: &str, font: FontId, color: egui::Color32| {
        let mut job = LayoutJob::simple_singleline(s.to_string(), font, color);
        job.wrap = TextWrapping::truncate_at_width(r.width());
        let g = painter.layout_job(job);
        let pos = if numeric {
            Pos2::new(r.right() - g.size().x, r.center().y - g.size().y / 2.0)
        } else {
            Pos2::new(r.left(), r.center().y - g.size().y / 2.0)
        };
        painter.galley(pos, g, color);
    };
    match cell {
        Cell::Text(s) => text(s, FontId::proportional(14.0), t.text),
        Cell::Strong(s) => text(s, FontId::new(14.0, semibold()), t.text),
        Cell::Weak(s) => text(s, FontId::proportional(13.5), t.text_weak),
        Cell::Mono(s) => text(s, FontId::new(13.0, FontFamily::Monospace), t.text),
        Cell::Money(s) => {
            let neg = s.trim_start().starts_with('-');
            text(
                s,
                FontId::new(13.5, FontFamily::Monospace),
                if neg {
                    theme::tone_solid(Tone::Danger, t.dark)
                } else {
                    t.text
                },
            )
        }
        Cell::Pill(s, tone) => {
            let size = widgets::pill_size(&painter, s);
            let x = if numeric {
                r.right() - size.x
            } else {
                r.left()
            };
            widgets::paint_pill(
                &painter,
                Pos2::new(x, r.center().y - size.y / 2.0),
                s,
                *tone,
                t.dark,
            );
        }
        Cell::Pills(v) => {
            let mut x = r.left();
            for (s, tone) in v {
                let size = widgets::pill_size(&painter, s);
                widgets::paint_pill(
                    &painter,
                    Pos2::new(x, r.center().y - size.y / 2.0),
                    s,
                    *tone,
                    t.dark,
                );
                x += size.x + 4.0;
            }
        }
        Cell::Person(s) => {
            widgets::paint_avatar(
                &painter,
                Pos2::new(r.left() + 11.0, r.center().y),
                11.0,
                s,
                t.dark,
            );
            let rr = Rect::from_min_max(Pos2::new(r.left() + 28.0, r.top()), r.max);
            let mut job =
                LayoutJob::simple_singleline(s.clone(), FontId::proportional(14.0), t.text);
            job.wrap = TextWrapping::truncate_at_width(rr.width());
            let g = painter.layout_job(job);
            painter.galley(
                Pos2::new(rr.left(), r.center().y - g.size().y / 2.0),
                g,
                t.text,
            );
        }
        Cell::Spark(v, tone) if v.len() >= 2 => {
            let (lo, hi) = v
                .iter()
                .fold((f32::MAX, f32::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
            let span = (hi - lo).max(1e-6);
            let w = r.width().min(110.0);
            let pts: Vec<Pos2> = v
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    Pos2::new(
                        r.left() + w * i as f32 / (v.len() - 1) as f32,
                        r.bottom() - 9.0 - (r.height() - 18.0) * (x - lo) / span,
                    )
                })
                .collect();
            painter.add(egui::Shape::line(
                pts,
                Stroke::new(2.0, theme::tone_solid(*tone, t.dark)),
            ));
        }
        Cell::Dot(tone) => {
            painter.circle_filled(
                egui::pos2(r.left() + 6.0, r.center().y),
                5.5,
                theme::tone_solid(*tone, t.dark),
            );
        }
        Cell::Spark(..) | Cell::Empty => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_sort_in_de_format() {
        assert_eq!(sort_number("1.234,56"), Some(1234.56));
        assert_eq!(sort_number("-12,5"), Some(-12.5));
        assert_eq!(sort_number("81%"), Some(81.0));
        assert_eq!(sort_number("abc"), None);
    }

    #[test]
    fn narrow_tables_hide_low_priority_columns_first() {
        let cols = [
            Col::new("Account", 2.4),
            Col::new("Health", 1.0),
            Col::num("Score", 0.6),
            Col::new("Trend", 1.0).prio(1),
            Col::num("Seats", 0.7).prio(2),
            Col::new("Notice", 1.0).prio(2),
        ];
        let wide = fit_widths(&cols, 1400.0);
        assert!(wide.iter().all(|w| *w > 0.0));
        assert!((wide.iter().sum::<f32>() - 1400.0).abs() < 1.0);
        let narrow = fit_widths(&cols, 420.0);
        assert_eq!(narrow[5], 0.0, "Notice (prio 2, last) goes first");
        assert_eq!(narrow[4], 0.0, "then Seats");
        assert!(
            narrow[0] > 0.0 && narrow[2] >= cols[2].min - 0.5,
            "Score keeps its minimum"
        );
        for (i, w) in narrow.iter().enumerate() {
            assert!(
                *w == 0.0 || *w + 0.5 >= cols[i].min,
                "col {i} below min: {w}"
            );
        }
    }

    #[test]
    fn csv_escapes() {
        let cols = [Col::new("Name", 1.0), Col::num("ARR", 1.0)];
        let rows = vec![(
            "1".to_string(),
            vec![Cell::Text("A;B".into()), Cell::Money("9.600,00".into())],
        )];
        let t = Table::new("t", &cols, &rows);
        assert_eq!(t.csv(), "Name;ARR\n\"A;B\";9.600,00\n");
    }
}
