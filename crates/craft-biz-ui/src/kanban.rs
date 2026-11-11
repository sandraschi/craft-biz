//! Kanban board for egui: columns from a workflow, optional swimlanes,
//! drag and drop with an insertion line, workflow rules that refuse a drop
//! with a visible reason, WIP limits, collapsible columns, right-click
//! "Move to", keyboard moves and undo.
//!
//! The board never changes data itself: it returns [`Event`]s and the app
//! turns them into `Command`s (so CLI, MCP and window share one code path).

use crate::theme::{self, semibold, Tokens, Tone};
use crate::widgets;
use egui::text::LayoutJob;
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontFamily, FontId, Galley, Id, Key, LayerId,
    Modifiers, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Column {
    pub id: String,
    pub title: String,
    pub tone: Tone,
    pub wip_limit: Option<usize>,
}

impl Column {
    pub fn new(id: impl Into<String>, title: impl Into<String>, tone: Tone) -> Column {
        Column {
            id: id.into(),
            title: title.into(),
            tone,
            wip_limit: None,
        }
    }
    pub fn wip(mut self, limit: usize) -> Column {
        self.wip_limit = Some(limit);
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct Lane {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Default)]
pub struct Card {
    pub id: String,
    pub column: String,
    /// Swimlane id ("" when the board has no lanes).
    pub lane: String,
    /// Short key shown above the title, e.g. "OPS-12".
    pub key: String,
    pub title: String,
    pub tags: Vec<(String, Tone)>,
    /// Colour of the left bar (priority, type, band).
    pub accent: Tone,
    /// Small pill top-right, e.g. ("High", Warn).
    pub badge: Option<(String, Tone)>,
    pub assignee: Option<String>,
    /// Short facts in the footer: due date, estimate, amount.
    pub meta: Vec<String>,
    /// Shown as a red "Blocked" marker with this reason as tooltip.
    pub blocked: Option<String>,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Moved {
        card: String,
        from: String,
        to: String,
        from_lane: String,
        to_lane: String,
        index: usize,
    },
    Refused {
        card: String,
        to: String,
        reason: String,
    },
    Select(String),
    Open(String),
    Add {
        column: String,
        lane: String,
    },
    Undo,
}

/// Workflow rule: may this card go into that column? `Err(reason)` refuses.
pub type Rule<'a> = &'a dyn Fn(&Card, &str) -> Result<(), String>;

#[derive(Clone)]
struct Drag {
    card: String,
    column: String,
    lane: String,
}

#[derive(Clone, Default)]
struct Mem {
    collapsed: HashSet<String>,
    grab: Vec2,
    cards: HashMap<String, Rect>,
    cells: HashMap<(String, String), Rect>,
}

fn board_id(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Id {
    Id::unique(("craft_kanban", id_salt))
}

/// Where a card was drawn last frame (screen coordinates). For tests, scroll-to and automation.
pub fn card_rect(
    ctx: &egui::Context,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    card: &str,
) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Mem>(board_id(id_salt)))
        .and_then(|m| m.cards.get(card).copied())
}

/// Where a column cell (column id, lane id) was drawn last frame.
pub fn cell_rect(
    ctx: &egui::Context,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    column: &str,
    lane: &str,
) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Mem>(board_id(id_salt)))
        .and_then(|m| {
            m.cells
                .get(&(column.to_string(), lane.to_string()))
                .copied()
        })
}

pub struct Board<'a> {
    id: Id,
    columns: &'a [Column],
    cards: &'a [Card],
    lanes: &'a [Lane],
    rule: Option<Rule<'a>>,
    selected: Option<&'a str>,
    col_width: f32,
    col_width_fixed: bool,
    add_buttons: bool,
    keyboard: bool,
}

const PAD: f32 = 10.0;
const GAP: f32 = 8.0;
const COLLAPSED_W: f32 = 46.0;

struct Layout {
    key: Arc<Galley>,
    title: Arc<Galley>,
    chips: Vec<(Vec2, String, Tone)>,
    meta: Option<Arc<Galley>>,
    badge_w: f32,
    height: f32,
}

