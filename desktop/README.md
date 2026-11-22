A failed export never stops the check-in: the file goes without its STEP and the result says why. With exports the ticked files are uploaded first (parked), then exported, then checked in with the STEP files; *Cancel* during an export skips it and ends the CAD program the script started (`teamcraft-started-pid:` line, `taskkill`); rejected COM calls (`RPC_E_CALL_REJECTED`, `RPC_E_SERVERCALL_RETRYLATER`) are retried for a minute; the whole error output goes to `<config>/export-logs/` (newest 20, 64 KiB each, local only) – #342 |# TeamCraft Desktop

The rich client of TeamCraft for **Linux, macOS and Windows**: a navigation
rail, a navigator tree of the project groups, object tabs with sub-tabs, a
context toolbar, a status bar – and a **local working folder** for
check-out/check-in.

It is a [Tauri 2](https://tauri.app) application: the user interface (`ui/`,
plain HTML/CSS/JavaScript, no build step) runs in the system web view (WebView2
on Windows, WKWebView on macOS, WebKitGTK on Linux); the Rust side (`src/`)
talks to the TeamCraft server and manages the workspace. The app needs a
TeamCraft server – it is a client, not an offline copy.

> **Work in progress:** there is no official release yet, and the app is
> provided as is, without warranty of any kind. See the
> [project README](../README.md) and [CONTRIBUTING.md](../CONTRIBUTING.md).

## Features

