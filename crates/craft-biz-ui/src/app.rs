//! The suite app frame. An app implements [`SuiteApp`] (views, dispatch,
//! tools); [`Shell`] supplies everything else: theme and fonts, Ctrl+wheel
//! zoom, navigation rail, top bar, Ctrl+K palette with schema forms and
//! file pickers, signals dock with drain and retry, toasts, result viewer,
//! persisted settings, `--screenshot` and `--smoke`.
//!
//! Views never change data directly: they call [`Frame::run`] (or open a
//! form with [`Frame::form`]); the shell dispatches after the frame, shows
//! the outcome and asks the app to reload. One code path with CLI and MCP.

use crate::palette::{self, Palette};
use crate::shell::{self, Shot, Toasts, UiSettings};
use crate::signals::{SignalAction, SignalsDock};
use crate::theme::{self, semibold, Tone};
use crate::widgets;
use craft_biz::bus::Bus;
use egui::{Context, Key, Modifiers, RichText, Ui};
use serde_json::Value;
use std::path::PathBuf;

/// One entry of the navigation rail.
pub struct NavEntry {
    pub icon: &'static str,
    pub label: String,
    /// Optional caption printed above this entry (starts a group).
    pub group: Option<String>,
    pub badge: Option<(usize, Tone)>,
}

impl NavEntry {
    pub fn new(icon: &'static str, label: impl Into<String>) -> NavEntry {
        NavEntry {
            icon,
            label: label.into(),
            group: None,
            badge: None,
        }
    }
    pub fn group(mut self, caption: impl Into<String>) -> Self {
        self.group = Some(caption.into());
        self
    }
    pub fn badge(mut self, n: usize, tone: Tone) -> Self {
        self.badge = Some((n, tone));
        self
    }
}

/// A queued tool call from a view.
#[derive(Debug, Clone)]
pub struct Call {
    pub tool: String,
    pub args: Value,
    /// Toast text on success ("" = derive from the tool name).
    pub label: String,
    /// Show the full result in the result window.
    pub show: bool,
    /// Ask for confirmation first (irreversible actions).
    pub confirm: Option<String>,
}

/// Per-frame services for views.
pub struct Frame {
    pub lang_de: bool,
    pub dark: bool,
    calls: Vec<Call>,
    forms: Vec<(String, Vec<(String, String)>)>,
    goto: Option<usize>,
    toast: Vec<(String, Tone)>,
    pub search: String,
}

impl Frame {
    /// Pick the German or English text.
    pub fn t<'a>(&self, de: &'a str, en: &'a str) -> &'a str {
        if self.lang_de {
            de
        } else {
            en
        }
    }
    /// Run a tool after this frame; toast + reload afterwards.
    pub fn run(&mut self, label: impl Into<String>, tool: &str, args: Value) {
        self.calls.push(Call {
            tool: tool.into(),
            args,
            label: label.into(),
            show: false,
            confirm: None,
        });
    }
    /// Run a tool and show its full result.
    pub fn run_show(&mut self, tool: &str, args: Value) {
        self.calls.push(Call {
            tool: tool.into(),
            args,
            label: String::new(),
            show: true,
            confirm: None,
        });
    }
    /// Run a tool only after the user confirms `question`.
    pub fn run_confirm(
        &mut self,
        question: impl Into<String>,
        label: impl Into<String>,
        tool: &str,
        args: Value,
    ) {
        self.calls.push(Call {
            tool: tool.into(),
            args,
            label: label.into(),
            show: false,
            confirm: Some(question.into()),
        });
    }
    /// Open the generated form of a tool, prefilled.
    pub fn form(&mut self, tool: &str, prefill: &[(&str, &str)]) {
        self.forms.push((
            tool.into(),
            prefill
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        ));
    }
    /// Switch to another view.
    pub fn goto(&mut self, view: usize) {
        self.goto = Some(view);
    }
    pub fn toast(&mut self, text: impl Into<String>, tone: Tone) {
        self.toast.push((text.into(), tone));
    }
}

/// Event kinds with a short description: (consumed, emitted).
pub type Wiring = (
    Vec<(&'static str, &'static str)>,
    Vec<(&'static str, &'static str)>,
);

