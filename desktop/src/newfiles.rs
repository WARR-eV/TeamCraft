//! *New on this computer*: files in the working folder that belong to no item
//! yet – typically parts and assemblies just saved in the CAD program.
//!
//! The worklist shows them in its section *New on this computer* and offers
//! to create items from them through the *New* dialog. This module only reads
//! the working folder and keeps the user's ignore list; it never uploads
//! anything – a file found here is a suggestion, never a check-in.
//!
//! What the scan ([`scan_root`]) reports, by the rules of the folder import
//! (`folderimport.rs`):
//!
//! * only files below a project group folder (`<workspace>/<GROUP>/…`); files
//!   directly in the working folder belong to no group;
//! * folders whose name starts with `.` (`.teamcraft`, `.git`) and the backup
//!   and system folders of [`SKIPPED_DIRS`] (`OldVersions`, …) are not
//!   entered, symbolic links are not followed, unreadable folders are skipped;
//! * lock, backup and temporary files ([`layout::is_ignored`]: `~$…`,
//!   `….teamcraft-download`, …) are left out;
//! * so are files that belong to an item already or are on their way to one:
//!   tracked in `.teamcraft/state.json`, added with *Add files…* and not
//!   checked in yet (`.teamcraft/added.json`), or a companion of a tracked
//!   file – the same name stem in the same folder, as the STEP export
//!   `Bracket.step` next to the tracked `Bracket.ipt`, which goes into that
//!   item's check-in;
//! * files the user chose to ignore (`.teamcraft/ignored.json`,
//!   [`new_files_ignore`]) are hidden and only counted; after a complete walk
//!   the entries of files that are gone are dropped from the list, after a
//!   walk that stopped at a limit or could not read a folder nothing is.
//!
//! Names are reported composed (NFC, [`layout::nfc`]) with `/` separators, as
//! the server stores them. The walk stops after [`MAX_ENTRIES`] directory
//! entries or [`MAX_FILES`] new files and says so (`truncated`), so a huge
//! folder never blocks the app. Grouping a drawing with its model and telling
//! CAD files from other files is left to the user interface. Supplier downloads
//! (STEP …) next to the native part saved from them get a hint
//! ([`companions::link`], #564).

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Mutex,
    time::UNIX_EPOCH,
};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    addfiles,
    api::ApiError,
    commands::workspace_root,
    companions,
    folderimport::SKIPPED_DIRS,
    layout::{self, STATE_DIR},
    AppState,
};

/// Directory entries (files and folders) one scan visits at most.
pub const MAX_ENTRIES: usize = 50_000;
/// New files one scan reports at most.
pub const MAX_FILES: usize = 2_000;
/// Paths one call of [`new_files_ignore`] or [`new_files_unignore`] takes.
pub const MAX_RELS: usize = 1_000;
/// Characters of one path in the ignore list.
pub const MAX_REL_CHARS: usize = 1_024;

/// The user's ignore list in the app's folder of the workspace.
const IGNORED_FILE: &str = "ignored.json";

/// Changes of the ignore list one at a time (read, change, write).
static IGNORED_LOCK: Mutex<()> = Mutex::new(());

/// A file in the working folder that belongs to no item yet.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NewFile {
    /// `<group>/<folder>/<name>` below the working folder, `/`-separated,
    /// composed – the path [`new_files_ignore`] takes.
    pub rel: String,
    /// Absolute path of the file as it is on disk (a name on a Mac may be
    /// decomposed there while `rel` and `name` are composed), for reading it.
    pub path: PathBuf,
    /// The project group folder the file is in (its name, composed).
    pub project: String,
    /// The folders between the group folder and the file (`Design/Struktur`;
    /// empty directly in the group folder).
    pub folder: String,
    /// File name, composed.
    pub name: String,
    pub size: u64,
    /// Last change, milliseconds since 1970 (0 if unknown).
    pub modified_ms: u64,
    /// A supplier download (STEP …) that belongs to this new native part
    /// (`rel`): one item for both (#564, [`companions::link`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub companion_of: Option<String>,
    /// A supplier download that belongs to the item of a tracked native part:
    /// *Add to item* (#564).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub belongs_to: Option<companions::Owner>,
    /// Why `companion_of` / `belongs_to` was set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<companions::Reason>,
}