impl<'a> Board<'a> {
    pub fn new(
        id_salt: impl std::hash::Hash + std::fmt::Debug,
        columns: &'a [Column],
        cards: &'a [Card],
    ) -> Board<'a> {
        Board {
            id: board_id(id_salt),
            columns,
            cards,
            lanes: &[],
            rule: None,
            selected: None,
            col_width: 284.0,
            col_width_fixed: false,
            add_buttons: true,
            keyboard: true,
        }
    }
    pub fn lanes(mut self, lanes: &'a [Lane]) -> Self {
        self.lanes = lanes;
        self
    }
    pub fn rule(mut self, rule: Rule<'a>) -> Self {
        self.rule = Some(rule);
        self
    }
    pub fn selected(mut self, id: Option<&'a str>) -> Self {
        self.selected = id;
        self
    }
    pub fn column_width(mut self, w: f32) -> Self {
        self.col_width = w.max(180.0);
        self.col_width_fixed = true;
        self
    }
    pub fn add_buttons(mut self, on: bool) -> Self {
        self.add_buttons = on;
        self
    }
    pub fn keyboard(mut self, on: bool) -> Self {
        self.keyboard = on;
        self
    }

    fn check(&self, card: &Card, to: &str) -> Result<(), String> {
        if card.column == to {
            return Ok(());
        }
        match self.rule {
            Some(r) => r(card, to),
            None => Ok(()),
        }
    }

    fn layout(&self, ui: &Ui, card: &Card, width: f32, t: &Tokens) -> Layout {
        let painter = ui.painter();
        let inner = width - 2.0 * PAD - 4.0;
        let badge_w = card
            .badge
            .as_ref()
            .map(|(b, _)| widgets::pill_size(painter, b).x + 6.0)
            .unwrap_or(0.0)
            + if card.blocked.is_some() {
                widgets::pill_size(painter, "Blocked").x + 6.0
            } else {
                0.0
            };
        let mut kj = LayoutJob::simple_singleline(
            card.key.clone(),
            FontId::new(12.0, FontFamily::Monospace),
            t.text_weak,
        );
        kj.wrap = egui::text::TextWrapping::truncate_at_width((inner - badge_w).max(30.0));
        let key = painter.layout_job(kj);
        let title_color = if card.done { t.text_weak } else { t.text };
        let title = painter.layout(
            card.title.clone(),
            FontId::new(14.5, semibold()),
            title_color,
            inner,
        );
        let mut y = PAD + key.size().y.max(18.0) + 4.0 + title.size().y + 6.0;
        let mut chips = Vec::new();
        if !card.tags.is_empty() {
            let mut x = 0.0;
            let mut row_h: f32 = 0.0;
            for (text, tone) in &card.tags {
                let s = widgets::pill_size(painter, text);
                if x > 0.0 && x + s.x > inner {
                    x = 0.0;
                    y += row_h + 4.0;
                    row_h = 0.0;
                }
                chips.push((Vec2::new(x, y), text.clone(), *tone));
                x += s.x + 4.0;
                row_h = row_h.max(s.y);
            }
            y += row_h + 6.0;
        }
        let meta = (!card.meta.is_empty()).then(|| {
            let mut j = LayoutJob::simple_singleline(
                card.meta.join("  |  "),
                FontId::proportional(12.5),
                t.text_weak,
            );
            j.wrap = egui::text::TextWrapping::truncate_at_width(inner - 30.0);
            painter.layout_job(j)
        });
        if meta.is_some() || card.assignee.is_some() {
            y += 24.0;
        }
        Layout {
            key,
            title,
            chips,
            meta,
            badge_w,
            height: y + PAD - 4.0,
        }
    }

    fn paint_card(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        card: &Card,
        l: &Layout,
        t: &Tokens,
        state: CardState,
    ) {
        let dark = t.dark;
        let lift = matches!(state, CardState::Hover | CardState::Ghost);
        let shadow = egui::epaint::Shadow {
            offset: [0, if lift { 6 } else { 1 }],
            blur: if lift { 18 } else { 3 },
            spread: 0,
            color: t.shadow.gamma_multiply(if lift { 1.2 } else { 0.7 }),
        };
        painter.add(shadow.as_shape(rect, CornerRadius::same(8)));
        painter.rect_filled(rect, CornerRadius::same(8), t.surface);
        let edge = match state {
            CardState::Selected => Stroke::new(2.0, t.accent),
            CardState::Hover | CardState::Ghost => Stroke::new(1.0, t.border_strong),
            CardState::Normal => Stroke::new(1.0, t.border),
        };
        painter.rect_stroke(rect, CornerRadius::same(8), edge, StrokeKind::Inside);
        let accent = if card.blocked.is_some() {
            Tone::Danger
        } else {
            card.accent
        };
        let bar = Rect::from_min_size(rect.min, Vec2::new(4.0, rect.height()));
        painter.rect_filled(
            bar,
            CornerRadius {
                nw: 8,
                sw: 8,
                ne: 0,
                se: 0,
            },
            theme::tone_solid(accent, dark),
        );
        let x0 = rect.left() + PAD + 4.0;
        let mut y = rect.top() + PAD;
        painter.galley(Pos2::new(x0, y + 1.0), l.key.clone(), t.text_weak);
        let mut bx = rect.right() - PAD;
        if let Some((b, tone)) = &card.badge {
            let s = widgets::pill_size(painter, b);
            bx -= s.x;
            widgets::paint_pill(painter, Pos2::new(bx, y - 1.0), b, *tone, dark);
            bx -= 6.0;
        }
        if card.blocked.is_some() {
            let s = widgets::pill_size(painter, "Blocked");
            bx -= s.x;
            widgets::paint_pill(
                painter,
                Pos2::new(bx, y - 1.0),
                "Blocked",
                Tone::Danger,
                dark,
            );
        }
        let _ = l.badge_w;
        y += l.key.size().y.max(18.0) + 4.0;
        if card.done {
            painter.text(
                Pos2::new(rect.right() - PAD, rect.bottom() - PAD),
                Align2::RIGHT_BOTTOM,
                "\u{2713}",
                FontId::new(15.0, semibold()),
                theme::tone_solid(Tone::Ok, dark),
            );
        }
        painter.galley(Pos2::new(x0, y), l.title.clone(), t.text);
        for (off, text, tone) in &l.chips {
            widgets::paint_pill(
                painter,
                Pos2::new(x0 + off.x, rect.top() + off.y),
                text,
                *tone,
                dark,
            );
        }
        let fy = rect.bottom() - PAD - 10.0;
        if let Some(m) = &l.meta {
            painter.galley(Pos2::new(x0, fy - m.size().y / 2.0), m.clone(), t.text_weak);
        }
        if let Some(a) = &card.assignee {
            let c = Pos2::new(
                rect.right() - PAD - 11.0 - if card.done { 18.0 } else { 0.0 },
                fy,
            );
            widgets::paint_avatar(painter, c, 11.0, a, dark);
        }
    }

    pub fn show(self, ui: &mut Ui) -> Vec<Event> {
        let mut events = Vec::new();
        let ctx = ui.ctx().clone();
        let t = theme::current(&ctx);
        let dark = t.dark;
        let mut mem: Mem = ctx.data(|d| d.get_temp(self.id)).unwrap_or_default();
        let drag: Option<Arc<Drag>> = egui::DragAndDrop::payload::<Drag>(&ctx);
        let dragged_card: Option<&Card> = drag
            .as_ref()
            .and_then(|d| self.cards.iter().find(|c| c.id == d.card));
        let implicit = [Lane::default()];
        let lanes: &[Lane] = if self.lanes.is_empty() {
            &implicit
        } else {
            self.lanes
        };
        // Fit the open columns to the visible width (180..=380 px each); scroll only below 180.
        let open_n = self
            .columns
            .iter()
            .filter(|c| !mem.collapsed.contains(&c.id))
            .count()
            .max(1);
        let closed_w =
            (self.columns.len() - open_n.min(self.columns.len())) as f32 * (COLLAPSED_W + GAP);
        let fit = ((ui.available_width() - closed_w - GAP * open_n as f32 - 14.0) / open_n as f32)
            .clamp(180.0, 380.0);
        let open_w = if self.col_width_fixed {
            self.col_width
        } else {
            fit
        };
        let widths: Vec<f32> = self
            .columns
            .iter()
            .map(|c| {
                if mem.collapsed.contains(&c.id) {
                    COLLAPSED_W
                } else {
                    open_w
                }
            })
            .collect();
        let pointer = ctx.pointer_interact_pos();
        mem.cards.clear();
        mem.cells.clear();

        egui::ScrollArea::both()
            .id_salt(self.id.with("scroll"))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Auto-scroll sideways while dragging near an edge.
                if let (Some(_), Some(p)) = (&drag, ctx.pointer_hover_pos()) {
                    let clip = ui.clip_rect();
                    if p.x > clip.right() - 40.0 {
                        ui.scroll_with_delta(Vec2::new(-12.0, 0.0));
                    } else if p.x < clip.left() + 40.0 {
                        ui.scroll_with_delta(Vec2::new(12.0, 0.0));
                    }
                }
                ui.spacing_mut().item_spacing = Vec2::new(GAP, GAP);

                // Column headers (once, above all lanes).
                ui.horizontal(|ui| {
                    for (ci, col) in self.columns.iter().enumerate() {
                        let w = widths[ci];
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(w, 42.0), Sense::click());
                        let count = self.cards.iter().filter(|c| c.column == col.id).count();
                        let over = col.wip_limit.is_some_and(|l| count > l);
                        let bg = if over {
                            theme::tone_colors(Tone::Danger, dark).1
                        } else {
                            t.sunken
                        };
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius {
                                nw: 8,
                                ne: 8,
                                sw: 2,
                                se: 2,
                            },
                            bg,
                        );
                        let strip = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 3.0));
                        ui.painter().rect_filled(
                            strip,
                            CornerRadius {
                                nw: 8,
                                ne: 8,
                                sw: 0,
                                se: 0,
                            },
                            theme::tone_solid(col.tone, dark),
                        );
                        let collapsed = mem.collapsed.contains(&col.id);
                        let count_text = match col.wip_limit {
                            Some(l) => format!("{count} / {l}"),
                            None => count.to_string(),
                        };
                        if collapsed {
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                &count_text,
                                FontId::new(13.0, semibold()),
                                t.text,
                            );
                        } else {
                            ui.painter().text(
                                rect.left_center() + Vec2::new(12.0, 1.0),
                                Align2::LEFT_CENTER,
                                col.title.to_uppercase(),
                                FontId::new(12.5, semibold()),
                                t.text,
                            );
                            let tx = rect.left()
                                + 12.0
                                + ui.painter()
                                    .layout_no_wrap(
                                        col.title.to_uppercase(),
                                        FontId::new(12.5, semibold()),
                                        t.text,
                                    )
                                    .size()
                                    .x
                                + 8.0;
                            let tone = if over { Tone::Danger } else { Tone::Neutral };
                            let s = widgets::pill_size(ui.painter(), &count_text);
                            widgets::paint_pill(
                                ui.painter(),
                                Pos2::new(tx, rect.center().y - s.y / 2.0 + 1.0),
                                &count_text,
                                tone,
                                dark,
                            );
                        }
                        let resp = resp.on_hover_text(if collapsed {
                            format!("{}: click to expand", col.title)
                        } else {
                            "Click to collapse".to_string()
                        });
                        if self.add_buttons && !collapsed {
                            let plus = Rect::from_center_size(
                                Pos2::new(rect.right() - 20.0, rect.center().y + 1.0),
                                Vec2::splat(24.0),
                            );
                            let pr = ui
                                .interact(plus, self.id.with(("add", ci)), Sense::click())
                                .on_hover_text("Add here")
                                .on_hover_cursor(CursorIcon::PointingHand);
                            if pr.hovered() {
                                ui.painter()
                                    .rect_filled(plus, CornerRadius::same(5), t.surface);
                            }
                            ui.painter().text(
                                plus.center(),
                                Align2::CENTER_CENTER,
                                "+",
                                FontId::new(18.0, semibold()),
                                t.text,
                            );
                            if pr.clicked() {
                                events.push(Event::Add {
                                    column: col.id.clone(),
                                    lane: lanes.first().map(|l| l.id.clone()).unwrap_or_default(),
                                });
                            }
                        }
                        if resp.clicked() && !self.add_buttons_hit(&ctx, rect) {
                            if collapsed {
                                mem.collapsed.remove(&col.id);
                            } else {
                                mem.collapsed.insert(col.id.clone());
                            }
                        }
                    }
                });

                for lane in lanes {
                    if !self.lanes.is_empty() {
                        let n = self.cards.iter().filter(|c| c.lane == lane.id).count();
                        ui.horizontal(|ui| {
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(&lane.title)
                                    .family(semibold())
                                    .size(14.0),
                            );
                            widgets::pill(ui, &n.to_string(), Tone::Neutral);
                        });
                    }
                    // Lay out all cells of the lane first so they share one height.
                    let mut cells: Vec<Vec<(&Card, Layout)>> = Vec::new();
                    for (ci, col) in self.columns.iter().enumerate() {
                        let mut v = Vec::new();
                        if !mem.collapsed.contains(&col.id) {
                            for c in self
                                .cards
                                .iter()
                                .filter(|c| c.column == col.id && c.lane == lane.id)
                            {
                                v.push((c, self.layout(ui, c, widths[ci] - 2.0 * GAP, &t)));
                            }
                        }
                        cells.push(v);
                    }
                    let content_h = cells
                        .iter()
                        .map(|v| v.iter().map(|(_, l)| l.height + GAP).sum::<f32>())
                        .fold(0.0f32, f32::max);
                    let cell_h = (content_h + 2.0 * GAP + 28.0).max(96.0);
                    ui.horizontal(|ui| {
                        for (ci, col) in self.columns.iter().enumerate() {
                            let w = widths[ci];
                            let (cell, cell_resp) =
                                ui.allocate_exact_size(Vec2::new(w, cell_h), Sense::hover());
                            mem.cells.insert((col.id.clone(), lane.id.clone()), cell);
                            let verdict = dragged_card.map(|c| self.check(c, &col.id));
                            let hovering =
                                drag.is_some() && pointer.is_some_and(|p| cell.contains(p));
                            let (fill, stroke) = match (&verdict, hovering) {
                                (Some(Err(_)), true) => (
                                    theme::tone_colors(Tone::Danger, dark).1,
                                    Stroke::new(2.0, theme::tone_solid(Tone::Danger, dark)),
                                ),
                                (Some(Err(_)), false) => (
                                    t.sunken,
                                    Stroke::new(
                                        1.0,
                                        theme::tone_solid(Tone::Danger, dark).gamma_multiply(0.5),
                                    ),
                                ),
                                (Some(Ok(())), true) => (t.accent_soft, Stroke::new(2.0, t.accent)),
                                (Some(Ok(())), false) => {
                                    (t.sunken, Stroke::new(1.0, t.accent.gamma_multiply(0.6)))
                                }
                                (None, _) => {
                                    (t.sunken, Stroke::new(1.0, t.border.gamma_multiply(0.6)))
                                }
                            };
                            ui.painter().rect_filled(
                                cell,
                                CornerRadius {
                                    nw: 2,
                                    ne: 2,
                                    sw: 8,
                                    se: 8,
                                },
                                fill,
                            );
                            ui.painter().rect_stroke(
                                cell,
                                CornerRadius {
                                    nw: 2,
                                    ne: 2,
                                    sw: 8,
                                    se: 8,
                                },
                                stroke,
                                StrokeKind::Inside,
                            );
                            if let (Some(Err(reason)), true) = (&verdict, hovering) {
                                let g = ui.painter().layout(
                                    format!("Not allowed: {reason}"),
                                    FontId::new(13.0, semibold()),
                                    theme::tone_colors(Tone::Danger, dark).0,
                                    w - 24.0,
                                );
                                ui.painter().galley(
                                    cell.left_top() + Vec2::new(12.0, 10.0),
                                    g,
                                    t.text,
                                );
                            }
                            // Cards.
                            let mut y = cell.top() + GAP;
                            let mut mids: Vec<(f32, &str)> = Vec::new();
                            for (card, l) in &cells[ci] {
                                let rect = Rect::from_min_size(
                                    Pos2::new(cell.left() + GAP, y),
                                    Vec2::new(w - 2.0 * GAP, l.height),
                                );
                                y += l.height + GAP;
                                mem.cards.insert(card.id.clone(), rect);
                                let cid = self.id.with(("card", &card.id));
                                let resp = ui
                                    .interact(rect, cid, Sense::click_and_drag())
                                    .on_hover_cursor(CursorIcon::Grab);
                                if resp.drag_started() {
                                    if let Some(p) = resp.interact_pointer_pos() {
                                        mem.grab = p - rect.min;
                                    }
                                }
                                resp.dnd_set_drag_payload(Drag {
                                    card: card.id.clone(),
                                    column: card.column.clone(),
                                    lane: card.lane.clone(),
                                });
                                let being_dragged = ctx.is_being_dragged(cid);
                                if being_dragged {
                                    ui.painter().rect_filled(
                                        rect,
                                        CornerRadius::same(8),
                                        t.accent_soft.gamma_multiply(0.6),
                                    );
                                    ui.painter().rect_stroke(
                                        rect,
                                        CornerRadius::same(8),
                                        Stroke::new(1.5, t.accent.gamma_multiply(0.7)),
                                        StrokeKind::Inside,
                                    );
                                    if let Some(p) = ctx.pointer_interact_pos() {
                                        let ghost = Rect::from_min_size(p - mem.grab, rect.size());
                                        let layer = ctx.layer_painter(LayerId::new(
                                            Order::Tooltip,
                                            self.id.with("ghost"),
                                        ));
                                        self.paint_card(
                                            &layer,
                                            ghost,
                                            card,
                                            l,
                                            &t,
                                            CardState::Ghost,
                                        );
                                    }
                                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                                } else {
                                    mids.push((rect.center().y, card.id.as_str()));
                                    let state = if self.selected == Some(card.id.as_str()) {
                                        CardState::Selected
                                    } else if resp.hovered() && drag.is_none() {
                                        CardState::Hover
                                    } else {
                                        CardState::Normal
                                    };
                                    if ui.is_rect_visible(rect) {
                                        self.paint_card(ui.painter(), rect, card, l, &t, state);
                                    }
                                }
                                if let Some(reason) = &card.blocked {
                                    if resp.hovered() && drag.is_none() {
                                        resp.clone().on_hover_text(format!("Blocked: {reason}"));
                                    }
                                }
                                if resp.double_clicked() {
                                    events.push(Event::Open(card.id.clone()));
                                } else if resp.clicked() {
                                    events.push(Event::Select(card.id.clone()));
                                }
                                resp.context_menu(|ui| {
                                    if ui.button("Open").clicked() {
                                        events.push(Event::Open(card.id.clone()));
                                        ui.close();
                                    }
                                    ui.separator();
                                    ui.label(egui::RichText::new("Move to").family(semibold()));
                                    for target in
                                        self.columns.iter().filter(|c| c.id != card.column)
                                    {
                                        let ok = self.check(card, &target.id);
                                        let r = ui.add_enabled(
                                            ok.is_ok(),
                                            egui::Button::new(&target.title),
                                        );
                                        let r = match &ok {
                                            Err(reason) => r.on_disabled_hover_text(reason),
                                            Ok(()) => r,
                                        };
                                        if r.clicked() {
                                            let index = self
                                                .cards
                                                .iter()
                                                .filter(|c| {
                                                    c.column == target.id && c.lane == card.lane
                                                })
                                                .count();
                                            events.push(Event::Moved {
                                                card: card.id.clone(),
                                                from: card.column.clone(),
                                                to: target.id.clone(),
                                                from_lane: card.lane.clone(),
                                                to_lane: card.lane.clone(),
                                                index,
                                            });
                                            ui.close();
                                        }
                                    }
                                });
                            }
                            if mem.collapsed.contains(&col.id) {
                                let n = self
                                    .cards
                                    .iter()
                                    .filter(|c| c.column == col.id && c.lane == lane.id)
                                    .count();
                                let g = ui.painter().layout_no_wrap(
                                    col.title.clone(),
                                    FontId::new(13.0, semibold()),
                                    t.text_weak,
                                );
                                let mut shape = egui::epaint::TextShape::new(
                                    Pos2::new(
                                        cell.center().x - g.size().y / 2.0,
                                        cell.top() + 14.0 + g.size().x,
                                    ),
                                    g,
                                    t.text_weak,
                                );
                                shape.angle = -std::f32::consts::FRAC_PI_2;
                                ui.painter().add(shape);
                                ui.painter().text(
                                    Pos2::new(cell.center().x, cell.bottom() - 16.0),
                                    Align2::CENTER_CENTER,
                                    n.to_string(),
                                    FontId::new(13.0, semibold()),
                                    t.text,
                                );
                            }
                            // Insertion line and drop.
                            let index = pointer
                                .map(|p| mids.iter().filter(|(m, _)| *m < p.y).count())
                                .unwrap_or(mids.len());
                            if hovering
                                && matches!(verdict, Some(Ok(())))
                                && !mem.collapsed.contains(&col.id)
                            {
                                let line_y = if index < mids.len() {
                                    let first_after = mids[index].0;
                                    let h = cells[ci]
                                        .iter()
                                        .find(|(c, _)| c.id == mids[index].1)
                                        .map(|(_, l)| l.height)
                                        .unwrap_or(0.0);
                                    first_after - h / 2.0 - GAP / 2.0
                                } else {
                                    mids.last()
                                        .map(|(m, id)| {
                                            m + cells[ci]
                                                .iter()
                                                .find(|(c, _)| c.id == *id)
                                                .map(|(_, l)| l.height / 2.0)
                                                .unwrap_or(0.0)
                                                + GAP / 2.0
                                        })
                                        .unwrap_or(cell.top() + GAP)
                                };
                                let a = Pos2::new(cell.left() + GAP, line_y);
                                let b = Pos2::new(cell.right() - GAP, line_y);
                                ui.painter()
                                    .line_segment([a, b], Stroke::new(3.0, t.accent));
                                ui.painter().circle_filled(a, 4.0, t.accent);
                            }
                            if let Some(d) = cell_resp.dnd_release_payload::<Drag>() {
                                if let Some(card) = self.cards.iter().find(|c| c.id == d.card) {
                                    match self.check(card, &col.id) {
                                        Ok(()) => {
                                            let own =
                                                cells[ci].iter().position(|(c, _)| c.id == card.id);
                                            let same_place = d.column == col.id
                                                && d.lane == lane.id
                                                && own == Some(index);
                                            if !same_place {
                                                events.push(Event::Moved {
                                                    card: d.card.clone(),
                                                    from: d.column.clone(),
                                                    to: col.id.clone(),
                                                    from_lane: d.lane.clone(),
                                                    to_lane: lane.id.clone(),
                                                    index,
                                                });
                                            }
                                        }
                                        Err(reason) => events.push(Event::Refused {
                                            card: d.card.clone(),
                                            to: col.id.clone(),
                                            reason,
                                        }),
                                    }
                                }
                            }
                        }
                    });
                    ui.add_space(4.0);
                }
            });

        if self.keyboard {
            self.keys(&ctx, &mut events);
        }
        ctx.data_mut(|d| d.insert_temp(self.id, mem));
        events
    }

    fn add_buttons_hit(&self, ctx: &egui::Context, header: Rect) -> bool {
        self.add_buttons
            && ctx
                .pointer_interact_pos()
                .is_some_and(|p| p.x > header.right() - 34.0)
    }

    fn keys(&self, ctx: &egui::Context, events: &mut Vec<Event>) {
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)) {
            events.push(Event::Undo);
        }
        let Some(sel) = self
            .selected
            .and_then(|s| self.cards.iter().find(|c| c.id == s))
        else {
            return;
        };
        let col_idx = self.columns.iter().position(|c| c.id == sel.column);
        let Some(ci) = col_idx else { return };
        let same: Vec<&Card> = self
            .cards
            .iter()
            .filter(|c| c.column == sel.column && c.lane == sel.lane)
            .collect();
        let pos = same.iter().position(|c| c.id == sel.id).unwrap_or(0);
        let (cmd, left, right, up, down, enter) = ctx.input(|i| {
            (
                i.modifiers.command,
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::Enter),
            )
        });
        let neighbour = |d: isize| -> Option<&Column> {
            self.columns
                .get((ci as isize + d) as usize)
                .filter(|_| ci as isize + d >= 0)
        };
        if enter {
            events.push(Event::Open(sel.id.clone()));
        } else if cmd && (left || right) {
            if let Some(target) = neighbour(if left { -1 } else { 1 }) {
                match self.check(sel, &target.id) {
                    Ok(()) => {
                        let index = self
                            .cards
                            .iter()
                            .filter(|c| c.column == target.id && c.lane == sel.lane)
                            .count();
                        events.push(Event::Moved {
                            card: sel.id.clone(),
                            from: sel.column.clone(),
                            to: target.id.clone(),
                            from_lane: sel.lane.clone(),
                            to_lane: sel.lane.clone(),
                            index,
                        });
                    }
                    Err(reason) => events.push(Event::Refused {
                        card: sel.id.clone(),
                        to: target.id.clone(),
                        reason,
                    }),
                }
            }
        } else if up && pos > 0 {
            events.push(Event::Select(same[pos - 1].id.clone()));
        } else if down && pos + 1 < same.len() {
            events.push(Event::Select(same[pos + 1].id.clone()));
        } else if left || right {
            let mut d: isize = if left { -1 } else { 1 };
            while let Some(target) = neighbour(d) {
                if let Some(c) = self
                    .cards
                    .iter()
                    .find(|c| c.column == target.id && c.lane == sel.lane)
                {
                    events.push(Event::Select(c.id.clone()));
                    break;
                }
                d += if left { -1 } else { 1 };
            }
        }
    }
}

#[derive(Clone, Copy)]
enum CardState {
    Normal,
    Hover,
    Selected,
    Ghost,
}

/// Apply a `Moved` event to a local list (for previews and tests): moves the
/// card into the target column/lane at `index` among that cell's cards.
pub fn apply_move(cards: &mut Vec<Card>, ev: &Event) {
    let Event::Moved {
        card,
        to,
        to_lane,
        index,
        ..
    } = ev
    else {
        return;
    };
    let Some(pos) = cards.iter().position(|c| &c.id == card) else {
        return;
    };
    let mut c = cards.remove(pos);
    c.column = to.clone();
    c.lane = to_lane.clone();
    let mut seen = 0;
    let mut at = cards.len();
    for (i, x) in cards.iter().enumerate() {
        if x.column == *to && x.lane == *to_lane {
            if seen == *index {
                at = i;
                break;
            }
            seen += 1;
            at = i + 1;
        }
    }
    cards.insert(at.min(cards.len()), c);
}

/// Placeholder colour for code that wants the board background.
pub fn board_background(t: &Tokens) -> Color32 {
    t.canvas
}