/// What a suite window provides.
pub trait SuiteApp {
    /// Display name, e.g. "GainCraft".
    fn name(&self) -> &'static str;
    /// Bus / binary name, e.g. "gaincraft".
    fn id(&self) -> &'static str;
    fn tone(&self) -> Tone;
    fn version(&self) -> &'static str;
    /// MCP tool catalog of the app.
    fn tools(&self) -> Vec<Value>;
    /// Dispatch one tool call (same path as CLI and MCP).
    fn call(&mut self, tool: &str, args: &Value) -> Result<Value, String>;
    /// Re-read everything the views show.
    fn reload(&mut self) -> Result<(), String>;
    fn nav(&self, f: &Frame) -> Vec<NavEntry>;
    /// Draw view number `view` into the central panel.
    fn view(&mut self, ui: &mut Ui, view: usize, f: &mut Frame);
    /// Right-hand detail panel for the current selection, if any.
    fn detail(&mut self, _ui: &mut Ui, _view: usize, _f: &mut Frame) -> bool {
        false
    }
    fn has_detail(&self, _view: usize) -> bool {
        false
    }
    /// Extra buttons in the top bar (right side, before the palette button).
    fn top_actions(&mut self, _ui: &mut Ui, _f: &mut Frame) {}
    fn bus(&self) -> &Bus;
    /// Event kinds consumed / emitted, with a short description each.
    fn wiring(&self) -> Wiring;
    /// Tool that drains the inbox, if the app consumes events.
    fn drain_tool(&self) -> Option<&'static str> {
        None
    }
    /// Values offered to palette forms (e.g. ("account", current key)).
    fn prefill(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    /// Short text for the status bar (db path, counts).
    fn status_info(&self) -> String {
        String::new()
    }
    /// Select a record by key (`--select`, deep links). Default: nothing.
    fn select(&mut self, _key: &str) {}
    /// Tool used by `--smoke` (prints its result and exits).
    fn smoke_tool(&self) -> &'static str;
}

/// Command line shared by every suite window.
#[derive(Debug, Clone, Default)]
pub struct Args {
    pub db: Option<PathBuf>,
    pub dark: Option<bool>,
    pub lang: Option<String>,
    pub view: Option<usize>,
    pub zoom: Option<f32>,
    pub screenshot: Option<PathBuf>,
    pub smoke: bool,
    pub demo: bool,
    pub size: Option<(f32, f32)>,
    pub select: Option<String>,
}

impl Args {
    pub fn parse_from(it: impl IntoIterator<Item = String>) -> Result<Args, String> {
        let mut a = Args::default();
        let mut it = it.into_iter();
        while let Some(arg) = it.next() {
            let mut val = |n: &str| it.next().ok_or_else(|| format!("{n} needs a value"));
            match arg.as_str() {
                "--db" => a.db = Some(PathBuf::from(val("--db")?)),
                "--dark" => a.dark = Some(true),
                "--light" => a.dark = Some(false),
                "--lang" => a.lang = Some(val("--lang")?),
                "--view" => {
                    a.view = Some(
                        val("--view")?
                            .parse()
                            .map_err(|_| "--view takes a number".to_string())?,
                    )
                }
                "--zoom" => {
                    a.zoom = Some(
                        val("--zoom")?
                            .parse()
                            .map_err(|_| "--zoom takes a number like 1.25".to_string())?,
                    )
                }
                "--screenshot" => a.screenshot = Some(PathBuf::from(val("--screenshot")?)),
                "--size" => {
                    let v = val("--size")?;
                    let (w, h) = v.split_once('x').ok_or("--size takes WxH, e.g. 1440x900")?;
                    a.size = Some((
                        w.parse().map_err(|_| "bad width")?,
                        h.parse().map_err(|_| "bad height")?,
                    ));
                }
                "--select" => a.select = Some(val("--select")?),
                "--smoke" => a.smoke = true,
                "--demo" => a.demo = true,
                "-h" | "--help" => return Err(HELP.into()),
                other => return Err(format!("unknown argument {other}\n\n{HELP}")),
            }
        }
        Ok(a)
    }
    pub fn parse() -> Result<Args, String> {
        Args::parse_from(std::env::args().skip(1))
    }
}

