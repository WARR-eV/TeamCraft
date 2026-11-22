# CAD add-ins: Inventor and Siemens NX first

## Question

Designers should see the TeamCraft state of the file they have open (item,
revision, lifecycle state, who holds the check-out, whether it is up to date)
and *check out*, *save and check in*, *get latest* and *show in TeamCraft*
**inside the CAD program**, without switching to the desktop app. Only an
add-in can warn *before* someone changes or saves a file that is not checked
out (#425; OPEN_WORK §4 *All CAD programs* and *Siemens NX*, §5 *Native CAD
add-ins*).

For Autodesk Inventor and Siemens NX – the team's main programs – this page
answers seven questions: the extension mechanism and runtime, the events, the
UI, the channel to the desktop app, licences, build/CI/tests, and install and
update. The other programs get a short assessment. No product code comes with
this page; the MVPs are follow-up issues.

## What exists already

- The desktop app drives Inventor and Solid Edge through their documented COM
  interfaces from PowerShell: the STEP export (`desktop/src/cadexport.rs`) and
  reusing a running Inventor (`desktop/src/cadlaunch.rs`, #350).
- Files that are not checked out by the user are made **read-only** in the
  working folder (`desktop/src/layout.rs` `read_only_wanted`,
  `desktop/src/workspace.rs` `set_read_only`). Inventor and NX already refuse
  to save over a read-only file, so the add-in's warning comes *on top of* a
  protection that works without it.
- The SolidCraft plugin talks to the desktop app through a bridge folder
  ([Talking to the desktop app from inside SolidCraft](solidcraft-bridge.md),
  `desktop/src/scbridge.rs`): the plugin holds **no token**, its requests are
  **untrusted intents** that the desktop app checks and the user confirms. Its
  logic is CAD-independent and tested against a fake host
  (`plugins/solidcraft/src/logic.rs`, trait `Host`).
- The server-side extension layer ([A sandboxed extension
  layer](extension-layer.md)) is a different thing: it runs admin-approved
  WebAssembly on the server. CAD add-ins run inside the CAD program on the
  user's computer.

## Sources

Only official vendor documentation, vendor support articles and developer
blogs, and public samples were read – no CAD program, SDK, interop assembly or
add-in was decompiled or disassembled (ADR 0005, CONTRIBUTING.md *Your own
work only*). Facts marked *(verify)* are from older documentation or
community reports and must be confirmed on a licensed machine with the
current release before the MVP relies on them. Read in November 2026.

| # | Source | Used for |
|---|--------|----------|
| A1 | Autodesk Inventor 2026 API help, *What is an Add-In?* – <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-API/files/CreatingAnAddIn_Overview.htm> | `.addin` manifest, the four manifest folders, registry-free registration, `LoadBehavior`, `SupportedSoftwareVersion*`, templates on .NET 8 since 2025 |
| A2 | Autodesk Platform Services blog, *.NET 8 now available for Inventor Design Automation* (19 Feb 2025) – <https://aps.autodesk.com/blog/net-8-now-available-inventor-design-automation> | Inventor 2025 moved from .NET Framework 4.8 to .NET 8 |
| A3 | Autodesk developer blog (JP), *Inventor 2025: porting custom programs to .NET 8* – <https://blog.autodesk.io/inventor-2025-migration-to-net8/> | .NET Framework add-ins may fail in 2025 and need porting; `Marshal.GetActiveObject` gone in .NET 8 |
| A4 | Autodesk developer blog, *.NET compatibility for Autodesk Inventor 2027 add-ins* – <https://blog.autodesk.io/net-compatibility-for-autodesk-inventor-2027-add-ins-what-developers-need-to-know/> | .NET 8 add-ins "expected to continue working in Inventor 2027 without modification" |
| A5 | Autodesk developer blog (JP), *Inventor 2027 .NET add-in development and Visual Studio 2026* – <https://blog.autodesk.io/inventor-2027-dotnet-addin-visualstudio-2026/> | Inventor 2027 add-in development moves to .NET 10 / Visual Studio 2026 |
| A6 | Autodesk developer blog, *Inventor 2027: improved dependency isolation for .NET add-ins* – <https://blog.autodesk.io/autodesk-inventor-2027-improved-dependency-isolation-for-net-add-ins/> | `UseInventorAssemblyContext`: an add-in can load its own library versions |
| A7 | Autodesk Inventor API reference (object model: `ApplicationEvents`, `FileUIEvents`, `FileAccessEvents`, `UserInterfaceManager`, `DockableWindows`, `ControlDefinitions`) – <https://help.autodesk.com/view/INVNTOR/2026/ENU/> (*Inventor API Reference Manual*) | events and UI objects |
| A8 | NuGet package `Autodesk.Inventor.Sdk` – <https://www.nuget.org/packages/Autodesk.Inventor.Sdk/> | a **community** package (owner not Autodesk) that bundles Inventor API assemblies – not used, see *Licences* |
| N1 | Siemens NX Open .NET / Python / C++ reference, `PartCollection` – <https://docs.plm.automation.siemens.com/data_services/resources/nx/12/nx_api/custom/en_US/nxopen_net/class_n_x_open_1_1_part_collection.html>, <https://docs.plm.automation.siemens.com/data_services/resources/nx/12/nx_api/custom/en_US/nxopen_python_ref/NXOpen.PartCollection.html> | `AddPartOpenedHandler`, `AddPartSavedHandler`, `AddPartSavedAsHandler`, `AddPartRenamedHandler`, `AddPartClosedHandler` |
| N2 | NX Open Python reference, `SessionLibraryUnloadOption` – <https://docs.plm.automation.siemens.com/data_services/resources/nx/12/nx_api/custom/en_US/nxopen_python_ref/NXOpen.SessionLibraryUnloadOption.html> | `GetUnloadOption`: keep an application loaded until NX ends |
| N3 | *NX Open Getting Started* (NX 1899) – <https://docs.plm.automation.siemens.com/data_services/resources/nx/1899/nx_api/custom/en_US/gs_nx_open/NXOpen_Getting_Started.pdf> | languages (C/C++, .NET, Java, Python), journals vs compiled applications, startup folders |
| N4 | Siemens support, *How to sign an NXOpen dll?* – <https://support.sw.siemens.com/en-US/okba/PL8705782/How-to-sign-an-NXOpen-dll/index.html> and *NXOpen DLL fails … NX Resource Bundle* – <https://support.sw.siemens.com/en-US/okba/PL8006804/NXOpen-DLL-fails-with-Assembly-not-been-compiled-with-a-NX-Resource-Bundle/index.html> | compiled NX Open code must be signed with an **author licence**, or the user's licence server must offer one |
| N5 | Siemens support, *Variable to point to a common startup folder* – <https://support.sw.siemens.com/en-US/okba/PL8507804/Variable-to-point-to-a-common-startup-folder/index.html> | `UGII_USER_DIR` points to a folder that contains `startup/` |
| N6 | Siemens support, *NX upward compatibility policy for .NET and other vendor tools* – <https://support.sw.siemens.com/en-US/okba/PL8006396/NX-Upward-Compatibility-Policy-for-NET-and-other-vendor-tools/index.html> | NX certifies the .NET version of its build; later versions "supported" on the vendor's claim |
| N7 | Open C reference, MenuScript (`ufsta`, `UF_MB_add_actions`, `custom_dirs.dat`) – <https://www.ugapi.com/doc/UFun/uf_mb/uf_mb_ref1.html> (mirror of the Siemens Open C reference) | startup entry point, menu actions, custom directories |
| F1 | FreeCAD wiki, *Workbench creation* and *Python scripting tutorial* – <https://wiki.freecad.org/Workbench_creation>, <https://wiki.freecad.org/Python_scripting_tutorial> | reference implementation (open source, LGPL-2.1+) |

Nothing here comes from a commercial PDM integration of either program; their
behaviour was not studied.

## Findings: Autodesk Inventor

### 1. Extension mechanism and runtime

- **Add-in** = a .NET class library that Inventor loads in process and
  activates through `ApplicationAddInServer.Activate(addInSite, firstTime)`;
  `addInSite.Application` is the entry into the API, where the add-in builds
  its UI and subscribes to events (A1).
- **Registry-free** registration through an XML **`.addin` manifest** is the
  recommended way; registry registration is legacy (A1). The manifest names
  `ClassId`, `ClientId`, `DisplayName`, `Assembly` (relative to the manifest
  allowed), `LoadAutomatically`, `LoadBehavior` (`1` = when any document
  opens; `0`, immediately at start, is discouraged), `UserUnloadable`,
  `Hidden` and the `SupportedSoftwareVersionEqualTo/GreaterThan/LessThan/
  NotEqualTo` filters – which are **ignored in a version-specific folder**.
- Manifest folders (A1):

  | Scope | Folder |
  |-------|--------|
  | all users, any version | `%ALLUSERSPROFILE%\Autodesk\Inventor Addins\` |
  | all users, one version (2024+) | `%PROGRAMFILES%\Autodesk\Inventor 20xx\Bin\Addins\` |
  | this user, one version | `%APPDATA%\Autodesk\Inventor 20xx\Addins\` |
  | this user, any version | `%APPDATA%\Autodesk\ApplicationPlugins\` |

- **Runtime per version:**

  | Inventor | Runtime of the add-in templates | Source |
  |----------|---------------------------------|--------|
  | ≤ 2024 | .NET Framework 4.8 | A2 |
  | 2025, 2026 | **.NET 8** (confirmed) | A1, A2, A3 |
  | 2027 | .NET 10 templates; .NET 8 add-ins "expected to continue working" | A4, A5 (the two blogs differ on whether a rebuild is needed – *verify*) |

  .NET Framework 4.8 add-ins *may* still load in 2025+, but Autodesk asks to
  port them (A3). Inventor 2027 can isolate an add-in's dependencies (A6).
- **Apprentice** (the UI-less API for properties and references) runs outside
  Inventor and is not needed: the server already reads references and
  instance counts itself ([Inventor files](inventor-display-preview.md),
  [instance counts](inventor-instance-counts.md)).
- **iLogic / VBA** are per-user automation, not a distribution channel – not
  used.

### 2. Events

All from the API reference (A7); exact signatures to be taken from the 2026
reference when the MVP is written.

| Need | Event |
|------|-------|
| before / after open | `ApplicationEvents.OnOpenDocument` (`BeforeOrAfter`) |
| before / after save | `ApplicationEvents.OnSaveDocument` (`kBefore` / `kAfter`; `HandlingCode` – whether a save can be cancelled is *verify*) |
| save as / rename | `FileUIEvents.OnFileSaveAsDialog`; after the save, `Document.FullFileName` changed |
| close | `ApplicationEvents.OnCloseDocument` |
| first change to a document | `ApplicationEvents.OnDocumentChange` (the add-in remembers per document that it warned) |
| file dialogs, metadata | `FileUIEvents.OnFileOpenDialog`, `OnPopulateFileMetadata` (the hook PDM systems use for numbering – later) |
| reference resolution | `FileAccessEvents.OnFileResolution` (later: point references into the working folder) |

### 3. UI

- A **ribbon tab** *TeamCraft* on the part, assembly, drawing and zero-document
  ribbons (`UserInterfaceManager.Ribbons[…].RibbonTabs.Add`), buttons from
  `ControlDefinitions.AddButtonDefinition` (A7).
- A **dockable window** (`UserInterfaceManager.DockableWindows.Add`, a WinForms
  or WPF control attached by window handle) with the status of the active
  document: item, revision, state, lock holder, up to date (A7).

## Findings: Siemens NX

### 1. Extension mechanism and runtime

- **NX Open** has C/C++, .NET (C#, VB), Java and Python bindings (N3).
- **Journals** (`.vb`, `.cs`, `.py`, `.java` source run by NX) need no signing
  but are one source file, cannot reference our own libraries and are meant to
  be started by the user (N3). A journal whose `GetUnloadOption` returns *until
  termination* stays loaded (N2), so it could keep callbacks – but it does not
  start by itself.
- **Compiled applications** (DLL / shared library) in a `startup/` folder load
  when NX starts (N3, N7). `UGII_USER_DIR` points to a folder that holds
  `startup/` (N5); more folders can be listed in `custom_dirs.dat` (N7;
  `UGII_CUSTOM_DIRECTORY_FILE` as a per-user list is *verify*). The entry point
  is `ufsta` / `Startup` (N7).
- Compiled NX Open code must be **signed** with a tool that needs an **NX Open
  author licence** (`SignDotNet` / `SignLibrary` and the NX signing resource in
  the project); an unsigned DLL runs only where the licence server has an
  author licence free, otherwise it fails validation (N4). This is the key
  constraint for an open-source add-in: whoever builds the release must hold
  an author licence, or each site signs its own build.
- **Runtime:** NX certifies the .NET version it was built with (N6); which one
  the current releases (NX 2412, 2506, 2512) use is in the release letters
  behind the support login – *verify* before choosing the target (.NET
  Framework 4.8 vs .NET 8).
- **Linux:** NX runs on Linux; NX Open .NET is Windows-only, C++, Java and
  Python are on both (N3; *verify* for the current release). A Linux add-in is
  C++ or Python, i.e. a second shell.

### 2. Events

`PartCollection` (N1): `AddPartOpenedHandler`, `AddPartSavedHandler`,
`AddPartSavedAsHandler` (save under a new name), `AddPartRenamedHandler`,
`AddPartClosedHandler` (*in the process of being closed*), each returning an
id for removal. They report **after** the fact; a callback *before* a part is
modified or saved was not found in the public reference – so the NX add-in
warns when a part that is not checked out is opened for editing and after it
was saved, and relies on the read-only file for the hard stop.

### 3. UI

Ribbon tabs and buttons come from ribbon/menu definition files (`.rtb`,
`.men`) in the application folder next to `startup/`, whose buttons call
actions the add-in registers at start (`UF_MB_add_actions`, N7); the exact
ribbon file syntax of current releases is *verify*. A status panel can be an
NX Open block dialog or a resource bar tab (*verify* the API on a current
release); a modeless dialog is the fallback.

## 4. Channel to the desktop app (both)

An in-process add-in, unlike the SolidCraft plugin, may use the network and
the file system freely. Options:

| Channel | For | Against |
|---------|-----|---------|
| Bridge folder (as SolidCraft) | proven, versioned JSON, works when the desktop app is down | polling; no immediate answer to "may I save?" |
| `127.0.0.1` + token file | simple in every language | every local process and every user on a shared machine can connect; needs a secret on disk |
| COM / NX from the desktop side | desktop already drives Inventor | no events in the CAD program, no UI there |
| **Per-user local IPC**: Windows named pipe with an ACL for the current user, a Unix domain socket `0600` in the user's runtime folder on Linux | no secret (the OS checks the user), push in both directions, available in Rust (tokio), .NET, C++ and Python | one more listener in the desktop app |

**Chosen: per-user local IPC**, with the bridge rules of SolidCraft:

- **No PAT, no server data beyond display state** in the add-in. The desktop
  app keeps the only token.
- Requests from the add-in are **untrusted intents**: a path must be inside
  the working folder and belong to a revision the desktop app knows; only the
  known actions; size-limited messages. *Check out* asks the user unless the
  user ticked *check out without asking from CAD add-ins*; *save and check
  in* opens the desktop app's own check-in dialog; *show* only shows.
- The desktop app never acts on add-in events alone (an event only refreshes
  the state or raises a warning).
- When the desktop app is not running, the add-in says so and offers to start
  it (`teamcraft://` link); it never fails the CAD program.
- The threat-model entry is written with the MVP
  (`docs/src/dev/threat-model.md`, *Planned: CAD add-ins*).

### Protocol sketch (shared by all add-ins)

Newline-delimited JSON over the pipe/socket, each message ≤ 16 KiB, `schema
"teamcraft.cad"`, major `version 1`; unknown fields ignored, a newer major
ends with "update the add-in" / "update TeamCraft".

```text
pipe:   \\.\pipe\teamcraft-cad-<user SID>        (Windows, ACL: this user only)
socket: $XDG_RUNTIME_DIR/teamcraft/cad.sock      (Linux, 0600)

add-in → desktop
  {"t":"hello","schema":"teamcraft.cad","version":1,
   "cad":"inventor","cad_version":"2026.2","addin_version":"0.1.0"}
  {"t":"watch","paths":["C:/…/TeamCraft/ACME/1002-A/bracket.ipt"]}   # open documents
  {"t":"event","kind":"opened|first_change|saved|saved_as|closed",
   "path":"…","old_path":"…"}                                       # old_path for saved_as/renamed
  {"t":"intent","id":7,"action":"check_out|check_in|get_latest|show","path":"…"}

desktop → add-in
  {"t":"welcome","version":1,"server_reachable":true,"user":"Display Name"}
  {"t":"state","path":"…","item":"1002","revision":"A","state":"in_work",
   "lock":{"by":"Display Name","me":false},"latest":"B","local_changes":true}
  {"t":"state","path":"…","unmanaged":true}                          # not in the working folder
  {"t":"intent_ack","id":7,"shown":true}                             # result arrives as a new "state"
  {"t":"warn","path":"…","text":"…"}                                 # e.g. saved while not checked out
```

The texts the add-in shows come from the desktop app (already translated,
`shared/i18n/`), so the add-ins carry no message catalogs of their own.

## 5. Licences and legal

- **Inventor:** the API and its interop assembly
  (`Autodesk.Inventor.Interop.dll`) ship with Inventor. The add-in
  **references the installed assembly and never redistributes it** (no copy
  in the repository or the installer, "copy local" off). The community NuGet
  package A8 bundles Inventor assemblies under a third party's name; it is not
  used. No App Store or partner programme is required to install an add-in
  (A1: a manifest in a folder is enough); the Autodesk Developer Network is
  optional.
- **NX:** the NXOpen assemblies (`NXOpen.dll`, `NXOpen.UF.dll`,
  `NXOpenUI.dll`, `NXOpen.Utilities.dll`) and headers ship with NX and are
  referenced in place, never copied. Signing needs an author licence (N4);
  the Siemens partner programme is optional. **Open:** whether the team's
  academic NX licence includes an NX Open author licence – if not, the MVP
  starts as a journal (status and intents, started by the user) and the
  signed DLL waits for a licence.
- **Education licences:** both vendors' education licences are for learning
  and teaching use; building and testing the add-in for the team's own use
  fits, the add-in contains no vendor code. The MVP issue records the licence
  each test machine used.
- **MPL-2.0:** the add-ins are our own source under MPL-2.0 (file-level
  copyleft); calling a proprietary API at run time is compatible – MPL has no
  "system library" problem like the GPL. Each new add-in project gets its row
  in the README licence table.

## 6. Build, CI and tests

- **CAD-independent core** shared by the add-ins: a `netstandard2.0` class
  library (usable from .NET 8/10 for Inventor and from .NET Framework 4.8 if
  NX needs it) with the protocol client, the state model and the decisions
  ("warn now?", "which buttons are enabled?") behind an `ICadHost` interface –
  the same split as `plugins/solidcraft/src/logic.rs`. Tested with a fake host
  by `dotnet test` on Linux in CI, plus protocol test vectors shared with the
  desktop app's Rust side (like the SolidCraft bridge vectors).
- **Thin shells** (Inventor add-in class, NX startup application) contain only
  event wiring and UI. They need the vendor's assemblies to compile, which CI
  must not have (no redistribution) – so they are compiled on a licensed
  Windows machine (later a self-hosted runner, if one is offered) and checked
  with a **manual test script**: install from the desktop app, open a
  checked-out and a not-checked-out file, change, save, save as, close,
  desktop app stopped, two CAD versions side by side.
- The desktop side (pipe/socket listener, intent checks) is Rust and tested
  in CI like `scbridge.rs`.
- **FreeCAD as reference implementation** (see below): an add-in that CI can
  run headless on Linux (`FreeCADCmd`) against the real desktop listener –
  the end-to-end test of the protocol without a licence.

## 7. Install and update

- **Desktop app → Settings → CAD add-ins:** lists the CAD programs it finds
  (Inventor versions as `cadlaunch.rs` finds them; NX installations) with
  *Install*, *Update*, *Remove*. The add-in files ship inside the desktop
  installer, so add-in and desktop app always speak the same protocol.
- **Inventor:** one .NET 8 build for 2025–2027 in
  `%APPDATA%\Autodesk\ApplicationPlugins\TeamCraft\` (per user, any version)
  with `SupportedSoftwareVersionGreaterThan` in the manifest; a separate .NET
  10 build only if 2027 turns out to need it. Machine-wide installs by IT go to
  `%ALLUSERSPROFILE%\Autodesk\Inventor Addins\`. Inventor ≤ 2024
  (.NET Framework) is not planned.
- **NX:** an application folder (`startup/` + ribbon files) per supported NX
  release, added to the user's custom directories rather than replacing a
  site's `UGII_USER_DIR`.
- **Policies (#378):** a new policy `CadAddins` (`off` / `allowed` /
  `required`) and the existing *Set by your organization* behaviour; with
  `required` the desktop app installs and updates the add-ins and hides
  *Remove*. Documented in `deploy/desktop-policies/` with the MVP.

## Decision

| Program | Decision |
|---------|----------|
| **Inventor** | **Build the MVP** (follow-up issue *Inventor add-in MVP*): .NET 8 add-in, registry-free `.addin`, ribbon tab + dockable status window, events open / first change / save / save as / close, actions *Check out*, *Save and check in*, *Get latest*, *Show in TeamCraft*, warning on a change to a file not checked out; Inventor 2025–2027 |
| **NX** | **Build the MVP after the licence check** (follow-up issue *NX add-in MVP*): Windows first, .NET startup application, signed if the academic licence has an author licence – otherwise a journal with status and intents first; events opened / saved / saved as / renamed / closed (no "before modify": the read-only file is the stop); Linux later as a C++ or Python shell |
| Shared | per-user local IPC (named pipe / Unix socket), protocol `teamcraft.cad` v1 above, CAD-independent `netstandard2.0` core with a fake host, desktop *Settings → CAD add-ins*, policy `CadAddins` |

## P2: short assessments

| Program | Mechanism | Assessment |
|---------|-----------|------------|
| SOLIDWORKS | .NET add-in (`ISwAddin`, registered COM class), events on the document objects, task pane | same shape as Inventor, the core is reusable; needs a licence to test – after Inventor |
| Solid Edge | COM add-in (`ISolidEdgeAddIn`), application and document events, edge bar pane | same shape as Inventor; no openly licensed test files ([CAD test files](cad-test-files.md)) – after Inventor |
| Fusion | Python or C++ add-ins with document events; files live in Autodesk's cloud, not a local folder | does not fit the working-folder model; only *show state* for exported local files – not planned |
| Creo | J-Link / Object TOOLKIT (Java, C/C++); the C toolkit needs a paid licence | not planned until a licence and a need exist |
| CATIA V5 | CAA (C++, partner programme) or COM automation from outside | CAA is out of reach for an open-source project; desktop-side COM later at most |
| **FreeCAD** | Python workbench, document observer (`App.addDocumentObserver`: created, start/finish save, deleted) | **reference implementation**: open source (LGPL-2.1+), runs headless on Linux, so CI tests the protocol end to end; also useful for FreeCAD users |

## Next

1. Issue *Inventor add-in MVP* (scope, test plan, licence notes).
2. Issue *NX add-in MVP* (licence check first, then scope and test plan).
3. With the first MVP: the desktop listener, the shared core, the threat-model
   entry, *Settings → CAD add-ins*, the `CadAddins` policy and the FreeCAD
   reference add-in in CI.
