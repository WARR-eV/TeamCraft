//! The bridge to the TeamCraft plugin for SolidCraft
//! (docs/src/research/solidcraft-bridge.md).
//!
//! SolidCraft plugins have no network, so the plugin and this app share one
//! folder, `<working folder>/.teamcraft/solidcraft`, which the user grants
//! the plugin as `teamcraft`. This app writes its heartbeat (`desktop.json`)
//! and the state of each revision it shows (`state-<key>.json`); the plugin
//! leaves requests (`request-<key>.<action>.json`: one per revision and
//! action, so asking twice replaces the first) that this app picks
//! up, checks and hands to the user interface (event `solidcraft-request`).
//!
//! The plugin's side is `plugins/solidcraft/src/bridge.rs`; both use the same
//! test vectors. No secrets are written here: display names and states only.
//! Requests are untrusted: only a revision of this working folder, only the
//! known actions, only fresh, small, regular files.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

use crate::layout;
use crate::workspace::Status;

/// The major version of the bridge files.
pub const VERSION: u64 = 1;
/// Largest request read.
pub const MAX_FILE: u64 = 4096;
/// Requests older than this are dropped (the desktop app was not running).
pub const MAX_AGE_SECS: u64 = 600;
/// The actions a request may ask for.
pub const ACTIONS: [&str; 3] = ["check_out", "check_in", "open"];

/// `<working folder>/.teamcraft/solidcraft`.
pub fn dir(root: &Path) -> PathBuf {
    root.join(layout::STATE_DIR).join("solidcraft")
}

/// The file-name key of a revision: every byte outside `A-Z a-z 0-9` as
/// `_XX`, item and revision joined by `.` (the plugin's `bridge::key`).
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

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The heartbeat: this app runs, and whether it reaches the server.
pub fn heartbeat(now: u64, server_reachable: bool) -> Value {
    json!({
        "schema": "teamcraft.solidcraft.desktop",
        "version": VERSION,
        "written_at": now,
        "server_reachable": server_reachable,
    })
}

/// The state of one revision as the plugin shows it.
pub fn state(now: u64, item: &str, revision: &str, status: &Status) -> Value {
    let local_changes = status
        .files
        .iter()
        .filter(|f| matches!(f.state.as_str(), "modified" | "new" | "conflict"))
        .count();
    json!({
        "schema": "teamcraft.solidcraft.state",
        "version": VERSION,
        "written_at": now,
        "item_number": item,
        "revision": revision,
        "checked_out": {
            "me": status.locked_by_me,
            "by_name": status.locked_by_name,
        },
        "local_changes": local_changes,
        "released": status.released.is_some(),
        "inaccessible": status.inaccessible,
    })
}

/// Writes `name` in the bridge folder: a hidden temporary file, then a
/// rename, so the plugin never reads half a file. Creates the folder (only
/// the user may enter it, where the system has modes).
pub fn write(root: &Path, name: &str, value: &Value) -> std::io::Result<()> {
    let dir = dir(root);
    if !dir.is_dir() {
        std::fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }
    }
    let temp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&temp, value.to_string())?;
    std::fs::rename(&temp, dir.join(name)).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// A request of the plugin, checked.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Request {
    pub action: String,
    pub item: String,
    pub revision: String,
    /// The revision's project group in this working folder.
    pub project: Option<String>,
}

/// The revisions of this working folder: (item, revision) → project group.
pub fn known_revisions(root: &Path) -> BTreeSet<(String, String, Option<String>)> {
    layout::State::load(root)
        .files
        .into_values()
        .map(|e| (e.item, e.revision, e.project))
        .collect()
}

/// Checks one request file. `Err` says why it is refused (the file is
/// removed either way).
pub fn check_request(
    name: &str,
    bytes: &[u8],
    now: u64,
    known: &BTreeSet<(String, String, Option<String>)>,
) -> Result<Request, String> {
    let rest = name
        .strip_prefix("request-")
        .and_then(|r| r.strip_suffix(".json"))
        .ok_or("not a request file name")?;
    if bytes.len() as u64 > MAX_FILE {
        return Err("too large".into());
    }
    let v: Value = serde_json::from_slice(bytes).map_err(|_| "not JSON")?;
    if v["schema"] != "teamcraft.solidcraft.request" {
        return Err("not a request".into());
    }
    match v["version"].as_u64() {
        Some(VERSION) => {}
        Some(n) if n > VERSION => return Err(format!("version {n} – update TeamCraft")),
        _ => return Err("no version".into()),
    }
    let action = v["action"].as_str().unwrap_or("");
    if !ACTIONS.contains(&action) {
        return Err(format!("unknown action {action:?}"));
    }
    let item = v["item_number"].as_str().unwrap_or("");
    let revision = v["revision"].as_str().unwrap_or("");
    let written = v["written_at"].as_u64().unwrap_or(0);
    // the name and the content must say the same
    let expected = format!("{}.{action}", key(item, revision));
    if rest != expected {
        return Err("name and content differ".into());
    }
    // a request from the plugin without a clock says 0: then the file's own
    // age is all there is, which the caller checked
    if written != 0 && (written + MAX_AGE_SECS < now || written > now + 60) {
        return Err("too old".into());
    }
    let project = known
        .iter()
        .find(|(i, r, _)| i == item && r == revision)
        .map(|(_, _, p)| p.clone())
        .ok_or_else(|| format!("{item}/{revision} is not in this working folder"))?;
    Ok(Request {
        action: action.into(),
        item: item.into(),
        revision: revision.into(),
        project,
    })
}

