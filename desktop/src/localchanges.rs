//! *Local changes* (#416): one list over the whole working folder – the
//! files of every revision the working folder tracks that are **modified**,
//! **outdated**, in **conflict**, **new** (a companion of a tracked file, as
//! the STEP export next to a part) or **missing**, grouped by revision.
//!
//! Two passes, both bounded so a huge folder never blocks the app:
//!
//! 1. A **local pass** without the network ([`local_pass`]): every entry of
//!    `.teamcraft/state.json` (at most `newfiles::MAX_ENTRIES`, the limits of *New on this computer*) is compared
//!    with the disk. A file that is gone is *missing*; a file whose size
//!    differs is *modified*; a file with the same size but another
//!    modification time is hashed (at most [`MAX_HASH_BYTES`] per scan) and
//!    *modified* if its SHA-256 differs from the base. A file beyond that
//!    budget (or without a known base hash) is *unknown*: compared first by
//!    the server pass, but never listed as modified on the local pass alone.
//!    An unchanged file is never read.
//! 2. A **server pass** ([`scan`]): the Workspace status of one revision
//!    after the other (`workspace::status_with` without side effects – the
//!    comparison of the Workspace tab, but no read-only flags, renames or
//!    state writes), at most [`MAX_REVISIONS`] – revisions with a local change
//!    first, then my check-outs, then a window over the rest that moves on
//!    with every scan ([`pick`]), so in a folder with more revisions every
//!    one is compared within a few scans. Only this pass knows *outdated* (a
//!    newer version on the server than the base) and *conflict* (changed
//!    here and on the server). The first failing call (server unreachable,
//!    signed out) stops the pass; the revisions not compared keep what the
//!    local pass found and say so (`compared` false).
//!
//! [`classify`] puts both together; it is pure and unit-tested. Nothing is
//! uploaded, downloaded or written here: the actions of the list (*Check out
//! (keep my file)*, *Get latest*, *Check in selected*) are the commands the
//! Workspace tab uses (`ui/js/local-changes.js`).

use std::{
    collections::{BTreeMap, HashMap},
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
    time::UNIX_EPOCH,
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    api::{Api, ApiError},
    layout::{self, Entry, State},
    newfiles::Limits,
    workspace,
};

/// Revisions the server pass compares at most per scan.
pub const MAX_REVISIONS: usize = 100;
/// Bytes the local pass hashes at most per scan (files with the same size
/// but a new modification time); beyond it a file is *unknown* until the
/// server pass compares it.
pub const MAX_HASH_BYTES: u64 = 512 * 1024 * 1024;

/// Where the window over the revisions without a local change and without
/// my check-out starts in the next scan ([`pick`]).
static ROTATION: Mutex<usize> = Mutex::new(0);

/// The file states the list shows (everything but `up_to_date`, `clash` and
/// `inaccessible`).
pub const SHOWN: [&str; 5] = ["modified", "outdated", "conflict", "new", "missing"];

/// A revision: `(project, item, revision)`, compared ignoring case.
pub type RevKey = (String, String, String);

fn key_of(project: Option<&str>, item: &str, revision: &str) -> RevKey {
    (
        project.unwrap_or("").to_lowercase(),
        item.to_lowercase(),
        revision.to_lowercase(),
    )
}

/// What the local pass found for one tracked file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disk {
    /// The file is in the working folder.
    pub exists: bool,
    /// Its content differs from the base: `Some(true)`, equals it:
    /// `Some(false)`, could not be checked (hash budget used up, no base
    /// hash, unreadable): `None` – only the server pass decides then.
    pub modified: Option<bool>,
    /// The file is writable (not read-only).
    pub writable: bool,
}

/// One file of a revision as the server pass (Workspace status) saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerFile {
    pub name: String,
    pub path: String,
    pub state: String,
    pub server_version: Option<i64>,
    pub base_version: Option<i64>,
    pub read_only: bool,
}

