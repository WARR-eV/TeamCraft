//! *Check out and open*: which CAD program on this computer opens a file,
//! and starting it.
//!
//! The installed programs are found where their installers put them – no
//! program is asked, nothing is read from inside a program:
//!
//! * **Windows:** the versioned folders below *Program Files*
//!   (`Autodesk\Inventor 2026\Bin\Inventor.exe`, `Siemens\Solid Edge 2025`,
//!   `Siemens\NX2312`, `PTC\Creo 10.0.0.0`, `Dassault Systemes\B33`,
//!   `FreeCAD 1.0`, `Rhino 8`, `SketchUp\SketchUp 2024`) and for SOLIDWORKS,
//!   whose folder carries no version, the keys `HKLM\SOFTWARE\SolidWorks\
//!   SOLIDWORKS <year>` with their `Setup\SolidWorks Folder` value (read
//!   with `reg query`).
//! * **macOS:** the app bundles in `/Applications` and `~/Applications`
//!   (FreeCAD, Rhino, SketchUp, Fusion).
//! * **Linux:** FreeCAD on the `PATH`.
//!
//! The choice ([`plan`]): a file of the project group's default program
//! opens in the group's version, or – not installed here – in the newest
//! installed version of that program. A group without a default, and a file
//! of another program, open in the newest installed version of the file's
//! own program (its format family from the server, `format_info.family`).
//! Without such a program, and for neutral files (STEP, PDF, …), the
//! operating system opens the file with its default application.
//!
//! Not tested against real installations in the development environment;
//! the paths are the installers' defaults.

use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};

/// A CAD program found on this computer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Installed {
    /// Format family of the server (`inventor`, `solidworks`, …).
    pub family: String,
    /// Version as the installer names it (`2026`, `10.0.0.0`, `B33`); empty
    /// when the installation does not tell.
    pub version: String,
    /// "Inventor 2026"
    pub label: String,
    /// The program (Windows, Linux) or the app bundle (macOS).
    pub path: PathBuf,
}

/// Where a program installs: below `parent`, one folder per version named
/// `prefix` + version, the program at `exe` inside it (empty = the folder
/// itself, a macOS app bundle).
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub family: &'static str,
    /// Short name for the button ("Inventor").
    pub short: &'static str,
    /// Folder below the base folder, `/`-separated (empty = the base itself).
    pub parent: &'static str,
    pub prefix: &'static str,
    pub exe: &'static str,
    /// A folder without a version counts too (`FreeCAD.app`).
    pub plain: bool,
}

const fn layout(
    family: &'static str,
    short: &'static str,
    parent: &'static str,
    prefix: &'static str,
    exe: &'static str,
) -> Layout {
    Layout {
        family,
        short,
        parent,
        prefix,
        exe,
        plain: false,
    }
}

/// Below each *Program Files* folder.
#[cfg_attr(not(windows), allow(dead_code))]
pub const WINDOWS: [Layout; 9] = [
    layout(
        "inventor",
        "Inventor",
        "Autodesk",
        "Inventor ",
        "Bin/Inventor.exe",
    ),
    layout(
        "solid-edge",
        "Solid Edge",
        "Siemens",
        "Solid Edge ",
        "Program/Edge.exe",
    ),
    layout("nx", "NX", "Siemens", "NX", "NXBIN/ugraf.exe"),
    layout(
        "creo",
        "Creo",
        "PTC",
        "Creo ",
        "Parametric/bin/parametric.exe",
    ),
    layout(
        "catia",
        "CATIA",
        "Dassault Systemes",
        "B",
        "win_b64/code/bin/CNEXT.exe",
    ),
    layout("freecad", "FreeCAD", "", "FreeCAD ", "bin/FreeCAD.exe"),
    layout("rhino", "Rhino", "", "Rhino ", "System/Rhino.exe"),
    layout(
        "sketchup",
        "SketchUp",
        "SketchUp",
        "SketchUp ",
        "SketchUp.exe",
    ),
    // the version comes from the registry (`solidworks_from_registry`);
    // this finds an installation the registry does not name
    Layout {
        family: "solidworks",
        short: "SolidWorks",
        parent: "SOLIDWORKS Corp",
        prefix: "SOLIDWORKS",
        exe: "SLDWORKS.exe",
        plain: true,
    },
];

