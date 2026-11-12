//! Top menu bar of the Craft Menu Bar Standard: `File  Edit  View  [app menus]
//! Window  Help`, `switch_on_hover`, keyboard shortcuts, and checks used by
//! tests and `--smoke` (every menu command is a real tool with real argument
//! names; every label has glyphs in the loaded fonts).
//!
//! Menu items never change data themselves: they resolve to a [`Picked`]
//! that the shell turns into a tool call, a form, a view switch or an action.

use egui::{Context, FontFamily, FontId, Key, KeyboardShortcut, Modifiers, Ui};
use serde_json::Value;

/// What a menu item does when chosen.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// Run a tool with these arguments (result shown).
    Run {
        tool: String,
        args: Value,
    },
    /// Open the generated form of a tool, prefilled.
    Form {
        tool: String,
        prefill: Vec<(String, String)>,
    },
    /// Shell action (`shell.*`) or app action (anything else).
    Action(String),
    /// Switch to view N.
    View(usize),
    Separator,
    Submenu(Vec<Item>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub label: String,
    pub kind: Kind,
    pub shortcut: Option<KeyboardShortcut>,
    /// Shortcut text shown when the key is handled elsewhere (text fields, egui).
    pub shortcut_text: Option<String>,
    pub enabled: bool,
    pub checked: Option<bool>,
    /// Tooltip, e.g. why an item is disabled.
    pub hint: Option<String>,
}

impl Item {
    fn new(label: impl Into<String>, kind: Kind) -> Item {
        Item {
            label: label.into(),
            kind,
            shortcut: None,
            shortcut_text: None,
            enabled: true,
            checked: None,
            hint: None,
        }
    }
    pub fn run(label: impl Into<String>, tool: &str, args: Value) -> Item {
        Item::new(
            label,
            Kind::Run {
                tool: tool.into(),
                args,
            },
        )
    }
    pub fn form(label: impl Into<String>, tool: &str, prefill: &[(&str, &str)]) -> Item {
        Item::new(
            label,
            Kind::Form {
                tool: tool.into(),
                prefill: prefill
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            },
        )
    }
    pub fn action(label: impl Into<String>, id: &str) -> Item {
        Item::new(label, Kind::Action(id.into()))
    }
    pub fn view(label: impl Into<String>, view: usize) -> Item {
        Item::new(label, Kind::View(view))
    }
    pub fn sep() -> Item {
        Item::new("", Kind::Separator)
    }
    pub fn sub(label: impl Into<String>, items: Vec<Item>) -> Item {
        Item::new(label, Kind::Submenu(items))
    }
    /// Global shortcut: the shell consumes it and picks this item.
    pub fn key(mut self, modifiers: Modifiers, key: Key) -> Item {
        self.shortcut = Some(KeyboardShortcut::new(modifiers, key));
        self
    }
    /// Shortcut text only (the key is handled by text fields or egui itself).
    pub fn key_text(mut self, text: &str) -> Item {
        self.shortcut_text = Some(text.into());
        self
    }
    pub fn enabled(mut self, on: bool) -> Item {
        self.enabled = on;
        self
    }
    pub fn checked(mut self, on: bool) -> Item {
        self.checked = Some(on);
        self
    }
    pub fn hint(mut self, text: impl Into<String>) -> Item {
        self.hint = Some(text.into());
        self
    }
    fn pick(&self) -> Option<Picked> {
        match &self.kind {
            Kind::Run { tool, args } => Some(Picked::Run(tool.clone(), args.clone())),
            Kind::Form { tool, prefill } => Some(Picked::Form(tool.clone(), prefill.clone())),
            Kind::Action(id) => Some(Picked::Action(id.clone())),
            Kind::View(v) => Some(Picked::View(*v)),
            Kind::Separator | Kind::Submenu(_) => None,
        }
    }
}

/// One top-level menu.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub title: String,
    pub items: Vec<Item>,
}

impl Menu {
    pub fn new(title: impl Into<String>, items: Vec<Item>) -> Menu {
        Menu {
            title: title.into(),
            items,
        }
    }
}

/// The result of choosing a menu item (click or shortcut).
#[derive(Debug, Clone, PartialEq)]
pub enum Picked {
    Run(String, Value),
    Form(String, Vec<(String, String)>),
    Action(String),
    View(usize),
}

