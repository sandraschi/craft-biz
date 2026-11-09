//! Command palette (Ctrl+K): every MCP tool of the app, searchable, with a
//! form generated from the tool's `inputSchema`. This guarantees that every
//! task the CLI or an agent can do is also doable from the window.

use crate::theme::{self, semibold, Tone};
use crate::widgets;
use egui::{Context, Key, Modifiers, RichText};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Str,
    Int,
    Num,
    Bool,
    Enum(Vec<String>),
    Json,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub kind: Kind,
    pub required: bool,
    pub nullable: bool,
    pub description: String,
    pub text: String,
    pub flag: bool,
    pub null: bool,
}

#[derive(Debug, Clone)]
pub struct Form {
    pub tool: String,
    pub description: String,
    pub fields: Vec<Field>,
    pub error: Option<String>,
}

impl Form {
    /// Build a form from an MCP tool object (`name`, `description`, `inputSchema`).
    pub fn from_tool(tool: &Value) -> Form {
        let schema = &tool["inputSchema"];
        let required: Vec<&str> = schema["required"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let mut fields = Vec::new();
        if let Some(props) = schema["properties"].as_object() {
            for (name, p) in props {
                let types: Vec<String> = match &p["type"] {
                    Value::String(s) => vec![s.clone()],
                    Value::Array(a) => a
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect(),
                    _ => vec!["string".into()],
                };
                let nullable = types.iter().any(|t| t == "null");
                let main = types
                    .iter()
                    .find(|t| *t != "null")
                    .cloned()
                    .unwrap_or_else(|| "string".into());
                let kind = if let Some(e) = p["enum"].as_array() {
                    Kind::Enum(
                        e.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect(),
                    )
                } else {
                    match main.as_str() {
                        "integer" => Kind::Int,
                        "number" => Kind::Num,
                        "boolean" => Kind::Bool,
                        "object" | "array" => Kind::Json,
                        _ => Kind::Str,
                    }
                };
                fields.push(Field {
                    name: name.clone(),
                    kind,
                    required: required.contains(&name.as_str()),
                    nullable,
                    description: p["description"].as_str().unwrap_or("").to_string(),
                    text: String::new(),
                    flag: false,
                    null: false,
                });
            }
        }
        fields.sort_by_key(|f| !f.required);
        Form {
            tool: tool["name"].as_str().unwrap_or("").to_string(),
            description: tool["description"].as_str().unwrap_or("").to_string(),
            fields,
            error: None,
        }
    }

    /// Arguments for `Command::from_tool`. Empty optional fields are left out.
    pub fn args(&self) -> Result<Value, String> {
        let mut m = Map::new();
        for f in &self.fields {
            if f.nullable && f.null {
                m.insert(f.name.clone(), Value::Null);
                continue;
            }
            let t = f.text.trim();
            let v = match &f.kind {
                Kind::Bool => {
                    if !f.flag && !f.required {
                        continue;
                    }
                    Value::Bool(f.flag)
                }
                _ if t.is_empty() => {
                    if f.required {
                        return Err(format!("{} is required", f.name));
                    }
                    continue;
                }
                Kind::Str | Kind::Enum(_) => Value::String(t.to_string()),
                Kind::Int => t
                    .parse::<i64>()
                    .map(Value::from)
                    .map_err(|_| format!("{}: whole number expected", f.name))?,
                Kind::Num => t
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .and_then(|n| serde_json::Number::from_f64(n).map(Value::Number))
                    .ok_or_else(|| format!("{}: number expected", f.name))?,
                Kind::Json => {
                    serde_json::from_str(t).map_err(|e| format!("{}: JSON: {e}", f.name))?
                }
            };
            m.insert(f.name.clone(), v);
        }
        Ok(Value::Object(m))
    }

    /// Prefill a field (e.g. the selected record's key).
    pub fn set(&mut self, name: &str, value: &str) {
        if let Some(f) = self.fields.iter_mut().find(|f| f.name == name) {
            f.text = value.to_string();
        }
    }
}

/// Palette state, owned by the app.
#[derive(Default)]
pub struct Palette {
    pub open: bool,
    query: String,
    highlight: usize,
    form: Option<Form>,
    focus_query: bool,
    /// Default values offered for common argument names (e.g. "account": current account).
    pub prefill: Vec<(String, String)>,
}

/// Fuzzy-ish match: all query words appear in name or description.
pub fn matches(tool: &Value, query: &str) -> bool {
    let hay = format!(
        "{} {}",
        tool["name"].as_str().unwrap_or(""),
        tool["description"].as_str().unwrap_or("")
    )
    .to_lowercase()
    .replace('_', " ");
    query
        .to_lowercase()
        .split_whitespace()
        .all(|w| hay.contains(w))
}

impl Palette {
    /// Ctrl+K toggles; Escape closes.
    pub fn handle_shortcut(&mut self, ctx: &Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K)) {
            if self.open {
                self.close();
            } else {
                self.open_list();
            }
        }
        if self.open && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if self.form.is_some() {
                self.form = None;
                self.focus_query = true;
            } else {
                self.close();
            }
        }
    }

    pub fn open_list(&mut self) {
        self.open = true;
        self.form = None;
        self.query.clear();
        self.highlight = 0;
        self.focus_query = true;
    }

    /// Jump straight to one tool's form (used by buttons that need extra arguments).
    pub fn open_tool(&mut self, tool: &Value) {
        let mut f = Form::from_tool(tool);
        for (k, v) in &self.prefill {
            f.set(k, v);
        }
        self.form = Some(f);
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.form = None;
    }

    /// Report a dispatch error back into the open form.
    pub fn fail(&mut self, error: String) {
        if let Some(f) = &mut self.form {
            f.error = Some(error);
        }
    }

    /// Draw; returns (tool name, arguments) when the user runs a tool.
    pub fn show(&mut self, ctx: &Context, tools: &[Value]) -> Option<(String, Value)> {
        if !self.open {
            return None;
        }
        let mut run = None;
        let mut open = true;
        let t = theme::current(ctx);
        egui::Window::new(RichText::new("Command palette").family(semibold()))
            .id(egui::Id::unique("craft_palette"))
            .collapsible(false)
            .resizable(false)
            .default_width(640.0)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 70.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_min_width(620.0);
                if let Some(form) = &mut self.form {
                    ui.horizontal(|ui| {
                        if ui.button("< All commands").clicked() {
                            run = Some(("__back".to_string(), Value::Null));
                        }
                        ui.label(
                            RichText::new(&form.tool)
                                .family(egui::FontFamily::Monospace)
                                .size(14.0),
                        );
                    });
                    ui.label(RichText::new(&form.description).color(t.text_weak));
                    ui.separator();
                    egui::Grid::new("palette_form")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            for f in &mut form.fields {
                                let label = if f.required {
                                    format!("{} *", f.name)
                                } else {
                                    f.name.clone()
                                };
                                ui.label(RichText::new(label).family(semibold()))
                                    .on_hover_text(&f.description);
                                ui.horizontal(|ui| {
                                    let enabled = !(f.nullable && f.null);
                                    ui.add_enabled_ui(enabled, |ui| match &f.kind {
                                        Kind::Bool => {
                                            ui.checkbox(&mut f.flag, "");
                                        }
                                        Kind::Enum(opts) => {
                                            egui::ComboBox::from_id_salt(("pal_enum", &f.name))
                                                .selected_text(if f.text.is_empty() {
                                                    "(choose)"
                                                } else {
                                                    f.text.as_str()
                                                })
                                                .show_ui(ui, |ui| {
                                                    if !f.required {
                                                        ui.selectable_value(
                                                            &mut f.text,
                                                            String::new(),
                                                            "(none)",
                                                        );
                                                    }
                                                    for o in opts {
                                                        ui.selectable_value(
                                                            &mut f.text,
                                                            o.clone(),
                                                            o,
                                                        );
                                                    }
                                                });
                                        }
                                        Kind::Json => {
                                            ui.add(
                                                egui::TextEdit::multiline(&mut f.text)
                                                    .code_editor()
                                                    .desired_rows(3)
                                                    .desired_width(380.0)
                                                    .hint_text("JSON"),
                                            );
                                        }
                                        _ => {
                                            ui.add(
                                                egui::TextEdit::singleline(&mut f.text)
                                                    .desired_width(380.0)
                                                    .hint_text(&f.description),
                                            );
                                        }
                                    });
                                    if f.nullable {
                                        ui.checkbox(&mut f.null, "clear")
                                            .on_hover_text("Send null: removes the value");
                                    }
                                });
                                ui.end_row();
                            }
                        });
                    if let Some(e) = &form.error {
                        ui.add_space(6.0);
                        let (fg, bg) = theme::tone_colors(Tone::Danger, t.dark);
                        egui::Frame::new()
                            .fill(bg)
                            .corner_radius(6)
                            .inner_margin(8)
                            .show(ui, |ui| {
                                ui.label(RichText::new(e).color(fg));
                            });
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let go = widgets::primary(ui, "Run").clicked()
                            || ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.command);
                        widgets::key_hint(ui, "Ctrl+Enter");
                        if go {
                            match form.args() {
                                Ok(a) => run = Some((form.tool.clone(), a)),
                                Err(e) => form.error = Some(e),
                            }
                        }
                    });
                } else {
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text("Type to search commands...")
                            .desired_width(f32::INFINITY),
                    );
                    if self.focus_query {
                        r.request_focus();
                        self.focus_query = false;
                    }
                    let list: Vec<&Value> =
                        tools.iter().filter(|tl| matches(tl, &self.query)).collect();
                    if r.changed() {
                        self.highlight = 0;
                    }
                    let (down, up, enter) = ui.input(|i| {
                        (
                            i.key_pressed(Key::ArrowDown),
                            i.key_pressed(Key::ArrowUp),
                            i.key_pressed(Key::Enter),
                        )
                    });
                    if down {
                        self.highlight = (self.highlight + 1).min(list.len().saturating_sub(1));
                    }
                    if up {
                        self.highlight = self.highlight.saturating_sub(1);
                    }
                    let mut chosen: Option<&Value> = None;
                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(ui, |ui| {
                            for (i, tl) in list.iter().enumerate() {
                                let sel = i == self.highlight;
                                let fill = if sel {
                                    t.accent_soft
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                let resp = egui::Frame::new()
                                    .fill(fill)
                                    .corner_radius(6)
                                    .inner_margin(egui::Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.label(
                                            RichText::new(tl["name"].as_str().unwrap_or(""))
                                                .family(egui::FontFamily::Monospace)
                                                .size(14.0),
                                        );
                                        ui.label(
                                            RichText::new(tl["description"].as_str().unwrap_or(""))
                                                .color(t.text_weak)
                                                .size(13.0),
                                        );
                                    });
                                let resp = ui.interact(
                                    resp.response.rect,
                                    egui::Id::unique(("pal_item", i)),
                                    egui::Sense::click(),
                                );
                                if sel && (down || up) {
                                    resp.scroll_to_me(None);
                                }
                                if resp.clicked() || (sel && enter) {
                                    chosen = Some(tl);
                                }
                            }
                            if list.is_empty() {
                                ui.label(RichText::new("No command matches.").color(t.text_weak));
                            }
                        });
                    if let Some(tl) = chosen {
                        let mut f = Form::from_tool(tl);
                        for (k, v) in &self.prefill {
                            f.set(k, v);
                        }
                        if f.fields.iter().all(|x| !x.required) && f.fields.is_empty() {
                            run = Some((f.tool.clone(), Value::Object(Map::new())));
                        } else {
                            self.form = Some(f);
                        }
                    }
                }
            });
        if !open {
            self.close();
        }
        match run {
            Some((n, _)) if n == "__back" => {
                self.form = None;
                self.focus_query = true;
                None
            }
            other => other,
        }
    }
}

