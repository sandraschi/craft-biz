//! The Help suite of the Craft Menu Bar Standard. The Command Palette is the
//! existing [`crate::palette`]; this module draws the other four dialogs:
//! Keyboard Shortcuts, Project Files & Docs, Licenses & Notices, About.

use crate::theme::{self, semibold};
use egui::{Context, RichText, Ui};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    Shortcuts,
    Docs,
    Licenses,
    About,
    Preferences,
}

/// Facts shown in About and Licenses. Apps fill this from their build script.
#[derive(Debug, Clone, Default)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub license: String,
    pub repository: String,
    /// Short commit SHA at build time ("unknown" outside git).
    pub commit: String,
    /// Build date (YYYY-MM-DD).
    pub built: String,
    /// One line about memory safety that is true for this app.
    pub safety: String,
    /// Third-party crates: (name, version, license), from Cargo.lock at build time.
    pub third_party: Vec<(String, String, String)>,
}

/// One row of the shortcut sheet: (group, keys, what it does).
pub type ShortcutRow = (String, String, String);

/// A document for Help > Project Files & Docs.
#[derive(Debug, Clone)]
pub struct Doc {
    pub title: String,
    /// Where the file lives in the repository (shown, and read when `text` is None).
    pub path: PathBuf,
    /// Copy compiled into the binary, so an installed app has its docs too.
    pub text: Option<&'static str>,
}

impl Doc {
    pub fn embedded(title: &str, path: PathBuf, text: &'static str) -> Doc {
        Doc {
            title: title.into(),
            path,
            text: Some(text),
        }
    }
    pub fn file(title: &str, path: PathBuf) -> Doc {
        Doc {
            title: title.into(),
            path,
            text: None,
        }
    }
    fn load(&self) -> Result<String, String> {
        match self.text {
            Some(t) => Ok(t.to_string()),
            None => std::fs::read_to_string(&self.path)
                .map_err(|e| format!("{}: {e}", self.path.display())),
        }
    }
    fn available(&self) -> bool {
        self.text.is_some() || self.path.exists()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelpTab {
    GetStarted,
    Functions,
    Usage,
    TipsTricks,
    PitfallsSafety,
    PlannedExtensions,
    Skills,
    OtherCrafts,
    #[default]
    Docs,
}

#[derive(Debug, Clone, Default)]
pub struct HelpSections {
    pub get_started: String,
    pub functions: String,
    pub usage: String,
    pub tips_tricks: String,
    pub pitfalls_safety: String,
    pub planned_extensions: String,
    pub other_crafts: String,
}

/// Split a `HELP.md` document by `## ` headings into the standard Help Center tabs.
pub fn parse_help_md(md: &str) -> HelpSections {
    let mut out = HelpSections::default();
    let mut current: Option<&'static str> = None;
    let mut buf = String::new();

    let flush = |sec: Option<&'static str>, text: &str, out: &mut HelpSections| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        match sec {
            Some("get_started") => out.get_started = trimmed.to_string(),
            Some("functions") => out.functions = trimmed.to_string(),
            Some("usage") => out.usage = trimmed.to_string(),
            Some("tips") => out.tips_tricks = trimmed.to_string(),
            Some("pitfalls") => out.pitfalls_safety = trimmed.to_string(),
            Some("planned") => out.planned_extensions = trimmed.to_string(),
            Some("other") => out.other_crafts = trimmed.to_string(),
            _ => {}
        }
    };

    for line in md.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            flush(current, &buf, &mut out);
            buf.clear();
            let h = heading.to_lowercase();
            current = if h.contains("get started") || h.contains("what ") {
                Some("get_started")
            } else if h.contains("function") {
                Some("functions")
            } else if h.contains("usage") {
                Some("usage")
            } else if h.contains("tip") {
                Some("tips")
            } else if h.contains("pitfall") || h.contains("safety") {
                Some("pitfalls")
            } else if h.contains("planned") {
                Some("planned")
            } else if h.contains("other craft") {
                Some("other")
            } else {
                None
            };
            buf.push_str(line);
            buf.push('\n');
        } else if current.is_some() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    flush(current, &buf, &mut out);
    out
}

#[derive(Default)]
pub struct HelpState {
    pub open: Option<Dialog>,
    pub tab: HelpTab,
    filter: String,
    doc: Option<(String, Result<String, String>)>,
    skill: Option<(String, Result<String, String>)>,
}

