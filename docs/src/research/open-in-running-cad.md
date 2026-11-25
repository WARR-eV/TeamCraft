# Opening a file in the running CAD program

**Question:** when the user clicks *Check out and open* (or *Open in CAD*) on
an Inventor file in TeamCraft Desktop on Windows while Inventor already runs,
how does the file get into that Inventor – reliably, with its window in
front – instead of a second instance? What can fail on a real machine, and
what should `desktop/src/cadlaunch.rs` change? Issues #350 and #483.
Investigated on 2026-11-25.

Only public documentation was used: Microsoft's Windows, COM and PowerShell
documentation, the Autodesk Inventor API help, Autodesk developer blogs and
forums, reseller support articles and the Tauri sources. No program was
decompiled or analysed (ADR 0005, CONTRIBUTING.md *Your own work only*).
Facts marked *(verify)* come from community reports or are inferences, and
must be checked on a Windows machine with Inventor (see *Manual test*).

## Summary

* Three documented mechanisms reach a running Inventor: **COM through the
  running object table** (ROT) – what TeamCraft does –, the **shell file
  association** (Inventor registers a DDE `open` command, which is why a
  double-click in Explorer usually goes to the running instance) and an
  **add-in inside Inventor**. `Inventor.exe <file>` always starts another
  instance.
* The ROT is partitioned by user token, integrity level and window station.
  An Inventor started as administrator, a TeamCraft started as
  administrator, or an Inventor in another Windows session cannot be reached
  by COM. Tauri's installer starts the app de-elevated, so an elevated
  TeamCraft is rare – an elevated Inventor is the likelier case.
* With several Inventor processes only one `Inventor.Application` entry is
  usable (in practice the oldest instance). Inventor 2024 and later no longer
  register open documents in the ROT *(verify)*, so enumerating the ROT cannot
  pick a particular instance – only an add-in can.
* The most likely reason for "it doesn't work" **after** the COM call worked
  is **the window**: Windows lets only the foreground process – or a process
  it allowed – bring a window to the front; otherwise the taskbar button
  flashes. The script's `WScript.Shell.AppActivate` runs in a windowless
  background PowerShell, does not promise to restore a minimized window and
  runs before the document opens. At the click TeamCraft itself is the
  foreground process: it should allow Inventor (`AllowSetForegroundWindow`)
  and raise Inventor's main window (`Application.MainFrameHWND`) itself.
* Managed computers can stop the PowerShell route: an execution policy set
  by Group Policy overrides `-ExecutionPolicy Bypass`, and under AppLocker or
  WDAC a script in `%TEMP%` runs in ConstrainedLanguage mode, where
  `[Runtime.InteropServices.Marshal]::GetActiveObject` and `New-Object
  -ComObject` are not allowed. Today both end in "none" (a silent second
  instance) or in the misleading note "started as administrator".
* A second instance started because another project is active most likely
  starts with **the same** project – the last one activated as default –, so
  it gains nothing *(verify)*. When the running Inventor has no document
  open, the working folder's project can be activated through the API
  (`DesignProject.Activate`) and the file opened there.
* `Marshal.GetActiveObject` exists in .NET Framework (Windows PowerShell
  5.1, which TeamCraft runs), not in .NET 5+ (PowerShell 7); the replacement
  is P/Invoke to `CLSIDFromProgID` and `GetActiveObject` in `oleaut32`.
  Inventor 2025's move to .NET 8 concerns in-process add-ins, not
  out-of-process COM clients.

The ranked recommendations are in section 7.

## What TeamCraft does today

`workspace_open_cad` (`desktop/src/commands.rs`) chooses the program
(`cadlaunch::plan`) and calls `cadlaunch::open_in_running`. For Inventor on
Windows, `running::ask` runs `tasklist` and, when an `Inventor.exe` runs,
writes `INVENTOR_OPEN_SCRIPT` to `%TEMP%` and starts `powershell.exe
-NoProfile -NonInteractive -ExecutionPolicy Bypass -File …` without a
window. The script calls `GetActiveObject('Inventor.Application')`; when that
fails it looks for a visible Inventor of the user's own session (`unreachable`)
or answers `none`. Otherwise it checks `Visible`, `InstallPath` (another
installation only when the group pins no version) and, when the working
folder has an `.ipj`, `ActiveDesignProject.FullFileName`; busy calls
(`RPC_E_CALL_REJECTED`, `RPC_E_SERVERCALL_RETRYLATER`) are asked again. It
answers `opening` before `Documents.Open` (a large assembly loads for
minutes), calls `WScript.Shell.AppActivate(<pid>)` and then opens or
activates the document; a later failure is reported as `failed <reason>`.
Every other answer starts `Inventor.exe <file>` with a note. Not yet run on a
real Windows machine with Inventor.