/// Show any tool result: a table for the first array of objects, then the JSON.
pub fn result_view(ui: &mut egui::Ui, v: &Value) {
    let t = theme::current(ui.ctx());
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            if !val.is_object() && !val.is_array() {
                widgets::field(ui, k, |ui| {
                    ui.label(scalar(val));
                });
            }
        }
        if let Some((k, Value::Array(rows))) = obj.iter().find(|(_, val)| {
            val.as_array()
                .is_some_and(|a| a.first().is_some_and(Value::is_object))
        }) {
            ui.add_space(6.0);
            widgets::section(ui, k, Some(rows.len()));
            let mut keys: Vec<String> = Vec::new();
            for r in rows.iter().filter_map(Value::as_object) {
                for (kk, vv) in r {
                    if !vv.is_object() && !vv.is_array() && !keys.contains(kk) && keys.len() < 8 {
                        keys.push(kk.clone());
                    }
                }
            }
            let cols: Vec<crate::table::Col> = keys
                .iter()
                .map(|k| crate::table::Col::new(k.clone(), 1.0))
                .collect();
            let data: Vec<(String, Vec<crate::table::Cell>)> = rows
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    (
                        i.to_string(),
                        keys.iter()
                            .map(|k| crate::table::Cell::Text(scalar(&r[k.as_str()])))
                            .collect(),
                    )
                })
                .collect();
            ui.push_id(("result_table", k), |ui| {
                ui.set_max_height(320.0);
                crate::table::Table::new(("result", k.as_str()), &cols, &data)
                    .filter_box(rows.len() > 8)
                    .show(ui);
            });
        }
    }
    egui::CollapsingHeader::new(RichText::new("Raw JSON").color(t.text_weak))
        .id_salt("raw_json")
        .show(ui, |ui| {
            let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
            ui.add(
                egui::TextEdit::multiline(&mut s)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(8)
                    .interactive(true),
            );
        });
}

