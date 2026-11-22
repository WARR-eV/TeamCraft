//! Which local files a CAD program has open, from the lock files the
//! program keeps next to them – shown in the Workspace tab and as a warning
//! at check-in (the file may have unsaved changes).
//!
//! SolidCraft (`.sc`, `.solidcraft`): while its desktop app has a design
//! open it keeps `.~<file name>.lock` beside it, a JSON object with the
//! computer (`host`), the `user`, the app version, the process and when the
//! design was opened and the lock last `refreshed` (Unix seconds). It is
//! refreshed every 60 s and removed on close; a lock not refreshed for
//! 180 s – or dated that far ahead – belongs to an app that is gone and is
//! ignored. Only a regular file of at most 4 KiB is read; a link or anything
//! else at that place is no lock. Sources: `docs/src/research/solidcraft-design.md`.
//! These lock files are never uploaded (`layout::is_ignored`: hidden names).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// SolidCraft refreshes its lock every 60 s; three missed refreshes = stale.
const STALE_SECS: u64 = 180;
/// A real lock is a few hundred bytes.
const MAX_LOCK_BYTES: u64 = 4096;

/// A CAD program that has a local file open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OpenIn {
    /// The program, as shown (`SolidCraft`).
    pub program: &'static str,
    /// The computer it runs on.
    pub computer: String,
    /// The user it runs as (may be empty).
    pub user: String,
    /// When the file was opened (Unix seconds).
    pub since: u64,
}

/// The program that has `path` open now, if its lock file says so.
pub fn open_in(path: &Path) -> Option<OpenIn> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    open_in_at(path, now)
}

fn open_in_at(path: &Path, now: u64) -> Option<OpenIn> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    if ext != "sc" && ext != "solidcraft" {
        return None;
    }
    let lock = read_lock(&solidcraft_lock_path(path)?)?;
    #[derive(serde::Deserialize)]
    struct Lock {
        #[serde(default)]
        host: String,
        #[serde(default)]
        user: String,
        #[serde(default)]
        opened: u64,
        refreshed: u64,
    }
    let lock: Lock = serde_json::from_slice(&lock).ok()?;
    let stale = now.saturating_sub(lock.refreshed) > STALE_SECS
        || lock.refreshed.saturating_sub(now) > STALE_SECS;
    if stale {
        return None;
    }
    let computer = lock.host.trim();
    Some(OpenIn {
        program: "SolidCraft",
        computer: if computer.is_empty() {
            "another computer".into()
        } else {
            computer.into()
        },
        user: lock.user.trim().to_string(),
        since: lock.opened,
    })
}

/// `.~<file name>.lock` in the design's folder.
fn solidcraft_lock_path(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy();
    Some(path.with_file_name(format!(".~{name}.lock")))
}

/// The lock's bytes: only a regular file (links are not followed) of at
/// most [`MAX_LOCK_BYTES`].
fn read_lock(path: &Path) -> Option<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_LOCK_BYTES {
        return None;
    }
    let mut data = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_LOCK_BYTES + 1)
        .read_to_end(&mut data)
        .ok()?;
    (data.len() as u64 <= MAX_LOCK_BYTES).then_some(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    const NOW: u64 = 1_800_000_000;

    fn lock(refreshed: u64) -> String {
        format!(
            r#"{{"host":"WS-12","user":"kim","app":"0.46.1","pid":4242,"opened":{},"refreshed":{refreshed}}}"#,
            NOW - 600
        )
    }

    #[test]
    fn a_fresh_lock_names_computer_and_user() {
        let d = dir();
        let design = d.path().join("Bracket.sc");
        std::fs::write(&design, b"{}").unwrap();
        assert_eq!(open_in_at(&design, NOW), None, "no lock");
        std::fs::write(d.path().join(".~Bracket.sc.lock"), lock(NOW - 30)).unwrap();
        assert_eq!(
            open_in_at(&design, NOW),
            Some(OpenIn {
                program: "SolidCraft",
                computer: "WS-12".into(),
                user: "kim".into(),
                since: NOW - 600,
            })
        );
        assert!(
            crate::layout::is_ignored(".~Bracket.sc.lock"),
            "never uploaded"
        );
        assert!(
            crate::layout::is_ignored("Bracket.sc.bak"),
            "backup never uploaded"
        );
    }

    #[test]
    fn stale_future_malformed_and_oversized_locks_are_ignored() {
        let d = dir();
        let design = d.path().join("plate.solidcraft");
        let lock_file = d.path().join(".~plate.solidcraft.lock");
        for (content, why) in [
            (lock(NOW - 181), "stale"),
            (lock(NOW + 181), "dated far ahead"),
            ("not json".to_string(), "malformed"),
            (r#"{"host":"x"}"#.to_string(), "no refresh time"),
            (" ".repeat(MAX_LOCK_BYTES as usize + 1), "oversized"),
        ] {
            std::fs::write(&lock_file, content).unwrap();
            assert_eq!(open_in_at(&design, NOW), None, "{why}");
        }
        std::fs::write(&lock_file, lock(NOW)).unwrap();
        assert!(open_in_at(&design, NOW).is_some());
    }

    #[test]
    fn only_solidcraft_designs_are_checked() {
        let d = dir();
        std::fs::write(d.path().join(".~bracket.ipt.lock"), lock(NOW)).unwrap();
        assert_eq!(open_in_at(&d.path().join("bracket.ipt"), NOW), None);
        assert_eq!(open_in_at(&d.path().join("noext"), NOW), None);
    }
}
