//! Gallery: the kit on one window with labelled demo data, for review screenshots.
//!
//! `cargo run -p craft-biz-ui --example gallery -- [--dark] [--view board|table]
//!  [--lanes] [--signals] [--zoom 1.5] [--screenshot out.png]`
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use craft_biz::bus::Bus;
use craft_biz_ui::kanban::{self, Board, Card, Column, Event, Lane};
use craft_biz_ui::palette::Palette;
use craft_biz_ui::shell::{self, Shot, Toasts, UiSettings};
use craft_biz_ui::signals::{SignalAction, SignalsDock};
use craft_biz_ui::table::{Cell, Col, Table, TableEvent};
use craft_biz_ui::{theme, widgets, Tone};
use eframe::egui;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::ExitCode;

const APP: &str = "taskcraft";

#[derive(Clone, Copy, PartialEq)]
enum View {
    Board,
    Table,
}

struct Gallery {
    settings: UiSettings,
    view: View,
    columns: Vec<Column>,
    lanes: Vec<Lane>,
    use_lanes: bool,
    cards: Vec<Card>,
    undo: Vec<Vec<Card>>,
    selected: Option<String>,
    cols: Vec<Col>,
    rows: Vec<(String, Vec<Cell>)>,
    sel_row: Option<String>,
    toasts: Toasts,
    palette: Palette,
    tools: Vec<Value>,
    dock: SignalsDock,
    bus: Bus,
    shot: Option<Shot>,
}

