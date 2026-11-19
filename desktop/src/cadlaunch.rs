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
//! **A running Inventor** ([`open_in_running`], Windows): when an Inventor
//! the user started is already running, is the installation [`plan`] chose
//! and has the working folder's Inventor project ([`project_file`]) active,
//! the file opens there as a new document window instead of in a second
//! instance (#350). Inventor is asked through its documented COM interface
//! (`GetActiveObject`, `InstallPath`,
//! `DesignProjectManager.ActiveDesignProject`, `Documents.Open`) by a short
//! Windows PowerShell script, like the STEP export (`cadexport.rs`). Any
//! other answer starts a new instance as before, with a note why.
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
    /// The file opened in an instance that was already running (#350).
    pub reused: bool,
    /// Why a running instance was not used (empty = nothing to say).
    pub note: String,
}

/// Chooses the program for a file (see the module documentation).
pub fn plan(installed: &[Installed], wish: &Wish) -> OpenPlan {
    let system = |reason: String, fallback: bool| OpenPlan {
        program: None,
        label: "Default app".into(),
        reason,
        fallback,
        reused: false,
        note: String::new(),
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
                reused: false,
                note: String::new(),
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

// ---------------------------------------------------------------------------
// A running Inventor (#350)
// ---------------------------------------------------------------------------

/// The extension of Inventor project files.
const PROJECT_EXTENSION: &str = "ipj";

/// How long a running Inventor may take to say whether it opens the file
/// (a busy or blocked instance: a new one starts instead). Opening the file
/// itself is not waited for – a large assembly loads for minutes.
#[cfg_attr(not(windows), allow(dead_code))]
pub const ASK_LIMIT: std::time::Duration = std::time::Duration::from_secs(20);

/// The Inventor project file (`.ipj`) of a file in the working folder: the
/// nearest folder, from the file's own up to the working folder `root`,
/// that holds exactly one. A folder with several makes it unknown, and so
/// does a file outside the working folder (`None` for both).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn project_file(file: &Path, root: &Path) -> Option<PathBuf> {
    for dir in file.ancestors().skip(1) {
        if !dir.starts_with(root) {
            return None;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut found = entries.flatten().map(|e| e.path()).filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case(PROJECT_EXTENSION))
            });
            if let Some(first) = found.next() {
                return found.next().is_none().then_some(first);
            }
        }
        if dir == root {
            return None;
        }
    }
    None
}

/// What a running Inventor answered when asked to open a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Running {
    /// No Inventor the user started runs, or it could not be asked in time.
    Nothing,
    /// It opens the file as a new document window.
    Opening,
    /// Another installation runs (its install folder).
    OtherVersion(String),
    /// It has another project active (its project file; empty = none).
    OtherProject(String),
}

/// One line of [`INVENTOR_OPEN_SCRIPT`]'s answer: `none`, `opening`,
/// `version <install folder>` or `project <project file>`; `None` for any
/// other line.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_answer(line: &str) -> Option<Running> {
    let line = line.trim();
    let rest = |word: &str| {
        line.strip_prefix(word)
            .filter(|r| r.is_empty() || r.starts_with(' '))
            .map(|r| r.trim().to_string())
    };
    match line {
        "none" => Some(Running::Nothing),
        "opening" => Some(Running::Opening),
        _ => rest("version")
            .map(Running::OtherVersion)
            .or_else(|| rest("project").map(Running::OtherProject)),
    }
}

/// The last part of a Windows or Unix path (`C:\…\Inventor 2025` →
/// `Inventor 2025`), without the `.ipj` of a project file.
fn last_part(path: &str) -> String {
    let name = path
        .trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default();
    let dot = name.len().saturating_sub(PROJECT_EXTENSION.len() + 1);
    match name.get(dot..) {
        Some(ext) if dot > 0 && ext.eq_ignore_ascii_case(".ipj") => name[..dot].to_string(),
        _ => name.to_string(),
    }
}