pub const HELP: &str = "Options:
  --db PATH          database (default: the app's usual location)
  --dark | --light   colour scheme (default: last used)
  --lang de|en       language (default: last used)
  --view N           start in view N (0 = first)
  --select KEY       open this record (ticket, account, contract...)
  --zoom F           zoom factor, e.g. 1.25 (Ctrl+wheel changes it live)
  --size WxH         window size, e.g. 1440x900
  --demo             fill an empty database with demo data first
  --screenshot PATH  render, save one PNG, exit
  --smoke            print the status tool result and exit (no window)
Keys: Ctrl+K command palette, Ctrl+wheel / Ctrl +/- zoom, Ctrl+0 reset, F5 reload.";

struct Confirm {
    question: String,
    call: Call,
}

/// The window around a [`SuiteApp`].
pub struct Shell<A: SuiteApp> {
    pub app: A,
    pub view: usize,
    settings: UiSettings,
    settings_path: Option<PathBuf>,
    installed: bool,
    toasts: Toasts,
    palette: Palette,
    dock: SignalsDock,
    shot: Option<Shot>,
    result: Option<(String, Value)>,
    confirm: Option<Confirm>,
    search: String,
    pub last_error: Option<String>,
}

impl<A: SuiteApp> Shell<A> {
    pub fn new(app: A, settings_path: Option<PathBuf>) -> Shell<A> {
        let settings = settings_path
            .as_deref()
            .map(UiSettings::load)
            .unwrap_or_default();
        let mut s = Shell {
            app,
            view: 0,
            settings,
            settings_path,
            installed: false,
            toasts: Toasts::default(),
            palette: Palette::default(),
            dock: SignalsDock::default(),
            shot: None,
            result: None,
            confirm: None,
            search: String::new(),
            last_error: None,
        };
        if let Err(e) = s.app.reload() {
            s.toasts.err(e.clone());
            s.last_error = Some(e);
        }
        s
    }

    /// Apply command-line overrides.
    pub fn with_args(mut self, a: &Args) -> Self {
        if let Some(d) = a.dark {
            self.settings.dark = d;
        }
        if let Some(l) = &a.lang {
            self.settings.lang = if l.starts_with("en") {
                "en".into()
            } else {
                "de".into()
            };
        }
        if let Some(z) = a.zoom {
            self.settings.zoom = z.clamp(shell::ZOOM_MIN, shell::ZOOM_MAX);
        }
        if let Some(v) = a.view {
            self.view = v;
        }
        self.shot = a.screenshot.clone().map(Shot::new);
        if let Some(k) = &a.select {
            self.app.select(k);
        }
        self
    }

    /// The command palette (tests inspect which form a gesture opened).
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Run a tool exactly as a view button would (tests, automation).
    pub fn run_tool(&mut self, tool: &str, args: Value) {
        self.exec(Call {
            tool: tool.into(),
            args,
            label: String::new(),
            show: false,
            confirm: None,
        });
    }

    pub fn settings(&self) -> &UiSettings {
        &self.settings
    }

    fn save_settings(&mut self) {
        if self.shot.is_some() {
            return;
        }
        if let Some(p) = &self.settings_path {
            if let Err(e) = self.settings.save(p) {
                self.toasts.err(format!("settings not saved: {e}"));
            }
        }
    }

    fn exec(&mut self, call: Call) {
        match self.app.call(&call.tool, &call.args) {
            Ok(v) => {
                let label = if call.label.is_empty() {
                    call.tool.replace('_', " ")
                } else {
                    call.label.clone()
                };
                self.toasts.ok(format!("{label}: done"));
                if call.show {
                    self.result = Some((call.tool.clone(), v));
                }
                self.palette.close();
                self.last_error = None;
            }
            Err(e) => {
                if self.palette.open {
                    self.palette.fail(e.clone());
                }
                self.toasts.err(e.clone());
                self.last_error = Some(e);
            }
        }
        if let Err(e) = self.app.reload() {
            self.toasts.err(e);
        }
        self.dock.refresh(self.app.bus(), self.app.id(), true);
    }

    /// Draw one frame into `ui` (the eframe impl and headless tests call this).
    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if !self.installed {
            shell::install(&ctx, &self.settings);
            self.installed = true;
        }
        if shell::handle_zoom(&ctx, &mut self.settings).is_some() {
            self.save_settings();
        }
        self.palette.handle_shortcut(&ctx);
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F5)) {
            match self.app.reload() {
                Ok(()) => self.toasts.ok("reloaded"),
                Err(e) => self.toasts.err(e),
            }
            self.dock.refresh(self.app.bus(), self.app.id(), true);
        }
        self.dock.refresh(self.app.bus(), self.app.id(), false);
        self.palette.prefill = self.app.prefill();

        let mut f = Frame {
            lang_de: self.settings.lang != "en",
            dark: self.settings.dark,
            calls: Vec::new(),
            forms: Vec::new(),
            goto: None,
            toast: Vec::new(),
            search: self.search.clone(),
        };
        let nav = self.app.nav(&f);
        if self.view >= nav.len() {
            self.view = 0;
        }
        let t = theme::current(&ctx);

        egui::Panel::top("suite_top")
            .frame(
                egui::Frame::new()
                    .fill(t.surface)
                    .inner_margin(egui::Margin::symmetric(14, 6))
                    .stroke(egui::Stroke::new(1.0, t.border)),
            )
            .show(ui, |ui| {
                let (waiting, rejected) = self.dock.counts();
                let name = self.app.name();
                let tone = self.app.tone();
                let mut open_palette = false;
                let mut toggle_dock = false;
                let lang_de = f.lang_de;
                let mut search = std::mem::take(&mut self.search);
                shell::top_bar(
                    ui,
                    name,
                    tone,
                    |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut search)
                                .hint_text(if lang_de { "Suchen..." } else { "Search..." })
                                .desired_width(280.0),
                        );
                    },
                    |ui| {
                        let sig_text = if rejected > 0 {
                            format!("Signals {waiting} / {rejected} !")
                        } else {
                            format!("Signals {waiting}")
                        };
                        let tone = if rejected > 0 {
                            Tone::Danger
                        } else if waiting > 0 {
                            Tone::Accent
                        } else {
                            Tone::Neutral
                        };
                        let (fg, bg) = theme::tone_colors(tone, ui.visuals().dark_mode);
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(sig_text).color(fg).family(semibold()),
                                )
                                .fill(bg),
                            )
                            .on_hover_text("Events from and to the other craft apps")
                            .clicked()
                        {
                            toggle_dock = true;
                        }
                        if ui
                            .button(if lang_de {
                                "Befehle  Ctrl+K"
                            } else {
                                "Commands  Ctrl+K"
                            })
                            .on_hover_text("Every action of this app, with a form")
                            .clicked()
                        {
                            open_palette = true;
                        }
                        self.app.top_actions(ui, &mut f);
                    },
                );
                self.search = search;
                if open_palette {
                    self.palette.open_list();
                }
                if toggle_dock {
                    self.dock.open = !self.dock.open;
                    self.dock.refresh(self.app.bus(), self.app.id(), true);
                }
            });
        f.search = self.search.clone();

        let mut status = shell::StatusClick::default();
        egui::Panel::bottom("suite_status")
            .frame(
                egui::Frame::new()
                    .fill(t.surface)
                    .inner_margin(egui::Margin::symmetric(12, 4))
                    .stroke(egui::Stroke::new(1.0, t.border)),
            )
            .show(ui, |ui| {
                let info = format!("{}  v{}", self.app.status_info(), self.app.version());
                let msg = self.toasts.latest().map(|(m, tone)| (m.to_string(), tone));
                ui.horizontal(|ui| {
                    let lang = if f.lang_de { "Deutsch" } else { "English" };
                    if ui.button(lang).on_hover_text("Deutsch / English").clicked() {
                        self.settings.lang = if f.lang_de { "en".into() } else { "de".into() };
                        self.save_settings();
                    }
                    status = shell::status_bar(
                        ui,
                        msg.as_ref().map(|(m, t)| (m.as_str(), *t)),
                        ctx.zoom_factor(),
                        self.settings.dark,
                        &info,
                    );
                });
            });
        if status.toggle_theme {
            let d = !self.settings.dark;
            shell::set_dark(&ctx, &mut self.settings, d);
            self.save_settings();
        }
        if status.reset_zoom {
            ctx.set_zoom_factor(1.0);
            self.settings.zoom = 1.0;
            self.save_settings();
        }

        egui::Panel::left("suite_nav")
            .resizable(false)
            .exact_size(214.0)
            .frame(
                egui::Frame::new()
                    .fill(t.surface)
                    .inner_margin(egui::Margin::symmetric(10, 10))
                    .stroke(egui::Stroke::new(1.0, t.border)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, n) in nav.iter().enumerate() {
                        if let Some(g) = &n.group {
                            shell::nav_caption(ui, g);
                        }
                        if shell::nav_item(ui, n.icon, &n.label, i == self.view, n.badge).clicked()
                        {
                            self.view = i;
                        }
                    }
                });
            });

        if self.dock.open {
            let (consumes, emits) = self.app.wiring();
            let mut act = None;
            egui::Panel::right("suite_signals")
                .resizable(true)
                .default_size(380.0)
                .frame(
                    egui::Frame::new()
                        .fill(t.surface)
                        .inner_margin(egui::Margin::same(12))
                        .stroke(egui::Stroke::new(1.0, t.border)),
                )
                .show(ui, |ui| {
                    act = self.dock.show(ui, &consumes, &emits);
                    if self.app.drain_tool().is_none() && !consumes.is_empty() {
                        ui.label("This app has no inbox tool.");
                    }
                });
            match act {
                Some(SignalAction::DrainAll) => {
                    if let Some(tool) = self.app.drain_tool() {
                        f.calls.push(Call {
                            tool: tool.into(),
                            args: serde_json::json!({}),
                            label: "Signals applied".into(),
                            show: true,
                            confirm: None,
                        });
                    }
                }
                Some(SignalAction::Retry(id)) => {
                    match self.app.bus().retry(self.app.id(), &id) {
                        Ok(()) => self.toasts.ok("event back in the inbox"),
                        Err(e) => self.toasts.err(e.to_string()),
                    }
                    self.dock.refresh(self.app.bus(), self.app.id(), true);
                }
                None => {}
            }
        } else if self.app.has_detail(self.view) {
            egui::Panel::right("suite_detail")
                .resizable(true)
                .default_size(400.0)
                .min_size(320.0)
                .frame(
                    egui::Frame::new()
                        .fill(t.surface)
                        .inner_margin(egui::Margin::same(14))
                        .stroke(egui::Stroke::new(1.0, t.border)),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("detail_scroll")
                        .show(ui, |ui| {
                            self.app.detail(ui, self.view, &mut f);
                        });
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(t.canvas)
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ui, |ui| {
                self.app.view(ui, self.view, &mut f);
            });

        // Palette and result window.
        let tools = self.app.tools();
        if let Some((tool, args)) = self.palette.show(&ctx, &tools) {
            f.calls.push(Call {
                tool,
                args,
                label: String::new(),
                show: true,
                confirm: None,
            });
        }
        for (tool, prefill) in std::mem::take(&mut f.forms) {
            if let Some(tl) = tools.iter().find(|t| t["name"] == tool.as_str()) {
                let saved = std::mem::take(&mut self.palette.prefill);
                self.palette.prefill = prefill.into_iter().chain(saved).collect();
                self.palette.open_tool(tl);
            } else {
                self.toasts.err(format!("unknown tool {tool}"));
            }
        }
        if let Some((tool, v)) = &self.result {
            let mut open = true;
            let title = tool.clone();
            egui::Window::new(RichText::new(format!("Result: {title}")).family(semibold()))
                .id(egui::Id::unique("suite_result"))
                .open(&mut open)
                .default_width(640.0)
                .default_height(460.0)
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| palette::result_view(ui, v));
                });
            if !open {
                self.result = None;
            }
        }
        // Confirmations.
        let mut answer = None;
        if let Some(c) = &self.confirm {
            egui::Window::new(RichText::new("Please confirm").family(semibold()))
                .id(egui::Id::unique("suite_confirm"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -40.0))
                .show(&ctx, |ui| {
                    ui.set_max_width(460.0);
                    ui.label(RichText::new(&c.question).size(15.5));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if widgets::primary(ui, "Yes, do it").clicked() {
                            answer = Some(true);
                        }
                        if ui.button("Cancel").clicked() {
                            answer = Some(false);
                        }
                    });
                });
        }
        match answer {
            Some(true) => {
                if let Some(c) = self.confirm.take() {
                    self.exec(c.call);
                }
            }
            Some(false) => self.confirm = None,
            None => {}
        }

        if let Some(v) = f.goto {
            self.view = v;
        }
        for (m, tone) in std::mem::take(&mut f.toast) {
            self.toasts.push(m, tone);
        }
        for mut call in std::mem::take(&mut f.calls) {
            if let Some(q) = call.confirm.take() {
                self.confirm = Some(Confirm { question: q, call });
            } else {
                self.exec(call);
            }
        }
        self.toasts.show(&ctx);
        if let Some(s) = &mut self.shot {
            s.tick(&ctx);
        }
    }

    /// `--smoke`: run the status tool and print it.
    pub fn smoke(&mut self) -> Result<Value, String> {
        let tool = self.app.smoke_tool();
        self.app.call(tool, &serde_json::json!({}))
    }

    /// Run the window (blocks until closed).
    pub fn run(self, size: Option<(f32, f32)>) -> Result<(), String> {
        let title = self.app.name().to_string();
        let (w, h) = size.unwrap_or((1440.0, 900.0));
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([w, h])
                .with_min_inner_size([980.0, 620.0])
                .with_title(title.clone()),
            ..Default::default()
        };
        eframe::run_native(&title, options, Box::new(move |_cc| Ok(Box::new(self))))
            .map_err(|e| e.to_string())
    }
}