/// Result of [`new_files_scan`].
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct NewFilesScan {
    /// Sorted by `rel`, ignoring case.
    pub files: Vec<NewFile>,
    /// The walk stopped at a limit: there may be more new files.
    pub truncated: bool,
    /// Files left out because the user ignored them.
    pub ignored: usize,
    /// The ignored files the walk met (their keys); after a complete walk an
    /// entry of the ignore list that is not among them has no file any more.
    #[serde(skip)]
    pub(crate) seen_ignored: BTreeSet<String>,
    /// The walk read every folder and stopped at no limit – only then may
    /// the ignore list be cleaned up (`prune_ignored`).
    #[serde(skip)]
    pub(crate) complete: bool,
}

/// Where the walk stops ([`MAX_ENTRIES`], [`MAX_FILES`]; smaller in tests).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    /// Directory entries visited.
    pub entries: usize,
    /// New files collected.
    pub files: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            entries: MAX_ENTRIES,
            files: MAX_FILES,
        }
    }
}

/// A folder still to walk.
struct Dir {
    path: PathBuf,
    /// The project group folder, composed; empty for the working folder.
    project: String,
    /// The folders between the group folder and this one, `/`-separated.
    folder: String,
}

/// Lists the files below the working folder that belong to no item yet (see
/// the module documentation). Only reads the disk; nothing is uploaded.
#[tauri::command]
pub async fn new_files_scan(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<NewFilesScan, ApiError> {
    let root = workspace_root(&app, &state);
    tokio::task::spawn_blocking(move || scan_workspace(&root))
        .await
        .map_err(|e| ApiError::local(format!("The scan for new files stopped: {e}")))
}

/// *Ignore*: hides files (their `rel` from [`new_files_scan`]) from later
/// scans. Returns the number of ignored files afterwards.
#[tauri::command]
pub fn new_files_ignore(
    app: AppHandle,
    state: State<'_, AppState>,
    rels: Vec<String>,
) -> Result<usize, ApiError> {
    change_ignored(&workspace_root(&app, &state), &rels, true)
}

/// *Show ignored*: takes files out of the ignore list again. Returns the
/// number of ignored files afterwards.
#[tauri::command]
pub fn new_files_unignore(
    app: AppHandle,
    state: State<'_, AppState>,
    rels: Vec<String>,
) -> Result<usize, ApiError> {
    change_ignored(&workspace_root(&app, &state), &rels, false)
}

/// The ignore list: the paths in [`layout::key_of`] form (lower case,
/// composed), sorted.
#[tauri::command]
pub fn new_files_ignored(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, ApiError> {
    let root = workspace_root(&app, &state);
    Ok(load_ignored(&root).into_iter().collect())
}

/// [`scan_root`] of the working folder with its state, the files added with
/// *Add files…* and the ignore list; a missing folder has no new files.
/// After a complete walk the ignore list loses the entries of files that are
/// gone – deleted, moved, or part of an item meanwhile (`prune_ignored`).
pub(crate) fn scan_workspace(root: &Path) -> NewFilesScan {
    if !root.is_dir() {
        return NewFilesScan::default();
    }
    let state = layout::State::load(root);
    let pending: HashSet<String> = addfiles::load(root)
        .iter()
        .map(|p| layout::key_of(&p.rel))
        .collect();
    let ignored: HashSet<String> = load_ignored(root).into_iter().collect();
    let mut scan = scan_root(root, &state, &pending, &ignored, Limits::default());
    // supplier downloads next to the part saved from them (#564)
    companions::link(root, &state, &mut scan);
    if scan.complete {
        let gone: Vec<String> = ignored
            .iter()
            .filter(|k| !scan.seen_ignored.contains(*k))
            .cloned()
            .collect();
        // best effort: a list that cannot be written is cleaned up next time
        let _ = prune_ignored(root, &gone);
    }
    scan
}

/// Walks the group folders below `root` for files that belong to no item:
/// not tracked in `state`, not in `pending_rels` (files added with *Add
/// files…*) and no companion of a tracked file; files in `ignored` are only
/// counted. Both sets hold [`layout::key_of`] of the paths. Never fails:
/// what cannot be read is skipped.
pub(crate) fn scan_root(
    root: &Path,
    state: &layout::State,
    pending_rels: &HashSet<String>,
    ignored: &HashSet<String>,
    limits: Limits,
) -> NewFilesScan {
    let stems = tracked_stems(state);
    let mut result = NewFilesScan::default();
    let mut visited = 0usize;
    let start = Dir {
        path: root.to_path_buf(),
        project: String::new(),
        folder: String::new(),
    };
    let mut stack = vec![start];
    // every folder and entry was read (else the ignore list stays as it is)
    let mut complete = true;
    'walk: while let Some(dir) = stack.pop() {
        // an unreadable folder is skipped, never an error
        let Ok(read) = std::fs::read_dir(&dir.path) else {
            complete = false;
            continue;
        };
        let mut entries = Vec::new();
        for entry in read {
            if visited >= limits.entries {
                result.truncated = true;
                break;
            }
            visited += 1;
            match entry {
                Ok(entry) => entries.push(entry),
                Err(_) => complete = false,
            }
        }
        // the same order on every scan, also when it stops at a limit
        entries.sort_by_cached_key(|e| e.file_name());
        let rel_dir = join(&dir.project, &dir.folder);
        let dir_stems = stems.get(&layout::key_of(&rel_dir));
        let mut subdirs = Vec::new();
        for entry in entries {
            let Ok(kind) = entry.file_type() else {
                complete = false;
                continue;
            };
            // a name that is no valid Unicode cannot become a server name
            let Ok(disk) = entry.file_name().into_string() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if !skipped_dir(&disk) {
                    subdirs.push(child(&dir, entry.path(), &disk));
                }
                continue;
            }
            // files directly in the working folder belong to no group
            if !kind.is_file() || dir.project.is_empty() || layout::is_ignored(&disk) {
                continue;
            }
            let name = layout::nfc(&disk);
            let rel = join(&rel_dir, &name);
            let key = layout::key_of(&rel);
            // a companion of a tracked file goes into that item's check-in
            let companion = dir_stems.is_some_and(|s| s.contains(&layout::stem_of(&name)));
            if companion || state.files.contains_key(&key) || pending_rels.contains(&key) {
                continue;
            }
            if ignored.contains(&key) {
                result.ignored += 1;
                result.seen_ignored.insert(key);
                continue;
            }
            if result.files.len() >= limits.files {
                result.truncated = true;
                break 'walk;
            }
            let meta = entry.metadata().ok();
            result.files.push(NewFile {
                rel,
                path: entry.path(),
                project: dir.project.clone(),
                folder: dir.folder.clone(),
                name,
                size: meta.as_ref().map_or(0, |m| m.len()),
                modified_ms: meta.as_ref().map_or(0, modified_ms_of),
                companion_of: None,
                belongs_to: None,
                reason: None,
            });
        }
        if result.truncated {
            break;
        }
        // taken from the end: the folders are walked in name order
        stack.extend(subdirs.into_iter().rev());
    }
    result.complete = complete && !result.truncated;
    result.files.sort_by_key(|f| f.rel.to_lowercase());
    result
}

