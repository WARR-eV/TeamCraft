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
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use serde::Serialize;

/// Longest time an export may take (large assemblies).
pub const TIME_LIMIT: Duration = Duration::from_secs(15 * 60);

/// Set by *Skip the export* in the check-in dialog ([`cancel`]): the running
/// export is stopped (its process killed) and reports [`SKIPPED`]. Exports
/// run one after the other, so one flag serves them all; each export clears
/// it when it starts.
static CANCEL: AtomicBool = AtomicBool::new(false);

/// What a skipped export answers.
pub const SKIPPED: &str = "The export was skipped.";

/// Stops the running STEP export (#342) – the check-in goes on without it.
pub fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
}

/// Folder of the export logs inside the app's config directory: the whole
/// error output of a failed export, kept on this computer only (#342).
pub const LOG_DIR: &str = "export-logs";
/// Export logs kept at most (the oldest go first).
const MAX_LOGS: usize = 20;
/// Size of one log at most; the rest is cut.
const MAX_LOG_BYTES: usize = 64 * 1024;
static LOGS: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Makes `<config>/export-logs` the folder failed exports are logged to.
pub fn set_log_dir(config_dir: &Path) {
    let _ = LOGS.set(config_dir.join(LOG_DIR));
}

/// Writes the whole error output of a failed export into the log folder
/// (at most [`MAX_LOGS`] files, each cut at [`MAX_LOG_BYTES`]); returns
/// the log's path.
fn write_log(dir: &Path, program: CadProgram, input: &Path, output: &str) -> Option<PathBuf> {
    std::fs::create_dir_all(dir).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let name = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect::<String>();
    let path = dir.join(format!("{now}-{name}.log"));
    let mut text = format!(
        "TeamCraft Desktop {}\nProgram: {}\nFile: {}\n\n{}",
        env!("CARGO_PKG_VERSION"),
        program.label(),
        input.display(),
        output
    );
    if text.len() > MAX_LOG_BYTES {
        let mut cut = MAX_LOG_BYTES;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("\n[cut]\n");
    }
    std::fs::write(&path, text).ok()?;
    // keep the newest MAX_LOGS (the names start with the time)
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut logs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "log"))
            .collect();
        if logs.len() > MAX_LOGS {
            logs.sort_by_key(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.split('-').next())
                    .and_then(|t| t.parse::<u128>().ok())
                    .unwrap_or(0)
            });
            for old in &logs[..logs.len() - MAX_LOGS] {
                let _ = std::fs::remove_file(old);
            }
        }
    }
    Some(path)
}

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

/// The line an export script prints when it started the CAD program itself
/// (`teamcraft-started-pid:<pid>`): that process is ended too when the
/// export is skipped or runs out of time, so no invisible Inventor with
/// gigabytes of memory stays behind.
const STARTED_PID: &str = "teamcraft-started-pid:";

/// The process id of a [`STARTED_PID`] line.
fn started_pid(line: &str) -> Option<u32> {
    line.trim()
        .strip_prefix(STARTED_PID)
        .and_then(|n| n.trim().parse().ok())
        .filter(|&pid| pid > 4)
}

/// Ends the CAD program an export script started (Windows: `taskkill`).
fn kill_started(pid: Option<u32>) {
    #[cfg(windows)]
    {
        if let Some(pid) = pid {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .creation_flags(0x0800_0000)
                .output();
        }
    }
    #[cfg(not(windows))]
    let _ = pid;
}

/// How an export process ended.
struct Ended {
    result: Result<(), String>,
    /// The whole error output (for the export log).
    stderr: String,
}

/// Waits for an export process within [`TIME_LIMIT`] – or until `cancel`
/// is set; the first non-empty line of its error output says why it failed.
fn wait_for(mut child: Child, program: CadProgram, cancel: Option<&AtomicBool>) -> Ended {
    // read both outputs on the side, so a full pipe never blocks
    let reader = child.stderr.take().map(|mut err| {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut text = String::new();
            let _ = err.read_to_string(&mut text);
            text
        })
    });
    let started_by_script = std::sync::Arc::new(std::sync::Mutex::new(None::<u32>));
    if let Some(out) = child.stdout.take() {
        let seen = started_by_script.clone();
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(pid) = started_pid(&line) {
                    *seen.lock().expect("pid lock") = Some(pid);
                }
            }
        });
    }
    let stop = |child: &mut Child, why: String| {
        let _ = child.kill();
        let _ = child.wait();
        kill_started(*started_by_script.lock().expect("pid lock"));
        Ended {
            result: Err(why),
            stderr: String::new(),
        }
    };
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) => {
                return stop(&mut child, SKIPPED.to_string());
            }
            Ok(None) if started.elapsed() > TIME_LIMIT => {
                let why = format!(
                    "{} did not finish the export within {} minutes.",
                    program.label(),
                    TIME_LIMIT.as_secs() / 60
                );
                return stop(&mut child, why);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(250)),
            Err(e) => {
                return Ended {
                    result: Err(format!("Export failed: {e}")),
                    stderr: String::new(),
                }
            }
        }
    };
    let stderr = reader.and_then(|r| r.join().ok()).unwrap_or_default();
    if status.success() {
        return Ended {
            result: Ok(()),
            stderr,
        };
    }
    let reason = stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no details")
        .to_string();
    Ended {
        result: Err(format!(
            "{} could not export the file: {reason}",
            program.label()
        )),
        stderr,
    }
}

