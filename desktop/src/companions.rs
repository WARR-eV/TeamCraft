//! Supplier downloads and the native part saved from them (#564).
//!
//! Purchased parts usually start as a STEP download (`ISWB25-64.step`); the
//! designer opens it in the CAD program and saves a native part from it,
//! often one folder down (`ISWB25-64/ISWB25-64.ipt`). Both belong to one
//! item. *New on this computer* groups files by name stem within a folder
//! only, so such a STEP ended up among the "other new files".
//!
//! [`link`] runs after the scan and gives a new exchange file (STEP, IGES,
//! Parasolid, ACIS, JT) one of two hints:
//!
//! * `companion_of` – the new native part it belongs to: both become one
//!   item in *Create items…*;
//! * `belongs_to` – the tracked native part (an item already) it belongs to:
//!   the worklist offers *Add to item*.
//!
//! Two signals, both local and exact (no geometry, no content guessing):
//!
//! 1. **The native part names the file** – a CAD program that imports a STEP
//!    keeps its source path in the part (Inventor stores the full path); the
//!    file name is searched in the part's bytes as ASCII and UTF-16LE,
//!    ignoring ASCII case, at most [`MAX_SCAN_BYTES`] per part.
//! 2. **The same name stem** after removing download decorations
//!    ([`download_stem`]: timestamps, ` (1)` counters, `_copy`).
//!
//! Either way the two files must be in the same folder or one folder apart
//! (the part directly above or below the download). A file with more than
//! one candidate gets no hint – the user decides.

use std::{io::Read, path::Path};

use serde::Serialize;

use crate::{
    layout::{self, State},
    newfiles::NewFilesScan,
};

/// Exchange formats a supplier download comes in.
pub const EXCHANGE_EXTENSIONS: [&str; 9] = [
    "step", "stp", "stpz", "igs", "iges", "x_t", "x_b", "sat", "jt",
];
/// Native part and assembly formats a download is saved as.
pub const MODEL_EXTENSIONS: [&str; 15] = [
    "ipt",
    "iam",
    "sldprt",
    "sldasm",
    "prt",
    "asm",
    "catpart",
    "catproduct",
    "par",
    "psm",
    "f3d",
    "fcstd",
    "3dm",
    "sc",
    "solidcraft",
];
/// Bytes of one native part searched for the download's name.
pub const MAX_SCAN_BYTES: u64 = 256 * 1024 * 1024;
/// Native parts searched per download at most (the nearest folders first).
const MAX_CANDIDATES: usize = 24;

/// A tracked file (an item already) a new file belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Owner {
    pub project: Option<String>,
    pub item: String,
    pub revision: String,
    /// The tracked native file (`rel` below the working folder).
    pub file: String,
}

/// Why a new file was matched – shown to the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The native part names the file (it was saved from it).
    SavedFrom,
    /// The same name (download decorations removed), same folder.
    SameName,
    /// The same name, the part in the folder above.
    SameNameAbove,
    /// The same name, the part in a folder below.
    SameNameBelow,
}

/// Extension of a file name, lower case, Creo iterations removed.
fn ext_of(name: &str) -> String {
    let (logical, _) = layout::logical_name(name);
    logical
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

pub fn is_exchange(name: &str) -> bool {
    EXCHANGE_EXTENSIONS.contains(&ext_of(name).as_str())
}

pub fn is_model(name: &str) -> bool {
    MODEL_EXTENSIONS.contains(&ext_of(name).as_str())
}

/// The name stem without what a download or a copy adds, lower case:
/// `ISWB25-64_20261128191413491` → `iswb25-64`, `Bolt (2)` → `bolt`,
/// `Bracket - Copy` → `bracket`. Part numbers with short digit groups
/// (`ISWB25-64`, `6202ZZ`) are kept.
pub fn download_stem(stem: &str) -> String {
    let mut s = stem.trim().to_lowercase();
    loop {
        let before = s.clone();
        for suffix in [" - copy", "_copy", " copy", "-copy"] {
            if let Some(rest) = s.strip_suffix(suffix) {
                s = rest.trim_end().to_string();
            }
        }
        // ` (2)`, `(2)`
        if s.ends_with(')') {
            if let Some(open) = s.rfind('(') {
                let inner = &s[open + 1..s.len() - 1];
                if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
                    s = s[..open].trim_end().to_string();
                }
            }
        }
        // `_20261128191413491`, `-1732791234`: a long digit run (a date, a
        // time stamp, an id) after `_` or `-`
        if let Some(pos) = s.rfind(['_', '-']) {
            let tail = &s[pos + 1..];
            if tail.len() >= 8 && tail.chars().all(|c| c.is_ascii_digit()) && pos > 0 {
                s.truncate(pos);
            }
        }
        if s == before {
            return s;
        }
    }
}

