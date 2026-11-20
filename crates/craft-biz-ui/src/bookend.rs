//! Menu bar for a window that does not use [`crate::app::Shell`].
//!
//! File, Edit, View, optional app menus, Window, Help. Help opens the five
//! standard dialogs. Zoom, theme, and quit change the window. Canvas items
//! that this window does not have stay on the menu and do nothing.

use crate::help::{self, AppInfo, Dialog, Doc, ShortcutRow};
use crate::menu::{self, Item, Menu, Picked};
use crate::shell::{ZOOM_MAX, ZOOM_MIN};
use crate::theme;
use egui::{Key, Modifiers, Ui};

/// What the window adds to the bookend.
pub struct Spec<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub description: &'a str,
    pub repository: &'a str,
    pub docs: &'a [Doc],
    pub dark: bool,
    pub zoom: f32,
    /// German page and menu copy. English when false.
    pub lang_de: bool,
    /// Extra File items, above Quit.
    pub file: Vec<Item>,
    /// Extra View items, above zoom.
    pub view: Vec<Item>,
    /// Menus between View and Window.
    pub middle: Vec<Menu>,
}

/// Theme, zoom, or an app action id (`shell.*` is handled here).
#[derive(Debug, Default)]
pub struct Change {
    pub action: Option<String>,
    pub dark: Option<bool>,
    pub zoom: Option<f32>,
    pub lang_de: Option<bool>,
}

#[derive(Default)]
pub struct Bar {
    help: help::HelpState,
    palette: bool,
    filter: String,
}

impl Bar {
    pub fn new() -> Bar {
        Bar::default()
    }

    /// Draw the bar and any open Help or palette window.
    pub fn show(&mut self, ui: &mut Ui, spec: &Spec<'_>) -> Change {
        let ctx = ui.ctx().clone();
        ctx.options_mut(|o| o.zoom_with_keyboard = true);
        if (ctx.zoom_factor() - spec.zoom).abs() > 1e-3 {
            ctx.set_zoom_factor(spec.zoom.clamp(ZOOM_MIN, ZOOM_MAX));
        }
        let wheel = ctx.input(|i| i.zoom_delta());
        let mut change = Change::default();
        if (wheel - 1.0).abs() > f32::EPSILON {
            let z = (ctx.zoom_factor() * wheel).clamp(ZOOM_MIN, ZOOM_MAX);
            ctx.set_zoom_factor(z);
            change.zoom = Some(z);
        }
        theme::apply(&ctx, spec.dark);

        let menus = menus(spec);
        let mut picked = menu::consume_shortcuts(&ctx, &menus);
        egui::Panel::top("craft_bookend").show(ui, |ui| {
            if let Some(p) = menu::bar(ui, &menus) {
                picked = Some(p);
            }
        });
        if let Some(p) = picked {
            self.apply_pick(&ctx, p, &mut change);
        }
        self.palette_ui(&ctx, &menus, spec, &mut change);

        let dark = change.dark.unwrap_or(spec.dark);
        let zoom = change.zoom.unwrap_or(spec.zoom);
        if let Some(d) = change.dark {
            theme::apply(&ctx, d);
        }
        let prefs = help::show(
            &ctx,
            &mut self.help,
            &info(spec),
            &shortcut_rows(&menus),
            spec.docs,
            spec.lang_de,
            dark,
            zoom,
        );
        if let Some(en) = prefs.lang_en {
            change.lang_de = Some(!en);
        }
        if let Some(d) = prefs.dark {
            change.dark = Some(d);
            theme::apply(&ctx, d);
        }
        if let Some(z) = prefs.zoom {
            let z = z.clamp(ZOOM_MIN, ZOOM_MAX);
            change.zoom = Some(z);
            ctx.set_zoom_factor(z);
        }
        change
    }
}