impl<A: SuiteApp> eframe::App for Shell<A> {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

/// Standard `main` body: parse args, open the app with `open(db)`, optional
/// demo seed, smoke or window. Returns the process exit code.
pub fn main_with<A: SuiteApp>(
    default_db: PathBuf,
    open: impl FnOnce(&std::path::Path) -> Result<A, String>,
    demo_tool: &str,
) -> std::process::ExitCode {
    use std::process::ExitCode;
    let args = match Args::parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return if e == HELP {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
    };
    let db = args.db.clone().unwrap_or(default_db);
    let mut app = match open(&db) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("cannot open {}: {e}", db.display());
            return ExitCode::FAILURE;
        }
    };
    if args.demo {
        if let Err(e) = app.call(demo_tool, &serde_json::json!({})) {
            eprintln!("demo seed: {e}");
        }
    }
    let mut sh = Shell::new(app, Some(shell::settings_path(&db))).with_args(&args);
    if args.smoke {
        return match sh.smoke() {
            Ok(v) => {
                println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("smoke: {e}");
                ExitCode::FAILURE
            }
        };
    }
    match sh.run(args.size) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Context helper for apps: is a text matched by the top-bar search?
pub fn search_hit(search: &str, fields: &[&str]) -> bool {
    let q = search.trim().to_lowercase();
    q.is_empty()
        || q.split_whitespace()
            .all(|w| fields.iter().any(|f| f.to_lowercase().contains(w)))
}

/// Unused-context guard for apps that do not need egui::Context directly.
pub fn noop(_c: &Context) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_parse() {
        let a = Args::parse_from(
            [
                "--db",
                "x.db",
                "--dark",
                "--view",
                "2",
                "--zoom",
                "1.5",
                "--size",
                "1280x800",
                "--screenshot",
                "s.png",
            ]
            .map(String::from),
        )
        .unwrap();
        assert_eq!(a.db, Some(PathBuf::from("x.db")));
        assert_eq!(
            (a.dark, a.view, a.zoom, a.size),
            (Some(true), Some(2), Some(1.5), Some((1280.0, 800.0)))
        );
        assert!(Args::parse_from(["--nope".to_string()]).is_err());
        assert!(Args::parse_from(["--view".to_string(), "x".to_string()]).is_err());
        assert!(search_hit("mock bird", &["Mockingbird Inc.", "x"]));
        assert!(!search_hit("zebra", &["Mockingbird"]));
    }
}