## 1. Getting a running COM automation server

* **Registration.** A program lists its application object as the active
  object of its class with `RegisterActiveObject`, which puts it into the ROT
  under the class's CLSID; `ACTIVEOBJECT_STRONG` (default) holds a reference,
  `ACTIVEOBJECT_WEAK` does not. For visible programs Microsoft recommends a
  weak registration plus `CoLockObjectExternal` while the program is visible,
  and `RevokeActiveObject` at shutdown [M4].
* **Lookup.** `GetActiveObject(clsid)` in `oleaut32` returns that object;
  .NET's `Marshal.GetActiveObject(progId)` turns the ProgID into a CLSID first
  [M10].
* **When a program registers** is up to the program. Office registers only
  after it first loses the focus, so `GetActiveObject` fails right after
  Office started; Microsoft's workaround is to retry about every half second,
  up to 20 times [M9]. Inventor documents no registration time; it documents
  `Application.Ready` and `ApplicationEvents.OnReady` for "initialization
  complete, including the add-ins loaded at startup" [A1, A2]. An Inventor in
  the process list that is not (yet) in the ROT is therefore most likely still
  starting – a reason to retry, not to give up at once.
* **Several instances.** Registering the same moniker again adds another ROT
  entry (`MK_S_MONIKERALREADYREGISTERED`); which object a later lookup
  returns is not defined [M5]. For Inventor, Autodesk forum answers report
  that all instances use the same moniker and `GetActiveObject` returns the
  first (oldest) instance [F1]. When *creating* an object, the ProgID
  `Inventor.Application` resolves to the last run or installed version [F4].
  Whether all Inventor versions share one CLSID is not documented
  *(verify: `reg query HKCR\Inventor.Application\CLSID`)*; because TeamCraft
  compares `InstallPath`, it does not matter which version answers.
* **Finding a specific instance.** `GetRunningObjectTable` →
  `IRunningObjectTable::EnumRunning` lists the monikers. With two Inventors
  there are two application entries, but both bind to the same instance [F1].
  Inventor used to register its open documents under file monikers, which let
  a client find the instance holding a given file; a forum report says this
  stopped with Inventor 2024, and the community workaround is an add-in that
  registers a per-session moniker [F1] *(verify)*. The ROT carries no process
  id; the documented way from an `Application` object to its process is
  `Application.MainFrameHWND` [A3] and `GetWindowThreadProcessId`.

## 2. Integrity levels, elevation and Windows sessions

* **Window stations and sessions.** Without `ROTFLAGS_ALLOWANYCLIENT` only
  clients in the window station that registered an object can connect to it
  [M5]. Every Windows session (Remote Desktop, fast user switching) has its
  own interactive window station, so an Inventor of another session is never
  reachable – and its window cannot be brought to the front. The script's
  `SessionId` filter matches this.
* **Elevation.** `ROTFLAGS_ALLOWANYCLIENT` is for servers registered as
  LocalService or RunAs; an "activate as activator" server – a normal desktop
  program such as Inventor – may use it only through the machine-wide
  `ROTFlags` value of its AppID [M5, M7]. Microsoft states that a class an
  elevated server registers "will not be available to non-elevated clients"
  [M6], and the Windows Vista compatibility team describes the ROT as
  partitioned by user token and then by integrity level [M8] – so the lookup
  fails in both directions. Inventor developers report the same: with
  Inventor *or* the client run as administrator, `GetActiveObject` fails;
  with both at the same level it works [F2]. Setting `ROTFlags` on
  Inventor's AppID would change a vendor registration machine-wide and open
  the entry to every client – not something TeamCraft should do.
* **TeamCraft started elevated.** The PowerShell child inherits the token and
  cannot see a normal Inventor. TeamCraft's installer is per user
  (`desktop/tauri.conf.json`, NSIS `installMode: currentUser`), and Tauri's
  NSIS template starts the app after installation and after an update (`/R`)
  through `nsis_tauri_utils::RunAsUser`, which uses the desktop shell's token
  when the installer itself is elevated [T1, T2]. An elevated TeamCraft
  therefore comes from a shortcut or compatibility setting "Run as
  administrator", not from the installer.
* **Telling the cases apart.** A visible Inventor in the user's session plus
  `MK_E_UNAVAILABLE` (`0x800401E3`) from `GetActiveObject` means: still
  starting, another integrity level, or PowerShell restricted (section 5).
  Own elevation: under UAC, `WindowsPrincipal.IsInRole(Administrator)` is true
  only with an elevated token. Inventor's elevation: Windows PowerShell 5.1's
  `Process.Path` reads the main module, which a normal process cannot do for
  an elevated one, so it is empty *(verify)*; from Rust, `OpenProcessToken`
  on an elevated process fails with access denied for a normal caller
  *(verify)*.