/// The result of an export, its error output logged when it failed
/// (`<config>/export-logs`) and the log named in the error.
fn logged(ended: Ended, program: CadProgram, input: &Path) -> Result<(), String> {
    let Err(why) = ended.result else {
        return Ok(());
    };
    if why == SKIPPED {
        return Err(why);
    }
    let text = if ended.stderr.trim().is_empty() {
        why.clone()
    } else {
        ended.stderr
    };
    match LOGS
        .get()
        .and_then(|dir| write_log(dir, program, input, &text))
    {
        Some(log) => Err(format!("{why} (log: {})", log.display())),
        None => Err(why),
    }
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
    logged(
        run_cli(cli, &args, Some(&CANCEL))?,
        CadProgram::SolidCraft,
        input,
    )
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
    run_cli(cli, &args, None)?.result
}

/// Runs `solidcraft-cli` with `args` – no shell, no window, no input – and
/// waits for it ([`wait_for`]); `Err` when it cannot be started.
fn run_cli(cli: &Path, args: &[&OsStr], cancel: Option<&AtomicBool>) -> Result<Ended, String> {
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
    Ok(wait_for(child, CadProgram::SolidCraft, cancel))
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
/// A COM call Inventor rejects because it is busy (`RPC_E_CALL_REJECTED`,
/// `RPC_E_SERVERCALL_RETRYLATER` – the user works in it) is tried again for
/// up to a minute (#342); an Inventor the script starts is named on stdout
/// ([`STARTED_PID`]), so a skipped export can end it.
const INVENTOR_SCRIPT: &str = r#"param([Parameter(Mandatory = $true)][string]$In, [Parameter(Mandatory = $true)][string]$Out)
$ErrorActionPreference = 'Stop'
function Invoke-Com([scriptblock]$Call) {
    for ($i = 0; ; $i++) {
        try { return (& $Call) }
        catch {
            $e = $_.Exception; while ($e.InnerException) { $e = $e.InnerException }
            if ($i -ge 120 -or ($e.HResult -ne -2147418111 -and $e.HResult -ne -2147417846)) { throw }
            Start-Sleep -Milliseconds 500
        }
    }
}
$started = $false
try { $app = [Runtime.InteropServices.Marshal]::GetActiveObject('Inventor.Application') }
catch {
    $before = @(Get-Process -Name 'Inventor' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $app = New-Object -ComObject 'Inventor.Application'; $started = $true
    Get-Process -Name 'Inventor' -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id } | ForEach-Object { "teamcraft-started-pid:$($_.Id)" }
}
$silent = Invoke-Com { $app.SilentOperation }
Invoke-Com { $app.SilentOperation = $true }
try {
    $doc = $null; $opened = $false
    foreach ($d in (Invoke-Com { $app.Documents })) { if ($d.FullFileName -ieq $In) { $doc = $d } }
    if ($null -eq $doc) { $doc = Invoke-Com { $app.Documents.Open($In, $false) }; $opened = $true }
    try { Invoke-Com { $doc.SaveAs($Out, $true) } } finally { if ($opened) { Invoke-Com { $doc.Close($true) } } }
} finally {
    Invoke-Com { $app.SilentOperation = $silent }
    if ($started) { $app.Quit() }
}
"#;

/// The same with Solid Edge: `SaveAs` with a `.stp` name translates, like
/// *Save As → STEP* in the program.
const SOLID_EDGE_SCRIPT: &str = r#"param([Parameter(Mandatory = $true)][string]$In, [Parameter(Mandatory = $true)][string]$Out)
$ErrorActionPreference = 'Stop'
function Invoke-Com([scriptblock]$Call) {
    for ($i = 0; ; $i++) {
        try { return (& $Call) }
        catch {
            $e = $_.Exception; while ($e.InnerException) { $e = $e.InnerException }
            if ($i -ge 120 -or ($e.HResult -ne -2147418111 -and $e.HResult -ne -2147417846)) { throw }
            Start-Sleep -Milliseconds 500
        }
    }
}
$started = $false
try { $app = [Runtime.InteropServices.Marshal]::GetActiveObject('SolidEdge.Application') }
catch {
    $before = @(Get-Process -Name 'Edge' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $app = New-Object -ComObject 'SolidEdge.Application'; $started = $true
    Get-Process -Name 'Edge' -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id } | ForEach-Object { "teamcraft-started-pid:$($_.Id)" }
}
$alerts = Invoke-Com { $app.DisplayAlerts }
Invoke-Com { $app.DisplayAlerts = $false }
try {
    $doc = $null; $opened = $false
    foreach ($d in (Invoke-Com { $app.Documents })) { if ($d.FullName -ieq $In) { $doc = $d } }
    if ($null -eq $doc) { $doc = Invoke-Com { $app.Documents.Open($In) }; $opened = $true }
    try { Invoke-Com { $doc.SaveAs($Out) } } finally { if ($opened) { Invoke-Com { $doc.Close($false) } } }
} finally {
    Invoke-Com { $app.DisplayAlerts = $alerts }
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
            // stdout names the CAD program the script started (STARTED_PID)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Cannot start Windows PowerShell: {e}"))?;
        logged(wait_for(child, program, Some(&CANCEL)), program, input)
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
    CANCEL.store(false, Ordering::Relaxed);
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

#[cfg(test)]
mod skip_and_log_tests {
    use super::*;

    #[test]
    fn the_started_program_is_read_from_its_line() {
        assert_eq!(started_pid("teamcraft-started-pid:4711"), Some(4711));
        assert_eq!(started_pid("  teamcraft-started-pid: 12 \r"), Some(12));
        assert_eq!(
            started_pid("teamcraft-started-pid:4"),
            None,
            "never a system process"
        );
        assert_eq!(started_pid("teamcraft-started-pid:x"), None);
        assert_eq!(started_pid("4711"), None);
        // the scripts name the program they start and retry rejected calls
        for script in [INVENTOR_SCRIPT, SOLID_EDGE_SCRIPT] {
            assert!(script.contains(STARTED_PID));
            assert!(script.contains("-2147418111") && script.contains("-2147417846"));
            assert!(script.contains("Invoke-Com { $doc.SaveAs("));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_skipped_export_stops_its_process_at_once() {
        let child = std::process::Command::new("sh")
            .args(["-c", "sleep 30"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let cancel = AtomicBool::new(false);
        let started = std::time::Instant::now();
        std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(300));
                cancel.store(true, Ordering::Relaxed);
            });
            let ended = wait_for(child, CadProgram::SolidCraft, Some(&cancel));
            assert_eq!(ended.result.unwrap_err(), SKIPPED);
        });
        assert!(started.elapsed() < Duration::from_secs(10));
        // a skipped export is not logged as a failure
        let skipped = Ended {
            result: Err(SKIPPED.into()),
            stderr: String::new(),
        };
        assert_eq!(
            logged(skipped, CadProgram::Inventor, Path::new("a.iam")).unwrap_err(),
            SKIPPED
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_whole_error_output_is_kept() {
        let child = std::process::Command::new("sh")
            .args([
                "-c",
                "echo 'first line' >&2; echo 'At line:12 char:5' >&2; exit 1",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let ended = wait_for(child, CadProgram::Inventor, None);
        assert_eq!(
            ended.result.as_ref().unwrap_err(),
            "Autodesk Inventor could not export the file: first line"
        );
        assert!(ended.stderr.contains("At line:12"), "{}", ended.stderr);
    }

    #[test]
    fn export_logs_are_cut_and_the_oldest_go() {
        let dir = tempfile::tempdir().unwrap();
        let big = "x".repeat(MAX_LOG_BYTES * 2);
        let first = write_log(
            dir.path(),
            CadProgram::Inventor,
            Path::new("/w/Rover top.iam"),
            &big,
        )
        .unwrap();
        let name = first.file_name().unwrap().to_str().unwrap();
        assert!(name.ends_with("-Rover_top.iam.log"), "{name}");
        let text = std::fs::read_to_string(&first).unwrap();
        assert!(text.len() <= MAX_LOG_BYTES + 10 && text.ends_with("[cut]\n"));
        assert!(text.contains("Program: Autodesk Inventor"));
        for i in 0..MAX_LOGS + 5 {
            std::thread::sleep(Duration::from_millis(2));
            write_log(
                dir.path(),
                CadProgram::Inventor,
                Path::new(&format!("p{i}.ipt")),
                "boom",
            )
            .unwrap();
        }
        let left = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(left, MAX_LOGS);
        assert!(!first.exists(), "the oldest log went first");
    }
}
