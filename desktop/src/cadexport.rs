//! STEP export of native CAD files through the CAD program installed on this
//! computer – the way the CAD add-ins of PDM systems create viewables at
//! check-in (findings in
//! `docs/src/research/cad-previews.md`). The server then shows the STEP in 3D.
//!
//! Windows only: Autodesk Inventor and Siemens Solid Edge are driven through
//! their documented COM interfaces by a short Windows PowerShell script
//! (`Documents.Open`, `SaveAs` with a `.stp` name). TeamCraft contains no
//! Autodesk or Siemens component – the user's own licence (student licences
//! included, within their terms) does the export. A document the user already
//! has open is exported as it is and left open. Not tested against real
//! Inventor or Solid Edge installations in the development environment.
//!
//! SolidCraft designs (`.sc`, `.solidcraft`) on every system: SolidCraft's
//! headless `solidcraft-cli run --in <design> --out <name>.stp` writes the
//! export (found through `SOLIDCRAFT_CLI`, `PATH` or the default install
//! folders, [`solidcraft_cli`]). A design whose timeline has a failing
//! feature is not exported – SolidCraft writes nothing then and says why.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Child,
    time::Duration,
};

use serde::Serialize;

/// Longest time an export may take (large assemblies).
pub const TIME_LIMIT: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CadProgram {
    Inventor,
    SolidEdge,
    #[serde(rename = "solidcraft")]
    SolidCraft,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl CadProgram {
    /// The programs driven through COM on Windows.
    pub const COM: [CadProgram; 2] = [CadProgram::Inventor, CadProgram::SolidEdge];

    /// COM class of the application (none for SolidCraft).
    pub fn prog_id(self) -> &'static str {
        match self {
            CadProgram::Inventor => "Inventor.Application",
            CadProgram::SolidEdge => "SolidEdge.Application",
            CadProgram::SolidCraft => "",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CadProgram::Inventor => "Autodesk Inventor",
            CadProgram::SolidEdge => "Siemens Solid Edge",
            CadProgram::SolidCraft => "SolidCraft",
        }
    }

    fn script(self) -> &'static str {
        match self {
            CadProgram::Inventor => INVENTOR_SCRIPT,
            CadProgram::SolidEdge => SOLID_EDGE_SCRIPT,
            CadProgram::SolidCraft => "",
        }
    }
}

const OLE_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

fn is_ole(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 8];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok()
        && head == OLE_MAGIC
}

/// A SolidCraft design rather than a script with the same extension: a ZIP
/// design container, or a JSON document with `"format": "solidcraft/N"`.
fn is_solidcraft_design(path: &Path) -> bool {
    use std::io::Read;
    let mut head = Vec::new();
    let read = std::fs::File::open(path).and_then(|f| f.take(1024).read_to_end(&mut head));
    if read.is_err() {
        return false;
    }
    let text = String::from_utf8_lossy(&head);
    head.starts_with(b"PK\x03\x04")
        || (text.trim_start().starts_with('{')
            && (text.contains("\"solidcraft/") || text.contains("\"solvecraft/")))
}

/// SolidCraft's command-line tool: `SOLIDCRAFT_CLI` (a path), else
/// `solidcraft-cli` on `PATH`, else the folders SolidCraft's installers use
/// (Windows `Program Files\SolidCraft`, the macOS app bundle, Linux
/// packages, `~/.local/bin` of the AppImage, the snap's `solidcraft.cli`).
pub fn solidcraft_cli() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    let home = env("HOME")
        .or_else(|| env("USERPROFILE"))
        .map(PathBuf::from);
    let candidates = cli_candidates(
        env("SOLIDCRAFT_CLI").map(PathBuf::from),
        env("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default(),
        home,
        env("ProgramFiles").map(PathBuf::from),
        env("LOCALAPPDATA").map(PathBuf::from),
    );
    candidates.into_iter().find(|p| p.is_file())
}