impl HelpState {
    pub fn open(&mut self, d: Dialog) {
        self.open = Some(d);
        if d == Dialog::Docs {
            self.tab = HelpTab::Docs;
        }
        self.filter.clear();
    }
    pub fn open_tab(&mut self, tab: HelpTab) {
        self.tab = tab;
        self.open = Some(Dialog::Docs);
        self.filter.clear();
    }
    pub fn open_doc(&mut self, title: String, body: String) {
        self.doc = Some((title, Ok(body)));
        self.tab = HelpTab::Docs;
        self.open = Some(Dialog::Docs);
        self.filter.clear();
    }
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

/// What the preferences dialog changed.
#[derive(Debug, Default, Clone, Copy)]
pub struct PrefsChange {
    pub dark: Option<bool>,
    pub lang_en: Option<bool>,
    pub zoom: Option<f32>,
}

fn window(title: &str) -> egui::Window<'static> {
    egui::Window::new(
        RichText::new(title.to_string())
            .family(semibold())
            .size(16.0),
    )
    .id(egui::Id::unique(format!("craft_help_{title}")))
    .collapsible(false)
    .resizable(true)
    .default_width(620.0)
    .default_height(480.0)
    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -20.0))
}

/// Draw the open dialog (if any). Esc closes it.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ctx: &Context,
    st: &mut HelpState,
    info: &AppInfo,
    shortcuts: &[ShortcutRow],
    docs: &[Doc],
    lang_de: bool,
    dark: bool,
    zoom: f32,
) -> PrefsChange {
    show_center(
        ctx,
        st,
        info,
        shortcuts,
        docs,
        &[],
        None,
        lang_de,
        dark,
        zoom,
    )
}

/// Draw the open dialog with optional `HELP.md` sections and bundled skills. Esc closes it.
#[allow(clippy::too_many_arguments)]
pub fn show_center(
    ctx: &Context,
    st: &mut HelpState,
    info: &AppInfo,
    shortcuts: &[ShortcutRow],
    docs: &[Doc],
    skills: &[Doc],
    help_md: Option<&str>,
    lang_de: bool,
    dark: bool,
    zoom: f32,
) -> PrefsChange {
    let mut change = PrefsChange::default();
    let Some(d) = st.open else { return change };
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        st.open = None;
        return change;
    }
    let t = |de: &'static str, en: &'static str| if lang_de { de } else { en };
    let mut open = true;
    match d {
        Dialog::Shortcuts => {
            window(t("Tastenkürzel", "Keyboard Shortcuts"))
                .open(&mut open)
                .show(ctx, |ui| shortcuts_ui(ui, st, shortcuts, lang_de));
        }
        Dialog::Docs => {
            window(&format!(
                "{} — {}",
                info.name,
                t("Hilfecenter", "Help Center")
            ))
            .open(&mut open)
            .default_width(880.0)
            .default_height(640.0)
            .show(ctx, |ui| {
                help_center_ui(ui, st, info, docs, skills, help_md, lang_de)
            });
        }
        Dialog::Licenses => {
            window(t("Lizenzen und Hinweise", "Licenses & Notices"))
                .open(&mut open)
                .show(ctx, |ui| licenses_ui(ui, st, info, lang_de));
        }
        Dialog::About => {
            window(&format!("{} {}", t("Über", "About"), info.name))
                .open(&mut open)
                .resizable(false)
                .default_width(460.0)
                .show(ctx, |ui| about_ui(ui, info, lang_de));
        }
        Dialog::Preferences => {
            window(t("Einstellungen", "Preferences"))
                .open(&mut open)
                .resizable(false)
                .default_width(420.0)
                .show(ctx, |ui| {
                    change = prefs_ui(ui, lang_de, dark, zoom);
                });
        }
    }
    if !open {
        st.open = None;
    }
    change
}