/// In `/Applications` and `~/Applications`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const MACOS: [Layout; 4] = [
    Layout {
        family: "freecad",
        short: "FreeCAD",
        parent: "",
        prefix: "FreeCAD",
        exe: "",
        plain: true,
    },
    layout("rhino", "Rhino", "", "Rhino ", ""),
    layout("sketchup", "SketchUp", "", "SketchUp ", "SketchUp.app"),
    Layout {
        family: "fusion",
        short: "Fusion",
        parent: "",
        prefix: "Autodesk Fusion",
        exe: "",
        plain: true,
    },
];

/// The short name of a family ("Inventor"), the family id when no layout
/// names it.
pub fn short_name(family: &str) -> String {
    WINDOWS
        .iter()
        .chain(MACOS.iter())
        .find(|l| l.family == family)
        .map_or_else(|| family.to_string(), |l| l.short.to_string())
}

fn label(short: &str, version: &str) -> String {
    if version.is_empty() {
        short.to_string()
    } else {
        format!("{short} {version}")
    }
}

/// The version in a folder name below `parent`, if the folder belongs to
/// the layout: `Inventor 2026` → `2026`, `Rhino 8.app` → `8`, `NX2312` →
/// `2312`. A version starts with a digit (`B33`: after the prefix `B`), so
/// `Inventor Nastran 2026` or `Backup` never count; with `plain` the bare
/// prefix counts too (version `""`).
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
pub fn folder_version(layout: &Layout, folder: &str) -> Option<String> {
    let head = folder.get(..layout.prefix.len())?;
    if !head.eq_ignore_ascii_case(layout.prefix) {
        return None;
    }
    let rest = &folder[layout.prefix.len()..];
    let rest = rest
        .strip_suffix(".app")
        .or_else(|| rest.strip_suffix(".APP"))
        .unwrap_or(rest)
        .trim();
    if rest.is_empty() {
        return layout.plain.then(String::new);
    }
    rest.starts_with(|c: char| c.is_ascii_digit())
        .then(|| rest.to_string())
}

/// Joins a `/`-separated relative path.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
fn join(base: &Path, rel: &str) -> PathBuf {
    rel.split('/')
        .filter(|p| !p.is_empty())
        .fold(base.to_path_buf(), |p, part| p.join(part))
}

/// The installations of the layouts below the base folders.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
pub fn scan(bases: &[PathBuf], layouts: &[Layout]) -> Vec<Installed> {
    let mut found = Vec::new();
    for base in bases {
        for layout in layouts {
            let parent = join(base, layout.parent);
            let Ok(entries) = std::fs::read_dir(&parent) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let Some(version) = folder_version(layout, &name) else {
                    continue;
                };
                let path = join(&entry.path(), layout.exe);
                let present = if layout.exe.is_empty() || layout.exe.ends_with(".app") {
                    path.is_dir()
                } else {
                    path.is_file()
                };
                if present && !found.iter().any(|f: &Installed| f.path == path) {
                    found.push(Installed {
                        family: layout.family.to_string(),
                        label: label(layout.short, &version),
                        version,
                        path,
                    });
                }
            }
        }
    }
    found
}

/// Subkeys of a `reg query <key>` listing whose last part starts with
/// `prefix` and goes on with a version: `…\SolidWorks\SOLIDWORKS 2025` →
/// (`2025`, the full key).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn reg_subkeys(listing: &str, prefix: &'static str) -> Vec<(String, String)> {
    let layout = layout("", "", "", prefix, "");
    listing
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("HKEY_"))
        .filter_map(|key| {
            let last = key.rsplit('\\').next()?;
            Some((folder_version(&layout, last)?, key.to_string()))
        })
        .collect()
}

/// The data of value `name` in a `reg query <key> /v <name>` answer
/// (`    SolidWorks Folder    REG_SZ    C:\Program Files\…\`).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn reg_value(answer: &str, name: &str) -> Option<String> {
    answer.lines().find_map(|line| {
        let rest = line.trim().strip_prefix(name)?.trim_start();
        let rest = rest
            .strip_prefix("REG_SZ")
            .or_else(|| rest.strip_prefix("REG_EXPAND_SZ"))?;
        let value = rest.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// Version parts as numbers: `10.0.2.0` → [10, 0, 2, 0], `B33` → [33].
fn numbers(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.parse().ok())
        .collect()
}

/// Orders versions by their numbers (`2026` > `2025`, `10.0` > `9.1`,
/// `B33` > `B28`); a known version is newer than none.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    numbers(a)
        .cmp(&numbers(b))
        .then_with(|| a.len().cmp(&b.len()))
}