/// The note shown when a running Inventor was not used (`None` when none
/// runs, or when it opened the file).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn running_note(
    running: &Running,
    wanted: &Installed,
    project: Option<&Path>,
) -> Option<String> {
    match running {
        Running::Nothing | Running::Opening => None,
        Running::OtherVersion(dir) => Some(format!(
            "{} is running, not {} – opened in a new instance.",
            last_part(dir),
            wanted.label
        )),
        Running::OtherProject(active) => {
            let active = if active.is_empty() {
                "no project".to_string()
            } else {
                format!("the project {}", last_part(active))
            };
            Some(match project {
                Some(own) => format!(
                    "Inventor is running with {active}, not {} – opened in a new instance.",
                    last_part(&own.to_string_lossy())
                ),
                None => format!(
                    "Inventor is running with {active}, and the working folder has no Inventor project file (.ipj) to compare it with – opened in a new instance."
                ),
            })
        }
    }
}

/// Whether a `tasklist /FO CSV /NH` answer lists the program `image`
/// (`"Inventor.exe","1234",…`; any other answer, a localised "no tasks"
/// message too, does not).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn tasklist_has(answer: &str, image: &str) -> bool {
    answer.lines().any(|line| {
        line.trim()
            .strip_prefix('"')
            .and_then(|l| l.split('"').next())
            .is_some_and(|name| name.eq_ignore_ascii_case(image))
    })
}