/// Where to look for `solidcraft-cli`, in order (see [`solidcraft_cli`]).
fn cli_candidates(
    explicit: Option<PathBuf>,
    path: Vec<PathBuf>,
    home: Option<PathBuf>,
    program_files: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
) -> Vec<PathBuf> {
    let exe = if cfg!(windows) {
        "solidcraft-cli.exe"
    } else {
        "solidcraft-cli"
    };
    let mut out: Vec<PathBuf> = explicit.into_iter().collect();
    out.extend(path.into_iter().map(|d| d.join(exe)));
    if cfg!(windows) {
        let pf = program_files.unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
        out.push(pf.join("SolidCraft").join(exe));
        if let Some(local) = local_app_data {
            out.push(local.join("Programs").join("SolidCraft").join(exe));
        }
    } else if cfg!(target_os = "macos") {
        out.push(PathBuf::from("/Applications/SolidCraft.app/Contents/MacOS").join(exe));
        if let Some(h) = &home {
            out.push(
                h.join("Applications/SolidCraft.app/Contents/MacOS")
                    .join(exe),
            );
        }
    } else {
        for dir in ["/usr/bin", "/usr/local/bin", "/opt/solidcraft"] {
            out.push(PathBuf::from(dir).join(exe));
        }
        if let Some(h) = &home {
            out.push(h.join(".local/bin").join(exe));
        }
        out.push(PathBuf::from("/snap/bin/solidcraft.cli"));
    }
    out
}

/// Waits for an export process within [`TIME_LIMIT`]; the first non-empty
/// line of its error output says why it failed.
fn wait_for(mut child: Child, program: CadProgram) -> Result<(), String> {
    // read the error output on the side, so a full pipe never blocks
    let reader = child.stderr.take().map(|mut err| {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut text = String::new();
            let _ = err.read_to_string(&mut text);
            text
        })
    });
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > TIME_LIMIT => {
                let _ = child.kill();
                return Err(format!(
                    "{} did not finish the export within {} minutes.",
                    program.label(),
                    TIME_LIMIT.as_secs() / 60
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(250)),
            Err(e) => return Err(format!("Export failed: {e}")),
        }
    };
    if status.success() {
        return Ok(());
    }
    let stderr = reader.and_then(|r| r.join().ok()).unwrap_or_default();
    let reason = stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no details");
    Err(format!(
        "{} could not export the file: {reason}",
        program.label()
    ))
}

/// `solidcraft-cli run --in <design> --out <output>` – no shell, no window.
fn run_solidcraft(cli: &Path, input: &Path, output: &Path) -> Result<(), String> {
    let args = [
        OsStr::new("run"),
        OsStr::new("--in"),
        input.as_os_str(),
        OsStr::new("--out"),
        output.as_os_str(),
    ];
    run_cli(cli, &args)
}

/// `solidcraft-cli run <script> --in <design> --save <design>`: runs a
/// command script on the design and saves it in place (SolidCraft keeps a
/// `.bak` of the previous file).
pub fn run_solidcraft_script(cli: &Path, script: &Path, design: &Path) -> Result<(), String> {
    let args = [
        OsStr::new("run"),
        script.as_os_str(),
        OsStr::new("--in"),
        design.as_os_str(),
        OsStr::new("--save"),
        design.as_os_str(),
    ];
    run_cli(cli, &args)
}

/// Runs `solidcraft-cli` with `args` – no shell, no window, no input – and
/// waits for it ([`wait_for`]).
fn run_cli(cli: &Path, args: &[&OsStr]) -> Result<(), String> {
    let mut command = std::process::Command::new(cli);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let child = command
        .spawn()
        .map_err(|e| format!("Cannot start {}: {e}", cli.display()))?;
    wait_for(child, CadProgram::SolidCraft)
}

/// The program that exports a file: Inventor parts and assemblies, Solid
/// Edge parts, sheet metal, weldments and assemblies (`.asm` only when it is
/// an OLE file – Creo assemblies use the same extension).
pub fn program_for(path: &Path) -> Option<CadProgram> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "ipt" | "iam" => Some(CadProgram::Inventor),
        "par" | "psm" | "pwd" => Some(CadProgram::SolidEdge),
        "asm" if is_ole(path) => Some(CadProgram::SolidEdge),
        "sc" | "solidcraft" if is_solidcraft_design(path) => Some(CadProgram::SolidCraft),
        _ => None,
    }
}

/// `bracket.ipt` → `bracket.stp` in the same folder.
pub fn step_path(input: &Path) -> PathBuf {
    input.with_extension("stp")
}

/// Opens the document invisibly (or uses it when it is already open), saves
/// a copy as STEP – Inventor picks the STEP translator by the extension.
const INVENTOR_SCRIPT: &str = r#"param([Parameter(Mandatory = $true)][string]$In, [Parameter(Mandatory = $true)][string]$Out)
$ErrorActionPreference = 'Stop'
$started = $false
try { $app = [Runtime.InteropServices.Marshal]::GetActiveObject('Inventor.Application') }
catch { $app = New-Object -ComObject 'Inventor.Application'; $started = $true }
$silent = $app.SilentOperation
$app.SilentOperation = $true
try {
    $doc = $null; $opened = $false
    foreach ($d in $app.Documents) { if ($d.FullFileName -ieq $In) { $doc = $d } }
    if ($null -eq $doc) { $doc = $app.Documents.Open($In, $false); $opened = $true }
    try { $doc.SaveAs($Out, $true) } finally { if ($opened) { $doc.Close($true) } }
} finally {
    $app.SilentOperation = $silent
    if ($started) { $app.Quit() }
}
"#;