/// How two folders (relative, `/`-separated) are related: `Some(0)` the same,
/// `Some(1)` `part_dir` is the folder above `file_dir`, `Some(-1)` a folder
/// directly below; `None` otherwise.
fn relation(file_dir: &str, part_dir: &str) -> Option<i8> {
    let (a, b) = (layout::key_of(file_dir), layout::key_of(part_dir));
    let parent = |p: &str| {
        p.rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_default()
    };
    if a == b {
        Some(0)
    } else if parent(&a) == b {
        Some(1)
    } else if parent(&b) == a {
        Some(-1)
    } else {
        None
    }
}

fn name_reason(rel: i8) -> Reason {
    match rel {
        0 => Reason::SameName,
        1 => Reason::SameNameAbove,
        _ => Reason::SameNameBelow,
    }
}

/// Does the file at `path` contain `name` (ASCII or UTF-16LE, ASCII case
/// ignored)? Reads at most [`MAX_SCAN_BYTES`]; an unreadable file does not.
pub fn mentions(path: &Path, name: &str) -> bool {
    let needle: Vec<u8> = name.bytes().map(|b| b.to_ascii_lowercase()).collect();
    if needle.is_empty() || !name.is_ascii() {
        return false;
    }
    let wide: Vec<u8> = needle.iter().flat_map(|&b| [b, 0]).collect();
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut reader = file.take(MAX_SCAN_BYTES);
    let keep = wide.len();
    let mut window: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let Ok(n) = reader.read(&mut buf) else {
            return false;
        };
        if n == 0 {
            return false;
        }
        window.extend(buf[..n].iter().map(|b| b.to_ascii_lowercase()));
        if find(&window, &needle) || find(&window, &wide) {
            return true;
        }
        let cut = window.len().saturating_sub(keep);
        window.drain(..cut);
    }
}