/// The name stems ([`layout::stem_of`]) of the tracked files per folder
/// ([`layout::key_of`] of the folder's relative path).
fn tracked_stems(state: &layout::State) -> HashMap<String, HashSet<String>> {
    let mut stems: HashMap<String, HashSet<String>> = HashMap::new();
    for entry in state.files.values() {
        let path = entry.path.replace('\\', "/");
        let (dir, name) = path.rsplit_once('/').unwrap_or(("", path.as_str()));
        stems
            .entry(layout::key_of(dir))
            .or_default()
            .insert(layout::stem_of(&layout::nfc(name)));
    }
    stems
}

/// Folders the scan does not enter: hidden ones (`.teamcraft`, `.git`) and
/// the backup and system folders of the folder import.
fn skipped_dir(name: &str) -> bool {
    name.starts_with('.') || SKIPPED_DIRS.contains(&name.to_lowercase().as_str())
}

/// The folder `disk` (its name on disk) inside `parent`, to walk.
fn child(parent: &Dir, path: PathBuf, disk: &str) -> Dir {
    let name = layout::nfc(disk);
    if parent.project.is_empty() {
        Dir {
            path,
            project: name,
            folder: String::new(),
        }
    } else {
        Dir {
            path,
            project: parent.project.clone(),
            folder: join(&parent.folder, &name),
        }
    }
}