impl Bar {
    fn apply_pick(&mut self, ctx: &egui::Context, pick: Picked, change: &mut Change) {
        let Picked::Action(id) = pick else {
            return;
        };
        match id.as_str() {
            "shell.quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            "shell.prefs" => self.help.open(Dialog::Preferences),
            "shell.palette" => {
                self.palette = true;
                self.filter.clear();
            }
            "shell.shortcuts" => self.help.open(Dialog::Shortcuts),
            "shell.docs" => self.help.open(Dialog::Docs),
            "shell.licenses" => self.help.open(Dialog::Licenses),
            "shell.about" => self.help.open(Dialog::About),
            "shell.zoom_in" => {
                let z = (ctx.zoom_factor() * 1.1).clamp(ZOOM_MIN, ZOOM_MAX);
                ctx.set_zoom_factor(z);
                change.zoom = Some(z);
            }
            "shell.zoom_out" => {
                let z = (ctx.zoom_factor() / 1.1).clamp(ZOOM_MIN, ZOOM_MAX);
                ctx.set_zoom_factor(z);
                change.zoom = Some(z);
            }
            "shell.zoom_reset" => {
                ctx.set_zoom_factor(1.0);
                change.zoom = Some(1.0);
            }
            "shell.theme_dark" => change.dark = Some(true),
            "shell.theme_light" => change.dark = Some(false),
            "shell.lang_de" => change.lang_de = Some(true),
            "shell.lang_en" => change.lang_de = Some(false),
            other => change.action = Some(other.to_string()),
        }
    }

    fn palette_ui(
        &mut self,
        ctx: &egui::Context,
        menus: &[Menu],
        spec: &Spec<'_>,
        change: &mut Change,
    ) {
        if !self.palette {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.palette = false;
            return;
        }
        let mut open = true;
        let mut chosen: Option<Picked> = None;
        egui::Window::new(format!("Command Palette - {}", spec.name))
            .collapsible(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.text_edit_singleline(&mut self.filter);
                let q = self.filter.to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for it in menu::leaves(menus) {
                            if !it.enabled || it.label.is_empty() {
                                continue;
                            }
                            if !q.is_empty() && !it.label.to_lowercase().contains(&q) {
                                continue;
                            }
                            if ui.button(&it.label).clicked() {
                                chosen = it_pick(it);
                            }
                        }
                    });
            });
        if !open {
            self.palette = false;
        }
        if let Some(p) = chosen {
            self.palette = false;
            self.apply_pick(ctx, p, change);
        }
    }
}

fn it_pick(it: &Item) -> Option<Picked> {
    match &it.kind {
        menu::Kind::Action(id) => Some(Picked::Action(id.clone())),
        menu::Kind::Run { tool, args } => Some(Picked::Run(tool.clone(), args.clone())),
        menu::Kind::Form { tool, prefill } => Some(Picked::Form(tool.clone(), prefill.clone())),
        menu::Kind::View(v) => Some(Picked::View(*v)),
        menu::Kind::Separator | menu::Kind::Submenu(_) => None,
    }
}