/// Draw the bar. Returns what was chosen this frame.
pub fn bar(ui: &mut Ui, menus: &[Menu]) -> Option<Picked> {
    let mut picked = None;
    egui::MenuBar::new().ui(ui, |ui| {
        let mut titles = Vec::with_capacity(menus.len());
        for m in menus {
            let r = ui.menu_button(m.title.as_str(), |ui| {
                ui.set_min_width(260.0);
                items_ui(ui, &m.items, &mut picked);
            });
            titles.push(r.response);
        }
        switch_on_hover(ui.ctx(), &titles);
    });
    picked
}

/// While one menu is open, hovering another title opens that one instead.
pub fn switch_on_hover(ctx: &Context, titles: &[egui::Response]) {
    let is_open =
        |t: &egui::Response| egui::Popup::is_id_open(ctx, egui::Popup::default_response_id(t));
    if !titles.iter().any(is_open) {
        return;
    }
    if let Some(title) = titles
        .iter()
        .find(|t| t.hovered() && !t.clicked() && !is_open(t))
    {
        egui::Popup::open_id(ctx, egui::Popup::default_response_id(title));
    }
}

fn items_ui(ui: &mut Ui, items: &[Item], picked: &mut Option<Picked>) {
    for it in items {
        match &it.kind {
            Kind::Separator => {
                ui.separator();
            }
            Kind::Submenu(children) => {
                let r = ui.add_enabled_ui(it.enabled, |ui| {
                    ui.menu_button(it.label.as_str(), |ui| {
                        ui.set_min_width(220.0);
                        items_ui(ui, children, picked);
                    })
                });
                if let Some(h) = &it.hint {
                    r.response.on_hover_text(h);
                }
            }
            _ => {
                let text = it
                    .shortcut
                    .as_ref()
                    .map(|s| ui.ctx().format_shortcut(s))
                    .or_else(|| it.shortcut_text.clone());
                let mut b = egui::Button::new(it.label.as_str());
                if let Some(t) = text {
                    b = b.shortcut_text(t);
                }
                if let Some(c) = it.checked {
                    b = b.selected(c);
                }
                let mut r = ui.add_enabled(it.enabled, b);
                if let Some(h) = &it.hint {
                    r = if it.enabled {
                        r.on_hover_text(h)
                    } else {
                        r.on_disabled_hover_text(h)
                    };
                }
                if r.clicked() {
                    *picked = it.pick();
                }
            }
        }
    }
}

/// Every chosen-able item, depth first.
pub fn leaves(menus: &[Menu]) -> Vec<&Item> {
    fn walk<'a>(items: &'a [Item], out: &mut Vec<&'a Item>) {
        for it in items {
            match &it.kind {
                Kind::Separator => {}
                Kind::Submenu(c) => walk(c, out),
                _ => out.push(it),
            }
        }
    }
    let mut out = Vec::new();
    for m in menus {
        walk(&m.items, &mut out);
    }
    out
}

/// Consume the first global shortcut that matches an enabled item.
pub fn consume_shortcuts(ctx: &Context, menus: &[Menu]) -> Option<Picked> {
    for it in leaves(menus) {
        if let (true, Some(s)) = (it.enabled, it.shortcut.as_ref()) {
            if ctx.input_mut(|i| i.consume_shortcut(s)) {
                return it.pick();
            }
        }
    }
    None
}