/// Whether an installed version is the wanted one: the same numbers
/// (`NX 2312` = `2312`), or the same text without case and spaces.
pub fn same_version(installed: &str, wanted: &str) -> bool {
    let squash = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let n = numbers(wanted);
    (!n.is_empty() && numbers(installed) == n) || squash(installed) == squash(wanted)
}

/// The installation of `family` to use: the wanted version if installed
/// (`exact`), otherwise the newest installed version.
pub fn choose<'a>(
    installed: &'a [Installed],
    family: &str,
    version: &str,
) -> Option<(&'a Installed, bool)> {
    let candidates = installed.iter().filter(|i| i.family == family);
    if !version.is_empty() {
        if let Some(hit) = candidates
            .clone()
            .filter(|i| same_version(&i.version, version))
            .max_by(|a, b| compare_versions(&a.version, &b.version))
        {
            return Some((hit, true));
        }
    }
    candidates
        .max_by(|a, b| compare_versions(&a.version, &b.version))
        .map(|i| (i, version.is_empty()))
}

/// What the user interface knows about a file and its project group.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Wish {
    /// The file's format family (`format_info.family`); empty = a neutral
    /// file.
    pub file_family: String,
    /// The group's default program (`default_cad`) and version.
    pub group_family: String,
    pub group_version: String,
}

/// How a file is opened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OpenPlan {
    /// The program; `None` = the operating system's default application.
    pub program: Option<Installed>,
    /// Text next to the button: "Inventor 2026", "Default app".
    pub label: String,
    /// Why this program (the tooltip).
    pub reason: String,
    /// The program that was asked for is not the one used (another
    /// version, or not installed).
    pub fallback: bool,
}

/// Chooses the program for a file (see the module documentation).
pub fn plan(installed: &[Installed], wish: &Wish) -> OpenPlan {
    let system = |reason: String, fallback: bool| OpenPlan {
        program: None,
        label: "Default app".into(),
        reason,
        fallback,
    };
    let file = wish.file_family.as_str();
    if file.is_empty() {
        return system(
            "Not the file of one CAD program – the operating system opens it with its default application.".into(),
            false,
        );
    }
    let from_group = !wish.group_family.is_empty() && wish.group_family == file;
    let version = if from_group {
        wish.group_version.as_str()
    } else {
        ""
    };
    let wanted = label(&short_name(file), version);
    match choose(installed, file, version) {
        Some((program, exact)) => {
            let reason = if from_group && exact && !version.is_empty() {
                format!("{wanted} is the default CAD program of the project group.")
            } else if from_group && !version.is_empty() {
                format!(
                    "{wanted}, the default of the project group, is not installed on this computer – the newest installed version opens the file."
                )
            } else if from_group {
                format!(
                    "{} is the default CAD program of the project group – the newest installed version opens the file.",
                    short_name(file)
                )
            } else if wish.group_family.is_empty() {
                format!(
                    "The project group has no default CAD program – the newest installed version of {} opens the file.",
                    short_name(file)
                )
            } else {
                format!(
                    "A {} file (the group's default is {}) – the newest installed version of {} opens it.",
                    short_name(file),
                    short_name(&wish.group_family),
                    short_name(file)
                )
            };
            OpenPlan {
                label: program.label.clone(),
                program: Some(program.clone()),
                reason,
                fallback: !exact,
            }
        }
        None => system(
            format!(
                "{wanted} was not found on this computer – the operating system opens the file with its default application."
            ),
            true,
        ),
    }
}

/// The folders the programs are looked for in on this system.
fn installed_now() -> Vec<Installed> {
    #[cfg(windows)]
    {
        let mut bases: Vec<PathBuf> = Vec::new();
        for var in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(dir) = std::env::var_os(var).map(PathBuf::from) {
                if !bases.contains(&dir) {
                    bases.push(dir);
                }
            }
        }
        let mut found = platform::solidworks_from_registry();
        for i in scan(&bases, &WINDOWS) {
            if !found.iter().any(|f| f.path == i.path) {
                found.push(i);
            }
        }
        found
    }
    #[cfg(target_os = "macos")]
    {
        let mut bases = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            bases.push(PathBuf::from(home).join("Applications"));
        }
        scan(&bases, &MACOS)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        on_path("freecad", &["freecad", "FreeCAD"])
    }
}