/// The same with Solid Edge: `SaveAs` with a `.stp` name translates, like
/// *Save As → STEP* in the program.
const SOLID_EDGE_SCRIPT: &str = r#"param([Parameter(Mandatory = $true)][string]$In, [Parameter(Mandatory = $true)][string]$Out)
$ErrorActionPreference = 'Stop'
$started = $false
try { $app = [Runtime.InteropServices.Marshal]::GetActiveObject('SolidEdge.Application') }
catch { $app = New-Object -ComObject 'SolidEdge.Application'; $started = $true }
$alerts = $app.DisplayAlerts
$app.DisplayAlerts = $false
try {
    $doc = $null; $opened = $false
    foreach ($d in $app.Documents) { if ($d.FullName -ieq $In) { $doc = $d } }
    if ($null -eq $doc) { $doc = $app.Documents.Open($In); $opened = $true }
    try { $doc.SaveAs($Out) } finally { if ($opened) { $doc.Close($false) } }
} finally {
    $app.DisplayAlerts = $alerts
    if ($started) { $app.Quit() }
}
"#;

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{os::windows::process::CommandExt, process::Command};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Whether the COM class of a program is registered (it is installed).
    pub fn registered(program: CadProgram) -> bool {
        Command::new("reg")
            .args(["query", &format!("HKCR\\{}\\CLSID", program.prog_id())])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .is_ok_and(|o| o.status.success())
    }

    pub fn run(program: CadProgram, input: &Path, output: &Path) -> Result<(), String> {
        let script = std::env::temp_dir().join(format!(
            "teamcraft-step-export-{}-{}.ps1",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::write(&script, program.script())
            .map_err(|e| format!("Cannot write the export script: {e}"))?;
        let result = wait(program, &script, input, output);
        let _ = std::fs::remove_file(&script);
        result
    }

    fn wait(program: CadProgram, script: &Path, input: &Path, output: &Path) -> Result<(), String> {
        let child = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .arg("-In")
            .arg(input)
            .arg("-Out")
            .arg(output)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Cannot start Windows PowerShell: {e}"))?;
        wait_for(child, program)
    }
}

/// The CAD programs installed on this computer that can export STEP.
pub fn available() -> Vec<CadProgram> {
    #[cfg(windows)]
    let mut found: Vec<CadProgram> = CadProgram::COM
        .into_iter()
        .filter(|p| platform::registered(*p))
        .collect();
    #[cfg(not(windows))]
    let mut found: Vec<CadProgram> = Vec::new();
    if solidcraft_cli().is_some() {
        found.push(CadProgram::SolidCraft);
    }
    found
}

/// Exports `input` to a STEP file next to it with the installed CAD program;
/// returns the path of the STEP file.
pub fn export_step(input: &Path) -> Result<PathBuf, String> {
    let program = program_for(input).ok_or_else(|| {
        format!(
            "{} is not an Inventor, Solid Edge or SolidCraft part or assembly.",
            input.display()
        )
    })?;
    let output = step_path(input);
    if program == CadProgram::SolidCraft {
        let cli = solidcraft_cli().ok_or_else(|| {
            "SolidCraft's command-line tool (solidcraft-cli) was not found – install SolidCraft \
             or set SOLIDCRAFT_CLI to its path."
                .to_string()
        })?;
        run_solidcraft(&cli, input, &output)?;
        return if output.is_file() {
            Ok(output)
        } else {
            Err(format!(
                "SolidCraft finished, but {} was not written.",
                output.display()
            ))
        };
    }
    #[cfg(windows)]
    {
        platform::run(program, input, &output)?;
        if output.is_file() {
            Ok(output)
        } else {
            Err(format!(
                "{} finished, but {} was not written.",
                program.label(),
                output.display()
            ))
        }
    }
    #[cfg(not(windows))]
    {
        let _ = output;
        Err(format!(
            "The STEP export through {} works on Windows only.",
            program.label()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_and_names() {
        let dir = std::env::temp_dir().join(format!("teamcraft-cadexport-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let se_asm = dir.join("frame.asm");
        let mut ole = OLE_MAGIC.to_vec();
        ole.extend_from_slice(&[0; 504]);
        std::fs::write(&se_asm, &ole).unwrap();
        let creo_asm = dir.join("gearbox.asm");
        std::fs::write(&creo_asm, b"#UGC:2 ASSEMBLY").unwrap();
        assert_eq!(
            program_for(Path::new("/w/Bracket.IPT")),
            Some(CadProgram::Inventor)
        );
        assert_eq!(
            program_for(Path::new("/w/rover.iam")),
            Some(CadProgram::Inventor)
        );
        assert_eq!(
            program_for(Path::new("/w/plate.psm")),
            Some(CadProgram::SolidEdge)
        );
        assert_eq!(program_for(&se_asm), Some(CadProgram::SolidEdge));
        assert_eq!(program_for(&creo_asm), None, "Creo assembly");
        assert_eq!(program_for(Path::new("/w/hub.step")), None);
        assert_eq!(
            step_path(Path::new("/w/Bracket.ipt")),
            PathBuf::from("/w/Bracket.stp")
        );
        let _ = std::fs::remove_dir_all(&dir);
        // the scripts never close a document the user had open
        for script in [INVENTOR_SCRIPT, SOLID_EDGE_SCRIPT] {
            assert!(script.contains("if ($opened)") && script.contains("if ($started)"));
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn other_systems_explain_the_limit() {
        let found = available();
        assert!(!found.contains(&CadProgram::Inventor) && !found.contains(&CadProgram::SolidEdge));
        let err = export_step(Path::new("/w/Bracket.ipt")).unwrap_err();
        assert!(err.contains("Windows only"), "{err}");
    }

    #[test]
    fn solidcraft_designs_but_not_scripts() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("frame.sc");
        std::fs::write(&zip, b"PK\x03\x04rest").unwrap();
        let json = dir.path().join("plate.solidcraft");
        std::fs::write(&json, br#"{"format":"solidcraft/40","features":[]}"#).unwrap();
        let script = dir.path().join("build.sc");
        std::fs::write(&script, b"#!/bin/sh\necho hi\n").unwrap();
        assert_eq!(program_for(&zip), Some(CadProgram::SolidCraft));
        assert_eq!(program_for(&json), Some(CadProgram::SolidCraft));
        assert_eq!(program_for(&script), None, "a script named .sc");
        assert_eq!(program_for(&dir.path().join("missing.sc")), None);
    }

    #[test]
    fn the_cli_is_looked_for_in_order() {
        let c = cli_candidates(
            Some(PathBuf::from("/opt/x/solidcraft-cli")),
            vec![PathBuf::from("/p1"), PathBuf::from("/p2")],
            Some(PathBuf::from("/home/kim")),
            None,
            None,
        );
        assert_eq!(
            c[0],
            PathBuf::from("/opt/x/solidcraft-cli"),
            "SOLIDCRAFT_CLI first"
        );
        assert!(
            c[1].starts_with("/p1") && c[2].starts_with("/p2"),
            "then PATH: {c:?}"
        );
        assert!(c.len() > 3, "then the install folders");
        if cfg!(target_os = "linux") {
            assert!(c.contains(&PathBuf::from("/home/kim/.local/bin/solidcraft-cli")));
            assert!(c.contains(&PathBuf::from("/snap/bin/solidcraft.cli")));
        }
    }

    #[cfg(unix)]
    #[test]
    fn solidcraft_export_runs_the_cli_and_reports_its_error() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let fake = |name: &str, body: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        // run --in IN --out OUT
        let ok = fake(
            "ok-cli",
            r#"[ "$1" = run ] && [ "$2" = --in ] && [ "$4" = --out ] && echo step > "$5""#,
        );
        let failing = fake(
            "bad-cli",
            "echo '1 timeline feature(s) failed, so nothing was written:' >&2; exit 1",
        );
        let design = dir.path().join("plate.sc");
        std::fs::write(&design, b"PK\x03\x04").unwrap();
        let out = step_path(&design);
        run_solidcraft(&ok, &design, &out).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "step\n");
        let err = run_solidcraft(&failing, &design, &out).unwrap_err();
        assert!(
            err.starts_with("SolidCraft could not export the file: 1 timeline feature"),
            "{err}"
        );
        let err = run_solidcraft(&dir.path().join("none"), &design, &out).unwrap_err();
        assert!(err.contains("Cannot start"), "{err}");
    }
}