fn shortcuts_ui(ui: &mut Ui, st: &mut HelpState, rows: &[ShortcutRow], de: bool) {
    let tk = theme::current(ui.ctx());
    ui.add(
        egui::TextEdit::singleline(&mut st.filter)
            .hint_text(if de { "Filtern..." } else { "Filter..." })
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);
    let q = st.filter.to_lowercase();
    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut group = "";
        egui::Grid::new("craft_shortcuts")
            .num_columns(2)
            .spacing([24.0, 6.0])
            .striped(true)
            .show(ui, |ui| {
                for (g, keys, what) in rows.iter().filter(|(g, k, w)| {
                    q.is_empty()
                        || g.to_lowercase().contains(&q)
                        || k.to_lowercase().contains(&q)
                        || w.to_lowercase().contains(&q)
                }) {
                    if g != group {
                        group = g;
                        ui.label(RichText::new(g).family(semibold()).color(tk.text_weak));
                        ui.end_row();
                    }
                    ui.label(RichText::new(keys).monospace().strong());
                    ui.label(what);
                    ui.end_row();
                }
            });
    });
}

fn help_center_ui(
    ui: &mut Ui,
    st: &mut HelpState,
    info: &AppInfo,
    docs: &[Doc],
    skills: &[Doc],
    help_md: Option<&str>,
    de: bool,
) {
    let mut tabs = vec![
        (
            HelpTab::GetStarted,
            if de { "Erste Schritte" } else { "Get Started" },
        ),
        (
            HelpTab::Functions,
            if de { "Funktionen" } else { "Functions" },
        ),
        (HelpTab::Usage, if de { "Bedienung" } else { "Usage" }),
        (
            HelpTab::TipsTricks,
            if de {
                "Tipps & Tricks"
            } else {
                "Tips & Tricks"
            },
        ),
        (
            HelpTab::PitfallsSafety,
            if de {
                "Sicherheit"
            } else {
                "Pitfalls & Safety"
            },
        ),
        (
            HelpTab::PlannedExtensions,
            if de { "Planung" } else { "Planned Extensions" },
        ),
    ];
    if !skills.is_empty() {
        tabs.push((
            HelpTab::Skills,
            if de { "Skills" } else { "Bundled Skills" },
        ));
    }
    tabs.push((
        HelpTab::OtherCrafts,
        if de { "Weitere Crafts" } else { "Other Crafts" },
    ));
    tabs.push((
        HelpTab::Docs,
        if de {
            "Projektdateien"
        } else {
            "Project Files & Docs"
        },
    ));

    ui.horizontal_wrapped(|ui| {
        for (tab, label) in tabs {
            if ui.selectable_label(st.tab == tab, label).clicked() {
                st.tab = tab;
            }
        }
    });
    ui.separator();

    let sections = help_md.map(parse_help_md).unwrap_or_default();
    match st.tab {
        HelpTab::GetStarted => {
            let fallback = format!(
                "# Getting Started - {}\n\n\
                 1. Use the left navigation rail to switch between workspace views.\n\
                 2. Press Ctrl+K for the Command Palette or F1 for Keyboard Shortcuts.\n\
                 3. Open Project Files & Docs to read embedded documentation.",
                info.name
            );
            let body = if sections.get_started.is_empty() {
                fallback
            } else {
                sections.get_started
            };
            section_text_ui(ui, "Get Started", &body, de);
        }
        HelpTab::Functions => {
            let body = if sections.functions.is_empty() {
                format!("No dedicated Functions section in {} HELP.md.", info.name)
            } else {
                sections.functions
            };
            section_text_ui(ui, "Functions", &body, de);
        }
        HelpTab::Usage => {
            let body = if sections.usage.is_empty() {
                format!(
                    "# Usage - {}\n\n- Press Ctrl+K for the Command Palette.\n- Run with `--help` for CLI options.",
                    info.name
                )
            } else {
                sections.usage
            };
            section_text_ui(ui, "Usage (GUI, CLI & MCP)", &body, de);
        }
        HelpTab::TipsTricks => {
            let body = if sections.tips_tricks.is_empty() {
                "No Tips & Tricks section recorded yet.".to_string()
            } else {
                sections.tips_tricks
            };
            section_text_ui(ui, "Tips & Tricks", &body, de);
        }
        HelpTab::PitfallsSafety => {
            let body = if sections.pitfalls_safety.is_empty() {
                if info.safety.is_empty() {
                    "No Common Pitfalls & Safety section recorded yet.".to_string()
                } else {
                    info.safety.clone()
                }
            } else {
                sections.pitfalls_safety
            };
            section_text_ui(ui, "Common Pitfalls & Safety", &body, de);
        }
        HelpTab::PlannedExtensions => {
            let body = if sections.planned_extensions.is_empty() {
                "Nothing planned right now.".to_string()
            } else {
                sections.planned_extensions
            };
            section_text_ui(ui, "Planned Extensions", &body, de);
        }
        HelpTab::OtherCrafts => {
            let body = if sections.other_crafts.is_empty() {
                "See Project Files & Docs for ecosystem references.".to_string()
            } else {
                sections.other_crafts
            };
            section_text_ui(ui, "Other Crafts", &body, de);
        }
        HelpTab::Skills => doc_list_ui(ui, &mut st.skill, skills, "craft_skills_list", de),
        HelpTab::Docs => doc_list_ui(ui, &mut st.doc, docs, "craft_docs_list", de),
    }
}

