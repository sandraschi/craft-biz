//! Signals dock: what other craft apps sent to this app (waiting, applied,
//! rejected) and what this app sent to others (and whether they took it).
//! Also lists every event kind the app consumes or emits, so the wiring is
//! visible even before the first event exists.

use crate::theme::{self, semibold, Tone};
use crate::widgets;
use craft_biz::bus::{Bus, Event, EventState};
use egui::{RichText, Ui};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum SignalAction {
    /// Apply all waiting events (the app runs its inbox-drain tool).
    DrainAll,
    /// Put a rejected event back into the inbox.
    Retry(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Incoming,
    Applied,
    Rejected,
    Outgoing,
}

#[derive(Default)]
pub struct SignalsDock {
    pub open: bool,
    tab: Tab,
    pending: Vec<Event>,
    done: Vec<Event>,
    failed: Vec<Event>,
    outgoing: Vec<(Event, EventState)>,
    last: Option<Instant>,
    pub error: Option<String>,
}

fn short_time(created: &str) -> String {
    created
        .get(..16)
        .map(|s| s.replace('T', " "))
        .unwrap_or_else(|| created.to_string())
}

fn summary(ev: &Event) -> String {
    let mut parts = Vec::new();
    if let Some(o) = ev.payload.as_object() {
        for (k, v) in o.iter().take(4) {
            let s = crate::palette::scalar(v);
            if !s.is_empty() && s.len() < 60 {
                parts.push(format!("{k}: {s}"));
            }
        }
    }
    parts.join("  |  ")
}

impl SignalsDock {
    pub fn counts(&self) -> (usize, usize) {
        (self.pending.len(), self.failed.len())
    }

    /// Re-read the bus (cheap file listing). Call every frame; it throttles itself to every 3 s.
    pub fn refresh(&mut self, bus: &Bus, app: &str, force: bool) {
        if !force
            && self
                .last
                .is_some_and(|l| l.elapsed() < Duration::from_secs(3))
        {
            return;
        }
        self.last = Some(Instant::now());
        let r = (|| -> Result<(), String> {
            self.pending = bus
                .history(app, EventState::Pending, 200)
                .map_err(|e| e.to_string())?;
            self.done = bus
                .history(app, EventState::Done, 100)
                .map_err(|e| e.to_string())?;
            self.failed = bus
                .history(app, EventState::Failed, 100)
                .map_err(|e| e.to_string())?;
            self.outgoing = bus.outgoing(app, 100).map_err(|e| e.to_string())?;
            Ok(())
        })();
        self.error = r.err();
    }

    /// Draw the dock contents (put it in a right side panel).
    /// `consumes` / `emits`: event kinds with a short description.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        consumes: &[(&str, &str)],
        emits: &[(&str, &str)],
    ) -> Option<SignalAction> {
        let t = theme::current(ui.ctx());
        let mut action = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Signals").family(semibold()).size(17.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    self.open = false;
                }
                if ui.button("Refresh").clicked() {
                    self.last = None;
                }
            });
        });
        ui.horizontal_wrapped(|ui| {
            for (tab, label, n, tone) in [
                (Tab::Incoming, "Waiting", self.pending.len(), Tone::Accent),
                (Tab::Applied, "Applied", self.done.len(), Tone::Ok),
                (Tab::Rejected, "Rejected", self.failed.len(), Tone::Danger),
                (Tab::Outgoing, "Sent", self.outgoing.len(), Tone::Neutral),
            ] {
                let text = format!("{label} {n}");
                let r = ui.selectable_label(
                    self.tab == tab,
                    RichText::new(text).family(semibold()).color(
                        if n > 0 && tab != Tab::Applied && tab != Tab::Outgoing {
                            theme::tone_colors(tone, t.dark).0
                        } else {
                            t.text
                        },
                    ),
                );
                if r.clicked() {
                    self.tab = tab;
                }
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(theme::tone_solid(Tone::Danger, t.dark), e);
        }
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("signals_scroll")
            .max_height(ui.available_height() - 150.0)
            .show(ui, |ui| match self.tab {
                Tab::Incoming => {
                    if !self.pending.is_empty()
                        && widgets::primary(ui, &format!("Apply all {} now", self.pending.len()))
                            .clicked()
                    {
                        action = Some(SignalAction::DrainAll);
                    }
                    list(
                        ui,
                        &self.pending,
                        |_| None,
                        "Nothing waiting. Events from other apps show up here.",
                    );
                }
                Tab::Applied => list(ui, &self.done, |_| None, "Nothing applied yet."),
                Tab::Rejected => {
                    for ev in &self.failed {
                        event_card(
                            ui,
                            ev,
                            Some((ev.error.as_deref().unwrap_or("rejected"), Tone::Danger)),
                        );
                        if ui
                            .button("Retry")
                            .on_hover_text("Put it back into the inbox; then apply")
                            .clicked()
                        {
                            action = Some(SignalAction::Retry(ev.id.clone()));
                        }
                        ui.add_space(4.0);
                    }
                    if self.failed.is_empty() {
                        widgets::empty_state(ui, "No rejected events", "Good.");
                    }
                }
                Tab::Outgoing => {
                    for (ev, state) in &self.outgoing {
                        let note = match state {
                            EventState::Pending => {
                                (format!("waiting at {}", ev.target), Tone::Accent)
                            }
                            EventState::Done => (format!("applied by {}", ev.target), Tone::Ok),
                            EventState::Failed => (
                                format!(
                                    "rejected by {}: {}",
                                    ev.target,
                                    ev.error.as_deref().unwrap_or("")
                                ),
                                Tone::Danger,
                            ),
                        };
                        event_card(ui, ev, Some((&note.0, note.1)));
                    }
                    if self.outgoing.is_empty() {
                        widgets::empty_state(
                            ui,
                            "Nothing sent yet",
                            "Events this app sends to others appear here with their state.",
                        );
                    }
                }
            });
        ui.separator();
        ui.label(RichText::new("Wiring").family(semibold()));
        for (kind, what) in consumes {
            ui.horizontal_wrapped(|ui| {
                widgets::pill(ui, "in", Tone::Accent);
                ui.label(RichText::new(*kind).family(egui::FontFamily::Monospace));
                ui.label(RichText::new(*what).color(t.text_weak));
            });
        }
        for (kind, what) in emits {
            ui.horizontal_wrapped(|ui| {
                widgets::pill(ui, "out", Tone::Violet);
                ui.label(RichText::new(*kind).family(egui::FontFamily::Monospace));
                ui.label(RichText::new(*what).color(t.text_weak));
            });
        }
        action
    }
}

fn list(ui: &mut Ui, evs: &[Event], note: impl Fn(&Event) -> Option<(&str, Tone)>, empty: &str) {
    for ev in evs {
        event_card(ui, ev, note(ev));
        ui.add_space(4.0);
    }
    if evs.is_empty() {
        widgets::empty_state(ui, "Empty", empty);
    }
}

fn event_card(ui: &mut Ui, ev: &Event, note: Option<(&str, Tone)>) {
    let t = theme::current(ui.ctx());
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            widgets::pill(ui, &ev.kind, Tone::Info);
            ui.label(RichText::new(format!("{} -> {}", ev.source, ev.target)).color(t.text_weak));
            ui.label(
                RichText::new(short_time(&ev.created))
                    .color(t.text_weak)
                    .size(12.5),
            );
        });
        let s = summary(ev);
        if !s.is_empty() {
            ui.label(RichText::new(s).size(13.0));
        }
        if let Some((n, tone)) = note {
            ui.label(
                RichText::new(n)
                    .color(theme::tone_colors(tone, t.dark).0)
                    .family(semibold())
                    .size(13.0),
            );
        }
    });
}
