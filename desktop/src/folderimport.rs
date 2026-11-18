//! Scan of a CAD folder for the import wizard (see
//! `docs/src/research/cad-folder-import.md`): every file below the folder,
//! without lock, backup and temporary files and without the backup folders
//! of the CAD programs (`OldVersions` of Inventor, …). The folder itself is
//! only read – the import uploads copies.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{layout, workspace};

/// More files than this are refused: import such trees in parts.
pub const MAX_FILES: usize = 20_000;

/// Folders that never hold design data.
const SKIPPED_DIRS: [&str; 7] = [
    "oldversions",
    ".git",
    ".svn",
    "__macosx",
    "$recycle.bin",
    "system volume information",
    "_backup",
];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScannedFile {
    /// Absolute path, for the upload.
    pub path: PathBuf,
    /// Path below the scanned folder with `/` separators, for the report and
    /// the folders of the upload (names composed, see `layout::nfc`).
    pub rel: String,
    /// File name, composed; `path` keeps the name on disk.
    pub name: String,
    pub size: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Scan {
    pub files: Vec<ScannedFile>,
    /// Lock, backup and temporary files that were left out.
    pub ignored: usize,
    /// Name of the scanned folder (for the project-files item).
    pub folder_name: String,
}

/// Walks `root` (symbolic links are not followed).
pub fn scan(root: &Path) -> Result<Scan, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a folder.", root.display()));
    }
    let mut result = Scan {
        folder_name: root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("CAD data")
            .to_string(),
        ..Default::default()
    };
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("{} cannot be read: {e}", dir.display()))?;
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if !SKIPPED_DIRS.contains(&name.to_lowercase().as_str()) && !name.starts_with('.') {
                    stack.push(entry.path());
                }
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            if workspace::is_ignored(&name) {
                result.ignored += 1;
                continue;
            }
            if result.files.len() >= MAX_FILES {
                return Err(format!(
                    "The folder holds more than {MAX_FILES} files – import its subfolders one by one."
                ));
            }
            let path = entry.path();
            // name and folder as the server stores them: composed (NFC), also
            // for a folder on a Mac, which hands out decomposed names
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .components()
                .map(|c| layout::nfc(&c.as_os_str().to_string_lossy()))
                .collect::<Vec<_>>()
                .join("/");
            let name = layout::nfc(&name);
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            result.files.push(ScannedFile {
                path,
                rel,
                name,
                size,
            });
        }
    }
    result.files.sort_by_key(|f| f.rel.to_lowercase());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_cad_folders() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Rover");
        for (rel, content) in [
            ("Rover.iam", "asm"),
            ("Rover.ipj", "project"),
            ("Parts/Wheel.ipt", "part"),
            ("Parts/Wheel.idw", "drawing"),
            ("Parts/OldVersions/Wheel.0001.ipt", "backup"),
            ("Parts/~$Wheel.ipt", "lock"),
            (".git/config", "vcs"),
        ] {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        let scan = scan(&root).unwrap();
        let rels: Vec<&str> = scan.files.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(
            rels,
            vec![
                "Parts/Wheel.idw",
                "Parts/Wheel.ipt",
                "Rover.iam",
                "Rover.ipj"
            ]
        );
        assert_eq!(scan.ignored, 1, "the lock file");
        assert_eq!(scan.folder_name, "Rover");
        assert_eq!(scan.files[1].size, 4);
        assert!(scan.files[1].path.is_absolute() || scan.files[1].path.starts_with(&root));
        assert!(super::scan(&root.join("missing")).is_err());
    }

    #[test]
    fn names_from_a_mac_are_composed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("GRAKSLER");
        // as macOS hands them out: decomposed (NFD)
        let disk = root.join("Rad Mitte").join("Planetentra\u{308}ger2.ipt");
        std::fs::create_dir_all(disk.parent().unwrap()).unwrap();
        std::fs::write(&disk, "part").unwrap();
        let sub = root.join("Za\u{308}hne");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("Ritzel.ipt"), "part").unwrap();
        let scan = scan(&root).unwrap();
        let files: Vec<(&str, &str)> = scan
            .files
            .iter()
            .map(|f| (f.rel.as_str(), f.name.as_str()))
            .collect();
        assert_eq!(
            files,
            vec![
                (
                    "Rad Mitte/Planetentr\u{e4}ger2.ipt",
                    "Planetentr\u{e4}ger2.ipt"
                ),
                ("Z\u{e4}hne/Ritzel.ipt", "Ritzel.ipt")
            ]
        );
        // the upload reads the file under its name on disk
        assert!(scan.files[0].path.is_file());
    }
}