## 3. Bringing Inventor's window to the front

* **The rules** [M1]: a process may set the foreground window only if it is a
  desktop app, the foreground process has not locked it
  (`LockSetForegroundWindow`) and no menu is active, **and** one of: the
  foreground lock timeout has expired, it *is* the foreground process, it was
  *started by* the foreground process, there is no foreground window, it
  received the last input event, or a debugger is involved. "It is possible
  for a process to be denied the right … even if it meets these conditions."
  When denied, Windows flashes the window's taskbar button instead.
* **Passing the right on.** A process that may set the foreground window can
  allow another one with `AllowSetForegroundWindow(pid)` (or `ASFW_ANY`); the
  permission ends with the next user input not directed at that process, or
  when another process is allowed [M2]. `CoAllowSetForegroundWindow` does the
  same for a COM server through its proxy [M3]. "You can't steal it, it has
  to be given to you" [M20]. The foreground lock timeout defaults to
  200 000 ms [M18], so "timeout expired" rarely helps right after the user
  clicked.
* **Who qualifies in TeamCraft's flow.** At the click, **TeamCraft is the
  foreground process**. The PowerShell child qualifies only through "started
  by the foreground process", runs without a window and may still be denied
  [M1]. Inventor does not qualify at all unless TeamCraft allows it.
  `WScript.Shell.AppActivate` accepts a process id and returns `False` when it
  fails [M17]; the Visual Basic `AppActivate` function is documented as not
  changing whether a window is minimized or maximized [M17], and the Windows
  Script Host page does not promise to restore one either – a minimized
  Inventor stays minimized *(verify)*.
* **What the user sees.** The file opens in an Inventor that stays behind
  TeamCraft or minimized, its taskbar button flashing, while TeamCraft's
  message says it opened in the running Inventor – which looks exactly like
  "it didn't work". A dialog Inventor shows while opening (unresolved
  references, file migration) is hidden the same way, and `Documents.Open`
  waits for it.
* **The fix** belongs in the desktop app, the process that holds the right:
  allow Inventor's process (`AllowSetForegroundWindow`, so its own dialogs can
  come forward), restore its main window when it is minimized (`IsIconic`,
  `ShowWindow(SW_RESTORE)`) and call `SetForegroundWindow` on it.
  `MainFrameHWND` is a 32-bit `Long`; window handles are 32-bit significant
  on 64-bit Windows, so it is sign-extended to a native handle [M19].

## 4. The shell file association

* **DDE verbs** [M14]: a verb with a `ddeexec` key makes the shell send its
  default value as a DDE command; `application` (default: the program of the
  verb's `command`) and `topic` (default `System`) name the conversation.
  When no DDE server answers, the shell starts the `command` and then sends
  `ifexec` (or the `ddeexec` command again). Microsoft deprecates DDE for
  verbs in favour of the DropTarget method [M15].
* **Inventor's registration**, as reseller support tips describe it: the
  document classes (`Inventor.PartDocument`, `Inventor.AssemblyDocument`, …)
  have `shell\open\command` = `"…\Bin\Inventor.exe" /dde` and
  `shell\open\ddeexec` = `[open("%1")]` [R2] *(verify with `reg query`)*. So
  opening an `.ipt` through the association hands the file to a running
  Inventor – which is why a double-click in Explorer usually opens it there
  [F3], while `Inventor.exe <file>` starts another instance. Users report a
  second instance when Inventor is busy, and after updates that broke the
  association; the reseller fix is *Open with … → Always use this app* [F3,
  R1]. DDE is window messaging and does not reach a program running at a
  higher integrity level; "There was a problem sending the command to the
  program" is the typical result [M21] *(verify for Inventor)*.
* **Several versions.** The conversation's application name defaults to the
  program name – `Inventor` for every version –, so any running Inventor
  answers, whichever version owns the association (inference from [M14] and
  [R2], *verify*). With none running, the association's version starts: the
  last installed or repaired one, or the user's *Open with* choice [R1] – not
  the project group's version.
* **TeamCraft's "Default app" route** is `tauri-plugin-opener` → the `open`
  crate → `ShellExecuteExW` with the default verb, no flags, on the calling
  thread [T3]. For DDE, `SEE_MASK_NOASYNC` makes `ShellExecuteEx` wait for the
  DDE part, and COM should be initialized (single-threaded apartment) [M16].
* **A valid alternative?** It is simpler and behaves like a double-click, but
  TeamCraft gets no answer (it cannot tell reuse from a new instance, nor
  report a failure), cannot apply the version or project rules, has the same
  elevation and session limits, and depends on registry state that users and
  IT change. Not the primary route – a fallback for when the COM route cannot
  run (PowerShell restricted) and the group pins no version.

## 5. Inventor specifics

* **Startup state:** `Application.Ready` (since version 9) is true once
  Inventor and its startup add-ins are initialized; `OnReady` notifies it
  [A1, A2]. `Visible` tells the user's instance from an invisible automation
  one (TeamCraft's own STEP export starts one with `New-Object -ComObject`
  when no Inventor runs, `cadexport.rs`).
* **Which installation:** `Application.InstallPath` [A4];
  `SoftwareVersion.DisplayVersion` gives the release year users know [A4] –
  better for notes than the folder name.
* **Window:** `Application.MainFrameHWND`, the main window's handle (since
  version 4) [A3].
