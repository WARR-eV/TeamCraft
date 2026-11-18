//! Scan of a CAD folder for the import wizard (see
//! `docs/src/research/cad-folder-import.md`): every file below the folder,
//! without lock, backup and temporary files and without the backup folders
//! of the CAD programs (`OldVersions` of Inventor, …). The folder itself is
//! only read – the import uploads copies.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::workspace;

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
    /// Path below the scanned folder with `/` separators, for the report.
    pub rel: String,
    pub name: String,
    pub size: u64,
}

/// Inventor files read for their version (assemblies first).
pub const VERSION_SAMPLE: usize = 40;
/// Bytes read from the start of each sampled file.
pub const VERSION_WINDOW: u64 = 4 << 20;

/// A CAD program version found in the files of the folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DetectedVersion {
    /// Format family (`inventor`).
    pub family: String,
    /// `2021`
    pub version: String,
    /// Sampled files last saved with it.
    pub files: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Scan {
    pub files: Vec<ScannedFile>,
    /// Versions of the CAD programs that saved the files, newest first
    /// (Inventor: the release year in the file properties, see
    /// [`inventor_year`]).
    pub versions: Vec<DetectedVersion>,
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
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/");
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
    result.versions = inventor_versions(&result.files);
    Ok(result)
}

/// The newest Inventor release year in the start of a file: Inventor writes
/// the release that saved a file into its document properties as text such
/// as `2021 (Build 250183000, 183)` (UTF-16, an OLE property set; seen in
/// the NIST samples of `tests/fixtures/cad`, see
/// `docs/src/research/cad-folder-import.md`). A file holds more than one
/// such text when it was created with an older release – the newest is the
/// release that saved it last. Years outside 2000–2099 are ignored.
pub fn inventor_year(bytes: &[u8]) -> Option<u16> {
    const BUILD: &str = " (Build ";
    let wide: Vec<u8> = BUILD.bytes().flat_map(|b| [b, 0]).collect();
    let mut best: Option<u16> = None;
    // (needle, bytes per character)
    for (needle, step) in [(wide.as_slice(), 2usize), (BUILD.as_bytes(), 1)] {
        for (at, window) in bytes.windows(needle.len()).enumerate() {
            if window != needle || at < 4 * step {
                continue;
            }
            let digit = |i: usize| -> Option<u16> {
                let pos = at - (4 - i) * step;
                let b = *bytes.get(pos)?;
                if step == 2 && bytes.get(pos + 1) != Some(&0) {
                    return None;
                }
                b.is_ascii_digit().then(|| u16::from(b - b'0'))
            };
            let year = (0..4).try_fold(0u16, |y, i| Some(y * 10 + digit(i)?));
            // a fifth digit in front: not a year
            let longer = at >= 5 * step && bytes[at - 5 * step].is_ascii_digit();
            if let Some(y) = year.filter(|y| (2000..=2099).contains(y) && !longer) {
                best = best.max(Some(y));
            }
        }
    }
    best
}

/// The Inventor versions of a sample of the scanned files (assemblies,
/// then parts, drawings and presentations, at most [`VERSION_SAMPLE`]; the
/// first [`VERSION_WINDOW`] bytes of each). Unreadable files are skipped.
fn inventor_versions(files: &[ScannedFile]) -> Vec<DetectedVersion> {
    use std::io::Read;
    let rank = |f: &&ScannedFile| {
        let ext = Path::new(&f.name)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        match ext.as_deref() {
            Some("iam") => Some(0),
            Some("ipt") => Some(1),
            Some("idw") => Some(2),
            Some("ipn") => Some(3),
            _ => None,
        }
    };
    let mut sample: Vec<&ScannedFile> = files.iter().filter(|f| rank(f).is_some()).collect();
    sample.sort_by_key(|f| rank(f));
    let mut years: Vec<(u16, usize)> = Vec::new();
    for f in sample.into_iter().take(VERSION_SAMPLE) {
        let mut head = Vec::new();
        let read = std::fs::File::open(&f.path)
            .and_then(|file| file.take(VERSION_WINDOW).read_to_end(&mut head));
        let Some(year) = read.ok().and_then(|_| inventor_year(&head)) else {
            continue;
        };
        match years.iter_mut().find(|(y, _)| *y == year) {
            Some((_, n)) => *n += 1,
            None => years.push((year, 1)),
        }
    }
    years.sort_by_key(|y| std::cmp::Reverse(y.0));
    years
        .into_iter()
        .map(|(year, files)| DetectedVersion {
            family: "inventor".into(),
            version: year.to_string(),
            files,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn inventor_years_come_from_the_build_text() {
        let mut bytes = vec![0xD0, 0xCF, 0x11, 0xE0];
        bytes.extend(utf16("2019 (Build 230000000, 100)"));
        bytes.extend([0u8; 7]);
        bytes.extend(utf16("2026 (Build 300000000, 200)"));
        assert_eq!(inventor_year(&bytes), Some(2026), "the newest release");
        assert_eq!(inventor_year(b"..2024 (Build 280000000, 1).."), Some(2024));
        for none in [
            utf16("Build 2026"),
            utf16("1999 (Build 1)"),
            utf16("12026 (Build 1)"),
            utf16("20x6 (Build 1)"),
            b"(Build ".to_vec(),
            Vec::new(),
        ] {
            assert_eq!(inventor_year(&none), None);
        }
    }

    /// The NIST parts in `tests/fixtures/cad` were saved with Inventor 2021.
    #[test]
    fn the_nist_inventor_parts_were_saved_with_2021() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/cad/nist");
        for rel in [
            "ctc-01/inventor-2021/INV_nist_ctc_01_asme1_2021.ipt",
            "ftc-11/inventor-2021/INV_nist_ftc_11_asme1_2021.ipt",
        ] {
            let bytes = std::fs::read(root.join(rel)).unwrap();
            assert_eq!(inventor_year(&bytes), Some(2021), "{rel}");
        }
        // and the scan reports it for the folder
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(
            root.join("ftc-11/inventor-2021/INV_nist_ftc_11_asme1_2021.ipt"),
            dir.path().join("Part.ipt"),
        )
        .unwrap();
        std::fs::write(dir.path().join("Other.ipt"), b"not an inventor file").unwrap();
        let scan = scan(dir.path()).unwrap();
        assert_eq!(
            scan.versions,
            vec![DetectedVersion {
                family: "inventor".into(),
                version: "2021".into(),
                files: 1
            }]
        );
    }

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
}