/// `a/b`; either part may be empty.
fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a}/{b}"),
    }
}

/// The last change in milliseconds since 1970, 0 if unknown.
fn modified_ms_of(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

// ---------------------------------------------------------------------------
// Ignore list: <workspace>/.teamcraft/ignored.json
// ---------------------------------------------------------------------------

fn ignored_path(root: &Path) -> PathBuf {
    root.join(STATE_DIR).join(IGNORED_FILE)
}

/// The stored ignore list ([`layout::key_of`] of the paths); a missing or
/// damaged file is an empty list, invalid entries are left out.
fn load_ignored(root: &Path) -> BTreeSet<String> {
    let keys: Vec<String> = std::fs::read(ignored_path(root))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    keys.into_iter()
        .filter(|k| valid_rel(k))
        .map(|k| layout::key_of(&k))
        .collect()
}

/// Writes the ignore list atomically (temporary file, then rename), as a
/// sorted JSON array; an empty list removes the file.
fn save_ignored(root: &Path, keys: &BTreeSet<String>) -> std::io::Result<()> {
    let path = ignored_path(root);
    if keys.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(keys).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path)
}

/// A path as the scan reports it: relative, `/`-separated, without empty,
/// `.` or `..` parts and at most [`MAX_REL_CHARS`] characters long.
fn valid_rel(rel: &str) -> bool {
    // `C:/…` is a path on a drive, not one below the working folder
    let drive = rel.as_bytes().get(1) == Some(&b':');
    !drive
        && !rel.contains('\\')
        && rel.chars().count() <= MAX_REL_CHARS
        && !rel.split('/').any(|s| matches!(s, "" | "." | ".."))
}

/// Drops the keys `gone` from the ignore list: entries a complete scan found
/// no file for. Only these keys go, so an entry added while the scan ran
/// stays.
fn prune_ignored(root: &Path, gone: &[String]) -> std::io::Result<()> {
    if gone.is_empty() {
        return Ok(());
    }
    let _guard = IGNORED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut stored = load_ignored(root);
    let before = stored.len();
    for key in gone {
        stored.remove(key);
    }
    if stored.len() != before {
        save_ignored(root, &stored)?;
    }
    Ok(())
}

/// [`layout::key_of`] of each path of `rels`; the whole call is refused when
/// one of them is not a path the scan reports.
fn keys_of(rels: &[String]) -> Result<Vec<String>, ApiError> {
    if rels.len() > MAX_RELS {
        return Err(ApiError::local(format!(
            "At most {MAX_RELS} files can be ignored or shown again at once."
        )));
    }
    let mut keys = Vec::with_capacity(rels.len());
    for rel in rels {
        if rel.chars().count() > MAX_REL_CHARS {
            return Err(ApiError::local(format!(
                "A path is longer than {MAX_REL_CHARS} characters."
            )));
        }
        if !valid_rel(rel) {
            return Err(ApiError::local(format!(
                "'{rel}' is not the path of a file below the working folder."
            )));
        }
        keys.push(layout::key_of(rel));
    }
    Ok(keys)
}