fn section_text_ui(ui: &mut Ui, title: &str, text: &str, de: bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).family(semibold()));
        if ui
            .small_button(if de { "Kopieren" } else { "Copy" })
            .clicked()
        {
            ui.ctx().copy_text(text.to_string());
        }
    });
    ui.separator();
    egui::ScrollArea::vertical()
        .id_salt(("craft_help_sec", title))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(RichText::new(text).monospace().size(13.0))
                    .wrap_mode(egui::TextWrapMode::Wrap),
            );
        });
}

fn doc_list_ui(
    ui: &mut Ui,
    selected: &mut Option<(String, Result<String, String>)>,
    docs: &[Doc],
    salt: &str,
    de: bool,
) {
    if selected.is_none() {
        if let Some(first) = docs.iter().find(|d| d.available()) {
            *selected = Some((first.title.clone(), first.load()));
        }
    }
    if docs.is_empty() && selected.is_none() {
        ui.label(if de {
            "Keine Dokumente vorhanden."
        } else {
            "This app lists no documents."
        });
        return;
    }
    egui::Panel::left(salt)
        .resizable(false)
        .exact_size(220.0)
        .show(ui, |ui| {
            for d in docs {
                let sel = selected.as_ref().is_some_and(|(t, _)| *t == d.title);
                let ok = d.available();
                let r = ui
                    .add_enabled(ok, egui::Button::selectable(sel, d.title.as_str()))
                    .on_hover_text(d.path.display().to_string());
                let r = if ok {
                    r
                } else {
                    r.on_disabled_hover_text(format!(
                        "{} {}",
                        if de { "nicht gefunden:" } else { "not found:" },
                        d.path.display()
                    ))
                };
                if r.clicked() {
                    *selected = Some((d.title.clone(), d.load()));
                }
            }
        });
    egui::CentralPanel::default().show(ui, |ui| match selected {
        None => {
            ui.label(if de {
                "Links ein Dokument wählen."
            } else {
                "Choose a document on the left."
            });
        }
        Some((title, Ok(text))) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(title.as_str()).family(semibold()));
                if ui
                    .small_button(if de { "Kopieren" } else { "Copy" })
                    .clicked()
                {
                    ui.ctx().copy_text(text.clone());
                }
            });
            ui.separator();
            egui::ScrollArea::both()
                .id_salt(("craft_doc_text", salt))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(text.as_str()).monospace().size(13.0))
                            .wrap_mode(egui::TextWrapMode::Wrap),
                    );
                });
        }
        Some((_, Err(e))) => {
            ui.colored_label(
                theme::tone_colors(crate::Tone::Danger, ui.visuals().dark_mode).0,
                e.as_str(),
            );
        }
    });
}