/// A program on the `PATH` (Linux), without a version.
#[cfg(not(any(windows, target_os = "macos")))]
fn on_path(family: &str, names: &[&str]) -> Vec<Installed> {
    let Some(path) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    std::env::split_paths(&path)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|p| p.is_file())
        .map(|path| Installed {
            family: family.to_string(),
            version: String::new(),
            label: short_name(family),
            path,
        })
        .into_iter()
        .collect()
}

/// The CAD programs of this computer, found once per start of the app.
pub fn installed() -> &'static [Installed] {
    static FOUND: OnceLock<Vec<Installed>> = OnceLock::new();
    FOUND.get_or_init(installed_now)
}

/// Starts `program` with `file` (macOS: `open -a <bundle> <file>`). The
/// program keeps running after the app; its exit is collected on the side.
pub fn launch(program: &Installed, file: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg("-a").arg(&program.path).arg(file);
        c
    };
    #[cfg(not(target_os = "macos"))]
    let mut command = {
        let mut c = std::process::Command::new(&program.path);
        c.arg(file);
        if let Some(dir) = file.parent() {
            c.current_dir(dir);
        }
        c
    };
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("{} could not be started: {e}", program.label))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{os::windows::process::CommandExt, process::Command};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn reg(args: &[&str]) -> Option<String> {
        let out = Command::new("reg")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// `HKLM\SOFTWARE\SolidWorks\SOLIDWORKS <year>\Setup` → `SolidWorks Folder`.
    pub fn solidworks_from_registry() -> Vec<Installed> {
        let Some(listing) = reg(&["query", r"HKLM\SOFTWARE\SolidWorks"]) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for (version, key) in reg_subkeys(&listing, "SOLIDWORKS ") {
            let setup = format!(r"{key}\Setup");
            let Some(folder) = reg(&["query", &setup, "/v", "SolidWorks Folder"])
                .and_then(|a| reg_value(&a, "SolidWorks Folder"))
            else {
                continue;
            };
            let path = PathBuf::from(folder.trim_end_matches('\\')).join("SLDWORKS.exe");
            if path.is_file() && !found.iter().any(|f: &Installed| f.path == path) {
                found.push(Installed {
                    family: "solidworks".into(),
                    label: label("SolidWorks", &version),
                    version,
                    path,
                });
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(family: &str, version: &str) -> Installed {
        Installed {
            family: family.into(),
            version: version.into(),
            label: label(&short_name(family), version),
            path: PathBuf::from(format!("/p/{family}-{version}")),
        }
    }

    #[test]
    fn folder_names_give_the_version() {
        let inventor = WINDOWS[0];
        assert_eq!(
            folder_version(&inventor, "Inventor 2026").as_deref(),
            Some("2026")
        );
        assert_eq!(
            folder_version(&inventor, "inventor 2025").as_deref(),
            Some("2025")
        );
        assert_eq!(folder_version(&inventor, "Inventor Nastran 2026"), None);
        assert_eq!(folder_version(&inventor, "Inventor"), None, "no version");
        assert_eq!(folder_version(&inventor, "Inv"), None);
        let nx = WINDOWS[2];
        assert_eq!(folder_version(&nx, "NX2312").as_deref(), Some("2312"));
        assert_eq!(folder_version(&nx, "NX 1980").as_deref(), Some("1980"));
        let catia = WINDOWS[4];
        assert_eq!(folder_version(&catia, "B33").as_deref(), Some("33"));
        assert_eq!(folder_version(&catia, "Backup"), None);
        let rhino = MACOS[1];
        assert_eq!(folder_version(&rhino, "Rhino 8.app").as_deref(), Some("8"));
        let freecad = MACOS[0];
        assert_eq!(folder_version(&freecad, "FreeCAD.app").as_deref(), Some(""));
        assert_eq!(folder_version(&freecad, "FreeCAD_old.app"), None);
    }

    #[test]
    fn scan_finds_versioned_installations() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().to_path_buf();
        for rel in [
            "Autodesk/Inventor 2025/Bin/Inventor.exe",
            "Autodesk/Inventor 2026/Bin/Inventor.exe",
            "Autodesk/Inventor Nastran 2026/Bin/Inventor.exe",
            "SOLIDWORKS Corp/SOLIDWORKS/SLDWORKS.exe",
            "FreeCAD 1.0/bin/FreeCAD.exe",
        ] {
            let p = join(&base, rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"").unwrap();
        }
        // a folder without the program is no installation
        std::fs::create_dir_all(join(&base, "Autodesk/Inventor 2024/Bin")).unwrap();
        let mut found: Vec<(String, String)> = scan(&[base.clone(), base], &WINDOWS)
            .into_iter()
            .map(|i| (i.family, i.label))
            .collect();
        found.sort();
        assert_eq!(
            found,
            vec![
                ("freecad".into(), "FreeCAD 1.0".into()),
                ("inventor".into(), "Inventor 2025".into()),
                ("inventor".into(), "Inventor 2026".into()),
                ("solidworks".into(), "SolidWorks".into()),
            ]
        );
    }

    #[test]
    fn registry_answers_are_read() {
        let listing = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\SolidWorks\\Applications\r\n\
            HKEY_LOCAL_MACHINE\\SOFTWARE\\SolidWorks\\SOLIDWORKS 2024\r\n\
            HKEY_LOCAL_MACHINE\\SOFTWARE\\SolidWorks\\SOLIDWORKS 2025\r\n";
        let keys = reg_subkeys(listing, "SOLIDWORKS ");
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[1].0, "2025");
        assert!(keys[1].1.ends_with("SOLIDWORKS 2025"));
        let answer = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\SolidWorks\\SOLIDWORKS 2025\\Setup\r\n    \
            SolidWorks Folder    REG_SZ    C:\\Program Files\\SOLIDWORKS Corp\\SOLIDWORKS\\\r\n";
        assert_eq!(
            reg_value(answer, "SolidWorks Folder").as_deref(),
            Some("C:\\Program Files\\SOLIDWORKS Corp\\SOLIDWORKS\\")
        );
        assert_eq!(reg_value(answer, "Other"), None);
    }

    #[test]
    fn versions_are_ordered_by_their_numbers() {
        assert_eq!(compare_versions("2026", "2025"), Ordering::Greater);
        assert_eq!(compare_versions("10.0.0.0", "9.1.0.0"), Ordering::Greater);
        assert_eq!(compare_versions("33", "28"), Ordering::Greater);
        assert_eq!(compare_versions("2312", "1980"), Ordering::Greater);
        assert_eq!(compare_versions("", "1"), Ordering::Less);
        assert!(same_version("2312", "NX 2312"));
        assert!(same_version("2026", "2026"));
        assert!(!same_version("2025", "2026"));
    }

    #[test]
    fn the_group_version_wins_else_the_newest() {
        let all = [
            inst("inventor", "2025"),
            inst("inventor", "2026"),
            inst("inventor", "2027"),
            inst("solidworks", "2025"),
        ];
        let (p, exact) = choose(&all, "inventor", "2026").unwrap();
        assert_eq!((p.version.as_str(), exact), ("2026", true));
        let (p, exact) = choose(&all, "inventor", "2024").unwrap();
        assert_eq!(
            (p.version.as_str(), exact),
            ("2027", false),
            "newest instead"
        );
        let (p, exact) = choose(&all, "inventor", "").unwrap();
        assert_eq!((p.version.as_str(), exact), ("2027", true));
        assert!(choose(&all, "nx", "").is_none());
    }

    #[test]
    fn plans_follow_the_group_and_the_file() {
        let all = [
            inst("inventor", "2025"),
            inst("inventor", "2026"),
            inst("solidworks", "2024"),
        ];
        let wish = |file: &str, group: &str, version: &str| Wish {
            file_family: file.into(),
            group_family: group.into(),
            group_version: version.into(),
        };
        // the group's version
        let p = plan(&all, &wish("inventor", "inventor", "2025"));
        assert_eq!(p.label, "Inventor 2025");
        assert!(!p.fallback);
        assert!(p
            .reason
            .contains("default CAD program of the project group"));
        // the group's version is missing: the newest one
        let p = plan(&all, &wish("inventor", "inventor", "2027"));
        assert_eq!(p.label, "Inventor 2026");
        assert!(p.fallback && p.reason.contains("not installed"));
        // no default: the newest version that fits the file
        let p = plan(&all, &wish("inventor", "", ""));
        assert_eq!(p.label, "Inventor 2026");
        assert!(p.reason.contains("no default CAD program"), "{}", p.reason);
        // a file of another program than the group's
        let p = plan(&all, &wish("solidworks", "inventor", "2026"));
        assert_eq!(p.label, "SolidWorks 2024");
        // not installed: the operating system
        let p = plan(&all, &wish("nx", "", ""));
        assert!(p.program.is_none() && p.fallback);
        assert!(p.reason.contains("NX was not found"), "{}", p.reason);
        // neutral files
        let p = plan(&all, &wish("", "inventor", "2026"));
        assert!(p.program.is_none() && !p.fallback);
    }
}
