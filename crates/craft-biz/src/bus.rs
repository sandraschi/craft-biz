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
