//! *Getting started* → *CAD setup* (#478): small setup files that point a
//! CAD program at a project group's folder in the working folder.
//!
//! * Inventor – a project file `<group>.ipj` whose workspace is its own
//!   folder (`.\`), so references resolve inside the group's folder;
//! * Solid Edge – a link management file `LinkMgmt.txt` with the group's
//!   folder as search path (Solid Edge reads it from the folder set in
//!   *Options → File Locations → Link Management*);
//! * NX – a text file with the search folder (`<group folder>\...`, the
//!   trailing `...` includes the subfolders) and where to enter it in the
//!   *Assembly Load Options*. TeamCraft does not write `load_options.def`
//!   itself: its syntax is not publicly documented.
//!
//! Sources of every format finding: `docs/src/research/cad-setup-files.md`.
//! The files were not checked with the real CAD programs yet – the user
//! interface calls them a starting point to check in the CAD program.
//!
//! Rules: the web view names only a group code and a [`Kind`], never a path;
//! the file goes into the group's folder below the working folder
//! ([`group_dir`]), which must exist (the group was fetched); a file that
//! exists is never overwritten ([`write_new`] opens with `create_new`).
//! [`content`] is pure, so the preview and the written file are the same text.

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::layout;

/// The setup files the wizard offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Inventor project file `<group>.ipj`.
    InventorProject,
    /// NX search folder for the assembly load options (a text file).
    NxSearchFolders,
    /// Solid Edge link management file `LinkMgmt.txt`.
    SolidEdgeLinks,
}

/// Name of the NX text file.
pub const NX_FILE: &str = "NX load options (TeamCraft).txt";
/// Name of the Solid Edge file (the name Solid Edge looks for).
pub const SOLID_EDGE_FILE: &str = "LinkMgmt.txt";

impl Kind {
    pub const ALL: [Kind; 3] = [
        Kind::InventorProject,
        Kind::NxSearchFolders,
        Kind::SolidEdgeLinks,
    ];

    /// The value the web view sends (`inventor`, `nx`, `solidedge`).
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::InventorProject => "inventor",
            Kind::NxSearchFolders => "nx",
            Kind::SolidEdgeLinks => "solidedge",
        }
    }

    pub fn parse(value: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == value)
    }

    /// The program's name for the user interface.
    pub fn program(self) -> &'static str {
        match self {
            Kind::InventorProject => "Inventor",
            Kind::NxSearchFolders => "NX",
            Kind::SolidEdgeLinks => "Solid Edge",
        }
    }

    /// The file name in the group's folder; `folder_name` is the name of
    /// that folder (the group code made safe, [`layout::safe_name`]).
    pub fn file_name(self, folder_name: &str) -> String {
        match self {
            Kind::InventorProject => format!("{folder_name}.ipj"),
            Kind::NxSearchFolders => NX_FILE.to_string(),
            Kind::SolidEdgeLinks => SOLID_EDGE_FILE.to_string(),
        }
    }
}

/// Text for an XML element: `&`, `<`, `>`, `"` and `'` escaped; control
/// characters other than tab, line feed and carriage return (not allowed in
/// XML 1.0) are dropped.
pub fn xml_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// `lines` joined with CRLF (`windows`) or LF, with a final line break.
fn join_lines(lines: &[String], windows: bool) -> String {
    let eol = if windows { "\r\n" } else { "\n" };
    let mut out = lines.join(eol);
    out.push_str(eol);
    out
}