* **Opening:** `Documents.Open(FullDocumentName, OpenVisible)` opens the
  master document; `Documents.OpenWithOptions(name, options, visible)` takes a
  `NameValueMap` (`DesignViewRepresentation`, `SkipAllUnresolvedFiles`,
  `DeferFlatPatternUpdate`, …) [A5] – not needed for a plain open.
  `Documents.VisibleDocuments` are the documents with a window,
  `Documents.Count` all documents in memory [A5].
* **Projects:** `DesignProjectManager.ActiveDesignProject` [A6].
  `DesignProject.Activate(SetAsDefaultProject)` requires all documents to be
  closed; the argument defaults to true and makes the project the default "for
  the next session of Inventor" [A6]. Autodesk's sample exits when
  `Documents.Count > 0`; projects not yet listed are added with
  `DesignProjects.AddExisting` [A6]. The help says to close all files before
  changing the active project, and that the *Default* project is active when
  none was chosen [A7]. So a **new instance starts with the default project –
  normally the one the running instance activated last**; starting a second
  instance because the running one has another project active does not give
  the working folder's project *(verify)*.
* **Command line:** no documented `Inventor.exe` switch selects a project;
  `Inventor.exe /?` lists the startup options in a message box [F5]. Inventor
  View documents `/Project` [A9]. Since Inventor 2020.3 the `.ipj`
  association runs `Bin\ipj.exe "%1"`, and a double-click on an `.ipj` starts
  a session with that project [F6]; whether it switches a running Inventor is
  *verify*. `Inventor.exe <file>` with Inventor running starts another
  instance (#350).
* **.NET:** `Marshal.GetActiveObject` is .NET Framework only and missing in
  .NET 5 and later; Microsoft's replacement is P/Invoke of `GetActiveObject`
  (after `CLSIDFromProgID`) [M10], and Autodesk's developer blog gives the
  same for AutoCAD 2025 on .NET 8 [A8]. Windows PowerShell 5.1 is built on
  .NET Framework 4.5+, PowerShell 7 on .NET [M11] – TeamCraft must keep
  calling `powershell.exe`, never `pwsh`. Inventor 2025 moved its **add-ins**
  to .NET 8 ([CAD add-ins](cad-addins.md), A2/A3 there); nothing documents a
  change for out-of-process COM clients *(verify with 2025–2027)*.
* **PowerShell on managed computers:** a Group Policy execution policy
  "overrides the execution policies set in PowerShell in all scopes", the
  `-ExecutionPolicy` parameter included [M12]. Under AppLocker or WDAC,
  scripts the policy does not trust run in ConstrainedLanguage mode: only
  `Scripting.Dictionary`, `Scripting.FileSystemObject` and `VBScript.RegExp`
  as COM objects, only allowed .NET types [M13] – `GetActiveObject` and
  `WScript.Shell` fail. `$ExecutionContext.SessionState.LanguageMode` tells
  the mode [M13].

## 6. Patterns for reaching the CAD session

These are the mechanisms the platform and the CAD vendor document; TeamCraft's
assessment follows. (The in-CAD add-in has its own page,
[CAD add-ins](cad-addins.md).)

| Pattern | How | For | Against |
|---|---|---|---|
| **Add-in inside the CAD program** | the vendor's add-in API (Inventor `ApplicationAddInServer`, `addInSite.Application` is the instance it lives in); the desktop app asks it over local IPC to open a file | exactly the right instance: each running Inventor's add-in announces version, process and active project; no ROT, no PowerShell; can raise its own window once the desktop app allowed its process; can switch projects when no document is open | must be installed and kept up to date per version (.NET 8 build); does nothing where it is not installed |
| **Out-of-process client through the ROT** | `GetActiveObject` + COM calls (TeamCraft today) | nothing to install; full API (version, project, open, window handle) | one usable entry per class, same user, integrity level and session only; busy calls; foreground rules; PowerShell policies |
| **Shell association** | `ShellExecuteEx` on the file (Inventor: DDE `[open("%1")]`) | simplest; behaves like a double-click | no answer, no version or project rule, DDE fragile (busy → second instance), deprecated, depends on the association state |
| **Start the program with the file** | `Inventor.exe <file>` | always works when installed | always a new instance – the fallback only |

For TeamCraft: keep the ROT client as the main route (no installation),
make it robust (section 7), use the shell association only as a narrow
fallback, and let the Inventor add-in MVP take over where it is installed –
its protocol (`teamcraft.cad` v1 in [CAD add-ins](cad-addins.md)) needs an
`open` message from the desktop app to the add-in for that.

## 7. Recommendations (ranked)

1. **Raise Inventor's window from the desktop app.** The script asks
   `MainFrameHWND` (inside the guarded `try`, with the deadline) and prints it
   on its own line right after `opening`; the reader thread in `running::ask`
   – part of the foreground process – checks that the handle belongs to an
   `Inventor.exe` process (`GetWindowThreadProcessId`, process ids from the
   `tasklist` answer), calls `AllowSetForegroundWindow(pid)`, restores a
   minimized window and calls `SetForegroundWindow`. The `AppActivate` block
   goes. `AllowSetForegroundWindow` for a single running Inventor already
   before the script starts lets Inventor's own dialogs come forward. Plain
   `user32` declarations, no new crate.
2. **Say why, precisely.** `unreachable` gets a reason: `constrained`
   (language mode not `FullLanguage`, checked first), `self-elevated`
   (`IsInRole(Administrator)`), `elevated` (Inventor's process not readable),
   `starting` (not in the ROT or not `Ready` by the deadline), `busy` (calls
   rejected until the deadline) or `rot <HRESULT>`; `running_note` names the
   real cause. A script that ends without an answer (execution policy) reads
   its bounded stderr and says "TeamCraft could not ask Inventor: …" instead of
   starting a second instance silently.
3. **Wait for a starting Inventor.** When a visible Inventor of the session is
   not in the ROT, ask `GetActiveObject` again every 500 ms until the deadline
   (the pattern of [M9]); after it answers, wait for `Ready` [A1].
4. **Do not start a second instance behind a busy or starting one.** For
   `busy` and `starting`, raise the running window (the user sees the dialog
   that blocks it) and show the note with an *Open in a new instance* action,
   instead of starting one at once. (A product decision: today every
   `unreachable` starts a new instance.)
5. **Project:** when the active project differs and the running Inventor has
   no document in memory (`Documents.Count = 0`), activate the working
   folder's `.ipj` (`DesignProjects` item with that `FullFileName`, else
   `AddExisting`, then `Activate(True)` [A6]) after `opening`, then open the
   file. With documents open, a new instance most likely gets the same project
   (section 5), so either open in the running instance with a note ("Inventor's
   active project is X, not Y – close all files to switch") or keep the new
   instance – to decide after the manual test confirms the default-project
   behaviour.
6. **Several Inventor processes:** map the answering instance to its process
   (`MainFrameHWND` → process id). When more than one visible Inventor runs in
   the session and the answering one is not the wanted installation, say that
   TeamCraft can reach only the first one started [F1] instead of "another
   version runs".
7. **Shell association as a narrow fallback:** only when the COM route cannot
   run (`constrained`, or the script did not run) and a visible Inventor runs
   in the session and the group pins no version: `ShellExecuteExW` with
   `SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI` on a thread with an STA [M16],
   after checking that the association's program is an `Inventor.exe`
   (`AssocQueryStringW`). The note says that the file was handed to Windows.
8. **Later:** call COM directly from Rust (the `windows` crate is already in
   the desktop lock file through Tauri, MIT/Apache-2.0): `CLSIDFromProgID`,
   `GetActiveObject`, `IDispatch`, the same retries – no PowerShell start, no
   execution policy, no language mode, and `CoAllowSetForegroundWindow`
   becomes possible; shared with `cadexport.rs`. Where the Inventor add-in is
   installed, it opens the file instead.

## Manual test on a real Windows machine

Facts only a machine with Inventor can confirm – recorded in the issue with
Windows and Inventor version. The release checklist has the short form of
steps 2–4 ([Release checklist](../dev/release-checklist.md), *a running
Inventor is reused*).

1. A diagnostic in a normal PowerShell 5.1 window while Inventor runs:

   ```powershell
   $ExecutionContext.SessionState.LanguageMode
   Get-ExecutionPolicy -List
   Get-Process Inventor | Select-Object Id, SessionId, MainWindowHandle, Path
   try {
       $a = [Runtime.InteropServices.Marshal]::GetActiveObject('Inventor.Application')
       $a.InstallPath; $a.SoftwareVersion.DisplayVersion; $a.Visible; $a.Ready; $a.MainFrameHWND
       $a.DesignProjectManager.ActiveDesignProject.FullFileName; $a.Documents.Count
   } catch { $e = $_.Exception; while ($e.InnerException) { $e = $e.InnerException }; '{0:X8} {1}' -f $e.HResult, $e.Message }
   reg query HKCR\.ipt
   reg query HKCR\Inventor.Application\CLSID
   reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.ipt\UserChoice
   ```

   and `reg query "HKCR\<ProgID from .ipt>\shell\open" /s` for the association
   and its `ddeexec`.
2. *Check out and open* with Inventor: not running; idle with no document;
   behind TeamCraft; minimized; with a modal dialog open; within seconds of
   starting it; with the file already open; with the file loaded only as a
   reference of an open assembly. Each time: did the file open in the running
   instance, did its window come to the front, what did the message say.
3. Inventor run as administrator; TeamCraft run as administrator; Inventor in
   another user's session.
4. Two Inventor versions installed, the older running, group without and with
   a pinned version; two instances of the same version.
5. Working folder with an `.ipj`: running Inventor with that project, with
   another project and no document, with another project and documents open.
   Close Inventor, start it again: which project is active (the default-project
   question of section 5)?
6. A double-click in Explorer on the same file with Inventor running, busy,
   and elevated (does it reuse the instance; which message appears).
7. If available, a managed computer with AppLocker or WDAC and a Group Policy
   execution policy.

## Sources

Read in November 2026. learn.microsoft.com, support.microsoft.com,
help.autodesk.com, devblogs.microsoft.com and the Autodesk forums could not
be fetched from the research environment: Microsoft pages were read in their
GitHub sources (MicrosoftDocs `sdk-api`, `win32`, `PowerShell-Docs`), the
others through search-engine excerpts of the pages listed; they are to be
re-read on the live pages before an implementation relies on a detail.

| # | Source | Used for |
|---|--------|----------|
| M1 | SetForegroundWindow – <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow> (source: <https://github.com/MicrosoftDocs/sdk-api/blob/docs/sdk-api-src/content/winuser/nf-winuser-setforegroundwindow.md>) | foreground rules, flashing taskbar button |
| M2 | AllowSetForegroundWindow – <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-allowsetforegroundwindow> | passing the right on, when it ends |
| M3 | CoAllowSetForegroundWindow – <https://learn.microsoft.com/en-us/windows/win32/api/objbase/nf-objbase-coallowsetforegroundwindow> | the COM variant |
| M4 | RegisterActiveObject – <https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-registeractiveobject> | ROT registration, strong/weak |
| M5 | IRunningObjectTable::Register – <https://learn.microsoft.com/en-us/windows/win32/api/objidl/nf-objidl-irunningobjecttable-register> | duplicate entries, window station, `ROTFLAGS_ALLOWANYCLIENT` |
| M6 | The COM Elevation Moniker – <https://learn.microsoft.com/en-us/windows/win32/com/the-com-elevation-moniker> | elevated registrations invisible to non-elevated clients |
| M7 | ROTFlags – <https://learn.microsoft.com/en-us/windows/win32/com/rotflags> | AppID value for activate-as-activator servers |
| M8 | Windows Vista Compatibility Team, *Integrity Level and the ROT* – <https://learn.microsoft.com/en-us/archive/blogs/vistacompatteam/integrity-level-and-the-rot> (search excerpt) | ROT partitioned by token and integrity level |
| M9 | KB 238610, *GetObject or GetActiveObject cannot find a running Office application* – <https://support.microsoft.com/kb/238610> (search excerpt) | late ROT registration, retry loop |
| M10 | Marshal.GetActiveObject – <https://learn.microsoft.com/en-us/dotnet/api/system.runtime.interopservices.marshal.getactiveobject>, remarks <https://learn.microsoft.com/en-us/dotnet/fundamentals/runtime-libraries/system-runtime-interopservices-marshal-getactiveobject>, Microsoft Q&A <https://learn.microsoft.com/en-us/answers/questions/1073368/net-5-0-method-not-found-system-object-system-runt> (search excerpts) | .NET Framework only; P/Invoke replacement |
| M11 | Differences between Windows PowerShell 5.1 and PowerShell 7 – <https://learn.microsoft.com/en-us/powershell/scripting/whats-new/differences-from-windows-powershell> (source: <https://github.com/MicrosoftDocs/PowerShell-Docs>) | runtimes of the two PowerShells |
| M12 | about_Execution_Policies (5.1) – <https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_execution_policies?view=powershell-5.1> | Group Policy overrides `-ExecutionPolicy` |
| M13 | about_Language_Modes (5.1) – <https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_language_modes?view=powershell-5.1> | ConstrainedLanguage: COM and .NET limits, AppLocker/WDAC |
| M14 | How to Associate Verbs with DDE Commands – <https://learn.microsoft.com/en-us/windows/win32/shell/how-to-associate-verbs-with-dde-commands> | `ddeexec`, `application`, `topic`, `ifexec` |
| M15 | Verbs and File Associations – <https://learn.microsoft.com/en-us/previous-versions//bb776883(v=vs.85)> (search excerpt) | DDE for verbs deprecated |
| M16 | SHELLEXECUTEINFOW – <https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-shellexecuteinfow>; ShellExecuteExW – <https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw> | `SEE_MASK_NOASYNC`, no process handle with DDE, COM initialization |
| M17 | WSH AppActivate – <https://learn.microsoft.com/en-us/previous-versions/wzcddbek(v=vs.85)>; Visual Basic AppActivate – <https://learn.microsoft.com/en-us/dotnet/api/microsoft.visualbasic.interaction.appactivate> (search excerpts) | process id, `False` on failure, minimized state unchanged |
| M18 | ForegroundLockTimeout – <https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-2000-server/cc957208(v=technet.10)> (search excerpt) | default 200 000 ms |
| M19 | Interprocess Communication Between 32-bit and 64-bit Applications – <https://learn.microsoft.com/en-us/windows/win32/winprog64/interprocess-communication> (search excerpt) | window handles are 32-bit significant |
| M20 | Raymond Chen, *Foreground activation permission is like love* – <https://devblogs.microsoft.com/oldnewthing/20090220-00/?p=19083> (search excerpt) | the right must be given |
| M21 | Microsoft, *Error when sending commands to the program in Excel* – <https://learn.microsoft.com/en-us/office/troubleshoot/excel/error-when-send-commands-in-excel> (search excerpt) | the DDE failure message |
| A1 | Inventor API, Application.Ready – <https://help.autodesk.com/cloudhelp/2022/ENU/Inventor-API/files/Application_Ready.htm> | startup complete |
| A2 | Inventor API, ApplicationEvents.OnReady – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-API/files/ApplicationEvents_OnReady.htm> | startup notification |
| A3 | Inventor API, Application.MainFrameHWND – <https://help.autodesk.com/cloudhelp/2022/ENU/Inventor-API/files/Application_MainFrameHWND.htm> | main window handle |
| A4 | Inventor API, Application.InstallPath – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-API/files/Application_InstallPath.htm>; SoftwareVersion – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-API/files/SoftwareVersion.htm> | installation and release year |
| A5 | Inventor API, Documents.Open – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-API/files/Documents_Open.htm>; Documents.OpenWithOptions – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-API/files/Documents_OpenWithOptions.htm>; Documents – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-API/files/Documents.htm> | opening, options, visible documents |
| A6 | Inventor API, DesignProject.Activate – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-API/files/DesignProject_Activate.htm>; *Set active project* sample – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-API/files/ProjectActivate_Sample.htm>; ActiveDesignProject – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-API/files/DesignProjectManager_ActiveDesignProject.htm>; APS blog, *Resolving referenced Inventor files* – <https://aps.autodesk.com/blog/resolving-referenced-inventor-files> | project activation rules, `AddExisting` |
| A7 | Inventor help, *To Work with Projects* – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-Help/files/GUID-34126F60-3093-4144-8AA5-809B4D35DCA1.htm>; *About Projects* – <https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-Help/files/GUID-FB5EA98D-E486-4DF2-AF16-C19A8A09AB69.htm> | close files before switching; Default project |
| A8 | Autodesk developer blog, *AutoCAD 2025: Marshal.GetActiveObject .NET Core* – <https://blog.autodesk.io/autocad-2025-marshalgetactiveobject-net-core/> | P/Invoke replacement on .NET 8 |
| A9 | Inventor help, *To Start Autodesk Inventor View* – <https://help.autodesk.com/cloudhelp/2017/ENU/Inventor-Help/files/GUID-FBBBF107-30CD-4236-8A01-AA8650D43578.htm> | `/Project` switch (Inventor View only) |
| F1 | Autodesk forum, *Multiple instances of Inventor – Standalone EXE* – <https://forums.autodesk.com/t5/inventor-programming-forum/multiple-instances-of-inventor-standalone-exe/m-p/13384606>; *Multiple running instances of Inventor and the API* – <https://forums.autodesk.com/t5/inventor-programming-forum/multiple-running-instances-of-inventor-and-the-api/m-p/6390799> | oldest instance, one moniker, document monikers gone since 2024 (community) |
| F2 | Autodesk forum, *Not able to create Inventor instance when Visual Studio in admin mode* – <https://forums.autodesk.com/t5/inventor-customization/not-able-to-create-inventor-instance-when-visual-studio-in-admin/m-p/6063010>; *Sometimes GetActiveObject does not work* – <https://forums.autodesk.com/t5/inventor-programming-forum/sometimes-getactiveobject-does-not-work/m-p/4404981> | elevation mismatch either way |
| F3 | Autodesk forum, *FYI – Multiple Instances of Inventor* – <https://forums.autodesk.com/t5/inventor-forum/fyi-multiple-instances-of-inventor/m-p/8560863>; *Inventor opening multiple instances for files* – <https://forums.autodesk.com/t5/inventor-forum/inventor-opening-multiple-instances-for-files/m-p/13265238> | double-click reuses the instance; busy → new instance |
| F4 | Autodesk forum, *Why Inventor API opens every time the same Inventor version* – <https://forums.autodesk.com/t5/inventor-programming-forum/why-inventor-api-opens-every-time-the-same-inventor-version/m-p/9849582>; *API starts Inventor 2019 not 2021* – <https://forums.autodesk.com/t5/inventor-programming-forum/api-starts-inventor-2019-not-2021/m-p/10861708> | ProgID resolves to the last run version |
| F5 | Autodesk forum, *Start Inventor via command line* – <https://forums.autodesk.com/t5/inventor-programming-forum/start-inventor-via-command-line/m-p/5472484> | `Inventor.exe /?` |
| F6 | Autodesk forum, *The .ipj file does not open Inventor 2020 with its project* – <https://forums.autodesk.com/t5/inventor-forum/the-ipj-file-does-not-open-inventor-2020-with-its-project/m-p/9008048>; IMAGINiT, *Double-click on an IPJ opens Project Manager instead of Inventor 2020* – <https://resources.imaginit.com/support-blog/double-click-on-an-ipj-opens-project-manager-instead-of-inventor-2020> | `.ipj` association, `ipj.exe` |
| R1 | ARKANCE, *Inventor – Open Inventor files in active instance* – <https://ukcommunity.arkance.world/hc/en-us/articles/21564800765842-Inventor-Open-Inventor-files-in-active-instance> (search excerpt) | association fix, version owning it |
| R2 | ARKANCE CZ tip 15028 – <https://www.cadforum.cz/cz/otvirani-dokumentu-inventoru-poklepanim-v-sezeni-nebo-read-only-tip15028> (Czech, search excerpt) | `Inventor.exe /dde`, `ddeexec [open("%1")]` |
| T1 | Tauri NSIS template – <https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi> | finish page and `/R` use `RunAsUser` |
| T2 | nsis-tauri-utils, `run_as_user` – <https://github.com/tauri-apps/nsis-tauri-utils/blob/0e7fc3e4bb8abef88a58c2f8a0644c75bf4334ab/crates/nsis-process/src/lib.rs> | de-elevated start through the shell's token |
| T3 | tauri-plugin-opener – <https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/opener/src/open.rs>, <https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/opener/Cargo.toml>; `open` crate – <https://github.com/Byron/open-rs/blob/main/src/windows.rs> | `ShellExecuteExW`, default verb, no flags |

No commercial PDM or PLM client was studied; the patterns in section 6 are
the mechanisms the sources above document.

## Status

* **Implemented** (`desktop/src/cadlaunch.rs`, `open_in_running`,
  `INVENTOR_OPEN_SCRIPT`): the ROT route through Windows PowerShell 5.1 with
  the version and project checks, the reuse when the working folder has no
  `.ipj` (#483), retries of busy calls, the session filter, the `unreachable`
  note, the answer before `Documents.Open` and the `failed` report; a running
  Inventor of any installed version is reused when the project group pins no
  version (#483). From section 7: R1 – the desktop app grants Inventor's
  processes the foreground (`AllowSetForegroundWindow`) before it asks, and
  raises the main window (`MainFrameHWND`, restored when minimized, only a
  window of an `Inventor.exe` process `tasklist` listed) instead of
  `AppActivate` (`foreground::raise`); R2 – the reasons `constrained`,
  `self-elevated`, `elevated`, `starting` and `busy`, each with its own note,
  and the first line of PowerShell's error output when the script did not
  run (`script_failure`); R3 – a visible Inventor that is not yet in the ROT,
  or not `Ready`, is asked again every 500 ms until the deadline.
* **Not implemented:** R4 (no second instance behind a busy or starting
  Inventor – a product decision), R5 (switching the project of an Inventor
  without open documents – to be decided after the manual test), R6 (naming
  the instance among several), R7 (the shell fallback) and R8 (direct COM
  from Rust, the add-in's `open` message).
* **Verified:** the answer parsing, notes, process ids, the window line and
  the script structure by unit tests, the Windows-only code by `cargo clippy
  --target x86_64-pc-windows-gnu` (type check, not run). **Nothing has been run on a Windows machine with Inventor**; the
  *Manual test* above lists what must be confirmed there.