/// Adds the paths `rels` to the ignore list (`ignore`) or takes them out
/// again; returns the number of ignored files afterwards.
fn change_ignored(root: &Path, rels: &[String], ignore: bool) -> Result<usize, ApiError> {
    let keys = keys_of(rels)?;
    if ignore && !root.is_dir() {
        return Err(ApiError::local("The working folder does not exist."));
    }
    let _guard = IGNORED_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut stored = load_ignored(root);
    for key in keys {
        if ignore {
            stored.insert(key);
        } else {
            stored.remove(&key);
        }
    }
    save_ignored(root, &stored)?;
    Ok(stored.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tracked file of the workspace state.
    fn entry(path: &str) -> layout::Entry {
        layout::Entry {
            path: path.into(),
            project: Some("GRAKSLER".into()),
            item: "PRT-000001".into(),
            revision: "A".into(),
            file_id: "f1".into(),
            filename: path.rsplit('/').next().unwrap_or(path).into(),
            version: 1,
            sha256: "00".into(),
            checked_out: true,
            size: 1,
            modified_ms: 0,
        }
    }

    /// Writes a file at `rel` below `root`, with its folders.
    fn write(root: &Path, rel: &str, content: &str) {
        let path = layout::join_rel(root, rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn owned(rels: &[&str]) -> Vec<String> {
        rels.iter().map(|r| r.to_string()).collect()
    }

    #[test]
    fn finds_the_files_that_belong_to_no_item() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for rel in [
            "GRAKSLER/Design/Struktur/Bracket.ipt",
            "GRAKSLER/Design/Struktur/Bracket.step",
            "GRAKSLER/Design/Struktur/Plate.ipt",
            "GRAKSLER/Design/Struktur/OldVersions/Plate.0001.ipt",
            "GRAKSLER/Design/Struktur/~$Plate.ipt",
            "GRAKSLER/Design/Struktur/Plate.ipt.teamcraft-download",
            "GRAKSLER/Design/Other/Bracket.step",
            "GRAKSLER/Design/Notes.pdf",
            "GRAKSLER/.git/config",
            "GRAKSLER/Frame.iam",
            "ROVER/Old.ipt",
            "README.txt",
        ] {
            write(root, rel, "x");
        }
        // tracked (spelt in another case), added with Add files…, ignored
        let tracked = entry("GRAKSLER/Design/Struktur/bracket.ipt");
        layout::State::store(root, [tracked]).unwrap();
        let added = addfiles::PlanItem {
            name: "Notes.pdf".into(),
            source: PathBuf::from("Notes.pdf"),
            folder: "Design".into(),
            rel: "GRAKSLER/Design/Notes.pdf".into(),
            new_version: false,
            exists_locally: true,
            in_place: true,
        };
        addfiles::record(root, "ASM-000004", "A", &[added]).unwrap();
        let old = owned(&["ROVER/Old.ipt"]);
        assert_eq!(change_ignored(root, &old, true).unwrap(), 1);

        let scan = scan_workspace(root);
        let rels: Vec<&str> = scan.files.iter().map(|f| f.rel.as_str()).collect();
        // Bracket.step in another folder is no companion of Bracket.ipt
        let expected = [
            "GRAKSLER/Design/Other/Bracket.step",
            "GRAKSLER/Design/Struktur/Plate.ipt",
            "GRAKSLER/Frame.iam",
        ];
        assert_eq!(rels, expected);
        assert_eq!(scan.ignored, 1, "ROVER/Old.ipt");
        assert!(!scan.truncated);
        let plate = &scan.files[1];
        let on_disk = layout::join_rel(root, expected[1]);
        assert_eq!(plate.path, on_disk);
        assert_eq!(plate.project, "GRAKSLER");
        assert_eq!(plate.folder, "Design/Struktur");
        assert_eq!(plate.name, "Plate.ipt");
        assert_eq!(plate.size, 1);
        assert!(plate.modified_ms > 0);
        let frame = &scan.files[2];
        assert_eq!(frame.name, "Frame.iam");
        assert!(frame.folder.is_empty(), "in the group folder");
        // without a working folder: nothing, and no error
        let missing = scan_workspace(&root.join("missing"));
        assert_eq!(missing, NewFilesScan::default());
    }

    #[test]
    fn decomposed_names_are_reported_composed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // as macOS hands them out: decomposed (NFD)
        let folder = "GRAKSLER/Za\u{308}hne";
        write(root, &format!("{folder}/Tra\u{308}ger.ipt"), "x");
        write(root, &format!("{folder}/Ritzel.ipt"), "x");
        // the state holds the composed path, as every path the app writes
        let tracked = entry("GRAKSLER/Z\u{e4}hne/Ritzel.ipt");
        layout::State::store(root, [tracked]).unwrap();
        let scan = scan_workspace(root);
        assert_eq!(scan.files.len(), 1, "Ritzel.ipt is tracked");
        let file = &scan.files[0];
        assert_eq!(file.rel, "GRAKSLER/Z\u{e4}hne/Tr\u{e4}ger.ipt");
        assert_eq!(file.folder, "Z\u{e4}hne");
        assert_eq!(file.name, "Tr\u{e4}ger.ipt");
        // the file is read under its name on disk
        assert!(file.path.ends_with("Tra\u{308}ger.ipt"));
        assert!(file.path.is_file());
    }

    #[test]
    fn the_walk_stops_at_its_limits() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for name in ["A.ipt", "B.ipt", "C.ipt"] {
            write(root, &format!("GRAKSLER/{name}"), "x");
        }
        let state = layout::State::default();
        let none: HashSet<String> = HashSet::new();
        let scan = |entries: usize, files: usize| {
            let limits = Limits { entries, files };
            scan_root(root, &state, &none, &none, limits)
        };
        // the group folder and its three files: four entries
        let all = scan(4, 3);
        assert_eq!(all.files.len(), 3);
        assert!(!all.truncated, "exactly at the limits");
        let few_files = scan(100, 2);
        assert_eq!(few_files.files.len(), 2);
        assert!(few_files.truncated);
        let few_entries = scan(3, 100);
        assert_eq!(few_entries.files.len(), 2);
        assert!(few_entries.truncated);
    }

    #[test]
    fn ignore_and_show_again() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let plate = owned(&["GRAKSLER/Design/Struktur/Plate.ipt"]);
        assert_eq!(change_ignored(root, &plate, true).unwrap(), 1);
        // the same file spelt in another case: still one entry
        let again = owned(&["graksler/design/struktur/PLATE.ipt"]);
        assert_eq!(change_ignored(root, &again, true).unwrap(), 1);
        let more = owned(&["ROVER/A.ipt", "GRAKSLER/B.ipt"]);
        assert_eq!(change_ignored(root, &more, true).unwrap(), 3);
        // stored as keys, sorted
        let bytes = std::fs::read(ignored_path(root)).unwrap();
        let stored: Vec<String> = serde_json::from_slice(&bytes).unwrap();
        let expected = [
            "graksler/b.ipt",
            "graksler/design/struktur/plate.ipt",
            "rover/a.ipt",
        ];
        assert_eq!(stored, expected);
        assert_eq!(load_ignored(root).len(), 3);
        // shown again
        assert_eq!(change_ignored(root, &more, false).unwrap(), 1);
        assert_eq!(change_ignored(root, &plate, false).unwrap(), 0);
        assert!(!ignored_path(root).exists(), "no empty file");
        assert_eq!(change_ignored(root, &plate, false).unwrap(), 0);
    }

    #[test]
    fn only_paths_below_the_working_folder_can_be_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let long = format!("GRAKSLER/{}.ipt", "x".repeat(MAX_REL_CHARS));
        for bad in [
            "",
            "/GRAKSLER/A.ipt",
            "GRAKSLER//A.ipt",
            "GRAKSLER/./A.ipt",
            "GRAKSLER/../A.ipt",
            "GRAKSLER/A.ipt/",
            "GRAKSLER\\A.ipt",
            "C:/GRAKSLER/A.ipt",
            long.as_str(),
        ] {
            let rels = owned(&["GRAKSLER/Good.ipt", bad]);
            assert!(change_ignored(root, &rels, true).is_err(), "{bad}");
        }
        assert!(load_ignored(root).is_empty(), "nothing stored");
        let many: Vec<String> = (0..=MAX_RELS).map(|i| format!("G/{i}")).collect();
        assert!(change_ignored(root, &many, true).is_err(), "too many");
        let stored = change_ignored(root, &many[1..], true).unwrap();
        assert_eq!(stored, MAX_RELS);
        // no working folder: refused, nothing is created
        let missing = root.join("missing");
        assert!(change_ignored(&missing, &many[..1], true).is_err());
        assert!(!missing.exists());
    }

    #[test]
    fn entries_of_files_that_are_gone_leave_the_ignore_list() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "GRAKSLER/Keep.ipt", "x");
        write(root, "GRAKSLER/Gone.ipt", "x");
        let both = owned(&["GRAKSLER/Keep.ipt", "GRAKSLER/Gone.ipt"]);
        assert_eq!(change_ignored(root, &both, true).unwrap(), 2);
        std::fs::remove_file(layout::join_rel(root, "GRAKSLER/Gone.ipt")).unwrap();
        // a walk that stopped at a limit proves nothing: nothing is dropped
        let state = layout::State::default();
        let none: HashSet<String> = HashSet::new();
        let ignored: HashSet<String> = load_ignored(root).into_iter().collect();
        let limits = Limits {
            entries: 1,
            files: 100,
        };
        let cut = scan_root(root, &state, &none, &ignored, limits);
        assert!(cut.truncated && !cut.complete);
        assert_eq!(load_ignored(root).len(), 2);
        // a complete scan drops the entry of the deleted file only
        let scan = scan_workspace(root);
        assert!(scan.complete);
        assert_eq!(scan.ignored, 1);
        let left: Vec<String> = load_ignored(root).into_iter().collect();
        assert_eq!(left, ["graksler/keep.ipt"]);
        // only the given keys go: an entry added meanwhile stays
        let added = owned(&["GRAKSLER/New.ipt"]);
        assert_eq!(change_ignored(root, &added, true).unwrap(), 2);
        prune_ignored(root, &owned(&["graksler/keep.ipt"])).unwrap();
        let left: Vec<String> = load_ignored(root).into_iter().collect();
        assert_eq!(left, ["graksler/new.ipt"]);
    }

    #[test]
    fn a_damaged_ignore_list_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let file = ".teamcraft/ignored.json";
        write(root, "GRAKSLER/Plate.ipt", "x");
        for damaged in ["{ not json", "{\"a\": 1}", "[1, 2]"] {
            write(root, file, damaged);
            assert!(load_ignored(root).is_empty(), "{damaged}");
            assert_eq!(scan_workspace(root).files.len(), 1);
        }
        // invalid entries of a list are left out
        let list = r#"["../x", "GRAKSLER/Plate.ipt"]"#;
        write(root, file, list);
        assert_eq!(load_ignored(root).len(), 1);
        assert_eq!(scan_workspace(root).ignored, 1);
        // the next change writes a valid list again
        write(root, file, "garbage");
        let rels = owned(&["GRAKSLER/Plate.ipt"]);
        assert_eq!(change_ignored(root, &rels, true).unwrap(), 1);
        let scan = scan_workspace(root);
        assert!(scan.files.is_empty());
        assert_eq!(scan.ignored, 1);
    }
}