fn find(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

fn dir_of(rel: &str) -> &str {
    rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

/// The best match among `candidates` (`(dir relation, stem, path, name)`):
/// a part that names the file wins over a name match; more than one part of
/// the best kind → no match. Returns the index and the reason.
fn best(
    file_name: &str,
    file_stem: &str,
    candidates: &[(i8, String, std::path::PathBuf)],
) -> Option<(usize, Reason)> {
    let mut named = Vec::new();
    let mut same = Vec::new();
    for (i, (rel, stem, path)) in candidates.iter().enumerate().take(MAX_CANDIDATES) {
        if mentions(path, file_name) {
            named.push(i);
        } else if download_stem(stem) == file_stem {
            same.push((i, *rel));
        }
    }
    match (named.as_slice(), same.as_slice()) {
        ([i], _) => Some((*i, Reason::SavedFrom)),
        ([], [(i, rel)]) => Some((*i, name_reason(*rel))),
        _ => None,
    }
}

/// Gives the new exchange files of `scan` their `companion_of` / `belongs_to`
/// hint (see the module documentation). Only reads files.
pub fn link(root: &Path, state: &State, scan: &mut NewFilesScan) {
    let models: Vec<(String, String, String, std::path::PathBuf)> = scan
        .files
        .iter()
        .filter(|f| is_model(&f.name))
        .map(|f| {
            (
                f.project.clone(),
                dir_of(&f.rel).to_string(),
                f.rel.clone(),
                f.path.clone(),
            )
        })
        .collect();
    for i in 0..scan.files.len() {
        let f = &scan.files[i];
        if !is_exchange(&f.name) {
            continue;
        }
        let file_dir = dir_of(&f.rel).to_string();
        let file_stem = download_stem(&layout::stem_of(&f.name));
        // a part of the same stem in the same folder: grouped by name already
        if models.iter().any(|(p, d, rel, _)| {
            p == &f.project
                && layout::key_of(d) == layout::key_of(&file_dir)
                && layout::stem_of(rel.rsplit('/').next().unwrap_or("")) == layout::stem_of(&f.name)
        }) {
            continue;
        }
        // 1. a new native part nearby
        let mut near: Vec<(i8, String, std::path::PathBuf, String)> = models
            .iter()
            .filter(|(p, _, _, _)| p == &f.project)
            .filter_map(|(_, d, rel, path)| {
                relation(&file_dir, d).map(|r| {
                    (
                        r,
                        layout::stem_of(rel.rsplit('/').next().unwrap_or("")),
                        path.clone(),
                        rel.clone(),
                    )
                })
            })
            .collect();
        near.sort_by_key(|(r, _, _, rel)| (r.abs(), rel.to_lowercase()));
        let cands: Vec<(i8, String, std::path::PathBuf)> = near
            .iter()
            .map(|(r, s, p, _)| (*r, s.clone(), p.clone()))
            .collect();
        let name = f.name.clone();
        if let Some((k, reason)) = best(&name, &file_stem, &cands) {
            let rel = near[k].3.clone();
            let f = &mut scan.files[i];
            f.companion_of = Some(rel);
            f.reason = Some(reason);
            continue;
        }
        // 2. a tracked native part nearby (an item already)
        let mut tracked: Vec<(i8, String, std::path::PathBuf, &layout::Entry)> = state
            .files
            .values()
            .filter(|e| is_model(&e.filename))
            .filter_map(|e| {
                let path = e.path.replace('\\', "/");
                relation(&file_dir, dir_of(&path)).map(|r| {
                    (
                        r,
                        layout::stem_of(&e.filename),
                        layout::join_rel(root, &path),
                        e,
                    )
                })
            })
            .collect();
        tracked.sort_by_key(|(r, _, _, e)| (r.abs(), e.path.to_lowercase()));
        let cands: Vec<(i8, String, std::path::PathBuf)> = tracked
            .iter()
            .map(|(r, s, p, _)| (*r, s.clone(), p.clone()))
            .collect();
        if let Some((k, reason)) = best(&name, &file_stem, &cands) {
            let e = tracked[k].3;
            let owner = Owner {
                project: e.project.clone(),
                item: e.item.clone(),
                revision: e.revision.clone(),
                file: e.path.replace('\\', "/"),
            };
            let f = &mut scan.files[i];
            f.belongs_to = Some(owner);
            f.reason = Some(reason);
        }
    }
    link_saved_from_tracked(state, scan);
}

/// The reverse case: a new native part saved from a download that is an
/// item already (only the strong signal – the part names the tracked file,
/// one folder apart at most; exactly one such file).
fn link_saved_from_tracked(state: &State, scan: &mut NewFilesScan) {
    let downloads: Vec<&layout::Entry> = state
        .files
        .values()
        .filter(|e| is_exchange(&e.filename))
        .collect();
    if downloads.is_empty() {
        return;
    }
    for f in scan.files.iter_mut() {
        if !is_model(&f.name) || f.companion_of.is_some() || f.belongs_to.is_some() {
            continue;
        }
        let file_dir = dir_of(&f.rel).to_string();
        let mut near: Vec<&&layout::Entry> = downloads
            .iter()
            .filter(|e| relation(&file_dir, dir_of(&e.path.replace('\\', "/"))).is_some())
            .collect();
        near.sort_by_key(|e| e.path.to_lowercase());
        let named: Vec<&&&layout::Entry> = near
            .iter()
            .take(MAX_CANDIDATES)
            .filter(|e| mentions(&f.path, &e.filename))
            .collect();
        if let [e] = named.as_slice() {
            f.belongs_to = Some(Owner {
                project: e.project.clone(),
                item: e.item.clone(),
                revision: e.revision.clone(),
                file: e.path.replace('\\', "/"),
            });
            f.reason = Some(Reason::SavedFrom);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::newfiles::NewFile;

    #[test]
    fn download_decorations_are_removed_part_numbers_kept() {
        assert_eq!(download_stem("ISWB25-64_20261128191413491"), "iswb25-64");
        assert_eq!(download_stem("ISWB25-64"), "iswb25-64");
        assert_eq!(download_stem("6202ZZ"), "6202zz");
        assert_eq!(download_stem("Bolt (2)"), "bolt");
        assert_eq!(download_stem("Bolt(12)"), "bolt");
        assert_eq!(download_stem("Bracket - Copy"), "bracket");
        assert_eq!(download_stem("DIN912-M6x20-1732791234"), "din912-m6x20");
        assert_eq!(
            download_stem("M6x1234567"),
            "m6x1234567",
            "no separator: kept"
        );
        assert_eq!(
            download_stem("123456789"),
            "123456789",
            "a stem that is only digits is kept"
        );
    }

    #[test]
    fn kinds_by_extension() {
        assert!(is_exchange("a.STEP") && is_exchange("a.stp") && is_exchange("a.x_t"));
        assert!(is_model("a.ipt") && is_model("b.prt.7") && is_model("c.SLDPRT"));
        assert!(!is_model("a.step") && !is_exchange("a.ipt") && !is_exchange("a.pdf"));
    }

    #[test]
    fn folders_one_apart_only() {
        assert_eq!(relation("G/a/b", "G/a/b"), Some(0));
        assert_eq!(relation("G/a/b", "G/a"), Some(1));
        assert_eq!(relation("G/a", "G/a/b"), Some(-1));
        assert_eq!(relation("G/a", "G/a/b/c"), None);
        assert_eq!(relation("G/a/x", "G/a/y"), None);
    }

    #[test]
    fn names_are_found_as_ascii_and_utf16() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("part.ipt");
        let mut bytes = vec![0u8; 3_000_000];
        let wide: Vec<u8> = r"C:\W\Kaufteile\ISWB25-64.step"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        // across the 1 MiB read boundary
        bytes.splice(
            (1 << 20) - 20..(1 << 20) - 20 + wide.len(),
            wide.iter().copied(),
        );
        std::fs::write(&p, &bytes).unwrap();
        assert!(mentions(&p, "iswb25-64.STEP"));
        assert!(!mentions(&p, "other.step"));
        std::fs::write(&p, b"xx source=Bolt.stp yy").unwrap();
        assert!(mentions(&p, "bolt.stp"));
    }

    fn new_file(root: &Path, rel: &str, content: &[u8]) -> NewFile {
        let path = layout::join_rel(root, rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        let (dir, name) = rel.rsplit_once('/').unwrap();
        let (project, folder) = dir.split_once('/').unwrap_or((dir, ""));
        NewFile {
            rel: rel.to_string(),
            path,
            project: project.to_string(),
            folder: folder.to_string(),
            name: name.to_string(),
            size: content.len() as u64,
            modified_ms: 0,
            companion_of: None,
            belongs_to: None,
            reason: None,
        }
    }

    #[test]
    fn a_download_joins_the_new_part_saved_from_it() {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path();
        let mut scan = NewFilesScan {
            files: vec![
                new_file(root, "G/Buy/ISWB25-64.step", b"ISO-10303-21;"),
                // the part one folder down names its source
                new_file(
                    root,
                    "G/Buy/ISWB25-64/ISWB25-64.ipt",
                    b"..G:\\Buy\\ISWB25-64.step..",
                ),
                // a download with a time stamp next to a part of the plain name
                new_file(root, "G/Buy/DIN912_20261128101010.stp", b"ISO"),
                new_file(root, "G/Buy/DIN912.ipt", b"no source"),
                new_file(root, "G/Other/lonely.step", b"ISO"),
            ],
            ..NewFilesScan::default()
        };
        link(root, &State::default(), &mut scan);
        let by = |rel: &str| scan.files.iter().find(|f| f.rel == rel).unwrap().clone();
        let a = by("G/Buy/ISWB25-64.step");
        assert_eq!(
            a.companion_of.as_deref(),
            Some("G/Buy/ISWB25-64/ISWB25-64.ipt")
        );
        assert_eq!(a.reason, Some(Reason::SavedFrom));
        let b = by("G/Buy/DIN912_20261128101010.stp");
        assert_eq!(b.companion_of.as_deref(), Some("G/Buy/DIN912.ipt"));
        assert_eq!(b.reason, Some(Reason::SameName));
        assert_eq!(by("G/Other/lonely.step").companion_of, None);
        assert_eq!(
            by("G/Buy/DIN912.ipt").companion_of,
            None,
            "models get no hint"
        );
    }

    #[test]
    fn a_download_next_to_a_tracked_part_belongs_to_its_item() {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path();
        let part = layout::join_rel(root, "G/Buy/ISWB25-64/ISWB25-64.ipt");
        std::fs::create_dir_all(part.parent().unwrap()).unwrap();
        std::fs::write(&part, b"saved from ISWB25-64.step").unwrap();
        let mut state = State::default();
        state.insert(layout::Entry {
            path: "G/Buy/ISWB25-64/ISWB25-64.ipt".into(),
            project: Some("G".into()),
            item: "PRT-387".into(),
            revision: "A".into(),
            file_id: "f".into(),
            filename: "ISWB25-64.ipt".into(),
            version: 1,
            sha256: "x".into(),
            checked_out: false,
            size: 0,
            modified_ms: 0,
        });
        let mut scan = NewFilesScan {
            files: vec![new_file(root, "G/Buy/ISWB25-64.step", b"ISO")],
            ..NewFilesScan::default()
        };
        link(root, &state, &mut scan);
        let f = &scan.files[0];
        assert_eq!(f.companion_of, None);
        let owner = f.belongs_to.as_ref().unwrap();
        assert_eq!(
            (owner.item.as_str(), owner.revision.as_str()),
            ("PRT-387", "A")
        );
        assert_eq!(owner.file, "G/Buy/ISWB25-64/ISWB25-64.ipt");
        assert_eq!(f.reason, Some(Reason::SavedFrom));
    }

    #[test]
    fn two_equally_good_parts_give_no_hint() {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path();
        let mut scan = NewFilesScan {
            files: vec![
                new_file(root, "G/a/Bolt.step", b"ISO"),
                new_file(root, "G/a/x/Bolt.ipt", b"-"),
                new_file(root, "G/Bolt.ipt", b"-"),
            ],
            ..NewFilesScan::default()
        };
        link(root, &State::default(), &mut scan);
        assert_eq!(scan.files[0].companion_of, None);
        assert_eq!(scan.files[0].reason, None);
    }

    #[test]
    fn a_new_part_saved_from_a_tracked_download_belongs_to_its_item() {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path();
        let mut state = State::default();
        state.insert(layout::Entry {
            path: "G/Buy/Guide.step".into(),
            project: Some("G".into()),
            item: "PRT-9".into(),
            revision: "B".into(),
            file_id: "f".into(),
            filename: "Guide.step".into(),
            version: 1,
            sha256: "x".into(),
            checked_out: false,
            size: 0,
            modified_ms: 0,
        });
        let mut scan = NewFilesScan {
            files: vec![
                new_file(
                    root,
                    "G/Buy/Guide/Guide.ipt",
                    b"import source G:\\Buy\\Guide.step",
                ),
                // a part of the same name that does not name the download: no hint
                new_file(root, "G/Buy/Guide2.ipt", b"-"),
            ],
            ..NewFilesScan::default()
        };
        link(root, &state, &mut scan);
        let owner = scan.files[0].belongs_to.as_ref().unwrap();
        assert_eq!(
            (owner.item.as_str(), owner.revision.as_str()),
            ("PRT-9", "B")
        );
        assert_eq!(scan.files[0].reason, Some(Reason::SavedFrom));
        assert_eq!(scan.files[1].belongs_to, None);
    }
}