pub fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "-".into(),
        Value::Array(a) => a.iter().map(scalar).collect::<Vec<_>>().join(", "),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool() -> Value {
        json!({ "name": "gain_account_update", "description": "Change account fields", "inputSchema": { "type": "object", "properties": {
            "account": {"type": "string"}, "seats": {"type": ["integer","null"]}, "active": {"type": "boolean"},
            "polarity": {"type": "string", "enum": ["negative","positive"]}, "set": {"type": "object"}, "modifier": {"type": "number"}
        }, "required": ["account"] } })
    }

    #[test]
    fn form_from_schema_builds_typed_args() {
        let mut f = Form::from_tool(&tool());
        assert_eq!(f.fields[0].name, "account", "required first");
        assert!(f.args().unwrap_err().contains("account is required"));
        f.set("account", "mock.example");
        f.set("seats", "12");
        f.set("modifier", "0,9");
        f.set("polarity", "positive");
        f.set("set", r#"{"a":1}"#);
        let a = f.args().unwrap();
        assert_eq!(
            a,
            json!({ "account": "mock.example", "seats": 12, "modifier": 0.9, "polarity": "positive", "set": {"a":1} })
        );
        f.set("seats", "twelve");
        assert!(f.args().unwrap_err().contains("whole number"));
        f.set("seats", "");
        if let Some(s) = f.fields.iter_mut().find(|x| x.name == "seats") {
            s.null = true;
        }
        assert_eq!(f.args().unwrap()["seats"], Value::Null, "nullable clears");
        assert!(matches!(
            f.fields.iter().find(|x| x.name == "polarity").unwrap().kind,
            Kind::Enum(_)
        ));
    }

    #[test]
    fn search_matches_words_in_name_and_description() {
        assert!(matches(&tool(), "account update"));
        assert!(matches(&tool(), "CHANGE fields"));
        assert!(!matches(&tool(), "invoice"));
    }
}
