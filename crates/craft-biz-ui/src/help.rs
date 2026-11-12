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

#[derive(Default)]
pub struct HelpState {
    pub open: Option<Dialog>,
    filter: String,
    doc: Option<(String, Result<String, String>)>,
}

impl HelpState {
    pub fn open(&mut self, d: Dialog) {
        self.open = Some(d);
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
            window(t("Projektdateien und Doku", "Project Files & Docs"))
                .open(&mut open)
                .default_width(820.0)
                .default_height(600.0)
                .show(ctx, |ui| docs_ui(ui, st, docs, lang_de));
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

fn docs_ui(ui: &mut Ui, st: &mut HelpState, docs: &[Doc], de: bool) {
    if docs.is_empty() {
        ui.label(if de {
            "Diese App nennt keine Dokumente."
        } else {
            "This app lists no documents."
        });
        return;
    }
    egui::Panel::left("craft_docs_list")
        .resizable(false)
        .exact_size(220.0)
        .show(ui, |ui| {
            for d in docs {
                let sel = st.doc.as_ref().is_some_and(|(t, _)| *t == d.title);
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
                    st.doc = Some((d.title.clone(), d.load()));
                }
            }
        });
    egui::CentralPanel::default().show(ui, |ui| match &st.doc {
        None => {
            ui.label(if de {
                "Links ein Dokument wählen."
            } else {
                "Choose a document on the left."
            });
        }
        Some((title, Ok(text))) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).family(semibold()));
                if ui
                    .small_button(if de { "Kopieren" } else { "Copy" })
                    .clicked()
                {
                    ui.ctx().copy_text(text.clone());
                }
            });
            ui.separator();
            egui::ScrollArea::both()
                .id_salt("craft_doc_text")
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
                e,
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
