//! File event bus between back-office craft apps (outbox/inbox/ack).
//!
//! Layout under the bus root (default `%LOCALAPPDATA%\craft-biz\bus`, override
//! with `CRAFT_BIZ_BUS`):
//!
//! ```text
//! <root>/<app>/inbox/<id>.json    waiting events
//! <root>/<app>/done/<id>.json     acknowledged
//! <root>/<app>/failed/<id>.json   rejected, with "error" added
//! ```
//!
//! No daemon: a target app consumes its inbox only when asked (`inbox drain`).
//! Writes are atomic (temp file + rename) so a reader never sees half an event.

use crate::{BizError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Largest serialized event accepted, in bytes.
pub const MAX_EVENT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub kind: String,
    pub source: String,
    pub target: String,
    pub created: String,
    pub payload: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// App names and event kinds: lower-case ASCII, digits, `-`, `.`, `_`, 1-64
/// chars, no leading dot. Keeps every path component inside the bus root.
pub fn validate_name(s: &str) -> Result<()> {
    let ok = !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('.')
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.')
        });
    if ok {
        Ok(())
    } else {
        Err(BizError::Invalid(format!("bad bus name {s:?}")))
    }
}

fn validate_id(s: &str) -> Result<()> {
    let ok = s.len() == 36 && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(BizError::Invalid(format!("bad event id {s:?}")))
    }
}

pub struct Bus {
    root: PathBuf,
}