/// The content of a setup file for the group folder `folder` (the absolute
/// path as the operating system writes it; `windows` picks `\` and CRLF).
/// `group` is the code shown in the NX note. Pure: the preview shows exactly
/// this text and [`create`] writes it.
pub fn content(kind: Kind, group: &str, folder: &str, windows: bool) -> String {
    match kind {
        // Inventor is Windows-only: CRLF. The workspace `.\` is the folder of
        // the project file itself (Inventor's default), so the file needs no
        // absolute path and keeps working when the working folder moves.
        Kind::InventorProject => join_lines(
            &[
                r#"<?xml version="1.0" encoding="UTF-8"?>"#.to_string(),
                "<!-- Created by TeamCraft Desktop as a starting point: check it in Inventor (Get Started > Projects). -->".to_string(),
                "<InventorProject>".to_string(),
                format!("  <WorkspacePath>{}</WorkspacePath>", xml_text(r".\")),
                "</InventorProject>".to_string(),
            ],
            true,
        ),
        // Solid Edge is Windows-only: CRLF. No comment lines – the file has
        // no documented comment syntax.
        Kind::SolidEdgeLinks => join_lines(
            &[
                "CONTAINER".to_string(),
                "RELATIVE".to_string(),
                "ABSOLUTE".to_string(),
                "BEGIN SEARCH PATH".to_string(),
                folder.to_string(),
                "END SEARCH PATH".to_string(),
            ],
            true,
        ),
        Kind::NxSearchFolders => {
            let sep = if windows { '\\' } else { '/' };
            let trimmed = folder.trim_end_matches(['\\', '/']);
            join_lines(
                &[
                    format!("TeamCraft Desktop - NX search folder of the project group {group}"),
                    "A starting point: check it in NX. TeamCraft does not change NX's own load options file.".to_string(),
                    String::new(),
                    "Search folder (the trailing ... includes all subfolders):".to_string(),
                    format!("{trimmed}{sep}..."),
                    String::new(),
                    "In NX: File > Options > Assembly Load Options".to_string(),
                    "  1. Load: From Search Folders".to_string(),
                    "  2. Add the search folder above".to_string(),
                    "  3. Save the load options (as default, or to a file that the".to_string(),
                    "     environment variable UGII_LOAD_OPTIONS names)".to_string(),
                ],
                windows,
            )
        }
    }
}

/// The folder of a project group in the working folder `root`: the group
/// code made safe ([`layout::safe_name`], as Get names it), one level below
/// `root` – a code can never name a path outside it.
pub fn group_dir(root: &Path, group: &str) -> Result<PathBuf, String> {
    let code = group.trim();
    if code.is_empty() {
        return Err("Choose a project group.".into());
    }
    let dir = root.join(layout::safe_name(code));
    if dir.parent() != Some(root) {
        return Err(format!("'{code}' is not a project group folder."));
    }
    Ok(dir)
}

/// The message when the file exists: it is never overwritten.
pub fn refusal(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir = path
        .parent()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    format!(
        "{name} already exists in {dir} – TeamCraft never overwrites it. Check it in your CAD program, or delete it to create it again."
    )
}

/// One setup file as the wizard shows it before anything is written.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Planned {
    /// `inventor`, `nx` or `solidedge`.
    pub kind: &'static str,
    pub program: &'static str,
    pub file_name: String,
    pub path: PathBuf,
    pub content: String,
    /// A file (or anything else) of that name is there: it is not created.
    pub exists: bool,
}

/// What [`create`] would write for `group` and `kind` in the working folder
/// `root`. The group's folder must exist (the group was fetched with *Get*).
pub fn plan(root: &Path, group: &str, kind: Kind) -> Result<Planned, String> {
    let dir = group_dir(root, group)?;
    if !dir.is_dir() {
        return Err(format!(
            "The project group {} is not in the working folder yet – get it first (Get project groups…).",
            group.trim()
        ));
    }
    let folder = dir.display().to_string();
    if folder.chars().any(char::is_control) {
        return Err(format!(
            "The folder {} has a control character in its path.",
            folder.escape_debug()
        ));
    }
    let folder_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = kind.file_name(&folder_name);
    let path = dir.join(&file_name);
    Ok(Planned {
        kind: kind.as_str(),
        program: kind.program(),
        exists: path.symlink_metadata().is_ok(),
        content: content(kind, group.trim(), &folder, cfg!(windows)),
        file_name,
        path,
    })
}

/// Writes `content` to `path`, which must not exist yet (`create_new`: also
/// a file that appeared after the preview is never overwritten). A failed
/// write removes the partial file.
pub fn write_new(path: &Path, content: &str) -> Result<(), String> {
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(refusal(path)),
        Err(e) => return Err(format!("{} could not be created: {e}", path.display())),
    };
    if let Err(e) = file
        .write_all(content.as_bytes())
        .and_then(|_| file.sync_all())
    {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(format!("{} could not be written: {e}", path.display()));
    }
    Ok(())
}

