//! The suite app frame. An app implements [`SuiteApp`] (views, dispatch,
//! tools); [`Shell`] supplies everything else: theme and fonts, Ctrl+wheel
//! zoom, navigation rail, top bar, Ctrl+K palette with schema forms and
//! file pickers, signals dock with drain and retry, toasts, result viewer,
//! persisted settings, `--screenshot` and `--smoke`.
//!
//! Views never change data directly: they call [`Frame::run`] (or open a
//! form with [`Frame::form`]); the shell dispatches after the frame, shows
//! the outcome and asks the app to reload. One code path with CLI and MCP.
//!
//! The shell also draws the menu bar of the Craft Menu Bar Standard: the
//! bookends `File Edit View ... Window Help` are the shell's, the app adds
//! items through [`SuiteApp::menus`] (domain menus such as Insert, Modify,
//! Commands go in the middle). Help holds the five dialogs: Command Palette,
//! Keyboard Shortcuts, Project Files & Docs, Licenses & Notices, About.

use crate::help::{self, AppInfo, Dialog, Doc, HelpState, ShortcutRow};
use crate::menu::{self, Item, Menu, Picked};
use crate::palette::{self, Palette};
use crate::shell::{self, Shot, Toasts, UiSettings};
use crate::signals::{SignalAction, SignalsDock};
use crate::theme::{self, semibold, Tone};
use crate::widgets;
use craft_biz::bus::Bus;
use egui::{Context, Key, Modifiers, RichText, Ui};
use serde_json::Value;
use std::path::{Path, PathBuf};

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
    /// A frame with no queued calls. Tests use it to draw one page.
    pub fn idle() -> Frame {
        Frame {
            lang_de: false,
            dark: false,
            calls: Vec::new(),
            forms: Vec::new(),
            goto: None,
            toast: Vec::new(),
            search: String::new(),
        }
    }

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

/// What an app adds to the shell's menu bar.
#[derive(Default)]
pub struct AppMenus {
    /// Top of File (New..., Import...).
    pub file_new: Vec<Item>,
    /// File > Save As... (e.g. a backup form).
    pub save_as: Option<Item>,
    /// File > Export... items.
    pub export: Vec<Item>,
    /// Top of View (views, perspectives). Empty: one item per navigation entry.
    pub view: Vec<Item>,
    /// Domain menus between View and Window (Insert, Modify, Commands...).
    pub middle: Vec<Menu>,
    /// Extra items at the end of Window.
    pub window: Vec<Item>,
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

    // ---- menu bar (all optional) ----