/// `--smoke` / test parity: every Run/Form item names a tool in the catalog,
/// and every argument it passes is a property of that tool's schema. Also
/// fails on two items sharing a shortcut. Returns the number of tool items.
pub fn parity(menus: &[Menu], tools: &[Value]) -> Result<usize, String> {
    let mut errors = Vec::new();
    let mut n = 0;
    let mut keys: Vec<(KeyboardShortcut, &str)> = Vec::new();
    for it in leaves(menus) {
        if let Some(s) = it.shortcut {
            if let Some((_, other)) = keys.iter().find(|(k, _)| *k == s) {
                errors.push(format!("{:?} used by {other:?} and {:?}", s, it.label));
            }
            keys.push((s, &it.label));
        }
        let (tool, args): (&str, Vec<String>) = match &it.kind {
            Kind::Run { tool, args } => (
                tool,
                args.as_object()
                    .map(|o| o.keys().cloned().collect())
                    .unwrap_or_default(),
            ),
            Kind::Form { tool, prefill } => {
                (tool, prefill.iter().map(|(k, _)| k.clone()).collect())
            }
            _ => continue,
        };
        n += 1;
        let Some(t) = tools.iter().find(|t| t["name"] == tool) else {
            errors.push(format!("menu item {:?}: unknown tool {tool}", it.label));
            continue;
        };
        let props = &t["inputSchema"]["properties"];
        for a in args {
            if props.get(&a).is_none() {
                errors.push(format!(
                    "menu item {:?}: tool {tool} has no argument {a}",
                    it.label
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(n)
    } else {
        Err(errors.join("; "))
    }
}

/// Characters of `s` that none of the loaded fonts can draw (they would
/// render as tofu boxes). Whitespace is ignored.
pub fn missing_glyphs(ctx: &Context, s: &str) -> Vec<char> {
    let families = [
        FontFamily::Proportional,
        crate::theme::semibold(),
        crate::theme::bold(),
        FontFamily::Monospace,
    ];
    ctx.fonts_mut(|f| {
        let mut out: Vec<char> = Vec::new();
        for c in s.chars().filter(|c| !c.is_whitespace()) {
            let ok = families
                .iter()
                .all(|fam| f.has_glyph(&FontId::new(14.0, fam.clone()), c));
            if !ok && !out.contains(&c) {
                out.push(c);
            }
        }
        out
    })
}

/// Tofu guard over every menu title and item label plus `extra` strings.
/// Returns (string, missing characters) for each failure.
pub fn tofu_report(ctx: &Context, menus: &[Menu], extra: &[String]) -> Vec<(String, Vec<char>)> {
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for it in items {
            if !it.label.is_empty() {
                out.push(it.label.clone());
            }
            if let Some(h) = &it.hint {
                out.push(h.clone());
            }
            if let Kind::Submenu(c) = &it.kind {
                walk(c, out);
            }
        }
    }
    let mut strings: Vec<String> = menus.iter().map(|m| m.title.clone()).collect();
    for m in menus {
        walk(&m.items, &mut strings);
    }
    strings.extend(extra.iter().cloned());
    strings
        .into_iter()
        .filter_map(|s| {
            let m = missing_glyphs(ctx, &s);
            (!m.is_empty()).then_some((s, m))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog() -> Vec<Value> {
        vec![json!({ "name": "x_add", "inputSchema": { "properties": { "name": {}, "due": {} } } })]
    }

    #[test]
    fn parity_catches_unknown_tools_bad_args_and_duplicate_keys() {
        let ok = vec![Menu::new(
            "Insert",
            vec![
                Item::form("Add", "x_add", &[("name", "n")]).key(Modifiers::COMMAND, Key::N),
                Item::sep(),
                Item::sub(
                    "More",
                    vec![Item::run("Add now", "x_add", json!({ "due": "1" }))],
                ),
                Item::action("Other", "app.other"),
            ],
        )];
        assert_eq!(parity(&ok, &catalog()), Ok(2));
        let bad = vec![Menu::new(
            "Insert",
            vec![
                Item::form("Add", "x_add", &[("nope", "n")]).key(Modifiers::COMMAND, Key::N),
                Item::run("Gone", "x_gone", json!({})).key(Modifiers::COMMAND, Key::N),
            ],
        )];
        let e = parity(&bad, &catalog()).unwrap_err();
        assert!(
            e.contains("no argument nope") && e.contains("unknown tool x_gone"),
            "{e}"
        );
        assert!(e.contains("used by"), "duplicate shortcut: {e}");
    }

    #[test]
    fn leaves_flatten_submenus_and_skip_separators() {
        let m = vec![Menu::new(
            "F",
            vec![
                Item::view("A", 0),
                Item::sep(),
                Item::sub("S", vec![Item::view("B", 1), Item::view("C", 2)]),
            ],
        )];
        let l: Vec<&str> = leaves(&m).iter().map(|i| i.label.as_str()).collect();
        assert_eq!(l, ["A", "B", "C"]);
    }

    #[test]
    fn missing_glyphs_finds_tofu_but_not_umlauts() {
        let ctx = Context::default();
        crate::theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        assert!(missing_glyphs(&ctx, "Kunden \u{e4}\u{f6}\u{fc} \u{df} 100%").is_empty());
        // CJK Extension F ideograph: none of the bundled fonts has it. (Not a
        // private-use code point: egui's icon font maps glyphs there.)
        assert_eq!(missing_glyphs(&ctx, "x\u{2CEB0}y"), vec!['\u{2CEB0}']);
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
    }
}
