// SPDX-License-Identifier: MPL-2.0
//! The bridge to the TeamCraft desktop app (docs/src/research/solidcraft-bridge.md).
//!
//! The plugin has no network. The desktop app and the plugin share one folder,
//! `<working folder>/.teamcraft/solidcraft`, which the user grants the plugin
//! as `teamcraft` when installing it. The desktop app writes its heartbeat
//! (`desktop.json`) and the state of each revision in the working folder
//! (`state-<key>.json`); the plugin writes requests
//! (`request-<key>.<time>.<action>.json`) that the desktop app carries out.
//! The format is the desktop app's too (`desktop/src/scbridge.rs`); the test
//! vectors below are the same on both sides.
//!
//! Nothing here can make a run fail by itself: a file that is not there is
//! an answer ("the desktop app has not written it"), and a file that does
//! not parse is reported, not trusted.

use serde_json::{json, Value};

/// The name of the folder grant in `plugin.json`.
pub const GRANT: &str = "teamcraft";
/// The major version of the files this plugin reads and writes.
pub const VERSION: u64 = 1;
/// Largest bridge file read (the desktop app writes far less).
pub const MAX_FILE: usize = 4096;
/// A heartbeat older than this means the desktop app is not running.
pub const STALE_SECS: u64 = 120;
/// The actions a request may ask for.
pub const ACTIONS: [&str; 3] = ["check_out", "check_in", "open"];

/// The file-name key of a revision: item number and revision code, every
/// byte outside `A-Z a-z 0-9` as `_XX` (hex), joined by `.` – reversible, so
/// two revisions never share a file, and safe as a file name everywhere.
pub fn key(item: &str, revision: &str) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() {
                    (b as char).to_string()
                } else {
                    format!("_{b:02X}")
                }
            })
            .collect::<String>()
    };
    format!("{}.{}", enc(item), enc(revision))
}

/// `state-<key>.json`.
pub fn state_file(item: &str, revision: &str) -> String {
    format!("state-{}.json", key(item, revision))
}

/// `request-<key>.<time>.<action>.json`.
pub fn request_file(item: &str, revision: &str, now: u64, action: &str) -> String {
    format!("request-{}.{now}.{action}.json", key(item, revision))
}

/// A bridge file, checked: its schema, a version this plugin understands.
#[derive(Debug, PartialEq)]
pub enum Read {
    /// The desktop app has not written it.
    Missing,
    /// Not JSON of the expected schema, or too large – not trusted.
    Damaged,
    /// Written by a newer desktop app (a newer major version).
    Newer(u64),
    Ok(Value),
}

/// Checks the bytes of a bridge file against `schema`.
pub fn parse(bytes: Option<&[u8]>, schema: &str) -> Read {
    let Some(bytes) = bytes else {
        return Read::Missing;
    };
    if bytes.len() > MAX_FILE {
        return Read::Damaged;
    }
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return Read::Damaged;
    };
    if value["schema"] != schema {
        return Read::Damaged;
    }
    match value["version"].as_u64() {
        Some(v) if v > VERSION => Read::Newer(v),
        Some(_) => Read::Ok(value),
        None => Read::Damaged,
    }
}

/// `3 min`, `2 h`, `just now` – how long ago `then` was.
pub fn age(now: u64, then: u64) -> String {
    let secs = now.saturating_sub(then);
    match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=172_799 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86_400),
    }
}

/// Whether the desktop app's heartbeat is fresh: `Some(server_reachable)`
/// when it is, `None` when the app is not running (or never ran).
pub fn desktop_alive(heartbeat: &Read, now: u64) -> Option<bool> {
    let Read::Ok(h) = heartbeat else {
        return None;
    };
    let written = h["written_at"].as_u64()?;
    // a clock that says 0 (no clock grant) cannot tell: trust the heartbeat
    (now == 0 || now.saturating_sub(written) <= STALE_SECS)
        .then(|| h["server_reachable"].as_bool().unwrap_or(false))
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}