/// Demo workflow: nothing goes from To do straight to Done, blocked cards stay put.
fn rule(c: &Card, to: &str) -> Result<(), String> {
    if let Some(why) = &c.blocked {
        return Err(format!("blocked: {why}"));
    }
    if c.column == "todo" && to == "done" {
        return Err("start the work first (To do, then In progress)".into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn card(
    id: &str,
    col: &str,
    lane: &str,
    title: &str,
    prio: (&str, Tone),
    tags: &[(&str, Tone)],
    who: &str,
    meta: &[&str],
) -> Card {
    Card {
        id: id.into(),
        column: col.into(),
        lane: lane.into(),
        key: id.into(),
        title: title.into(),
        tags: tags
            .iter()
            .map(|(t, tone)| (t.to_string(), *tone))
            .collect(),
        accent: prio.1,
        badge: Some((prio.0.into(), prio.1)),
        assignee: Some(who.into()),
        meta: meta.iter().map(|m| m.to_string()).collect(),
        blocked: None,
        done: col == "done",
    }
}

fn demo_cards() -> Vec<Card> {
    let anna = "Anna Mockova";
    let joe = "Joe Mocky";
    let hannes = "Hannes Mockinger";
    let mut v = vec![
        card(
            "OPS-12",
            "todo",
            "anna",
            "Kassenbuch-Export als CSV fuer den Steuerberater (DEMO)",
            ("High", Tone::Warn),
            &[("buchcraft", Tone::Teal), ("export", Tone::Neutral)],
            anna,
            &["due 14.11.", "5 h"],
        ),
        card(
            "OPS-15",
            "todo",
            "joe",
            "[Support] Webshop schon wieder down - DRINGEND",
            ("Urgent", Tone::Danger),
            &[("support", Tone::Violet), ("bug", Tone::Danger)],
            joe,
            &["from D-3", "due 10.11."],
        ),
        card(
            "OPS-16",
            "todo",
            "joe",
            "[Contract] Notice deadline C-2026-001 Mock Software GmbH",
            ("High", Tone::Warn),
            &[("contract", Tone::Info)],
            joe,
            &["due 30.11."],
        ),
        card(
            "OPS-9",
            "doing",
            "anna",
            "AZG-Wochenpruefung: 11. Stunde am Freitag warnen",
            ("Medium", Tone::Accent),
            &[("teamcraft", Tone::Teal)],
            anna,
            &["8 h", "3 h left"],
        ),
        card(
            "OPS-11",
            "doing",
            "joe",
            "QR-Etiketten: Bogen 3x8 als PDF",
            ("Low", Tone::Neutral),
            &[("assetcraft", Tone::Teal), ("print", Tone::Neutral)],
            hannes,
            &["4 h"],
        ),
        card(
            "OPS-13",
            "doing",
            "joe",
            "Mahnlauf: zweite Stufe mit Verzugszinsen",
            ("High", Tone::Warn),
            &[("buchcraft", Tone::Teal)],
            joe,
            &["waiting for tax advice"],
        ),
        card(
            "OPS-7",
            "review",
            "anna",
            "Health-Score: Gewichte im Fenster einstellbar",
            ("Medium", Tone::Accent),
            &[("gaincraft", Tone::Teal), ("ui", Tone::Violet)],
            anna,
            &["PR open", "2 h"],
        ),
        card(
            "OPS-4",
            "done",
            "anna",
            "Feiertage 2027 im Kalender",
            ("Low", Tone::Neutral),
            &[("craft-biz", Tone::Teal)],
            hannes,
            &["done 06.11."],
        ),
        card(
            "OPS-5",
            "done",
            "joe",
            "SLA-Uhr pausiert bei Status pending",
            ("Medium", Tone::Accent),
            &[("deskcraft", Tone::Teal)],
            joe,
            &["done 07.11."],
        ),
    ];
    if let Some(c) = v.iter_mut().find(|c| c.id == "OPS-13") {
        c.blocked = Some("waiting for tax advice".into());
    }
    v
}

fn demo_rows() -> (Vec<Col>, Vec<(String, Vec<Cell>)>) {
    let cols = vec![
        Col::new("Account", 2.4),
        Col::new("Health", 0.9),
        Col::num("Score", 0.6),
        Col::new("Trend", 1.1),
        Col::new("Owner", 1.4),
        Col::num("ARR", 1.0),
        Col::new("Open CTAs", 1.8),
        Col::new("Notice", 0.9),
    ];
    let row = |id: &str,
               name: &str,
               band: (&str, Tone),
               score: i64,
               trend: &[f32],
               owner: &str,
               arr: &str,
               ctas: &[(&str, Tone)],
               notice: &str| {
        (
            id.to_string(),
            vec![
                Cell::Strong(name.into()),
                Cell::Pill(band.0.into(), band.1),
                Cell::Mono(score.to_string()),
                Cell::Spark(trend.to_vec(), band.1),
                Cell::Person(owner.into()),
                Cell::Money(arr.into()),
                Cell::Pills(ctas.iter().map(|(c, t)| (c.to_string(), *t)).collect()),
                Cell::Weak(notice.into()),
            ],
        )
    };
    let rows = vec![
        row(
            "1",
            "Hannes Mockinger e.U. (DEMO)",
            ("red", Tone::Danger),
            30,
            &[71.0, 66.0, 58.0, 49.0, 41.0, 30.0],
            "Joe Mocky",
            "5.400,00",
            &[("low health", Tone::Danger), ("unhappy", Tone::Warn)],
            "31.08.2027",
        ),
        row(
            "2",
            "Mockingbird Inc. (DEMO)",
            ("yellow", Tone::Warn),
            63,
            &[88.0, 86.0, 85.0, 80.0, 70.0, 63.0],
            "Joe Mocky",
            "24.000,00",
            &[("usage drop", Tone::Warn)],
            "-",
        ),
        row(
            "3",
            "Mock Software GmbH (DEMO)",
            ("green", Tone::Ok),
            81,
            &[78.0, 80.0, 79.0, 82.0, 81.0, 81.0],
            "Anna Mockova",
            "9.600,00",
            &[("renewal", Tone::Info)],
            "30.11.2026",
        ),
        row(
            "4",
            "Mockerl & Partner OG (DEMO)",
            ("green", Tone::Ok),
            88,
            &[80.0, 83.0, 85.0, 86.0, 88.0, 88.0],
            "Anna Mockova",
            "12.000,00",
            &[],
            "31.03.2027",
        ),
        row(
            "5",
            "Mockova Consulting KG (DEMO)",
            ("yellow", Tone::Warn),
            55,
            &[60.0, 61.0, 57.0, 58.0, 54.0, 55.0],
            "Hannes Mockinger",
            "-1.200,00",
            &[("seats unused", Tone::Neutral)],
            "30.06.2027",
        ),
    ];
    (cols, rows)
}

fn demo_tools() -> Vec<Value> {
    vec![
        json!({ "name": "task_issue_create", "description": "Create an issue in a project.", "inputSchema": { "type": "object", "properties": {
            "project": {"type": "string", "description": "Project key, e.g. OPS"}, "title": {"type": "string"},
            "priority": {"type": "string", "enum": ["low", "medium", "high", "urgent"]}, "estimate_hours": {"type": "integer"}, "assignee": {"type": ["string", "null"]}
        }, "required": ["project", "title"] } }),
        json!({ "name": "task_issue_move", "description": "Move an issue to another status (workflow rules apply).", "inputSchema": { "type": "object", "properties": {
            "issue": {"type": "string"}, "to": {"type": "string", "enum": ["todo", "doing", "review", "done"]}
        }, "required": ["issue", "to"] } }),
        json!({ "name": "task_inbox_drain", "description": "Apply events from deskcraft, contractcraft and teamcraft.", "inputSchema": { "type": "object", "properties": {} } }),
    ]
}

fn demo_bus() -> Bus {
    let root = std::env::temp_dir().join("craft-biz-ui-gallery-bus");
    let _ = std::fs::remove_dir_all(&root);
    let bus = Bus::new(root);
    let _ = bus.emit("deskcraft", APP, "ticket.escalated", json!({ "ticket_id": "D-3", "subject": "Webshop schon wieder down - DRINGEND", "priority": "urgent" }));
    let _ = bus.emit("contractcraft", APP, "contract.deadline", json!({ "contract_id": "C-2026-001", "counterparty": "Mock Software GmbH (DEMO)", "due": "2026-11-30" }));
    if let Ok(ev) = bus.emit(
        "teamcraft",
        APP,
        "capacity.changed",
        json!({ "week": "2026-W46", "hours": 62 }),
    ) {
        let _ = bus.ack(APP, &ev.id);
    }
    if let Ok(ev) = bus.emit(
        "deskcraft",
        APP,
        "ticket.escalated",
        json!({ "ticket_id": "D-7", "project": "XYZ" }),
    ) {
        let _ = bus.reject(APP, &ev.id, "unknown project XYZ");
    }
    bus
}

impl Gallery {
    fn new(
        settings: UiSettings,
        view: View,
        use_lanes: bool,
        signals: bool,
        shot: Option<PathBuf>,
    ) -> Gallery {
        let (cols, rows) = demo_rows();
        let mut dock = SignalsDock::default();
        if signals {
            dock.open = true;
        }
        Gallery {
            settings,
            view,
            columns: vec![
                Column::new("todo", "To do", Tone::Neutral),
                Column::new("doing", "In progress", Tone::Accent).wip(2),
                Column::new("review", "In review", Tone::Violet),
                Column::new("done", "Done", Tone::Ok),
            ],
            lanes: vec![
                Lane {
                    id: "anna".into(),
                    title: "Anna Mockova".into(),
                },
                Lane {
                    id: "joe".into(),
                    title: "Joe Mocky".into(),
                },
            ],
            use_lanes,
            cards: demo_cards(),
            undo: Vec::new(),
            selected: Some("OPS-9".into()),
            cols,
            rows,
            sel_row: Some("2".into()),
            toasts: Toasts::default(),
            palette: Palette::default(),
            tools: demo_tools(),
            dock,
            bus: demo_bus(),
            shot: shot.map(Shot::new),
        }
    }

    fn board_events(&mut self, events: Vec<Event>) {
        for e in events {
            match &e {
                Event::Moved { card, to, .. } => {
                    self.undo.push(self.cards.clone());
                    kanban::apply_move(&mut self.cards, &e);
                    let done = to == "done";
                    if let Some(c) = self.cards.iter_mut().find(|c| &c.id == card) {
                        c.done = done;
                    }
                    let title = self
                        .columns
                        .iter()
                        .find(|c| &c.id == to)
                        .map(|c| c.title.clone())
                        .unwrap_or_default();
                    self.toasts
                        .ok(format!("{card} moved to {title}  (Ctrl+Z to undo)"));
                }
                Event::Refused { card, reason, .. } => self.toasts.err(format!("{card}: {reason}")),
                Event::Select(id) => self.selected = Some(id.clone()),
                Event::Open(id) => self.toasts.push(format!("open {id}"), Tone::Info),
                Event::Add { column, .. } => self
                    .toasts
                    .push(format!("new issue in {column}"), Tone::Info),
                Event::Undo => {
                    if let Some(prev) = self.undo.pop() {
                        self.cards = prev;
                        self.toasts.push("Move undone", Tone::Info);
                    }
                }
            }
        }
    }
}

impl eframe::App for Gallery {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        shell::handle_zoom(&ctx, &mut self.settings);
        self.palette.handle_shortcut(&ctx);
        self.dock.refresh(&self.bus, APP, false);
        let t = theme::current(&ctx);
        let bar = egui::Frame::new()
            .fill(t.surface)
            .inner_margin(egui::Margin::symmetric(14, 8));

        let (pending, failed) = self.dock.counts();
        let (mut open_palette, mut toggle_dock, mut create) = (false, false, false);
        egui::Panel::top("g_top").frame(bar).show(ui, |ui| {
            shell::top_bar(
                ui,
                APP,
                Tone::Accent,
                |ui| {
                    if ui
                        .add(
                            egui::Button::new("Search or run a command...")
                                .min_size(egui::vec2(300.0, 30.0)),
                        )
                        .clicked()
                    {
                        open_palette = true;
                    }
                    widgets::key_hint(ui, "Ctrl+K");
                },
                |ui| {
                    if widgets::primary(ui, "+ Create").clicked() {
                        create = true;
                    }
                    let label = if pending + failed > 0 {
                        format!("Signals  {}", pending + failed)
                    } else {
                        "Signals".to_string()
                    };
                    if ui
                        .button(label)
                        .on_hover_text("Events from and to other craft apps")
                        .clicked()
                    {
                        toggle_dock = true;
                    }
                },
            );
        });
        if open_palette {
            self.palette.open_list();
        }
        if create {
            if let Some(tool) = self.tools.first() {
                self.palette.open_tool(tool);
            }
        }
        if toggle_dock {
            self.dock.open = !self.dock.open;
        }

        let mut click = shell::StatusClick::default();
        egui::Panel::bottom("g_status")
            .frame(
                egui::Frame::new()
                    .fill(t.surface)
                    .inner_margin(egui::Margin::symmetric(14, 5)),
            )
            .show(ui, |ui| {
                click = shell::status_bar(
                    ui,
                    self.toasts.latest(),
                    ctx.zoom_factor(),
                    self.settings.dark,
                    "craft-biz-ui gallery  |  DEMO data",
                );
            });
        if click.toggle_theme {
            let dark = !self.settings.dark;
            shell::set_dark(&ctx, &mut self.settings, dark);
        }
        if click.reset_zoom {
            ctx.set_zoom_factor(1.0);
        }

        egui::Panel::left("g_nav")
            .exact_size(210.0)
            .frame(
                egui::Frame::new()
                    .fill(t.surface)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ui, |ui| {
                shell::nav_caption(ui, "Project OPS");
                if shell::nav_item(ui, "\u{25A6}", "Board", self.view == View::Board, None)
                    .clicked()
                {
                    self.view = View::Board;
                }
                if shell::nav_item(
                    ui,
                    "\u{2630}",
                    "Accounts (table)",
                    self.view == View::Table,
                    None,
                )
                .clicked()
                {
                    self.view = View::Table;
                }
                shell::nav_item(ui, "\u{2691}", "Backlog", false, Some((14, Tone::Neutral)));
                shell::nav_item(ui, "\u{25F7}", "Sprints", false, None);
                shell::nav_caption(ui, "Suite");
                shell::nav_item(
                    ui,
                    "\u{21C4}",
                    "Signals",
                    self.dock.open,
                    Some((pending, Tone::Accent)),
                );
                shell::nav_item(ui, "\u{2699}", "Settings", false, None);
            });

        if self.dock.open {
            let mut action = None;
            egui::Panel::right("g_dock")
                .default_size(390.0)
                .frame(
                    egui::Frame::new()
                        .fill(t.surface)
                        .inner_margin(egui::Margin::same(12)),
                )
                .show(ui, |ui| {
                    action = self.dock.show(
                        ui,
                        &[
                            ("ticket.escalated", "deskcraft ticket becomes an issue"),
                            (
                                "contract.deadline",
                                "contractcraft notice date becomes an issue",
                            ),
                            ("capacity.changed", "teamcraft hours per week"),
                        ],
                        &[],
                    );
                });
            match action {
                Some(SignalAction::DrainAll) => {
                    let evs = self.bus.pending(APP).unwrap_or_default();
                    for ev in &evs {
                        let _ = self.bus.ack(APP, &ev.id);
                    }
                    self.toasts.ok(format!("{} events applied", evs.len()));
                    self.dock.refresh(&self.bus, APP, true);
                }
                Some(SignalAction::Retry(id)) => {
                    match self.bus.retry(APP, &id) {
                        Ok(()) => self.toasts.ok("Event is back in the inbox"),
                        Err(e) => self.toasts.err(e.to_string()),
                    }
                    self.dock.refresh(&self.bus, APP, true);
                }
                None => {}
            }
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.canvas).inner_margin(egui::Margin::same(16))).show(ui, |ui| match self.view {
            View::Board => {
                let mut lanes = self.use_lanes;
                widgets::page_header(ui, "Sprint 46 board", "OPS  |  9 issues  |  team capacity this week: 62 h (from teamcraft)  |  drag cards, right-click to move, Ctrl+arrows", |ui| {
                    ui.checkbox(&mut lanes, "Swimlanes by person");
                });
                self.use_lanes = lanes;
                let events = {
                    let r: kanban::Rule = &rule;
                    let mut b = Board::new("gallery", &self.columns, &self.cards).rule(r).selected(self.selected.as_deref());
                    if self.use_lanes {
                        b = b.lanes(&self.lanes);
                    }
                    b.show(ui)
                };
                self.board_events(events);
            }
            View::Table => {
                widgets::page_header(ui, "Accounts", "Customer health, worst first  |  click a header to sort, type to filter", |ui| {
                    let _ = widgets::primary(ui, "Import usage CSV");
                    let _ = ui.button("Export view");
                });
                ui.horizontal(|ui| {
                    widgets::kpi(ui, "ARR at risk", "29.400,00", Some(("2 accounts", Tone::Danger)));
                    widgets::kpi(ui, "Active accounts", "5", None);
                    widgets::kpi(ui, "Open CTAs", "5", Some(("1 due today", Tone::Warn)));
                    widgets::kpi(ui, "Next notice date", "30.11.2026", Some(("21 days", Tone::Info)));
                });
                ui.add_space(10.0);
                widgets::card(ui, |ui| {
                    let events = Table::new("gallery_accounts", &self.cols, &self.rows).selected(self.sel_row.as_deref()).show(ui);
                    for e in events {
                        match e {
                            TableEvent::Select(id) | TableEvent::Open(id) => self.sel_row = Some(id),
                        }
                    }
                });
            }
        });

        if let Some((tool, args)) = self.palette.show(&ctx, &self.tools) {
            self.toasts.ok(format!("{tool} {args}"));
            self.palette.close();
        }
        self.toasts.show(&ctx);
        if let Some(s) = &mut self.shot {
            s.tick(&ctx);
        }
    }
}

fn main() -> ExitCode {
    let mut settings = UiSettings::default();
    let (mut view, mut lanes, mut signals, mut shot) = (View::Board, false, false, None);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dark" => settings.dark = true,
            "--lanes" => lanes = true,
            "--signals" => signals = true,
            "--view" => {
                view = if it.next().as_deref() == Some("table") {
                    View::Table
                } else {
                    View::Board
                }
            }
            "--zoom" => settings.zoom = it.next().and_then(|z| z.parse().ok()).unwrap_or(1.0),
            "--screenshot" => shot = it.next().map(PathBuf::from),
            other => {
                eprintln!("gallery: unknown argument {other}");
                return ExitCode::FAILURE;
            }
        }
    }
    let app = Gallery::new(settings.clone(), view, lanes, signals, shot);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 880.0])
            .with_title("craft-biz-ui gallery"),
        ..Default::default()
    };
    let result = eframe::run_native(
        "craft-biz-ui gallery",
        options,
        Box::new(move |cc| {
            shell::install(&cc.egui_ctx, &settings);
            Ok(Box::new(app))
        }),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gallery: {e}");
            ExitCode::FAILURE
        }
    }
}