impl Bus {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Bus { root: root.into() }
    }

    /// `CRAFT_BIZ_BUS`, else `%LOCALAPPDATA%\craft-biz\bus`, else `./craft-biz-bus`.
    pub fn default_root() -> PathBuf {
        if let Ok(p) = std::env::var("CRAFT_BIZ_BUS") {
            if !p.trim().is_empty() {
                return PathBuf::from(p);
            }
        }
        if let Ok(p) = std::env::var("LOCALAPPDATA") {
            if !p.trim().is_empty() {
                return Path::new(&p).join("craft-biz").join("bus");
            }
        }
        PathBuf::from("craft-biz-bus")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn dir(&self, app: &str, sub: &str) -> Result<PathBuf> {
        validate_name(app)?;
        Ok(self.root.join(app).join(sub))
    }

    /// Write an event into `target`'s inbox. Returns the event.
    pub fn emit(&self, source: &str, target: &str, kind: &str, payload: Value) -> Result<Event> {
        validate_name(source)?;
        validate_name(kind)?;
        let event = Event {
            id: uuid::Uuid::now_v7().to_string(),
            kind: kind.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            created: chrono::Utc::now().to_rfc3339(),
            payload,
            error: None,
        };
        let bytes = serde_json::to_vec_pretty(&event)?;
        if bytes.len() > MAX_EVENT_BYTES {
            return Err(BizError::Invalid("event payload too large".into()));
        }
        let inbox = self.dir(target, "inbox")?;
        fs::create_dir_all(&inbox)?;
        write_atomic(&inbox, &event.id, &bytes)?;
        Ok(event)
    }

    /// Waiting events for `app`, oldest first (ids are UUIDv7, so name order is time order).
    /// Unreadable or oversized files are moved to `failed/` rather than blocking the queue.
    pub fn pending(&self, app: &str) -> Result<Vec<Event>> {
        let inbox = self.dir(app, "inbox")?;
        if !inbox.exists() {
            return Ok(Vec::new());
        }
        let mut names: Vec<String> = fs::read_dir(&inbox)?
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".json"))
            .collect();
        names.sort();
        let mut out = Vec::new();
        for name in names {
            let path = inbox.join(&name);
            let parsed = fs::metadata(&path)
                .map_err(BizError::from)
                .and_then(|m| {
                    if m.len() as usize > MAX_EVENT_BYTES {
                        Err(BizError::Invalid("event file too large".into()))
                    } else {
                        Ok(())
                    }
                })
                .and_then(|_| fs::read(&path).map_err(BizError::from))
                .and_then(|b| serde_json::from_slice::<Event>(&b).map_err(BizError::from));
            match parsed {
                Ok(ev) if validate_id(&ev.id).is_ok() && format!("{}.json", ev.id) == name => {
                    out.push(ev)
                }
                Ok(_) | Err(_) => {
                    let failed = self.dir(app, "failed")?;
                    fs::create_dir_all(&failed)?;
                    fs::rename(&path, failed.join(&name))?;
                }
            }
        }
        Ok(out)
    }

    /// Mark an event handled.
    pub fn ack(&self, app: &str, id: &str) -> Result<()> {
        validate_id(id)?;
        let from = self.dir(app, "inbox")?.join(format!("{id}.json"));
        let done = self.dir(app, "done")?;
        fs::create_dir_all(&done)?;
        fs::rename(from, done.join(format!("{id}.json")))?;
        Ok(())
    }

    /// Mark an event rejected, recording why.
    pub fn reject(&self, app: &str, id: &str, reason: &str) -> Result<()> {
        validate_id(id)?;
        let from = self.dir(app, "inbox")?.join(format!("{id}.json"));
        let bytes = fs::read(&from)?;
        let mut ev: Event = serde_json::from_slice(&bytes)?;
        ev.error = Some(reason.chars().take(2000).collect());
        let failed = self.dir(app, "failed")?;
        fs::create_dir_all(&failed)?;
        write_atomic(&failed, id, &serde_json::to_vec_pretty(&ev)?)?;
        fs::remove_file(from)?;
        Ok(())
    }

    /// Events of `app` in one state, newest first, at most `limit`.
    /// Unreadable files are skipped (pending() quarantines them).
    pub fn history(&self, app: &str, state: EventState, limit: usize) -> Result<Vec<Event>> {
        read_events(&self.dir(app, state.dir())?, limit)
    }

    /// Events that `source` sent to any app, with their state at the target,
    /// newest first, at most `limit`. This is how a sender sees whether the
    /// other app applied or rejected what it sent.
    pub fn outgoing(&self, source: &str, limit: usize) -> Result<Vec<(Event, EventState)>> {
        validate_name(source)?;
        let mut out = Vec::new();
        if !self.root.exists() {
            return Ok(out);
        }
        for entry in fs::read_dir(&self.root)?.filter_map(|e| e.ok()) {
            let Ok(app) = entry.file_name().into_string() else {
                continue;
            };
            if validate_name(&app).is_err() || !entry.path().is_dir() {
                continue;
            }
            for state in [EventState::Pending, EventState::Done, EventState::Failed] {
                for ev in read_events(&self.dir(&app, state.dir())?, usize::MAX)? {
                    if ev.source == source {
                        out.push((ev, state));
                    }
                }
            }
        }
        out.sort_by(|a, b| b.0.id.cmp(&a.0.id));
        out.truncate(limit);
        Ok(out)
    }

    /// Put a rejected event back into the inbox (error cleared) to try again.
    pub fn retry(&self, app: &str, id: &str) -> Result<()> {
        validate_id(id)?;
        let from = self.dir(app, "failed")?.join(format!("{id}.json"));
        let mut ev: Event = serde_json::from_slice(&fs::read(&from)?)?;
        ev.error = None;
        let inbox = self.dir(app, "inbox")?;
        fs::create_dir_all(&inbox)?;
        write_atomic(&inbox, id, &serde_json::to_vec_pretty(&ev)?)?;
        fs::remove_file(from)?;
        Ok(())
    }
}

/// Where an event sits at its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventState {
    Pending,
    Done,
    Failed,
}

impl EventState {
    fn dir(self) -> &'static str {
        match self {
            EventState::Pending => "inbox",
            EventState::Done => "done",
            EventState::Failed => "failed",
        }
    }
}

fn read_events(dir: &Path, limit: usize) -> Result<Vec<Event>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".json") && !n.starts_with('.'))
        .collect();
    names.sort_by(|a, b| b.cmp(a));
    let mut out = Vec::new();
    for name in names {
        if out.len() >= limit {
            break;
        }
        let path = dir.join(&name);
        let small = fs::metadata(&path).is_ok_and(|m| m.len() as usize <= MAX_EVENT_BYTES);
        if !small {
            continue;
        }
        if let Some(ev) = fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Event>(&b).ok())
        {
            out.push(ev);
        }
    }
    Ok(out)
}