/// Takes the pending requests: each request file is read, checked and
/// removed. Links, folders and other files are left alone or removed
/// unread; a refused request is logged, not acted on.
pub fn take_requests(root: &Path, now: u64) -> Vec<Request> {
    let dir = dir(root);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let known = known_revisions(root);
    let mut out = Vec::new();
    for entry in entries.flatten().take(256) {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("request-") || !name.ends_with(".json") {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let fresh_file = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|age| age.as_secs() <= MAX_AGE_SECS);
        let result = if !meta.file_type().is_file() {
            Err("not a regular file".to_string())
        } else if meta.len() > MAX_FILE {
            Err("too large".to_string())
        } else if !fresh_file {
            Err("too old".to_string())
        } else {
            std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| check_request(&name, &bytes, now, &known))
        };
        let _ = std::fs::remove_file(&path);
        match result {
            Ok(request) => out.push(request),
            Err(why) => eprintln!("SolidCraft request {name} refused: {why}"),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same vectors as `plugins/solidcraft/src/bridge.rs`.
    #[test]
    fn keys_match_the_plugin() {
        assert_eq!(key("PRT-000042", "A"), "PRT_2D000042.A");
        assert_eq!(key("A.B", "c"), "A_2EB.c");
        assert_eq!(key("Ä/1", "B"), "_C3_84_2F1.B");
        assert_eq!(state_file("PRT-000042", "A"), "state-PRT_2D000042.A.json");
    }

    fn known() -> BTreeSet<(String, String, Option<String>)> {
        BTreeSet::from([("PRT-000042".into(), "A".into(), Some("ROVER".into()))])
    }

    fn request(action: &str, at: u64) -> (String, Vec<u8>) {
        let name = format!("request-PRT_2D000042.A.{action}.json");
        let body = json!({"schema": "teamcraft.solidcraft.request", "version": 1,
                          "action": action, "item_number": "PRT-000042", "revision": "A",
                          "written_at": at});
        (name, body.to_string().into_bytes())
    }

    #[test]
    fn requests_are_checked_before_they_are_acted_on() {
        let now = 1_700_000_000;
        let (name, bytes) = request("check_in", now - 5);
        let r = check_request(&name, &bytes, now, &known()).unwrap();
        assert_eq!(r.action, "check_in");
        assert_eq!(r.project.as_deref(), Some("ROVER"));
        // too old, from the future, unknown action, unknown revision
        let (name, bytes) = request("check_in", now - MAX_AGE_SECS - 1);
        assert!(check_request(&name, &bytes, now, &known()).is_err());
        let (name, bytes) = request("open", now + 3600);
        assert!(check_request(&name, &bytes, now, &known()).is_err());
        let (name, bytes) = request("delete_everything", now);
        assert!(check_request(&name, &bytes, now, &known()).is_err());
        let (name, bytes) = request("open", now);
        assert!(check_request(&name, &bytes, now, &BTreeSet::new()).is_err());
        // the name must match the content, the version must be known
        assert!(check_request("request-x.1.open.json", &bytes, now, &known()).is_err());
        let newer = String::from_utf8(bytes.clone())
            .unwrap()
            .replace("\"version\":1", "\"version\":2");
        let e = check_request(&name, newer.as_bytes(), now, &known()).unwrap_err();
        assert!(e.contains("update TeamCraft"), "{e}");
        assert!(check_request(&name, b"{", now, &known()).is_err());
    }

    #[test]
    fn requests_are_taken_once_and_bad_ones_removed() {
        let root = std::env::temp_dir().join(format!("tc-scbridge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let state = layout::State {
            version: 1,
            files: std::collections::BTreeMap::from([(
                "rover/bracket.sc".to_string(),
                layout::Entry {
                    path: "rover/bracket.sc".into(),
                    project: Some("ROVER".into()),
                    item: "PRT-000042".into(),
                    revision: "A".into(),
                    file_id: "f".into(),
                    filename: "bracket.sc".into(),
                    version: 1,
                    sha256: String::new(),
                    checked_out: true,
                    size: 0,
                    modified_ms: 0,
                },
            )]),
        };
        std::fs::create_dir_all(root.join(layout::STATE_DIR)).unwrap();
        std::fs::write(
            layout::State::path(&root),
            serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        write(&root, "desktop.json", &heartbeat(1, true)).unwrap();
        let now = now_secs();
        let (good, bytes) = request("open", now);
        std::fs::write(dir(&root).join(&good), bytes).unwrap();
        std::fs::write(dir(&root).join("request-bad.json"), b"{}").unwrap();
        let taken = take_requests(&root, now);
        assert_eq!(taken.len(), 1, "{taken:?}");
        assert_eq!(taken[0].item, "PRT-000042");
        assert!(take_requests(&root, now).is_empty(), "taken once");
        assert!(
            !dir(&root).join("request-bad.json").exists(),
            "refused ones go"
        );
        assert!(dir(&root).join("desktop.json").exists(), "other files stay");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_state_names_no_secret() {
        let status = Status {
            dir: PathBuf::from("/w"),
            exists: true,
            locked_by_me: false,
            locked_by_name: Some("Kim Lee".into()),
            files: Vec::new(),
            inaccessible: false,
            released: None,
            bom_structure: false,
        };
        let s = state(5, "PRT-000042", "A", &status);
        assert_eq!(s["checked_out"]["by_name"], "Kim Lee");
        assert_eq!(s["schema"], "teamcraft.solidcraft.state");
        let text = s.to_string();
        assert!(!text.contains("token") && !text.contains("/w"), "{text}");
    }
}