/// Asks a running Inventor to open a file, and answers with one line (see
/// [`parse_answer`]) – before `Documents.Open`, so the app does not wait
/// for a large assembly to load. Only an instance the user can see counts:
/// an invisible one is an automation run (a STEP export, `cadexport.rs`).
#[cfg_attr(not(windows), allow(dead_code))]
pub const INVENTOR_OPEN_SCRIPT: &str = r#"param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$InstallDir, [string]$Project = '')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
function Say([string]$Line) { [Console]::Out.WriteLine($Line); [Console]::Out.Flush() }
function Same([string]$A, [string]$B) {
    if (-not $A -or -not $B) { return $false }
    return [IO.Path]::GetFullPath($A).TrimEnd('\') -ieq [IO.Path]::GetFullPath($B).TrimEnd('\')
}
try { $app = [Runtime.InteropServices.Marshal]::GetActiveObject('Inventor.Application') }
catch { Say 'none'; exit 0 }
if (-not $app.Visible) { Say 'none'; exit 0 }
$dir = [string]$app.InstallPath
if (-not (Same $dir $InstallDir)) { Say "version $dir"; exit 0 }
$active = ''
$current = $app.DesignProjectManager.ActiveDesignProject
if ($null -ne $current) { $active = [string]$current.FullFileName }
if (-not (Same $active $Project)) { Say "project $active"; exit 0 }
Say 'opening'
try {
    $shell = New-Object -ComObject WScript.Shell
    foreach ($p in @(Get-Process -Name Inventor -ErrorAction SilentlyContinue)) {
        if ($p.Path -and (Same (Split-Path (Split-Path $p.Path)) $InstallDir)) { [void]$shell.AppActivate($p.Id); break }
    }
} catch { }
$doc = $null
foreach ($d in $app.Documents) { if (Same $d.FullFileName $Path) { $doc = $d } }
if ($null -eq $doc) { [void]$app.Documents.Open($Path, $true) } else { [void]$doc.Activate() }
"#;

/// How *Check out and open* goes on after asking a running instance.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))] // `Opened` is only constructed on Windows
pub enum Reuse {
    /// The running instance opens the file.
    Opened,
    /// Start a new instance; the note says why a running one was not used
    /// (empty = none runs).
    NewInstance(String),
}

/// Opens `file` in a running instance of `program` when that is the
/// installation [`plan`] chose and has the working folder's project active
/// (#350; Inventor on Windows – anywhere else nothing is asked and a new
/// instance starts, as before).
pub fn open_in_running(program: &Installed, file: &Path, root: &Path) -> Reuse {
    if program.family != "inventor" {
        return Reuse::NewInstance(String::new());
    }
    #[cfg(windows)]
    {
        let project = project_file(file, root);
        match running::ask(program, file, project.as_deref()) {
            Running::Opening => Reuse::Opened,
            other => Reuse::NewInstance(
                running_note(&other, program, project.as_deref()).unwrap_or_default(),
            ),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (file, root);
        Reuse::NewInstance(String::new())
    }
}

#[cfg(windows)]
mod running {
    use super::*;
    use std::{
        io::BufRead,
        os::windows::process::CommandExt,
        process::{Command, Stdio},
        sync::mpsc,
    };

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Whether an Inventor process runs at all – `tasklist` starts much
    /// faster than PowerShell, which is then only started when one does.
    fn inventor_runs() -> bool {
        Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Inventor.exe", "/FO", "CSV", "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .is_ok_and(|o| tasklist_has(&String::from_utf8_lossy(&o.stdout), "Inventor.exe"))
    }

    pub fn ask(program: &Installed, file: &Path, project: Option<&Path>) -> Running {
        // Bin\Inventor.exe → the install folder Inventor reports
        let Some(install) = program.path.parent().and_then(Path::parent) else {
            return Running::Nothing;
        };
        if !inventor_runs() {
            return Running::Nothing;
        }
        let script = std::env::temp_dir().join(format!(
            "teamcraft-open-in-inventor-{}-{}.ps1",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        if std::fs::write(&script, INVENTOR_OPEN_SCRIPT).is_err() {
            return Running::Nothing;
        }
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&script)
            .arg("-Path")
            .arg(file)
            .arg("-InstallDir")
            .arg(install);
        if let Some(project) = project {
            command.arg("-Project").arg(project);
        }
        let mut child = match command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                let _ = std::fs::remove_file(&script);
                return Running::Nothing;
            }
        };
        let (tx, rx) = mpsc::channel();
        if let Some(out) = child.stdout.take() {
            std::thread::spawn(move || {
                let mut answered = false;
                for line in std::io::BufReader::new(out).lines() {
                    let Ok(line) = line else { break };
                    if !answered {
                        if let Some(answer) = parse_answer(&line) {
                            answered = true;
                            let _ = tx.send(answer);
                        }
                    }
                }
            });
        }
        let (answer, stuck) = match rx.recv_timeout(ASK_LIMIT) {
            Ok(answer) => (answer, false),
            Err(mpsc::RecvTimeoutError::Timeout) => (Running::Nothing, true),
            Err(mpsc::RecvTimeoutError::Disconnected) => (Running::Nothing, false),
        };
        // the script goes on opening the file; collected on the side
        std::thread::spawn(move || {
            if stuck {
                let _ = child.kill();
            }
            let _ = child.wait();
            let _ = std::fs::remove_file(&script);
        });
        answer
    }
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

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    #[test]
    fn the_nearest_single_project_file_counts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("TeamCraft");
        let design = root.join("GRAKSLER").join("Design");
        let part = design.join("Arm").join("A-100").join("arm.ipt");
        touch(&part);
        // nothing up to the working folder: unknown
        assert_eq!(project_file(&part, &root), None);
        // the group's project file, found from a sub folder
        touch(&design.join("Graksler 4.3.IPJ"));
        assert_eq!(
            project_file(&part, &root),
            Some(design.join("Graksler 4.3.IPJ"))
        );
        // a nearer one wins
        touch(&design.join("Arm").join("Arm.ipj"));
        assert_eq!(
            project_file(&part, &root),
            Some(design.join("Arm").join("Arm.ipj"))
        );
        // two in the nearest folder that has any: unknown, not the next one up
        touch(&design.join("Arm").join("Old.ipj"));
        assert_eq!(project_file(&part, &root), None);
        // the working folder itself is searched, nothing above it
        let other = root.join("OTHER").join("x.ipt");
        touch(&other);
        touch(&dir.path().join("Above.ipj"));
        assert_eq!(project_file(&other, &root), None);
        touch(&root.join("Root.ipj"));
        assert_eq!(project_file(&other, &root), Some(root.join("Root.ipj")));
        // a file outside the working folder
        assert_eq!(project_file(&dir.path().join("loose.ipt"), &root), None);
    }

    #[test]
    fn the_running_inventor_answers_are_read() {
        assert_eq!(parse_answer("none\r"), Some(Running::Nothing));
        assert_eq!(parse_answer(" opening "), Some(Running::Opening));
        assert_eq!(
            parse_answer(r"version C:\Program Files\Autodesk\Inventor 2025\"),
            Some(Running::OtherVersion(
                r"C:\Program Files\Autodesk\Inventor 2025\".into()
            ))
        );
        assert_eq!(
            parse_answer(r"project C:\TeamCraft\OTHER\Other.ipj"),
            Some(Running::OtherProject(
                r"C:\TeamCraft\OTHER\Other.ipj".into()
            ))
        );
        // no active project: the bare word
        assert_eq!(
            parse_answer("project "),
            Some(Running::OtherProject(String::new()))
        );
        assert_eq!(parse_answer("projects x"), None);
        assert_eq!(parse_answer("WARNING: something"), None);
        assert_eq!(parse_answer(""), None);
    }

    #[test]
    fn the_script_says_only_what_is_read() {
        // every answer the script prints is one parse_answer knows
        let says: Vec<&str> = INVENTOR_OPEN_SCRIPT
            .lines()
            .flat_map(|l| l.split("Say ").skip(1))
            .map(|s| {
                s.split(';')
                    .next()
                    .unwrap()
                    .trim()
                    .trim_matches(['\'', '"'])
            })
            .collect();
        assert_eq!(
            says,
            vec!["none", "none", "version $dir", "project $active", "opening"]
        );
        for said in says {
            let line = said
                .replace("$dir", r"C:\x")
                .replace("$active", r"C:\p.ipj");
            assert!(parse_answer(&line).is_some(), "{line}");
        }
        // the answer comes before the file is opened (a large assembly loads
        // for minutes), and only the user's visible instance is used
        let opening = INVENTOR_OPEN_SCRIPT.find("Say 'opening'").unwrap();
        assert!(INVENTOR_OPEN_SCRIPT.find("Documents.Open").unwrap() > opening);
        assert!(INVENTOR_OPEN_SCRIPT.contains("if (-not $app.Visible)"));
        assert!(!INVENTOR_OPEN_SCRIPT.contains("New-Object -ComObject 'Inventor.Application'"));
    }

    #[test]
    fn notes_say_why_a_new_instance_starts() {
        let wanted = inst("inventor", "2026");
        assert_eq!(running_note(&Running::Nothing, &wanted, None), None);
        assert_eq!(running_note(&Running::Opening, &wanted, None), None);
        let other = Running::OtherVersion(r"C:\Program Files\Autodesk\Inventor 2025\".into());
        assert_eq!(
            running_note(&other, &wanted, None).unwrap(),
            "Inventor 2025 is running, not Inventor 2026 – opened in a new instance."
        );
        let own = PathBuf::from("/w/GRAKSLER/Design/Graksler 4.3.ipj");
        let project = Running::OtherProject(
            r"C:\Users\Public\Documents\Autodesk\Inventor 2026\Default.ipj".into(),
        );
        assert_eq!(
            running_note(&project, &wanted, Some(&own)).unwrap(),
            "Inventor is running with the project Default, not Graksler 4.3 – opened in a new instance."
        );
        let none = Running::OtherProject(String::new());
        let note = running_note(&none, &wanted, None).unwrap();
        assert!(
            note.starts_with("Inventor is running with no project,"),
            "{note}"
        );
        assert!(note.contains("no Inventor project file (.ipj)"), "{note}");
    }

    #[test]
    fn tasklist_answers_are_read() {
        let running = "\"Inventor.exe\",\"10244\",\"Console\",\"1\",\"1.234.567 K\"\r\n";
        assert!(tasklist_has(running, "Inventor.exe"));
        assert!(tasklist_has(&running.to_lowercase(), "Inventor.exe"));
        assert!(!tasklist_has(
            "INFORMATION: Es werden keine Aufgaben mit den angegebenen Kriterien ausgeführt.\r\n",
            "Inventor.exe"
        ));
        assert!(!tasklist_has(
            "\"InventorHelper.exe\",\"1\"",
            "Inventor.exe"
        ));
        assert!(!tasklist_has("", "Inventor.exe"));
    }

    #[test]
    fn only_inventor_is_asked_and_only_on_windows() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.sldprt");
        assert_eq!(
            open_in_running(&inst("solidworks", "2025"), &file, dir.path()),
            Reuse::NewInstance(String::new())
        );
        #[cfg(not(windows))]
        assert_eq!(
            open_in_running(&inst("inventor", "2026"), &file, dir.path()),
            Reuse::NewInstance(String::new())
        );
    }
}