    /// The window's language changed (or was set at start). For app text
    /// built outside a view, such as undo labels.
    fn set_lang(&mut self, _de: bool) {}
    /// Items the app adds to the menu bar.
    fn menus(&self, _f: &Frame) -> AppMenus {
        AppMenus::default()
    }
    /// An app action chosen from a menu (`Item::action` ids not starting with `shell.`).
    fn menu_action(&mut self, _id: &str, _f: &mut Frame) {}
    /// "Undo <what>" if there is something to undo.
    fn undo_label(&self) -> Option<String> {
        None
    }
    fn redo_label(&self) -> Option<String> {
        None
    }
    fn undo(&mut self) -> Result<String, String> {
        Err("nothing to undo".into())
    }
    fn redo(&mut self) -> Result<String, String> {
        Err("nothing to redo".into())
    }
    /// Text for Edit > Copy when no text field has focus (the selected record).
    fn copy_text(&self, _view: usize) -> Option<String> {
        None
    }
    /// File > Save. Apps that write every change at once say so.
    fn save(&mut self) -> Result<String, String> {
        Ok("every change is saved when it is made".into())
    }
    /// File > Close: drop the selection, back to the first view.
    fn close(&mut self) {}
    /// View that holds the app's settings (Edit > Preferences). None: shell dialog.
    fn preferences_view(&self) -> Option<usize> {
        None
    }
    /// About / Licenses facts.
    fn info(&self) -> AppInfo {
        AppInfo {
            name: self.name().into(),
            version: self.version().into(),
            license: "MIT OR Apache-2.0".into(),
            ..AppInfo::default()
        }
    }
    /// Help > Project Files & Docs.
    fn docs(&self) -> Vec<Doc> {
        Vec::new()
    }
    /// Keys handled inside views (board arrows...), for the shortcut sheet.
    fn extra_shortcuts(&self, _de: bool) -> Vec<ShortcutRow> {
        Vec::new()
    }
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

/// Edit menu operation applied to the text field that had focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditOp {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

type Opener<A> = Box<dyn Fn(&Path) -> Result<A, String>>;

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
    help: HelpState,
    opener: Option<Opener<A>>,
    db: Option<PathBuf>,
    last_focus: Option<egui::Id>,
    /// Selection of `last_focus` (a text field loses it when the menu takes focus).
    last_sel: Option<egui::text::CCursorRange>,
    pending_edit: Option<(egui::Id, EditOp)>,
    pending: Vec<Picked>,
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
            help: HelpState::default(),
            opener: None,
            db: None,
            last_focus: None,
            last_sel: None,
            pending_edit: None,
            pending: Vec::new(),
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
        self.app.set_lang(self.settings.lang != "en");
        self.shot = a.screenshot.clone().map(Shot::new);
        if let Some(k) = &a.select {
            self.app.select(k);
        }
        self
    }

    /// Let File > Open / Open Recent switch databases with `open`.
    pub fn with_opener(
        mut self,
        db: PathBuf,
        open: impl Fn(&Path) -> Result<A, String> + 'static,
    ) -> Self {
        self.settings.push_recent(&db);
        self.db = Some(db);
        self.opener = Some(Box::new(open));
        self
    }

    /// The command palette (tests inspect which form a gesture opened).
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Which Help dialog is open.
    pub fn help_open(&self) -> Option<Dialog> {
        self.help.open
    }

    /// Choose a menu item as if clicked; applied on the next frame (tests, automation).
    pub fn trigger(&mut self, p: Picked) {
        self.pending.push(p);
    }

    /// Find a menu item by its path of labels, e.g. `["File", "Export..."]`.
    pub fn menu_item(&self, path: &[&str]) -> Option<Item> {
        let f = self.frame();
        let menus = self.build_menus(&f, &self.app.nav(&f), false);
        let (first, rest) = path.split_first()?;
        let mut items = &menus.iter().find(|m| m.title == *first)?.items;
        let mut found = None;
        for (i, label) in rest.iter().enumerate() {
            let it = items.iter().find(|it| it.label == *label)?;
            if i + 1 == rest.len() {
                found = Some(it.clone());
            } else if let menu::Kind::Submenu(c) = &it.kind {
                items = c;
            } else {
                return None;
            }
        }
        found
    }

    /// The whole menu bar as it would be drawn now.
    pub fn menus(&self) -> Vec<Menu> {
        let f = self.frame();
        self.build_menus(&f, &self.app.nav(&f), false)
    }

    fn frame(&self) -> Frame {
        Frame {
            lang_de: self.settings.lang != "en",
            dark: self.settings.dark,
            calls: Vec::new(),
            forms: Vec::new(),
            goto: None,
            toast: Vec::new(),
            search: self.search.clone(),
        }
    }

    /// File, Edit, View, app menus, Window, Help.
    fn build_menus(&self, f: &Frame, nav: &[NavEntry], text_focus: bool) -> Vec<Menu> {
        let de = f.lang_de;
        let t = |a: &str, b: &str| if de { a.to_string() } else { b.to_string() };
        let am = self.app.menus(f);
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;

        // File
        let mut file = am.file_new;
        if !file.is_empty() {
            file.push(Item::sep());
        }
        let can_open = self.opener.is_some();
        file.push(
            Item::action(
                t("Datenbank öffnen...", "Open Database..."),
                "shell.open_db",
            )
            .key(cmd_shift, Key::O)
            .enabled(can_open),
        );
        let mut recent: Vec<Item> = self
            .settings
            .recent
            .iter()
            .map(|r| Item::action(r.clone(), &format!("shell.recent:{r}")).enabled(can_open))
            .collect();
        if recent.is_empty() {
            recent.push(Item::action(t("(leer)", "(empty)"), "shell.none").enabled(false));
        } else {
            recent.push(Item::sep());
            recent.push(Item::action(
                t("Liste leeren", "Clear Recent Files"),
                "shell.clear_recent",
            ));
        }
        file.push(Item::sub(t("Zuletzt geöffnet", "Open Recent"), recent));
        file.push(Item::action(t("Speichern", "Save"), "shell.save").key(cmd, Key::S));
        if let Some(sa) = am.save_as {
            file.push(sa.key(cmd_shift, Key::S));
        }
        if !am.export.is_empty() {
            file.push(Item::sep());
            file.extend(am.export);
        }
        file.push(Item::sep());
        file.push(Item::action(t("Schließen", "Close"), "shell.close").key(cmd, Key::W));
        file.push(Item::action(t("Beenden", "Quit"), "shell.quit").key(cmd, Key::Q));

        // Edit: Undo/Redo keys belong to a focused text field.
        let undo = self.app.undo_label();
        let redo = self.app.redo_label();
        let mut u = Item::action(
            undo.clone()
                .map(|l| format!("{} {l}", t("Rückgängig:", "Undo")))
                .unwrap_or_else(|| t("Rückgängig", "Undo")),
            "shell.undo",
        )
        .enabled(undo.is_some())
        .hint(t("Nichts rückgängig zu machen", "Nothing to undo"));
        let mut r = Item::action(
            redo.clone()
                .map(|l| format!("{} {l}", t("Wiederholen:", "Redo")))
                .unwrap_or_else(|| t("Wiederholen", "Redo")),
            "shell.redo",
        )
        .enabled(redo.is_some())
        .hint(t("Nichts zu wiederholen", "Nothing to redo"));
        if text_focus {
            u = u.key_text("Ctrl+Z");
            r = r.key_text("Ctrl+Y");
        } else {
            u = u.key(cmd, Key::Z);
            r = r.key(cmd, Key::Y);
        }
        let edit = vec![
            u,
            r,
            Item::sep(),
            Item::action(t("Ausschneiden", "Cut"), "shell.cut").key_text("Ctrl+X"),
            Item::action(t("Kopieren", "Copy"), "shell.copy").key_text("Ctrl+C"),
            Item::action(t("Einfügen", "Paste"), "shell.paste").key_text("Ctrl+V"),
            Item::action(t("Alles auswählen", "Select All"), "shell.select_all").key_text("Ctrl+A"),
            Item::sep(),
            Item::action(t("Einstellungen...", "Preferences..."), "shell.prefs")
                .key(cmd, Key::Comma),
        ];

        // View
        let mut view = if am.view.is_empty() {
            nav.iter()
                .enumerate()
                .map(|(i, n)| Item::view(n.label.clone(), i))
                .collect()
        } else {
            am.view
        };
        let dark = self.settings.dark;
        view.extend([
            Item::sep(),
            Item::action(t("Vergrößern", "Zoom In"), "shell.zoom_in").key_text("Ctrl+="),
            Item::action(t("Verkleinern", "Zoom Out"), "shell.zoom_out").key_text("Ctrl+-"),
            Item::action(t("Zoom 100%", "Reset Zoom (100%)"), "shell.zoom_reset")
                .key_text("Ctrl+0"),
            Item::sep(),
            Item::sub(
                t("Farbschema", "Interface Theme"),
                vec![
                    Item::action(t("Dunkel", "Dark"), "shell.theme_dark").checked(dark),
                    Item::action(t("Hell", "Light"), "shell.theme_light").checked(!dark),
                ],
            ),
            Item::sub(
                t("Sprache", "Language"),
                vec![
                    Item::action("Deutsch", "shell.lang_de").checked(de),
                    Item::action("English", "shell.lang_en").checked(!de),
                ],
            ),
            Item::action(t("Neu laden", "Reload"), "shell.reload").key(Modifiers::NONE, Key::F5),
        ]);

        // Window
        let l = self.settings.layout;
        let mut window = vec![
            Item::action(
                t("Layout zurücksetzen", "Reset Layout"),
                "shell.layout_reset",
            ),
            Item::sep(),
            Item::action(
                t("Navigationsleiste", "Navigation Rail"),
                "shell.toggle_nav",
            )
            .checked(l.nav),
            Item::action(t("Schnellleiste", "Quick View Row"), "shell.toggle_quick")
                .checked(l.quick),
            Item::action(t("Detailbereich", "Detail Panel"), "shell.toggle_detail")
                .checked(l.detail),
            Item::action(t("Signale", "Signals Dock"), "shell.toggle_dock").checked(self.dock.open),
            Item::action(t("Statusleiste", "Status Bar"), "shell.toggle_status").checked(l.status),
        ];
        if !am.window.is_empty() {
            window.push(Item::sep());
            window.extend(am.window);
        }

        // Help: the five-item suite.
        let help = vec![
            Item::action(t("Befehle...", "Command Palette..."), "shell.palette").key(cmd, Key::K),
            Item::action(
                t("Tastenkürzel...", "Keyboard Shortcuts..."),
                "shell.shortcuts",
            )
            .key(Modifiers::NONE, Key::F1),
            Item::action(
                t("Projektdateien und Doku...", "Project Files & Docs..."),
                "shell.docs",
            ),
            Item::action(
                t("Lizenzen und Hinweise...", "Licenses & Notices..."),
                "shell.licenses",
            ),
            Item::action(
                format!("{} {}", t("Über", "About"), self.app.name()),
                "shell.about",
            ),
        ];

        let mut menus = vec![
            Menu::new(t("Datei", "File"), file),
            Menu::new(t("Bearbeiten", "Edit"), edit),
            Menu::new(t("Ansicht", "View"), view),
        ];
        menus.extend(am.middle);
        menus.push(Menu::new(t("Fenster", "Window"), window));
        menus.push(Menu::new(t("Hilfe", "Help"), help));
        menus
    }

    /// The shortcut sheet: every menu key plus the keys views and the shell handle.
    pub fn shortcut_rows(&self) -> Vec<ShortcutRow> {
        let f = self.frame();
        let de = f.lang_de;
        let menus = self.build_menus(&f, &self.app.nav(&f), false);
        let mut rows = Vec::new();
        fn walk(
            ctx_fmt: &dyn Fn(&egui::KeyboardShortcut) -> String,
            group: &str,
            items: &[Item],
            rows: &mut Vec<ShortcutRow>,
        ) {
            for it in items {
                if let menu::Kind::Submenu(c) = &it.kind {
                    walk(ctx_fmt, group, c, rows);
                    continue;
                }
                let keys = it
                    .shortcut
                    .as_ref()
                    .map(ctx_fmt)
                    .or_else(|| it.shortcut_text.clone());
                if let Some(k) = keys {
                    rows.push((group.to_string(), k, it.label.clone()));
                }
            }
        }
        let fmt = |s: &egui::KeyboardShortcut| s.format(&egui::ModifierNames::NAMES, false);
        for m in &menus {
            walk(&fmt, &m.title, &m.items, &mut rows);
        }
        let g = if de { "Allgemein" } else { "General" };
        for (k, w) in [
            (
                "Ctrl+Shift+P",
                if de {
                    "Befehle (wie Ctrl+K)"
                } else {
                    "Command palette (same as Ctrl+K)"
                },
            ),
            (
                "Ctrl+/",
                if de {
                    "Tastenkürzel (wie F1)"
                } else {
                    "Keyboard shortcuts (same as F1)"
                },
            ),
            ("Ctrl+Mouse wheel", if de { "Zoomen" } else { "Zoom" }),
            (
                "Esc",
                if de {
                    "Dialog schließen"
                } else {
                    "Close a dialog"
                },
            ),
        ] {
            rows.push((g.to_string(), k.to_string(), w.to_string()));
        }
        rows.extend(self.app.extra_shortcuts(de));
        rows
    }

    fn reopen(&mut self, path: PathBuf) {
        let Some(open) = &self.opener else {
            self.toasts.err("this window cannot switch databases");
            return;
        };
        match open(&path) {
            Ok(app) => {
                self.app = app;
                self.view = 0;
                self.result = None;
                self.settings.push_recent(&path);
                self.toasts.ok(format!("opened {}", path.display()));
                self.db = Some(path);
                self.save_settings();
                if let Err(e) = self.app.reload() {
                    self.toasts.err(e);
                }
            }
            Err(e) => self.toasts.err(format!("{}: {e}", path.display())),
        }
    }

    fn edit_op(&mut self, ctx: &Context, op: EditOp) {
        if let Some(id) = self.last_focus {
            self.pending_edit = Some((id, op));
            ctx.request_repaint();
            return;
        }
        let de = self.settings.lang != "en";
        if op == EditOp::Copy {
            if let Some(text) = self.app.copy_text(self.view) {
                ctx.copy_text(text);
                self.toasts.ok(if de { "kopiert" } else { "copied" });
                return;
            }
        }
        self.toasts.push(
            if de {
                "Zuerst ein Textfeld anklicken"
            } else {
                "Click into a text field first"
            },
            Tone::Neutral,
        );
    }

    /// Apply a chosen menu item.
    fn handle_pick(&mut self, ctx: &Context, p: Picked, f: &mut Frame) {
        match p {
            Picked::Run(tool, args) => f.calls.push(Call {
                tool,
                args,
                label: String::new(),
                show: true,
                confirm: None,
            }),
            Picked::Form(tool, prefill) => f.forms.push((tool, prefill)),
            Picked::View(v) => f.goto = Some(v),
            Picked::Action(id) => {
                if let Some(rest) = id.strip_prefix("shell.") {
                    self.shell_action(ctx, rest, f);
                } else {
                    self.app.menu_action(&id, f);
                }
            }
        }
    }

    fn shell_action(&mut self, ctx: &Context, id: &str, f: &mut Frame) {
        let de = f.lang_de;
        let mut layout_changed = false;
        match id {
            "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            "close" => {
                self.app.close();
                self.view = 0;
                self.result = None;
                self.help.open = None;
                self.palette.close();
            }
            "save" => {
                self.save_settings();
                match self.app.save() {
                    Ok(m) => self.toasts.ok(m),
                    Err(e) => self.toasts.err(e),
                }
            }
            "open_db" => {
                let mut dlg = rfd::FileDialog::new()
                    .add_filter("SQLite", &["db", "sqlite", "sqlite3"])
                    .add_filter(if de { "Alle Dateien" } else { "All files" }, &["*"]);
                if let Some(dir) = self.db.as_deref().and_then(Path::parent) {
                    dlg = dlg.set_directory(dir);
                }
                if let Some(p) = dlg.pick_file() {
                    self.reopen(p);
                }
            }
            "clear_recent" => {
                self.settings.recent.clear();
                if let Some(db) = self.db.clone() {
                    self.settings.push_recent(&db);
                }
                self.save_settings();
            }
            "undo" | "redo" => {
                let r = if id == "undo" {
                    self.app.undo()
                } else {
                    self.app.redo()
                };
                match r {
                    Ok(m) => self.toasts.ok(m),
                    Err(e) => self.toasts.err(e),
                }
                if let Err(e) = self.app.reload() {
                    self.toasts.err(e);
                }
            }
            "cut" => self.edit_op(ctx, EditOp::Cut),
            "copy" => self.edit_op(ctx, EditOp::Copy),
            "paste" => self.edit_op(ctx, EditOp::Paste),
            "select_all" => self.edit_op(ctx, EditOp::SelectAll),
            "prefs" => match self.app.preferences_view() {
                Some(v) => f.goto = Some(v),
                None => self.help.open(Dialog::Preferences),
            },
            "zoom_in" | "zoom_out" | "zoom_reset" => {
                let z = match id {
                    "zoom_in" => ctx.zoom_factor() * 1.1,
                    "zoom_out" => ctx.zoom_factor() / 1.1,
                    _ => 1.0,
                }
                .clamp(shell::ZOOM_MIN, shell::ZOOM_MAX);
                ctx.set_zoom_factor(z);
                self.settings.zoom = z;
                self.save_settings();
            }
            "theme_dark" | "theme_light" => {
                shell::set_dark(ctx, &mut self.settings, id == "theme_dark");
                self.save_settings();
            }
            "lang_de" | "lang_en" => {
                self.settings.lang = if id == "lang_de" { "de" } else { "en" }.into();
                self.save_settings();
            }
            "reload" => {
                match self.app.reload() {
                    Ok(()) => self.toasts.ok(if de { "neu geladen" } else { "reloaded" }),
                    Err(e) => self.toasts.err(e),
                }
                self.dock.refresh(self.app.bus(), self.app.id(), true);
            }
            "layout_reset" => {
                self.settings.layout = shell::Layout::default();
                self.dock.open = false;
                layout_changed = true;
            }
            "toggle_nav" => {
                self.settings.layout.nav = !self.settings.layout.nav;
                layout_changed = true;
            }
            "toggle_quick" => {
                self.settings.layout.quick = !self.settings.layout.quick;
                layout_changed = true;
            }
            "toggle_detail" => {
                self.settings.layout.detail = !self.settings.layout.detail;
                layout_changed = true;
            }
            "toggle_status" => {
                self.settings.layout.status = !self.settings.layout.status;
                layout_changed = true;
            }
            "toggle_dock" => {
                self.dock.open = !self.dock.open;
                self.dock.refresh(self.app.bus(), self.app.id(), true);
            }
            "palette" => self.palette.open_list(),
            "shortcuts" => self.help.open(Dialog::Shortcuts),
            "docs" => self.help.open(Dialog::Docs),
            "licenses" => self.help.open(Dialog::Licenses),
            "about" => self.help.open(Dialog::About),
            "none" => {}
            other => {
                if let Some(p) = other.strip_prefix("recent:") {
                    self.reopen(PathBuf::from(p));
                } else {
                    self.toasts.err(format!("unknown menu action {other}"));
                }
            }
        }
        if layout_changed {
            self.save_settings();
        }
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
        // Esc closes a Help dialog before anything else can take the key.
        if self.help.is_open() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.help.open = None;
        }
        self.app.set_lang(self.settings.lang != "en");
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

        // Edit menu: remember the text field that had focus, and apply a
        // pending Cut/Copy/Paste/Select All to it before widgets run.
        let focused = ctx.memory(|m| m.focused());
        if let Some(id) = focused {
            self.last_focus = Some(id);
            self.last_sel =
                egui::TextEdit::load_state(&ctx, id).and_then(|s| s.cursor.char_range());
        }
        if let Some((id, op)) = self.pending_edit.take() {
            ctx.memory_mut(|m| m.request_focus(id));
            // Put back the selection the field had before the menu took focus.
            if let (Some(range), Some(mut st)) =
                (self.last_sel, egui::TextEdit::load_state(&ctx, id))
            {
                st.cursor.set_char_range(Some(range));
                st.store(&ctx, id);
            }
            match op {
                EditOp::Paste => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
                EditOp::Copy => ctx.input_mut(|i| i.events.push(egui::Event::Copy)),
                EditOp::Cut => ctx.input_mut(|i| i.events.push(egui::Event::Cut)),
                EditOp::SelectAll => ctx.input_mut(|i| {
                    i.events.push(egui::Event::Key {
                        key: Key::A,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::COMMAND,
                    })
                }),
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::P)) {
            self.palette.open_list();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Slash)) {
            self.help.open(Dialog::Shortcuts);
        }

        let mut f = self.frame();
        let nav = self.app.nav(&f);
        if self.view >= nav.len() {
            self.view = 0;
        }
        let t = theme::current(&ctx);

        let menus = self.build_menus(&f, &nav, focused.is_some());
        let mut picks = std::mem::take(&mut self.pending);
        if !self.help.is_open() {
            if let Some(p) = menu::consume_shortcuts(&ctx, &menus) {
                picks.push(p);
            }
        }
        egui::Panel::top("suite_menu")
            .frame(
                egui::Frame::new()
                    .fill(t.surface)
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ui, |ui| {
                if let Some(p) = menu::bar(ui, &menus) {
                    picks.push(p);
                }
            });
        for p in picks {
            self.handle_pick(&ctx, p, &mut f);
        }
        if self.settings.layout.quick {
            egui::Panel::top("suite_quick")
                .frame(
                    egui::Frame::new()
                        .fill(t.canvas)
                        .inner_margin(egui::Margin::symmetric(12, 4))
                        .stroke(egui::Stroke::new(1.0, t.border)),
                )
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (i, n) in nav.iter().enumerate() {
                            let label = format!("{}  {}", n.icon, n.label);
                            if ui
                                .selectable_label(i == self.view, label)
                                .on_hover_text(if f.lang_de {
                                    "Ansicht wechseln (auch im Menü Ansicht)"
                                } else {
                                    "Switch view (also in the View menu)"
                                })
                                .clicked()
                            {
                                self.view = i;
                            }
                        }
                    });
                });
        }

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
        if self.settings.layout.status {
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
        }
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

        if self.settings.layout.nav {
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
                            if shell::nav_item(ui, n.icon, &n.label, i == self.view, n.badge)
                                .clicked()
                            {
                                self.view = i;
                            }
                        }
                    });
                });
        }

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
        } else if self.settings.layout.detail && self.app.has_detail(self.view) {
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
        // Help dialogs.
        if self.help.is_open() {
            let info = self.app.info();
            let rows = self.shortcut_rows();
            let docs = self.app.docs();
            let ch = help::show(
                &ctx,
                &mut self.help,
                &info,
                &rows,
                &docs,
                f.lang_de,
                self.settings.dark,
                ctx.zoom_factor(),
            );
            if let Some(d) = ch.dark {
                shell::set_dark(&ctx, &mut self.settings, d);
            }
            if let Some(en) = ch.lang_en {
                self.settings.lang = if en { "en" } else { "de" }.into();
            }
            if let Some(z) = ch.zoom {
                ctx.set_zoom_factor(z);
                self.settings.zoom = z;
            }
            if ch.dark.is_some() || ch.lang_en.is_some() || ch.zoom.is_some() {
                self.save_settings();
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

    /// `--smoke`: run the status tool, then check the menu bar without a
    /// window: every menu command is a real tool with real argument names,
    /// no shortcut is used twice, every label has glyphs in the loaded fonts.
    pub fn smoke(&mut self) -> Result<Value, String> {
        let tool = self.app.smoke_tool();
        let mut v = self.app.call(tool, &serde_json::json!({}))?;
        let check = self.menu_check()?;
        if let Some(o) = v.as_object_mut() {
            o.insert("menu_check".into(), check);
        }
        Ok(v)
    }

    /// Menu parity and tofu guard (both languages). Err lists every problem.
    pub fn menu_check(&mut self) -> Result<Value, String> {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        let mut warm = ctx.run_ui(egui::RawInput::default(), |_| {});
        warm.textures_delta.clear();
        let tools = self.app.tools();
        let lang = self.settings.lang.clone();
        let mut out = serde_json::Map::new();
        let mut errors = Vec::new();
        for l in ["de", "en"] {
            self.settings.lang = l.into();
            let menus = self.menus();
            let n_tools = menu::parity(&menus, &tools).map_err(|e| format!("[{l}] {e}"));
            let mut extra: Vec<String> = self
                .shortcut_rows()
                .into_iter()
                .flat_map(|(g, k, w)| [g, k, w])
                .collect();
            let f = self.frame();
            extra.extend(
                self.app
                    .nav(&f)
                    .into_iter()
                    .flat_map(|n| [n.icon.to_string(), n.label]),
            );
            let tofu = menu::tofu_report(&ctx, &menus, &extra);
            // Glyph lookups may queue font-atlas updates; nobody renders them.
            let mut drain = ctx.run_ui(egui::RawInput::default(), |_| {});
            drain.textures_delta.clear();
            match n_tools {
                Ok(n) => {
                    out.insert(
                        l.into(),
                        serde_json::json!({
                            "menus": menus.iter().map(|m| m.title.clone()).collect::<Vec<_>>(),
                            "items": menu::leaves(&menus).len(),
                            "tool_items": n,
                        }),
                    );
                }
                Err(e) => errors.push(e),
            }
            for (s, chars) in tofu {
                errors.push(format!("[{l}] tofu in {s:?}: {chars:?}"));
            }
        }
        self.settings.lang = lang;
        if errors.is_empty() {
            Ok(Value::Object(out))
        } else {
            Err(errors.join("\n"))
        }
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
pub fn main_with<A: SuiteApp + 'static>(
    default_db: PathBuf,
    open: impl Fn(&std::path::Path) -> Result<A, String> + 'static,
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
    let mut sh = Shell::new(app, Some(shell::settings_path(&db)))
        .with_args(&args)
        .with_opener(db.clone(), open);
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