/// The bar a window would draw for this spec.
pub fn menus(spec: &Spec<'_>) -> Vec<Menu> {
    let de = spec.lang_de;
    let t = |a: &str, b: &str| if de { a.to_string() } else { b.to_string() };
    let cmd = Modifiers::COMMAND;
    let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
    let no_canvas = t(
        "Dieses Fenster hat keine Zeichenfläche.",
        "This window has no canvas.",
    );
    let no_stack = t(
        "Dieses Fenster hat keinen Undo-Stapel.",
        "This window has no undo stack.",
    );
    let text_field = t(
        "Das fokussierte Textfeld übernimmt das.",
        "The focused text field handles this.",
    );

    let mut file = spec.file.clone();
    if !file.is_empty() {
        file.push(Item::sep());
    }
    file.push(
        Item::action(t("Beenden", "Quit"), "shell.quit")
            .key(cmd, Key::Q)
            .key_text("Ctrl+Q"),
    );

    let edit = vec![
        Item::action(t("Rückgängig", "Undo"), "shell.none")
            .enabled(false)
            .hint(no_stack.clone())
            .key_text("Ctrl+Z"),
        Item::action(t("Wiederholen", "Redo"), "shell.none")
            .enabled(false)
            .hint(no_stack)
            .key_text("Ctrl+Y"),
        Item::sep(),
        Item::action(t("Ausschneiden", "Cut"), "shell.none")
            .enabled(false)
            .hint(text_field.clone())
            .key_text("Ctrl+X"),
        Item::action(t("Kopieren", "Copy"), "shell.none")
            .enabled(false)
            .hint(text_field.clone())
            .key_text("Ctrl+C"),
        Item::action(t("Einfügen", "Paste"), "shell.none")
            .enabled(false)
            .hint(text_field.clone())
            .key_text("Ctrl+V"),
        Item::action(t("Alles auswählen", "Select All"), "shell.none")
            .enabled(false)
            .hint(text_field)
            .key_text("Ctrl+A"),
        Item::sep(),
        Item::action(t("Einstellungen...", "Preferences..."), "shell.prefs")
            .key(cmd, Key::Comma)
            .key_text("Ctrl+,"),
    ];

    let mut view = spec.view.clone();
    if !view.is_empty() {
        view.push(Item::sep());
    }
    view.extend([
        Item::action(t("Vergrößern", "Zoom In"), "shell.zoom_in").key_text("Ctrl+="),
        Item::action(t("Verkleinern", "Zoom Out"), "shell.zoom_out").key_text("Ctrl+-"),
        Item::action(t("An Fenster anpassen", "Fit to Window"), "shell.none")
            .enabled(false)
            .hint(no_canvas.clone())
            .key_text("Ctrl+0"),
        Item::action(t("Zoom 100%", "Reset Zoom (100%)"), "shell.zoom_reset").key_text("Ctrl+1"),
        Item::sep(),
        Item::action(t("Raster", "Toggle Grid"), "shell.none")
            .enabled(false)
            .hint(no_canvas.clone()),
        Item::action(t("Hilfslinien", "Toggle Guides"), "shell.none")
            .enabled(false)
            .hint(no_canvas.clone()),
        Item::action(t("Zwiebelschicht", "Onion Skinning"), "shell.none")
            .enabled(false)
            .hint(no_canvas),
        Item::sep(),
        Item::sub(
            t("Farbschema", "Interface Theme"),
            vec![
                Item::action(t("Dunkel", "Dark"), "shell.theme_dark").checked(spec.dark),
                Item::action(t("Hell", "Light"), "shell.theme_light").checked(!spec.dark),
            ],
        ),
        Item::sub(
            t("Sprache", "Language"),
            vec![
                Item::action("Deutsch", "shell.lang_de").checked(de),
                Item::action("English", "shell.lang_en").checked(!de),
            ],
        ),
    ]);

    let window = vec![
        Item::action(t("Layout zurücksetzen", "Reset Panel Layout"), "shell.none")
            .enabled(false)
            .hint(t(
                "Dieses Fenster dockt keine Paneele an.",
                "This window does not dock panels.",
            )),
    ];

    let help = vec![
        Item::action(
            t("Befehlspalette...", "Command Palette..."),
            "shell.palette",
        )
        .key(cmd_shift, Key::P)
        .key_text("Ctrl+Shift+P"),
        Item::action(
            t("Tastenkürzel...", "Keyboard Shortcuts..."),
            "shell.shortcuts",
        )
        .key(cmd, Key::Slash)
        .key_text("Ctrl+/"),
        Item::action(
            t("Projektdateien und Doku...", "Project Files & Docs..."),
            "shell.docs",
        ),
        Item::action(
            t("Lizenzen und Hinweise...", "Licenses & Notices..."),
            "shell.licenses",
        ),
        Item::action(
            format!("{} {}", t("Über", "About"), spec.name),
            "shell.about",
        ),
    ];

    let mut out = vec![
        Menu::new(t("Datei", "File"), file),
        Menu::new(t("Bearbeiten", "Edit"), edit),
        Menu::new(t("Ansicht", "View"), view),
    ];
    out.extend(spec.middle.clone());
    out.push(Menu::new(t("Fenster", "Window"), window));
    out.push(Menu::new(t("Hilfe", "Help"), help));
    out
}