/// The sentence for a fresh state: who has it checked out, the latest
/// revision on the server, the lifecycle state, local changes.
pub fn describe_state(state: &Value) -> String {
    let mut parts = Vec::new();
    let me = state["checked_out"]["me"].as_bool().unwrap_or(false);
    match state["checked_out"]["by_name"].as_str() {
        _ if me => parts.push("Checked out by you".to_string()),
        Some(name) if !name.is_empty() => parts.push(format!("Checked out by {name}")),
        _ => parts.push("Not checked out".to_string()),
    }
    let revision = text(state, "revision");
    let latest = text(state, "latest_revision");
    if !latest.is_empty() && latest != revision {
        parts.push(format!(
            "latest on the server {latest} (this is {revision})"
        ));
    }
    let lifecycle = text(state, "lifecycle");
    if !lifecycle.is_empty() {
        parts.push(lifecycle.to_string());
    }
    let changed = state["local_changes"].as_u64().unwrap_or(0);
    if changed > 0 {
        parts.push(format!("{changed} file(s) changed here"));
    }
    if state["released"].as_bool().unwrap_or(false) {
        parts.push("your check-out was released by someone else".to_string());
    }
    parts.join(" · ")
}

/// The request the plugin writes for `action`.
pub fn request(item: &str, revision: &str, action: &str, now: u64) -> Value {
    json!({
        "schema": "teamcraft.solidcraft.request",
        "version": VERSION,
        "action": action,
        "item_number": item,
        "revision": revision,
        "written_at": now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same vectors as `desktop/src/scbridge.rs`.
    #[test]
    fn keys_are_reversible_and_safe() {
        assert_eq!(key("PRT-000042", "A"), "PRT_2D000042.A");
        assert_eq!(key("A.B", "c"), "A_2EB.c");
        assert_eq!(key("Ä/1", "B"), "_C3_84_2F1.B");
        assert_ne!(key("A_2D", "B"), key("A-", "B"));
        assert_eq!(state_file("PRT-000042", "A"), "state-PRT_2D000042.A.json");
        assert_eq!(
            request_file("PRT-000042", "A", 1_700_000_000, "check_in"),
            "request-PRT_2D000042.A.1700000000.check_in.json"
        );
        for name in [
            state_file("../x", ".."),
            request_file("a b", "c", 1, "open"),
        ] {
            assert!(name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)));
            assert!(!name.starts_with('.'));
        }
    }

    #[test]
    fn files_are_checked_before_they_are_trusted() {
        let schema = "teamcraft.solidcraft.state";
        assert_eq!(parse(None, schema), Read::Missing);
        assert_eq!(parse(Some(b"{not json"), schema), Read::Damaged);
        assert_eq!(
            parse(Some(br#"{"schema":"other","version":1}"#), schema),
            Read::Damaged
        );
        assert_eq!(
            parse(
                Some(br#"{"schema":"teamcraft.solidcraft.state","version":2}"#),
                schema
            ),
            Read::Newer(2)
        );
        assert_eq!(parse(Some(&[b' '; MAX_FILE + 1]), schema), Read::Damaged);
        assert!(matches!(
            parse(
                Some(br#"{"schema":"teamcraft.solidcraft.state","version":1,"x":1}"#),
                schema
            ),
            Read::Ok(_)
        ));
    }

    #[test]
    fn the_heartbeat_tells_whether_the_desktop_app_runs() {
        let beat = |at: u64, online: bool| {
            Read::Ok(
                json!({"schema": "teamcraft.solidcraft.desktop", "version": 1,
                            "written_at": at, "server_reachable": online}),
            )
        };
        assert_eq!(desktop_alive(&beat(1000, true), 1060), Some(true));
        assert_eq!(desktop_alive(&beat(1000, false), 1060), Some(false));
        assert_eq!(
            desktop_alive(&beat(1000, true), 1000 + STALE_SECS + 1),
            None
        );
        assert_eq!(desktop_alive(&Read::Missing, 5), None);
        assert_eq!(desktop_alive(&beat(1000, true), 0), Some(true), "no clock");
        assert_eq!(age(1000, 1000), "just now");
        assert_eq!(age(1000 + 180, 1000), "3 min ago");
        assert_eq!(age(1000 + 7200, 1000), "2 h ago");
    }

    #[test]
    fn a_state_reads_as_one_sentence() {
        let s = json!({"revision": "A", "latest_revision": "B", "lifecycle": "In work",
                       "checked_out": {"me": false, "by_name": "Kim Lee"}, "local_changes": 2});
        assert_eq!(
            describe_state(&s),
            "Checked out by Kim Lee · latest on the server B (this is A) · In work · 2 file(s) changed here"
        );
        let mine = json!({"revision": "A", "latest_revision": "A", "checked_out": {"me": true}});
        assert_eq!(describe_state(&mine), "Checked out by you");
        assert_eq!(describe_state(&json!({})), "Not checked out");
    }
}