/// One revision as the server pass saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerRevision {
    pub locked_by_me: bool,
    pub inaccessible: bool,
    pub dir: PathBuf,
    pub files: Vec<ServerFile>,
}

impl From<&workspace::Status> for ServerRevision {
    fn from(s: &workspace::Status) -> ServerRevision {
        ServerRevision {
            locked_by_me: s.locked_by_me,
            inaccessible: s.inaccessible,
            dir: s.dir.clone(),
            files: s
                .files
                .iter()
                .map(|f| ServerFile {
                    name: f.name.clone(),
                    path: f.path.clone(),
                    state: f.state.clone(),
                    server_version: f.server_version,
                    base_version: f.base_version,
                    read_only: f.read_only,
                })
                .collect(),
        }
    }
}

/// A file of the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChangedFile {
    /// Name on the server.
    pub name: String,
    /// Relative path in the working folder.
    pub path: String,
    /// `modified`, `outdated`, `conflict`, `new` or `missing`.
    pub state: String,
    pub server_version: Option<i64>,
    /// The server version the local copy came from, if known.
    pub base_version: Option<i64>,
    pub writable: bool,
    /// Changed here although the revision is not checked out by me (a file
    /// whose read-only flag was cleared): *Check out (keep my file)*.
    pub without_checkout: bool,
}

/// A revision with at least one file of the list (or an error).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RevisionChanges {
    pub project: Option<String>,
    pub item: String,
    pub code: String,
    /// The revision is checked out by me (server pass) or was when its files
    /// were last written (local pass).
    pub checked_out: bool,
    /// The server pass compared it: `outdated` and `conflict` are known.
    pub compared: bool,
    /// The server answers "not found" (no longer accessible).
    pub inaccessible: bool,
    /// Why it could not be compared (the server pass stopped there).
    pub error: Option<String>,
    /// The folder of its files (absolute).
    pub dir: PathBuf,
    pub files: Vec<ChangedFile>,
}

/// Result of [`scan`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct LocalChangesScan {
    /// Sorted by item and revision.
    pub revisions: Vec<RevisionChanges>,
    /// A limit was hit (entries or files): there may be more changes than
    /// listed. More revisions than [`MAX_REVISIONS`] show in `compared` <
    /// `tracked` instead.
    pub truncated: bool,
    /// Revisions the server pass compared.
    pub compared: usize,
    /// Revisions the working folder tracks (as far as the scan looked).
    pub tracked: usize,
}

// ---------------------------------------------------------------------------
// Local pass
// ---------------------------------------------------------------------------

/// The tracked files read from the state, grouped by revision, and what the
/// disk says about each of them.
#[derive(Clone, Debug, Default)]
pub struct LocalPass {
    pub entries: Vec<Entry>,
    /// [`layout::key_of`] of `entry.path` → disk state.
    pub disk: HashMap<String, Disk>,
    /// The state held more than `limits.entries` files.
    pub truncated: bool,
}