| Area | What the desktop app does |
|---|---|
| Sign-in | **Sign in with the browser** (single sign-on or password, confirmed on a TeamCraft page) or a personal access token; *cancel* stops the sign-in at once |
| Getting started | A short wizard after the **first sign-in** and from *Help → Getting Started…* (#422): server (skipped when a policy fixes it), sign-in (done), **working folder** (what it is, the path, *Change…* with the folder picker, *Open folder*), **project group** (the groups you are a member of, *Get project groups…* – latest revision, also in work; skipped without groups or when a finished Get on this computer already fetched them into this working folder), **CAD setup** (for information only: the Inventor `.ipj`, NX load options and Solid Edge search paths a later version will offer – nothing is created) and **your daily loop** (*Check out and open* → save in CAD → *Check in…*). Every step has *Skip*, *Back*, *Next* and *Don't show again*; the wizard creates no file or folder itself, only through the app's own confirmed actions. *Finish* or *Don't show again* is stored as `onboarding` in `settings.json`; an app that starts already signed in (an update from an older version) does not show it by itself. Step rules in `ui/js/onboarding-rules.js`, tested by `scripts/desktop-onboarding-rules.test.mjs` (`node`); the dialog in `ui/js/onboarding.js` |
| Connections | Several servers and several accounts on one server (#322, #323): *Settings → Connections* lists the saved ones (server, account, working folder) with *Switch* and *Remove*; *Add another account…* signs in a second account on the same server, *Switch server…* (and the connect screen) changes the server **without signing out**. The sign-in of the connection left behind is parked in the keychain (`api-token@<id>`, or a private `token-<id>` file without one) and comes back when you switch to it; the window reloads. Every connection after the first has its **own working folder** next to the default one (`TeamCraft-<server>`, numbered for a second account), never inside another, so files of servers with the same item numbers never mix; the first keeps its folder. At most 20 connections; removing one keeps its folder on disk (`src/profiles.rs`) A sign-in kept from an app version before the connections gets its account noted the first time the server confirms it (`GET /me`, #356) |
| Local test server | Downloads *with local server* only – an app of its own, *TeamCraft Local* (own identifier, settings and keychain entry): on its first start, with no server configured, and with *Try TeamCraft on this computer* on the connect screen it starts the bundled `teamcraft-server local` (private PostgreSQL + server on 127.0.0.1), signs in through the browser (the first start creates the administrator there) and starts the server again at every launch while it is the configured server; it stops when the app ends (`src/localserver.rs`, log in `local-server.log`) |
| Navigator | *Home*: a file browser of the **active project group** – *Files*: its folders and the files in them, as in the working folder (folders first, then the files by name – case-blind, digits by value; one folder per request from `GET /projects/{code}/files`, 500 entries at a time with *show … more*; items without files below the files of their own folder; **+** on a folder row = *New item here*; a checked-out row is marked with a bar in the lock colour and a light tint, also in *Structure* – #362; with an older server the items instead; a folder's ☆ makes it the group's **favourite folder**: Home starts there under a banner in the accent colour – *not the top of the group*, *Whole group*, *Remove favourite* – and the folder of the group's Inventor project file `.ipj` (`project_folders`) is suggested; kept per computer, server and group, #352), *All items*: every item in one list, or the *Structure* view. The name comes first (the number/revision dimmed after it); sorted by name, number, type or state (kept per computer); *Projects* (`Ctrl/Cmd+5`): choose the active group (mine first), create a group (*New*, CAD editors), *Get…* groups into the workspace, *Import CAD folder…*. Both are loaded again on *Refresh* (a group created in the web UI shows without a restart); *My Worklist* (check-outs, reviews, my open changes – updated at once after a check-out, cancelled check-out, check-in or acknowledgement in a tab, #347); change orders by status; full-text search |
| Check-outs tab | Administrators and the managers of a group (`managed_projects` of `GET /me`): the lock button next to the group under *Projects* opens a *Check-outs* tab – who has what checked out, as a graph (`js/lock-layout.js`, the server's layout from `services::lock_overview::graph`, tested by `scripts/desktop-lock-layout.test.mjs`; styles in `shared/css/components.css`) and a list per person; a click opens the revision (`js/lock-overview.js`, `GET /projects/{code}/checkouts`) – #428 |
| Right-click menu | On items and files in the navigator, Home *Files* / *All items* / *Structure*, the worklist's check-outs and the Graph cards (rows with `data-item` + `data-rev`, one listener in `js/item-menu.js`): *Check out and open* / *Open in CAD*, *Check out*, *Check in…*, *Get…*, *Show in folder*, *Copy number*, *Where used*, *Request change…*, *Open in browser*; the entries and the tooltip of a disabled one come from the toolbar's rules (`js/item-menu-rules.js`, tested by `scripts/desktop-item-menu.test.mjs`); Shift+F10 / context-menu key, arrows, `Esc` – #417 |
| Toolbar | Below the tab strip, acting on the active tab: grouped buttons that keep their place (revision: Lock · Work · Lifecycle · Collaborate; change: Review · Lifecycle · Collaborate; request: Triage · Requester · Collaborate), the **next step highlighted** (*Check out*, *Check in…* with local changes, *Release…* on a clean check-out, *Revise* when released, *Submit for review* / *Approve change* on a change), greyed-out buttons with a tooltip saying why (*Check in…* has an upload arrow, never the download arrow of *Get…*, #409; *Edit attributes* as in the web UI, #410), rare actions (*Pack and Go…*, *Open in browser*, *Refresh*, *Mark obsolete…*) in **More ▾** – keyboard: `Enter`/`Space`/`↓` open, arrows move, `Esc` closes |
| Tab order | Drag a tab to move it (pointer events, an insertion bar shows the landing place, the strip scrolls at its edges, `Esc` cancels; a plain click still activates, a dragged tab keeps its state and is not activated). Keyboard: arrow keys, `Home`, `End` move the focus between the tabs (WAI-ARIA tabs, roving tabindex, `Enter`/`Space` activates), `Ctrl+Shift+PageUp/PageDown` move the active tab. Right click (or the menu key): *Move to start*, *Move to end*, *Close*, *Close others*, *Close tabs to the right*. The order is kept while the app runs (open tabs are not restored on start). Rules in `ui/js/tab-order.js`, tested by `scripts/desktop-tab-order.test.mjs` (`node`); the dragging itself is checked by hand |
| Revision tabs | Overview (with *At a glance*: the preview image from the CAD file, files, BOM lines, where used, references, change orders, open requests – and a link to the similar parts in the browser), Files (versions, restore, open), Structure (multi-level BOM, revision rules, roll-up), Where used, References, **Graph** (see below), Changes, History, **3D viewer** (STEP, meshes, point clouds – converted by the server), Workspace |
| Graph | A navigable reference/BOM graph around the revision: the focused item in the middle, its BOM children and references below, the assemblies that use it above. Node **colour** = lifecycle state, **size** and the ⚙ badge = the item's own BOM complexity, **×n in the top-left corner** = how many instances of that part or sub-assembly the assembly contains (summed over its BOM lines, ×1 included). Every node is clickable and takes the tab to that item – a sub-assembly shows its own graph, anything else its overview, the item in the middle its own overview – with a **back control that stays in the same place** (the left of the sub-tab row) and leads back the way you came, one step per click; `Ctrl`/`⌘`+click opens a node in a tab of its own. Mouse-wheel zooms on the cursor; drag to pan; the view fits itself while details load until you zoom or pan, a double-click on the background fits it again. More than 8 neighbours on one side are laid out in rows; above 48 only the first 48 are fetched for preview and complexity (one request each), the rest keep a basic card. Preview images and filenames on the nodes; pure SVG, no external engine |
| Working folder | One folder per project group that **mirrors the folder structure of the files** (`<workspace>/<group>/<folder>/<file>`, the folder is stored with every file on the server), one copy of each file – the revision is metadata in `<workspace>/.teamcraft/state.json` (item, revision, file id, version, SHA-256, checked out). Files you have not checked out are **read-only** on disk; *Check out* takes the lock and makes them writable; *Check in…* uploads the chosen files as new versions, releases the lock in one step and makes them read-only again (files above 32 MiB go first as chunked, resumable uploads, then join the same check-in transaction). The check-in dialog shows the progress, and **Cancel** stops the check-in until its last byte is sent – before the next STEP export, between two chunks (the upload sessions are discarded, `DELETE /uploads/{id}`) or half-way through the streamed request with the small files: nothing is stored, the revision stays checked out (`src/abortable.rs`). Every check-in names the version each file is based on, so the server refuses (409) to overwrite a version somebody else checked in meanwhile. The Workspace tab shows *up to date* / *modified* / *outdated* / *conflict* / *new* / *missing* / *name clash* per file (from the server's file status) with its path, read-only flag and the lock owner. Two items with the same folder and file name are reported, never overwritten. Lock and backup files of the CAD programs are skipped; a SolidCraft design that is open in SolidCraft (a fresh `.~<file>.lock` beside it, `src/cadlock.rs`) is marked *Open in SolidCraft* with the computer and user, and the check-in dialog warns that unsaved changes there are not checked in; Creo iterations count as one file. A tracked revision the server no longer shows (you left a restricted project group) is listed as *No longer accessible*: its local copies stay untouched and are never synced again – delete them yourself if you must In *Check in…*, a new part or assembly file next to a revision that has a model already is offered unticked, with *New item from this file* (#359) |
| Get | *Get…* on a revision: the item with its BOM (revision rule *latest* or *released*) and the parts its CAD files reference, resolved by the server (`…/workset`); only missing and outdated files are downloaded (SHA-256, checked after the download), recorded one by one so an interrupted Get continues where it stopped – also inside a large file: the bytes received stay in `<name>.<sha>.teamcraft-download` and the next Get asks only for the rest (`Range` / `If-Range`, `src/resume.rs`), checks the SHA-256 of the whole file and renames it into place; progress and *Cancel* in the status bar. No browser downloads |
| Add files | *Add files…* (Files section and *More ▾*): file picker → the files are **copied** into the working folder of the revision (checked out first if needed; sub-folder selectable, originals untouched) and listed as *new* in *Check in…* (option *Check in now*; remembered in `.teamcraft/added.json` until checked in). A name that exists is a new version and asks first. The Files section says why it is unavailable (locked by someone else, in review, released = frozen → Revise). Pure logic and tests in `src/addfiles.rs`; drag and drop onto the window is not supported (`dragDropEnabled` is off) |
| Old workspaces | Folders of the older layout `<group>/<item>/<revision>/` (and `<item>/<revision>/`) are found after the sign-in; the app offers to move the files into the new layout – never overwriting a file, keeping files whose place is taken and listing them, writing every step to `.teamcraft/migration.log` |
| Check in all | *Check in all* in the worklist (navigator and home page): the check-outs with local changes (modified, new, conflicting files) are listed first, with the offer to check them in with their files; then every check-out without local changes is checked in on the server (lock released, one revision at a time); check-outs with a conflict stay checked out. *Sign out* asks the same: *Check in and sign out*, *Park and sign out*, *Sign out anyway*. The list is the pure `layout::local_changes` |
| Park | *Park…* (Workspace tab, *More ▾*, the check-in dialog): the ticked files go up as new versions through the upload endpoint (`POST …/files`) and the check-out **stays** – work in progress survives a lost laptop. After somebody else released your check-out, the Workspace tab shows *Released by X; your local changes are kept and need a new check-out* (`released` of the server's file status) |
| Native CAD previews | The preview image stored in Inventor and Solid Edge files in the Files tab, a picture of the 3D preview for NX and SOLIDWORKS files; on Windows *STEP* in the Workspace tab and *Create STEP exports … first* at check-in export with the installed Inventor or Solid Edge (COM automation through Windows PowerShell; uses your own licence); on every system SolidCraft designs (`.sc`) export through SolidCraft's `solidcraft-cli run --in … --out ….stp` (found through `SOLIDCRAFT_CLI`, `PATH` or the default install folders). A failed export never stops the check-in: the file goes without its STEP and the result says why |
| SolidCraft item data | *Item data* in the Workspace tab (SolidCraft designs of your check-out, not open in SolidCraft): `solidcraft-cli run <script> --in <design> --save <design>` with one `document.metadata.set` – `part_number`, `revision`, `description` and `teamcraft.pdm` (item and revision ids, server); `src/scprops.rs` |
| SolidCraft plugin bridge | The TeamCraft plugin inside SolidCraft has no network, so the app shares `<workspace>/.teamcraft/solidcraft` with it (`src/scbridge.rs`, `ui/js/solidcraft-bridge.js`): every 30 s a heartbeat (`desktop.json`: the app runs, the server answers), on every Workspace refresh the state of the revision (`state-<key>.json`: checked out by, local changes, released), every 5 s the plugin's requests (`request-<key>.<action>.json`, one per revision and action) are picked up, checked (a revision of this working folder, a known action, fresh, small, a regular file) and removed: *open* shows the revision, *check out* asks first, *check in* opens the check-in dialog. Only for a working folder that exists; no secrets in the folder |
| Check out and open | *Check out and open* on the revision toolbar (*Open in CAD* for one's own check-out) takes the lock, gets the files and opens the main CAD file; the chip next to it names the program (e.g. *Inventor 2026*). The program: the project group's default CAD program (`default_cad`, `default_cad_version` of `GET /api/v1/projects`) in its version, else its newest installed version; without a default, or for a file of another program, the newest installed version of the program that fits the file (`format_info.family`); else the system's default application. Installed programs are found by their install folders (Windows *Program Files*, SOLIDWORKS via `reg query`; macOS `/Applications`; FreeCAD on the Linux `PATH`), once per start – `src/cadlaunch.rs`, `ui/js/cad-open.js`; *Open* in the Files tab uses the same choice. On Windows an Inventor that already runs – the chosen installation, with the working folder's Inventor project active (the `.ipj` in the nearest folder up to the working folder root) – opens the file as a new window instead of a second instance (#350; asked through COM by a Windows PowerShell script: `GetActiveObject`, `InstallPath`, `ActiveDesignProject`, `Documents.Open`); otherwise a new instance starts and the message says why. After *Cancel* during the download, or with files that could not be copied, nothing is opened (#348, `ui/js/get-rules.js`). *Import CAD folder* detects the folder's program (Inventor with the release year read from the files, `folderimport::inventor_year`) and offers to set it as the group's default |
| Preferred CAD program | The account setting of the web UI (`preferred_cad` of `GET /api/v1/me`, read at sign-in): when a revision has files of that program, the Files tab lists the native files of other CAD programs (`format_info.family`) in a collapsed *N files in other CAD formats (…)* group; neutral files (STEP, JT, STL, PDF, …) stay visible. Grouped in `ui/js/revision.js` (`groupByFamily`) with the same rule as the server's `cadformat::group_by_family`; display only |
| Whole project groups locally | the download icon in the navigator (*Tools → Get Project Groups…*): copies the files of every item of the chosen groups – latest revision (also in work, the default since #355) or latest released – into the working folder with their folder structure; unchanged files are skipped, locally modified files kept, name clashes reported. For new team members or a fresh computer |
| Import a CAD folder | the folder icon in the navigator (*Tools → Import CAD Folder…*): scans an existing folder (dry run: items, duplicate file names, skipped lock/backup files), then creates one item per part or assembly with the drawings and exports of the same name, project files in one document item, optional BOM lines from the assembly references, and checks every item in right after its files (assemblies once more after their BOM lines), so an interrupted import never leaves items checked out – a new run of the same folder also releases the check-outs an earlier, interrupted run left behind; a final report lists the new items and files, BOM lines, check-ins and problems (with *Open* for the top assembly); interrupted imports continue where they stopped (remembered per project group, so the same folder can also be imported into another group). Tutorial: *Help → How to Import a CAD Folder* |
| New item | *New* (`Ctrl/Cmd+N`): choose the CAD files first – name and type follow from the first file; they are uploaded right after the item is created. *Folder* (prefilled with the folder last opened in *Files*, or the one of *New item here*) is the item's folder in the group – the files go there, and the item is listed there while it has none – unless the files are in the working folder: then their group and folder are the item's place, taken from where they are, and every file keeps its own folder (#359). Files from elsewhere are copied into the item's folder (never moved, nothing overwritten). Before creating, duplicates are refused: a tracked or added file, a file another item has at the same place (`POST /projects/{code}/file-check`); the same name in another folder of the group needs *Create anyway*, the same content is a warning. The upload asks the server to refuse a clash atomically (`refuse_clash`, `refuse_same_name`), the item is created with an idempotency key from its main file, and the uploaded files are recorded in `state.json` (`new_item_places`, `new_item_copy`, `upload_files` with `track`) |
| New on this computer | Worklist section (#359, editors): the parts and assemblies in the working folder that belong to no item yet, one suggestion per model file with its drawing and exports of the same name in the same folder, newest first, counted on the rail. A click opens *New* prefilled, *Create items…* creates the ticked ones after a summary of what is created and what is refused, *×* ignores a file (`.teamcraft/ignored.json`, *Show ignored*; entries of files that are gone are dropped after a complete scan). The scan (`desktop/src/newfiles.rs`, `new_files_scan`) skips tracked and added files, companions of tracked files, lock/backup/temporary files, `OldVersions` and hidden folders; at most 50 000 entries and 2 000 files per scan, run when the worklist opens or the window gets the focus back (at most every 15 s). Nothing is uploaded without the dialog or the summary |
| App version | The version of the app above the logo at the bottom of the rail, always visible (#357) |
| Import BOM | *Import BOM…* in the *Structure* tab (and the *More* menu) of a revision you have checked out, *Tools → Import BOM as New Assembly…* for a new one: a BOM exported by the CAD program (CSV, TSV, Excel, ODS or a STEP assembly, chosen in the native dialog) goes to the server's BOM import as a dry run first – the dialog shows the counts of lines, new and existing items, the notes and the first lines – and *Import* creates the lines and missing items in one transaction (numbers from the file or the group's numbering scheme, reuse of existing items optional) |
| Release and changes | *Release…* creates and submits a change order – with the unreleased dependencies (BOM children, drawings) by default; the own user name is refused as reviewer, with nobody else in the group it creates a draft, and a refused revision or reviewer cancels the half-made draft again (#291); *Release readiness* panel with *Add all*; on a draft *Add reviewer* by user name, *Remove* and *Cancel change*, *Submit for review* disabled with the reason while a revision or reviewer is missing; approve/reject from the worklist |
| Change requests | *Request change…* on a revision tab and *Request…* on a file (Files and Workspace tabs) raise a change request prefilled with item, revision and file name, with an optional screenshot or PDF (native file dialog, uploaded by the Rust side); request tabs with discussion, decision (accept / defer / reject), *Create change order*, attachments; *Requests assigned to me* and *My open requests* in the worklist |
| Change propagation | *Dependents to check* grouped by dependent revision (one worklist line with a count, expandable; on the home page **Open**, **Revise**, **Acknowledge all…** per dependent and **Acknowledge…** per change), *Check needed* on the revision tab, references, *As released* rule, impact analysis |
| Notifications | Bell with unread count and toasts; **Watch** / **Unwatch** |
| Agent | **Agent** in the rail (`Ctrl/Cmd+4`, or *Agent* in a revision or change tab) – the app's one place for AI, on one of two backends. **Company AI**: the server's assistant (the web UI's *Ask TeamCraft*, inline in the Agent tab; the key stays on the server) – it knows the last revision (and section, BOM rule), change or request tab, offers matching quick actions (*Draft release notes*, *Check before release*, *Brief me for this review* …), the drafts checkbox and one conversation kept on the server; read-only unless drafts are allowed. **Local agent**: an agent you configure under *Settings → Agents…* (Claude Code through its ACP adapter, Gemini CLI, any [Agent Client Protocol](https://agentclientprotocol.com) agent; none by default). The app starts it in the workspace folder and hands it TeamCraft's MCP server with your token. Streamed conversation, tool calls, plan, permission prompts (*Allow once* / *Reject*; calls of TeamCraft's own read-only MCP tools – by the server's `tools/list` annotations – are allowed once without a prompt and marked *Allowed automatically (read-only)*, since the server answers with your rights only; write tools, other servers' tools, the agent's own tools and anything not identified for certain still ask; *Settings → Agents… → Ask before read-only TeamCraft tools* asks for everything), the agent's own model picker (remembered per agent), *Cancel*, quick prompts from the server's MCP prompts, the open revision or change as context. Through the app the agent may only read text files in the workspace – no writing, no terminal. With both on the server, *Company AI* / *Local agent* in the Agent toolbar switches (remembered per server and user; default: the local agent if one is set up). When the server sets `TEAMCRAFT_LOCAL_AGENTS=off` the Agent view runs on the company AI only, the Rust side ends a running agent and refuses to start one (the app asks the server again when the Agent view opens, every minute and before every message, so a changed setting needs no restart); without a company AI the view says that AI is switched off. See [the guide](../docs/src/guide/assistant.md#acp-agents-in-the-desktop-app) |
| Pack and Go, STL | ZIP of a revision and its structure, with a progress dialog and *Cancel* (also while the server packs); written as `<name>.teamcraft-download` and renamed when complete, removed on cancel or error; **Export STL…** from the 3D viewer |
| Usage report | Once a day of use the app sends its own TeamCraft server (`POST /api/v1/usage`) an anonymous report of the day before (`src/usage.rs`, #383): a random installation id, versions, OS, architecture, package type, minutes with the window in front, and counts of fixed actions (checkout, checkin, park, get_files, open_cad, open_3d_preview …) and STEP exports per program – never names, numbers, paths or anything typed. *Settings → Privacy → Show what is sent* shows it. The server shares an aggregate with the developers only if its administrator turns that on (admin guide: *Privacy*) |
| Report a problem | *Help → Report a Problem…* and *Report…* on every error message (toasts and dialog errors, #381): one review dialog with the title "<action>: <error>" (cut at a word, `…`), a description with the action, key inputs, the error and the versions, "What were you trying to do?" (left out when empty) and, ticked, the last 50 error messages of the session – all redacted in Rust like crash reports (`src/report.rs`: home folder → `~`, addresses → `[url]`, tokens → `[token]`). Nothing leaves the app without a click: *Open issue form* (GitHub, or *Settings → Report problems to*: the operator's own form, which gets `title` and `body`, or the policy `ReportUrl`; a long text is cut in the link and copied in full), *Copy report*, *Save report…* (work offline). The web UI's error pages have *Copy details* (status, message, page, time, version) |
| Managed deployment | Policies per computer or user, read once at start and never sent (`src/policy.rs`, #378): `ServerUrl` (+ `AllowOtherServers`), `CheckForUpdates`, `CrashReportPrompt`, `WorkspaceRoot`, `OrganizationName`, `UsageStatistics` – Windows registry `Software\Policies\TeamCraft` (HKLM wins over HKCU; ADMX/ADML in `deploy/desktop-policies/windows`), macOS managed preferences of `io.github.rudalabs.teamcraft` (`plutil`), Linux `/etc/teamcraft/policies.json`. Wrong types and out-of-range values count as not set; fixed settings show *Set by your organization* and are disabled; *Managed by …* in Settings and About. Admin guide: *Deploying the desktop app* |
| Crash reports | Panics, uncaught UI errors and sessions that ended unexpectedly (web view crash, force-quit hang) are written to a local, redacted report (no home folder, server addresses, tokens or item data). At the next start the app shows it and asks: *Report on GitHub…* (a prefilled issue you submit yourself), *Copy* or *Delete report*. Nothing is sent without that choice; *Settings → Crash reports* turns the question off |
| Language | *Settings → Language* (#379): *As the system* (`navigator.languages`: exactly, then by base language, else English), *English*, *Deutsch* – kept on this computer, applied at the next start. Messages come from `shared/i18n/<lang>.json` through `shared/js/i18n.js` (`t(id, params)`, English fallback); translated so far: the *Settings* dialog |
| Appearance | The same skins as the web UI: *TeamCraft* follows the light or dark appearance of the operating system (and switches with it), *TeamCraft Light* and *TeamCraft Dark* stay fixed, the distribution skins are dark. The window chrome (on Linux the GTK menu bar) asks the system for the matching light or dark variant; a GTK theme without a dark variant keeps its light menu bar (the screenshot job sets `GTK_THEME=Adwaita:dark` for its dark run) |

Keyboard: `Ctrl/Cmd+N` new item, `Ctrl/Cmd+F` search, `F5` refresh,
`Ctrl/Cmd+1/2/3/4/5` Home/Worklist/Changes/Agent/Projects, `Ctrl/Cmd+W` close tab,
`Ctrl+PageUp/PageDown` (or `Ctrl+Tab` / `Ctrl+Shift+Tab`) previous/next tab,
`Ctrl+Shift+PageUp/PageDown` move the active tab left/right; zoom with
`Ctrl` + mouse wheel or `Ctrl/Cmd` + `+` / `-` / `0`.

## Continuous integration

`.github/workflows/desktop.yml` runs on every change to `desktop/`,
`static/`, `shared/` or `media/`: `cargo fmt`, `clippy` and the tests, a build, and a
**start-up smoke test** (the app is started with a fresh profile – on Linux
under a virtual display – and must still be running after 20 seconds) on
Linux x86_64 (Ubuntu 24.04 and 26.04) and ARM64, macOS ARM64 and Windows
x86_64; Windows ARM64 runs as a preview that never blocks. A separate job
parses every script of `ui/`, the web UI and `shared/` (`node --check`; Node.js is only
used by CI, the app has no build step), checks the module imports and runs the shared
rules against `shared/test-vectors/`. Nothing is packaged or published.

## Download or run from source

The installers of every tagged version (0.x pre-releases) are on the
[Releases page](https://github.com/WARR-eV/TeamCraft/releases): Linux
AppImage/`.deb`/`.rpm` and a Flatpak bundle for x86_64 and aarch64, macOS
`.dmg`, Windows x64 and ARM64 `.msi`/`-setup.exe` plus a portable `.zip` with
`TeamCraft.exe`, which needs the WebView2 runtime of Windows 10/11 – see
*Download* in the [main README](../README.md#download).

**With local server.** `TeamCraft-with-local-server_…` (Windows x64
`-setup.exe`, macOS `.dmg`, Linux x86_64 `.AppImage`) is the same app plus the
server (`teamcraft-server`, a Tauri `externalBin`) and PostgreSQL's programs
(resources `postgresql/`), built as an app of its own: *TeamCraft Local*, identifier
`io.github.rudalabs.teamcraft.local` (the overlay sets `productName` and `identifier`; the token's keychain
service follows the identifier, `settings::set_keychain_service`), so it never takes over the desktop app's
server and token. Without a configured server (its first start) it starts the local server by itself; its
connect screen offers *Try TeamCraft on this computer* (`src/localserver.rs`): the app starts `teamcraft-server local
--port <p> --exit-with-stdin`, waits for `/readyz` and signs in through the
browser; the server stops when the app ends. The release job `local-server`
builds it with `tauri.local-server.conf.json`; to build it locally, put the
server binary at `binaries/teamcraft-server-<target triple>[.exe]` and
PostgreSQL's `bin/`, `lib/`, `share/` under `local-server/postgresql/`, then
`cargo tauri build --config tauri.local-server.conf.json`. See
[Try it on your computer](../docs/src/guide/local-server.md).

To build and run the app locally, Linux needs
the WebKitGTK development files:

```sh
# Ubuntu / Debian
sudo apt install libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
# Arch / CachyOS
sudo pacman -S webkit2gtk-4.1 libappindicator-gtk3 librsvg xdotool
```

```sh
cd desktop
cargo run                      # start the server first: cargo run in the repository root
cargo test                     # settings, API paths, PKCE + loopback sign-in, working-folder layout, Get and migration planning, read-only files
cargo clippy --all-targets -- -D warnings
```

Windows: WebView2 is part of Windows 10/11. On **Windows ARM64** the build needs
the ARM64 MSVC component and LLVM (`clang`, for the `ring` crate) – setup, a
build in a Parallels VM (about 3 minutes) and a server on another computer are
in [platforms/windows.md](../platforms/windows.md).

On the first start enter the server address (`localhost:8080` for a local
server, `host:port` otherwise) and click **Sign in with the browser**: the browser asks you to
confirm the sign-in for this computer, the app receives a personal access
token (expires after 30 days without use, every use renews it; revocable on your account page in the web UI).
When the server runs a version the app does not fit (before 1.0: another
`major.minor`; from 1.0: another major version or an older minor version), the
sign-in screen – or, if you are already signed in, a toast in the main window –
shows a warning once; signing in and working stay possible. The rules are in the
[support policy](../docs/src/admin/support-policy.md#desktop-app-and-server-versions)
(`src/compat.rs`).

Server addresses with plain `http://` to another computer get a warning in the connect dialog
and in *Settings*: the token and all data would cross the network unencrypted (`localhost` and
loopback addresses are fine).

Update notice: at most once a day the app asks the GitHub releases API
(`api.github.com/repos/WARR-eV/TeamCraft/releases/latest`, no redirects, 10 s timeout) for the latest
release. A newer version shows a notice in the status bar; it opens the **update window** (#377): the
release notes (`releases/tags/v<version>`), *Update now* / *Download*, *Skip this version* (= *×*),
*Remind me later* (three days) and *Release page*. *Update now* picks this installation's file
(`update::install_kind`, `update::pick_asset`: MSI under *Program Files*, `-setup.exe` under
`AppData\Local`, the portable zip, the `.dmg`, the AppImage; *with local server* builds their own
files), downloads it into the Downloads folder – only from
`github.com/WARR-eV/TeamCraft/releases/download/v<version>/`, redirects over https only, at most 1 GiB – and
checks it against the release's `SHA256SUMS.txt`; a missing line or a mismatch deletes the file and
nothing changes. A verified Windows installer starts after a question and the app closes; a `.dmg`
opens; a zip or AppImage is shown in its folder (replacing it in place is not built yet). Flatpak and
`.deb`/`.rpm` installations show the package-manager way instead. Nothing is downloaded without a
click, and not while a check-in or Get runs. *Help → Check for Updates…* asks at once, also with the
daily check off, and says *up to date* or the error. *Settings → Updates* switches the daily check off.

Where the app keeps its data:

* settings: the per-user configuration directory
  (`~/.config/io.github.rudalabs.teamcraft`, `~/Library/Application Support/io.github.rudalabs.teamcraft`,
  `%APPDATA%\io.github.rudalabs.teamcraft`);
* the token: the OS keychain (Windows Credential Manager, macOS Keychain, the Secret Service on
  Linux – GNOME Keyring, KWallet), entry `io.github.rudalabs.teamcraft` / `api-token`. Without a
  keychain (a Linux session without Secret Service) it falls back to the file `token` in the
  configuration directory, readable by the user only, and the app warns at the start and in
  *Settings*; the next start with a keychain moves the file into it (also the file of older app
  versions). On macOS an unsigned build may ask once whether TeamCraft may use the keychain item (again after an update);
* the working folder: `Documents/TeamCraft/<project group>/<folder>/<file>` with `Documents/TeamCraft/.teamcraft/state.json` (configurable in *Settings*).
* TeamCraft Local (downloads *with local server*): its own settings folder and keychain service
  `io.github.rudalabs.teamcraft.local` (`~/.config/io.github.rudalabs.teamcraft.local`, …), so its server and
  token are separate from the desktop app's;
* the local test server (downloads *with local server*): its data in the per-user data directory shared
  with `teamcraft local` (`~/.local/share/teamcraft/local-server`, `~/Library/Application Support/TeamCraft/local-server`,
  `%LOCALAPPDATA%\TeamCraft\local-server`), its port in the settings, its output in `local-server.log` next to them.

## Layout

`build.rs` copies the assets the app shares with the web UI into `ui/static/`
(git-ignored): the whole `shared/` tree (skins, fonts, the 3D viewer, the assistant,
formatters, state glyphs, markdown-lite) to
`ui/static/shared/` and the logos and icons from `media/`. The app loads them
from the same URLs as the web UI (`/static/shared/…`, `/static/media/…`); the
test `shared_ui_files_are_mirrored` checks the copy byte for byte. What belongs
into `shared/` and its rules: [`shared/README.md`](../shared/README.md). Desktop modules
import shared code by URL (`import { label } from "/static/shared/js/format.js";`) and
never keep their own copy: `scripts/check-desktop-modules.mjs` fails on a desktop module
that declares a name a shared module exports.

```text
src/main.rs        Tauri setup, native menu, state
src/commands.rs    the commands the UI calls (every network and file access)
src/api.rs         REST client (/api/v1, bearer token) – the web view never talks to the server
src/abortable.rs   Cancel of long operations: streamed check-in body that stops half-way, Pack and Go download dropped on cancel
src/signin.rs      browser sign-in: PKCE, loopback listener on 127.0.0.1
src/localserver.rs *Try TeamCraft on this computer*: finds the bundled teamcraft-server and postgresql/bin, starts `teamcraft-server local`, waits for /readyz
src/compat.rs      version handshake (pure): does the app fit the server version (also tested by tests/desktop_compat.rs)
src/layout.rs      working-folder rules (pure): folder mapping, state.json, Get planning, migration planning, local changes before Check in all / sign-out
src/resume.rs      Get resumes a large file half-way: partial file per content, Range/If-Range, SHA-256 check before the rename
src/workspace.rs   Get, status, read-only handling, check-in, migration of the old layout
src/settings.rs    settings.json (incl. the configured agents), the token in the OS keychain (file fallback), http:// check
src/update.rs      update notice and window: latest GitHub release at most daily, version comparison, install kind,
                   release file, SHA256SUMS.txt verification of the download (#377)
src/scprops.rs     item number, revision and name written into a SolidCraft design's properties through solidcraft-cli (#369)
src/scbridge.rs    the folder shared with the SolidCraft plugin: heartbeat, revision state, the plugin's requests (checked, then the UI)
src/cadlock.rs     files a CAD program has open, from its lock files (SolidCraft `.~<file>.lock`; stale locks ignored)
src/crash.rs       crash reports: panic hook, session markers (unexpected exits), UI errors – redacted, kept locally until the user shares them
src/acp.rs         Agent Client Protocol client: agent subprocess, JSON-RPC over stdio, session, permissions, workspace-only reads
src/autoallow.rs   (pure) which permission requests are allowed without a prompt: read-only TeamCraft MCP tools only, fail closed
ui/                index.html, app.css (layout), js/ (ES modules per view, no build step; js/main.js starts the app)
capabilities/      Tauri permissions of the window (dialogs only – files and URLs are opened by Rust)
icons/             app icons, rendered from icons/app-icon.svg with `tauri icon`
```

The browser sign-in uses the server endpoints `/desktop/authorize` (web page,
confirmation) and `POST /api/v1/desktop/token` (code + PKCE verifier → token);
see `src/services/desktop.rs` in the server.