fn licenses_ui(ui: &mut Ui, st: &mut HelpState, info: &AppInfo, de: bool) {
    ui.label(
        RichText::new(format!(
            "{} {} - {}",
            info.name,
            info.version,
            if info.license.is_empty() {
                "MIT OR Apache-2.0"
            } else {
                &info.license
            }
        ))
        .family(semibold()),
    );
    ui.label(if de {
        "Sie dürfen die Software unter der MIT-Lizenz oder der Apache-Lizenz 2.0 verwenden, nach Ihrer Wahl. Ohne Gewährleistung."
    } else {
        "You may use this software under the MIT license or the Apache License 2.0, at your option. Provided without warranty."
    });
    if !info.safety.is_empty() {
        ui.add_space(4.0);
        ui.label(&info.safety);
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new(format!(
            "{} ({})",
            if de {
                "Fremdkomponenten"
            } else {
                "Third-party components"
            },
            info.third_party.len()
        ))
        .family(semibold()),
    );
    ui.add(
        egui::TextEdit::singleline(&mut st.filter)
            .hint_text(if de { "Filtern..." } else { "Filter..." })
            .desired_width(f32::INFINITY),
    );
    let q = st.filter.to_lowercase();
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("craft_licenses")
            .num_columns(3)
            .striped(true)
            .spacing([18.0, 4.0])
            .show(ui, |ui| {
                for (name, ver, lic) in info.third_party.iter().filter(|(n, _, l)| {
                    q.is_empty() || n.to_lowercase().contains(&q) || l.to_lowercase().contains(&q)
                }) {
                    ui.label(RichText::new(name).monospace());
                    ui.label(RichText::new(ver).monospace());
                    ui.label(lic);
                    ui.end_row();
                }
            });
        if info.third_party.is_empty() {
            ui.label(if de {
                "Liste beim Build nicht erzeugt."
            } else {
                "List was not generated at build time."
            });
        }
    });
}

fn about_ui(ui: &mut Ui, info: &AppInfo, de: bool) {
    ui.vertical_centered(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new(&info.name).family(theme::bold()).size(26.0));
        ui.label(RichText::new(format!("Version {}", info.version)).size(15.0));
        ui.add_space(4.0);
        if !info.description.is_empty() {
            ui.label(&info.description);
        }
    });
    ui.add_space(10.0);
    egui::Grid::new("craft_about")
        .num_columns(2)
        .spacing([16.0, 6.0])
        .show(ui, |ui| {
            let rows = [
                (if de { "Build" } else { "Built" }, info.built.as_str()),
                ("Commit", info.commit.as_str()),
                (if de { "Lizenz" } else { "License" }, info.license.as_str()),
                (
                    if de { "Quelle" } else { "Source" },
                    info.repository.as_str(),
                ),
            ];
            for (k, v) in rows {
                if v.is_empty() {
                    continue;
                }
                ui.label(RichText::new(k).family(semibold()));
                ui.label(RichText::new(v).monospace());
                ui.end_row();
            }
        });
    if !info.safety.is_empty() {
        ui.add_space(8.0);
        ui.label(&info.safety);
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if !info.repository.is_empty()
            && ui
                .button(if de {
                    "Quelle kopieren"
                } else {
                    "Copy source link"
                })
                .clicked()
        {
            ui.ctx().copy_text(info.repository.clone());
        }
        if ui
            .button(if de {
                "Versionsinfo kopieren"
            } else {
                "Copy version info"
            })
            .clicked()
        {
            ui.ctx().copy_text(format!(
                "{} {} ({} {})",
                info.name, info.version, info.commit, info.built
            ));
        }
    });
}

fn prefs_ui(ui: &mut Ui, de: bool, dark: bool, zoom: f32) -> PrefsChange {
    let mut c = PrefsChange::default();
    egui::Grid::new("craft_prefs")
        .num_columns(2)
        .spacing([16.0, 10.0])
        .show(ui, |ui| {
            ui.label(if de { "Farbschema" } else { "Theme" });
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(!dark, if de { "Hell" } else { "Light" })
                    .clicked()
                {
                    c.dark = Some(false);
                }
                if ui
                    .selectable_label(dark, if de { "Dunkel" } else { "Dark" })
                    .clicked()
                {
                    c.dark = Some(true);
                }
            });
            ui.end_row();
            ui.label(if de { "Sprache" } else { "Language" });
            ui.horizontal(|ui| {
                if ui.selectable_label(de, "Deutsch").clicked() {
                    c.lang_en = Some(false);
                }
                if ui.selectable_label(!de, "English").clicked() {
                    c.lang_en = Some(true);
                }
            });
            ui.end_row();
            ui.label("Zoom");
            let mut z = zoom;
            if ui
                .add(
                    egui::Slider::new(&mut z, crate::shell::ZOOM_MIN..=crate::shell::ZOOM_MAX)
                        .step_by(0.05)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                )
                .changed()
            {
                c.zoom = Some(z);
            }
            ui.end_row();
        });
    ui.add_space(6.0);
    ui.label(
        RichText::new(if de {
            "Wird in ui.json neben der Datenbank gespeichert."
        } else {
            "Saved in ui.json next to the database."
        })
        .color(theme::current(ui.ctx()).text_weak),
    );
    c
}