/// Creates the setup file `kind` of `group` and returns its path.
pub fn create(root: &Path, group: &str, kind: Kind) -> Result<PathBuf, String> {
    let planned = plan(root, group, kind)?;
    if planned.exists {
        return Err(refusal(&planned.path));
    }
    write_new(&planned.path, &planned.content)?;
    Ok(planned.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip() {
        for kind in Kind::ALL {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(Kind::parse("catia"), None);
        assert_eq!(Kind::parse("../x"), None);
        assert_eq!(Kind::parse(""), None);
    }

    #[test]
    fn xml_text_escapes_markup() {
        assert_eq!(
            xml_text(r#"A&B <C> "D" 'E'"#),
            "A&amp;B &lt;C&gt; &quot;D&quot; &apos;E&apos;"
        );
        assert_eq!(xml_text("Jörg\u{1}"), "Jörg");
        assert_eq!(xml_text(r".\"), r".\");
    }

    #[test]
    fn inventor_project_is_relative_and_crlf() {
        let text = content(
            Kind::InventorProject,
            "ROVER",
            r"C:\Users\Jörg Müller\TeamCraft\Rover & Co",
            true,
        );
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n"));
        assert!(text.contains("<WorkspacePath>.\\</WorkspacePath>"));
        // no absolute path, so nothing of the folder needs escaping
        assert!(!text.contains("Jörg"));
        assert!(!text.contains(" & "));
        assert!(text.ends_with("</InventorProject>\r\n"));
        assert!(!text.replace("\r\n", "").contains('\n'));
        // a comment must not contain `--`
        let comment = &text[text.find("<!--").unwrap() + 4..text.find("-->").unwrap()];
        assert!(!comment.contains("--"));
        // the same text on every system (Inventor runs on Windows)
        assert_eq!(
            text,
            content(
                Kind::InventorProject,
                "ROVER",
                "/home/j/TeamCraft/ROVER",
                false
            )
        );
    }

    #[test]
    fn solid_edge_search_path_with_spaces_and_umlauts() {
        let text = content(
            Kind::SolidEdgeLinks,
            "Wärmetauscher",
            r"C:\Users\Jörg Müller\Documents\TeamCraft\Wärmetauscher",
            true,
        );
        assert_eq!(
            text,
            "CONTAINER\r\nRELATIVE\r\nABSOLUTE\r\nBEGIN SEARCH PATH\r\nC:\\Users\\Jörg Müller\\Documents\\TeamCraft\\Wärmetauscher\r\nEND SEARCH PATH\r\n"
        );
    }

    #[test]
    fn nx_search_folder_includes_subfolders() {
        let win = content(
            Kind::NxSearchFolders,
            "ROVER",
            r"D:\CAD Daten\TeamCraft\ROVER\",
            true,
        );
        assert!(win.contains("\r\nD:\\CAD Daten\\TeamCraft\\ROVER\\...\r\n"));
        assert!(win.contains("project group ROVER"));
        let unix = content(
            Kind::NxSearchFolders,
            "Größe",
            "/home/jörg/TeamCraft/Größe",
            false,
        );
        assert!(unix.contains("\n/home/jörg/TeamCraft/Größe/...\n"));
        assert!(!unix.contains('\r'));
    }

    #[test]
    fn file_names() {
        assert_eq!(Kind::InventorProject.file_name("Rover 2"), "Rover 2.ipj");
        assert_eq!(Kind::SolidEdgeLinks.file_name("x"), "LinkMgmt.txt");
        assert_eq!(Kind::NxSearchFolders.file_name("x"), NX_FILE);
    }

    #[test]
    fn group_dir_stays_below_the_root() {
        let root = Path::new("/ws");
        assert_eq!(group_dir(root, " ROVER ").unwrap(), root.join("ROVER"));
        for bad in ["..", "../etc", "a/b", r"a\..\b", "/abs", r"C:\x"] {
            let dir = group_dir(root, bad).unwrap();
            assert_eq!(dir.parent(), Some(root), "{bad}");
        }
        assert!(group_dir(root, "  ").is_err());
    }

    #[test]
    fn plan_needs_the_fetched_group_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let err = plan(tmp.path(), "ROVER", Kind::InventorProject).unwrap_err();
        assert!(err.contains("get it first"), "{err}");
        std::fs::create_dir(tmp.path().join("Größe Teile")).unwrap();
        let planned = plan(tmp.path(), "Größe Teile", Kind::InventorProject).unwrap();
        assert_eq!(planned.file_name, "Größe Teile.ipj");
        assert_eq!(
            planned.path,
            tmp.path().join("Größe Teile").join("Größe Teile.ipj")
        );
        assert!(!planned.exists);
        assert_eq!(planned.kind, "inventor");
    }

    #[test]
    fn create_writes_the_preview_and_never_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("ROVER");
        std::fs::create_dir(&dir).unwrap();
        let planned = plan(tmp.path(), "ROVER", Kind::SolidEdgeLinks).unwrap();
        let path = create(tmp.path(), "ROVER", Kind::SolidEdgeLinks).unwrap();
        assert_eq!(path, dir.join("LinkMgmt.txt"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), planned.content);

        // a second time: refused, the file stays as it is
        std::fs::write(&path, "mine").unwrap();
        assert!(
            plan(tmp.path(), "ROVER", Kind::SolidEdgeLinks)
                .unwrap()
                .exists
        );
        let err = create(tmp.path(), "ROVER", Kind::SolidEdgeLinks).unwrap_err();
        assert!(err.contains("never overwrites"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine");

        // a file that appeared after the preview: create_new refuses too
        let err = write_new(&path, "other").unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine");
    }
}