fn write_atomic(dir: &Path, id: &str, bytes: &[u8]) -> Result<()> {
    let tmp = dir.join(format!(".{id}.tmp"));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, dir.join(format!("{id}.json")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_bus(tag: &str) -> Bus {
        let dir =
            std::env::temp_dir().join(format!("craft-biz-bus-test-{tag}-{}", uuid::Uuid::now_v7()));
        Bus::new(dir)
    }

    #[test]
    fn emit_pending_ack_roundtrip() {
        let bus = temp_bus("rt");
        let a = bus
            .emit(
                "assetcraft",
                "buchcraft",
                "asset.depreciation_due",
                json!({"amount_cents": 133_333}),
            )
            .unwrap();
        let b = bus
            .emit(
                "assetcraft",
                "buchcraft",
                "asset.depreciation_due",
                json!({"amount_cents": 1}),
            )
            .unwrap();
        let pending = bus.pending("buchcraft").unwrap();
        assert_eq!(
            pending.iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
            vec![a.id.clone(), b.id.clone()]
        );
        bus.ack("buchcraft", &a.id).unwrap();
        bus.reject("buchcraft", &b.id, "account 7000 closed")
            .unwrap();
        assert!(bus.pending("buchcraft").unwrap().is_empty());
        let failed = bus
            .root()
            .join("buchcraft")
            .join("failed")
            .join(format!("{}.json", b.id));
        let ev: Event = serde_json::from_slice(&fs::read(failed).unwrap()).unwrap();
        assert_eq!(ev.error.as_deref(), Some("account 7000 closed"));
        let _ = fs::remove_dir_all(bus.root());
    }

    #[test]
    fn history_outgoing_and_retry() {
        let bus = temp_bus("hist");
        let a = bus
            .emit("deskcraft", "taskcraft", "ticket.escalated", json!({}))
            .unwrap();
        let b = bus
            .emit("deskcraft", "gaincraft", "signal.negative", json!({}))
            .unwrap();
        let c = bus
            .emit("contractcraft", "gaincraft", "contract.executed", json!({}))
            .unwrap();
        bus.ack("taskcraft", &a.id).unwrap();
        bus.reject("gaincraft", &b.id, "no account").unwrap();
        let out = bus.outgoing("deskcraft", 10).unwrap();
        assert_eq!(out.len(), 2, "only deskcraft's events");
        assert_eq!(
            (out[0].0.id.clone(), out[0].1),
            (b.id.clone(), EventState::Failed),
            "newest first"
        );
        assert_eq!(out[1].1, EventState::Done);
        assert_eq!(
            bus.history("gaincraft", EventState::Failed, 10).unwrap()[0]
                .error
                .as_deref(),
            Some("no account")
        );
        assert_eq!(
            bus.history("gaincraft", EventState::Pending, 10).unwrap()[0].id,
            c.id
        );
        bus.retry("gaincraft", &b.id).unwrap();
        let pending = bus.pending("gaincraft").unwrap();
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().all(|e| e.error.is_none()));
        assert!(bus
            .history("gaincraft", EventState::Failed, 10)
            .unwrap()
            .is_empty());
        assert!(bus.retry("gaincraft", "../x").is_err());
        assert!(Bus::new(bus.root().join("nothing"))
            .outgoing("deskcraft", 5)
            .unwrap()
            .is_empty());
        let _ = fs::remove_dir_all(bus.root());
    }

    #[test]
    fn hostile_names_rejected() {
        let bus = temp_bus("hostile");
        for bad in ["..", "../x", "a/b", "A", "", ".hidden", "c:\\x"] {
            assert!(
                bus.emit("assetcraft", bad, "k", json!({})).is_err(),
                "{bad:?}"
            );
        }
        assert!(bus.ack("buchcraft", "../../etc/passwd").is_err());
        assert!(bus.pending("..").is_err());
    }

    #[test]
    fn junk_files_are_quarantined_not_fatal() {
        let bus = temp_bus("junk");
        let inbox = bus.root().join("taskcraft").join("inbox");
        fs::create_dir_all(&inbox).unwrap();
        fs::write(inbox.join("garbage.json"), b"{not json").unwrap();
        let ok = bus
            .emit(
                "deskcraft",
                "taskcraft",
                "ticket.escalated",
                json!({"ticket": 7}),
            )
            .unwrap();
        let pending = bus.pending("taskcraft").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending.first().map(|e| e.id.clone()), Some(ok.id));
        assert!(bus
            .root()
            .join("taskcraft")
            .join("failed")
            .join("garbage.json")
            .exists());
        let _ = fs::remove_dir_all(bus.root());
    }
}