fn modified_ms(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// What the disk says about one tracked file; `budget` is the number of
/// bytes that may still be hashed.
pub(crate) fn disk_of(root: &Path, e: &Entry, budget: &mut u64) -> Disk {
    let path = layout::join_rel(root, &e.path);
    let meta = match std::fs::metadata(&path) {
        Ok(m) if m.is_file() => m,
        _ => {
            return Disk {
                exists: false,
                modified: Some(false),
                writable: false,
            }
        }
    };
    let writable = !workspace::is_read_only(&path);
    let (size, mtime) = (meta.len(), modified_ms(&meta));
    let modified = if size != e.size && e.size != 0 {
        // a known size that differs: changed for sure (no size known – an
        // old state – is decided by the hash)
        Some(true)
    } else if size == e.size && mtime == e.modified_ms && mtime != 0 {
        Some(false)
    } else {
        hash_differs(&path, e, size, budget)
    };
    Disk {
        exists: true,
        modified,
        writable,
    }
}

/// `Some(differs)`, or `None` when the file cannot be checked here (no base
/// hash, beyond the hash budget, unreadable).
fn hash_differs(path: &Path, e: &Entry, size: u64, budget: &mut u64) -> Option<bool> {
    if e.sha256.is_empty() || size > *budget {
        return None;
    }
    *budget -= size;
    sha256_file(path).ok().map(|s| s != e.sha256)
}

/// Compares the tracked files of `state` with the disk below `root`, at most
/// `limits.entries` of them. Never fails: what cannot be read counts as
/// missing or modified.
pub(crate) fn local_pass(root: &Path, state: &State, limits: Limits) -> LocalPass {
    let mut out = LocalPass::default();
    let mut budget = MAX_HASH_BYTES;
    for e in state.files.values() {
        if out.entries.len() >= limits.entries {
            out.truncated = true;
            break;
        }
        out.disk
            .insert(layout::key_of(&e.path), disk_of(root, e, &mut budget));
        out.entries.push(e.clone());
    }
    out
}

// ---------------------------------------------------------------------------
// Pure rules
// ---------------------------------------------------------------------------

/// Sort key of a revision in the server pass: rank, item, revision order.
type Rank = (u8, String, (usize, String));

/// The revisions of `entries` in the order the server pass compares them,
/// with their rank: revisions with a local change (missing, modified or not
/// checkable here) first (0), then the ones checked out by me (1), then the
/// rest (2) – each part by item and revision.
pub fn server_order(entries: &[Entry], disk: &HashMap<String, Disk>) -> Vec<(RevKey, u8)> {
    // key → (rank, item, revision order)
    let mut ranks: BTreeMap<RevKey, Rank> = BTreeMap::new();
    for e in entries {
        let key = key_of(e.project.as_deref(), &e.item, &e.revision);
        let d = disk.get(&layout::key_of(&e.path));
        let rank = if d.is_some_and(|d| !d.exists || d.modified != Some(false)) {
            0
        } else if e.checked_out {
            1
        } else {
            2
        };
        let slot = ranks.entry(key).or_insert((
            rank,
            e.item.to_lowercase(),
            layout::revision_order(&e.revision),
        ));
        slot.0 = slot.0.min(rank);
    }
    let mut keys: Vec<(RevKey, Rank)> = ranks.into_iter().collect();
    keys.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    keys.into_iter().map(|(k, r)| (k, r.0)).collect()
}

/// The revisions one scan compares (at most `max`) and where the window
/// starts next time: every revision of rank 0 and 1 (local changes, my
/// check-outs) as far as `max` allows, then the rest from `cursor` on,
/// wrapping around – so over successive scans every revision of a large
/// working folder is compared for *outdated* files, not only the first ones.
pub fn pick(order: &[(RevKey, u8)], cursor: usize, max: usize) -> (Vec<RevKey>, usize) {
    let mut out: Vec<RevKey> = order
        .iter()
        .filter(|(_, r)| *r < 2)
        .take(max)
        .map(|(k, _)| k.clone())
        .collect();
    let rest: Vec<&RevKey> = order
        .iter()
        .filter(|(_, r)| *r >= 2)
        .map(|(k, _)| k)
        .collect();
    if rest.is_empty() {
        return (out, 0);
    }
    let room = max.saturating_sub(out.len()).min(rest.len());
    let start = cursor % rest.len();
    out.extend((0..room).map(|i| rest[(start + i) % rest.len()].clone()));
    (out, (start + room) % rest.len())
}

/// The list: per revision of `entries` either the files of its server status
/// (`server`, when compared) in a [`SHOWN`] state, or the missing and
/// modified files of the local pass (`disk`). `errors` holds the revisions
/// the server pass failed on. Revisions without such a file and without an
/// error are left out; at most `max_files` files are listed (the second
/// value is true when some were left out).
pub fn classify(
    root: &Path,
    entries: &[Entry],
    disk: &HashMap<String, Disk>,
    server: &HashMap<RevKey, ServerRevision>,
    errors: &HashMap<RevKey, String>,
    max_files: usize,
) -> (Vec<RevisionChanges>, bool) {
    let mut groups: BTreeMap<RevKey, Vec<&Entry>> = BTreeMap::new();
    for e in entries {
        groups
            .entry(key_of(e.project.as_deref(), &e.item, &e.revision))
            .or_default()
            .push(e);
    }
    let mut out = Vec::new();
    let mut listed = 0usize;
    let mut cut = false;
    for (key, list) in groups {
        let first = list[0];
        let local_checked_out = list.iter().any(|e| e.checked_out);
        let dir_of_first = layout::join_rel(root, &first.path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| root.to_path_buf());
        let mut rev = RevisionChanges {
            project: first.project.clone(),
            item: first.item.clone(),
            code: first.revision.clone(),
            checked_out: local_checked_out,
            compared: false,
            inaccessible: false,
            error: errors.get(&key).cloned(),
            dir: dir_of_first,
            files: Vec::new(),
        };
        if let Some(s) = server.get(&key) {
            rev.compared = true;
            rev.inaccessible = s.inaccessible;
            rev.checked_out = s.locked_by_me;
            if !s.dir.as_os_str().is_empty() {
                rev.dir = s.dir.clone();
            }
            for f in &s.files {
                if !SHOWN.contains(&f.state.as_str()) {
                    continue;
                }
                let changed = matches!(f.state.as_str(), "modified" | "conflict" | "new");
                rev.files.push(ChangedFile {
                    name: f.name.clone(),
                    path: f.path.clone(),
                    state: f.state.clone(),
                    server_version: f.server_version,
                    base_version: f.base_version,
                    writable: f.state != "missing" && !f.read_only,
                    without_checkout: changed && !s.locked_by_me && !s.inaccessible,
                });
            }
        } else {
            for e in &list {
                let Some(d) = disk.get(&layout::key_of(&e.path)) else {
                    continue;
                };
                // a file that could not be checked here (`None`) waits for
                // the server pass: never "modified" – and never offered a
                // check-out – on a guess
                let state = if !d.exists {
                    "missing"
                } else if d.modified == Some(true) {
                    "modified"
                } else {
                    continue;
                };
                rev.files.push(ChangedFile {
                    name: e.filename.clone(),
                    path: e.path.clone(),
                    state: state.to_string(),
                    server_version: None,
                    base_version: Some(e.version),
                    writable: d.writable,
                    without_checkout: state == "modified" && !e.checked_out,
                });
            }
        }
        if rev.files.is_empty() && rev.error.is_none() {
            continue;
        }
        if listed + rev.files.len() > max_files {
            cut = true;
            continue;
        }
        listed += rev.files.len();
        rev.files
            .sort_by_key(|f| (f.path.to_lowercase(), f.name.to_lowercase()));
        out.push(rev);
    }
    out.sort_by_key(|r| (r.item.to_lowercase(), layout::revision_order(&r.code)));
    (out, cut)
}

// ---------------------------------------------------------------------------
// The scan
// ---------------------------------------------------------------------------

/// The whole scan: local pass, then the server pass over at most
/// [`MAX_REVISIONS`] revisions ([`pick`], the window moves on with every
/// scan), then [`classify`].
pub async fn scan(api: &Api, root: &Path) -> Result<LocalChangesScan, ApiError> {
    if !root.is_dir() {
        return Ok(LocalChangesScan::default());
    }
    let limits = Limits::default();
    let dir = root.to_path_buf();
    let local = tokio::task::spawn_blocking(move || {
        let state = State::load(&dir);
        local_pass(&dir, &state, limits)
    })
    .await
    .map_err(|e| ApiError::local(format!("The scan for local changes stopped: {e}")))?;
    let order = server_order(&local.entries, &local.disk);
    let tracked = order.len();
    let chosen = {
        let mut cursor = ROTATION.lock().unwrap_or_else(|e| e.into_inner());
        let (chosen, next) = pick(&order, *cursor, MAX_REVISIONS);
        *cursor = next;
        chosen
    };
    let names: HashMap<RevKey, (Option<String>, String, String)> = local
        .entries
        .iter()
        .map(|e| {
            (
                key_of(e.project.as_deref(), &e.item, &e.revision),
                (e.project.clone(), e.item.clone(), e.revision.clone()),
            )
        })
        .collect();
    let mut server = HashMap::new();
    let mut errors = HashMap::new();
    for key in &chosen {
        let Some((project, item, code)) = names.get(key) else {
            continue;
        };
        // compares only: no read-only flags, renames or state writes
        match workspace::status_with(api, root, project.as_deref(), item, code, false).await {
            Ok(status) => {
                server.insert(key.clone(), ServerRevision::from(&status));
            }
            Err(e) => {
                // unreachable server, signed out …: the rest keeps the local view
                errors.insert(key.clone(), e.to_string());
                break;
            }
        }
    }
    let compared = server.len();
    let (revisions, cut) = classify(
        root,
        &local.entries,
        &local.disk,
        &server,
        &errors,
        limits.files,
    );
    Ok(LocalChangesScan {
        revisions,
        // more revisions than one scan compares is not "truncated": the
        // window moves on and `tracked`/`compared` say it
        truncated: local.truncated || cut,
        compared,
        tracked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(item: &str, rev: &str, name: &str, checked_out: bool) -> Entry {
        Entry {
            path: format!("ROVER/{item}/{name}"),
            project: Some("ROVER".into()),
            item: item.into(),
            revision: rev.into(),
            file_id: format!("id-{name}"),
            filename: name.into(),
            version: 2,
            sha256: "a".repeat(64),
            checked_out,
            size: 10,
            modified_ms: 1000,
        }
    }

    fn disk(exists: bool, modified: bool, writable: bool) -> Disk {
        Disk {
            exists,
            modified: Some(modified),
            writable,
        }
    }

    fn server_file(e: &Entry, state: &str, server_version: Option<i64>) -> ServerFile {
        ServerFile {
            name: e.filename.clone(),
            path: e.path.clone(),
            state: state.into(),
            server_version,
            base_version: Some(e.version),
            read_only: false,
        }
    }

    fn key(e: &Entry) -> RevKey {
        key_of(e.project.as_deref(), &e.item, &e.revision)
    }

    fn run(
        entries: &[Entry],
        d: &[(&Entry, Disk)],
        server: &[(&Entry, ServerRevision)],
    ) -> Vec<RevisionChanges> {
        let disk: HashMap<String, Disk> = d
            .iter()
            .map(|(e, d)| (layout::key_of(&e.path), d.clone()))
            .collect();
        let server: HashMap<RevKey, ServerRevision> =
            server.iter().map(|(e, s)| (key(e), s.clone())).collect();
        classify(
            Path::new("/ws"),
            entries,
            &disk,
            &server,
            &HashMap::new(),
            1000,
        )
        .0
    }

    #[test]
    fn modified_without_checkout_from_the_local_pass() {
        let e = entry("PRT-1", "A", "bracket.prt", false);
        let out = run(&[e.clone()], &[(&e, disk(true, true, true))], &[]);
        assert_eq!(out.len(), 1);
        let f = &out[0].files[0];
        assert_eq!(f.state, "modified");
        assert!(f.without_checkout);
        assert!(f.writable);
        assert!(!out[0].compared);
    }

    #[test]
    fn modified_in_my_checkout_needs_no_checkout() {
        let e = entry("PRT-1", "A", "bracket.prt", true);
        let out = run(&[e.clone()], &[(&e, disk(true, true, true))], &[]);
        assert!(!out[0].files[0].without_checkout);
        assert!(out[0].checked_out);
    }

    #[test]
    fn conflict_and_outdated_come_from_the_server_pass() {
        let a = entry("PRT-1", "A", "bracket.prt", false);
        let b = entry("PRT-1", "A", "bracket.drw", false);
        let s = ServerRevision {
            locked_by_me: false,
            inaccessible: false,
            dir: PathBuf::from("/ws/ROVER/PRT-1"),
            files: vec![
                server_file(&a, "conflict", Some(3)),
                server_file(&b, "outdated", Some(4)),
            ],
        };
        let out = run(
            &[a.clone(), b.clone()],
            &[(&a, disk(true, true, true)), (&b, disk(true, false, false))],
            &[(&a, s)],
        );
        assert!(out[0].compared);
        let states: Vec<_> = out[0].files.iter().map(|f| f.state.as_str()).collect();
        assert_eq!(states, ["outdated", "conflict"]);
        let conflict = &out[0].files[1];
        assert!(conflict.without_checkout);
        assert_eq!(conflict.server_version, Some(3));
        assert_eq!(conflict.base_version, Some(2));
        assert!(!out[0].files[0].without_checkout);
    }

    #[test]
    fn missing_and_unchanged_files() {
        let a = entry("PRT-1", "A", "bracket.prt", false);
        let b = entry("PRT-2", "B", "plate.prt", false);
        let out = run(
            &[a.clone(), b.clone()],
            &[
                (&a, disk(false, false, false)),
                (&b, disk(true, false, false)),
            ],
            &[],
        );
        assert_eq!(out.len(), 1, "an unchanged revision is not listed");
        assert_eq!(out[0].files[0].state, "missing");
        assert!(!out[0].files[0].without_checkout);
    }

    #[test]
    fn up_to_date_and_clash_from_the_server_are_left_out() {
        let a = entry("PRT-1", "A", "bracket.prt", true);
        let s = ServerRevision {
            locked_by_me: true,
            inaccessible: false,
            dir: PathBuf::new(),
            files: vec![
                server_file(&a, "up_to_date", Some(2)),
                server_file(&a, "clash", None),
            ],
        };
        let out = run(&[a.clone()], &[(&a, disk(true, false, true))], &[(&a, s)]);
        assert!(out.is_empty());
    }

    #[test]
    fn server_order_puts_local_changes_and_checkouts_first() {
        let rest = entry("PRT-1", "A", "a.prt", false);
        let mine = entry("PRT-2", "A", "b.prt", true);
        let changed = entry("PRT-3", "A", "c.prt", false);
        let disk: HashMap<String, Disk> = [
            (layout::key_of(&rest.path), disk(true, false, false)),
            (layout::key_of(&mine.path), disk(true, false, true)),
            (layout::key_of(&changed.path), disk(true, true, true)),
        ]
        .into_iter()
        .collect();
        let order = server_order(&[rest.clone(), mine.clone(), changed.clone()], &disk);
        assert_eq!(
            order,
            vec![(key(&changed), 0), (key(&mine), 1), (key(&rest), 2)]
        );
    }

    fn rk(i: usize) -> RevKey {
        ("rover".into(), format!("prt-{i}"), "a".into())
    }

    #[test]
    fn pick_rotates_over_the_rest_and_keeps_changes_first() {
        let mut order = vec![(rk(100), 0), (rk(101), 1)];
        order.extend((0..5).map(|i| (rk(i), 2)));
        // room for 2 of the rest per scan: 0,1 – 2,3 – 4,0 – …
        let (a, next) = pick(&order, 0, 4);
        assert_eq!(a, vec![rk(100), rk(101), rk(0), rk(1)]);
        let (b, next) = pick(&order, next, 4);
        assert_eq!(b, vec![rk(100), rk(101), rk(2), rk(3)]);
        let (c, next) = pick(&order, next, 4);
        assert_eq!(c, vec![rk(100), rk(101), rk(4), rk(0)]);
        assert_eq!(next, 1);
        // everything fits: each revision once, whatever the cursor
        let (all, _) = pick(&order, 3, 100);
        assert_eq!(all.len(), 7);
        let unique: std::collections::BTreeSet<_> = all.iter().collect();
        assert_eq!(unique.len(), 7);
        // only changes: no room for the rest, the cursor stays in range
        let (few, next) = pick(&order, 2, 1);
        assert_eq!(few, vec![rk(100)]);
        assert_eq!(next, 2);
        assert_eq!(pick(&[], 9, 10), (vec![], 0));
    }

    #[test]
    fn an_unchecked_file_is_compared_first_but_never_listed_on_a_guess() {
        let e = entry("PRT-1", "A", "huge.prt", false);
        let unknown = Disk {
            exists: true,
            modified: None,
            writable: true,
        };
        let d: HashMap<String, Disk> = [(layout::key_of(&e.path), unknown.clone())]
            .into_iter()
            .collect();
        assert_eq!(server_order(&[e.clone()], &d), vec![(key(&e), 0)]);
        let out = run(&[e.clone()], &[(&e, unknown)], &[]);
        assert!(out.is_empty(), "not modified, no check-out offered");
    }

    #[test]
    fn file_cap_truncates() {
        let entries: Vec<Entry> = (0..5)
            .map(|i| entry(&format!("PRT-{i}"), "A", "p.prt", false))
            .collect();
        let disk: HashMap<String, Disk> = entries
            .iter()
            .map(|e| (layout::key_of(&e.path), disk(false, false, false)))
            .collect();
        let (out, cut) = classify(
            Path::new("/ws"),
            &entries,
            &disk,
            &HashMap::new(),
            &HashMap::new(),
            3,
        );
        assert_eq!(out.len(), 3);
        assert!(cut);
    }

    #[test]
    fn a_failed_revision_is_listed_with_its_error() {
        let a = entry("PRT-1", "A", "bracket.prt", false);
        let errors: HashMap<RevKey, String> = [(key(&a), "server unreachable".to_string())]
            .into_iter()
            .collect();
        let disk: HashMap<String, Disk> = [(layout::key_of(&a.path), disk(true, false, false))]
            .into_iter()
            .collect();
        let (out, _) = classify(Path::new("/ws"), &[a], &disk, &HashMap::new(), &errors, 10);
        assert_eq!(out[0].error.as_deref(), Some("server unreachable"));
        assert!(out[0].files.is_empty());
    }

    #[test]
    fn local_pass_reads_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("ROVER/PRT-1")).unwrap();
        let same = b"0123456789";
        std::fs::write(root.join("ROVER/PRT-1/same.prt"), same).unwrap();
        std::fs::write(root.join("ROVER/PRT-1/bigger.prt"), b"01234567890123").unwrap();
        let mut state = State::default();
        let mut add = |name: &str, sha: String| {
            let mut e = entry("PRT-1", "A", name, false);
            e.sha256 = sha;
            state.insert(e);
        };
        // same size, other time → hashed: the content equals the base
        add("same.prt", hex::encode(Sha256::digest(same)));
        add("bigger.prt", "b".repeat(64));
        add("gone.prt", "c".repeat(64));
        let pass = local_pass(root, &state, Limits::default());
        assert!(!pass.truncated);
        let get = |name: &str| pass.disk[&layout::key_of(&format!("ROVER/PRT-1/{name}"))].clone();
        assert!(get("same.prt").exists && get("same.prt").modified == Some(false));
        assert_eq!(get("bigger.prt").modified, Some(true));
        assert!(!get("gone.prt").exists);
        let small = local_pass(
            root,
            &state,
            Limits {
                entries: 2,
                files: 10,
            },
        );
        assert!(small.truncated);
        assert_eq!(small.entries.len(), 2);
    }
}