/// File, Edit, View, then Window, Help, and the five Help items.
pub fn check(menus: &[Menu]) -> Result<(), String> {
    let titles: Vec<&str> = menus.iter().map(|m| m.title.as_str()).collect();
    let n = titles.len();
    if n < 5
        || titles.first() != Some(&"File")
        || titles.get(1) != Some(&"Edit")
        || titles.get(2) != Some(&"View")
        || titles.get(n - 2) != Some(&"Window")
        || titles.last() != Some(&"Help")
    {
        return Err(format!("menu titles: {}", titles.join(", ")));
    }
    let help = menus
        .iter()
        .find(|m| m.title == "Help")
        .map(|m| &m.items)
        .ok_or_else(|| "Help missing".to_string())?;
    for label in [
        "Command Palette...",
        "Keyboard Shortcuts...",
        "Project Files & Docs...",
        "Licenses & Notices...",
    ] {
        if !help.iter().any(|it| it.label == label) {
            return Err(format!("Help is missing {label}"));
        }
    }
    if !help.iter().any(|it| it.label.starts_with("About ")) {
        return Err("Help is missing About".into());
    }
    Ok(())
}

/// Docs next to a crate whose manifest sits in `crates/<app>/`.
pub fn repo_docs(manifest_dir: &str) -> Vec<Doc> {
    let root = std::path::PathBuf::from(manifest_dir).join("../..");
    vec![
        Doc::file("PRD", root.join("docs/PRD.md")),
        Doc::file("README", root.join("README.md")),
        Doc::file("Changelog", root.join("CHANGELOG.md")),
        Doc::file("Handover", root.join("HANDOVER.md")),
    ]
}

/// German when `de` is set, otherwise English.
pub fn t<'a>(de: bool, german: &'a str, english: &'a str) -> &'a str {
    if de {
        german
    } else {
        english
    }
}

/// `--smoke` check for the bookend with no app-specific items.
pub fn smoke(name: &str, version: &str, description: &str) -> Result<(), String> {
    let spec = Spec {
        name,
        version,
        description,
        repository: "",
        docs: &[],
        dark: true,
        zoom: 1.0,
        lang_de: false,
        file: Vec::new(),
        view: Vec::new(),
        middle: Vec::new(),
    };
    check(&menus(&spec))
}

fn info(spec: &Spec<'_>) -> AppInfo {
    AppInfo {
        name: spec.name.into(),
        version: spec.version.into(),
        description: spec.description.into(),
        license: "MIT OR Apache-2.0".into(),
        repository: spec.repository.into(),
        commit: "not recorded".into(),
        built: "not recorded".into(),
        safety:
            "Application code denies unwrap, expect, and panic. Dependencies are not audited here."
                .into(),
        third_party: Vec::new(),
    }
}

fn shortcut_rows(menus: &[Menu]) -> Vec<ShortcutRow> {
    let mut rows = Vec::new();
    for m in menus {
        walk(&m.title, &m.items, &mut rows);
    }
    rows
}

fn walk(group: &str, items: &[Item], out: &mut Vec<ShortcutRow>) {
    for it in items {
        match &it.kind {
            menu::Kind::Submenu(children) => walk(&it.label, children, out),
            menu::Kind::Separator => {}
            _ => {
                if let Some(keys) = &it.shortcut_text {
                    out.push((group.to_string(), keys.clone(), it.label.clone()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookend_keeps_file_edit_view_window_help_and_the_help_suite() {
        let err = smoke("BillCraft", "0.1.0", "invoices");
        assert!(err.is_ok(), "{err:?}");
        let docs = repo_docs(env!("CARGO_MANIFEST_DIR"));
        let spec = Spec {
            name: "BillCraft",
            version: "0.1.0",
            description: "invoices",
            repository: "",
            docs: &docs,
            dark: true,
            zoom: 1.0,
            lang_de: false,
            file: vec![Item::action("Refresh", "app.refresh")],
            view: vec![Item::action("Invoices", "view.invoices").checked(true)],
            middle: vec![Menu::new(
                "Invoice",
                vec![Item::action("Issue", "app.issue")],
            )],
        };
        let menus = menus(&spec);
        let err = check(&menus);
        assert!(err.is_ok(), "{err:?}");
        assert!(menus.iter().any(|m| m.title == "Invoice"));
    }
}
