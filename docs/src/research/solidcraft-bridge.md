# Talking to the desktop app from inside SolidCraft

## Question

The TeamCraft plugin for SolidCraft (`plugins/solidcraft`) knows only what is
in the open design. Can it show live TeamCraft state (checked out by whom,
the latest revision on the server, the lifecycle state) and ask the desktop
app to act (check out, check in, open the item) – and still never fail hard
when the desktop app is not running?

## Sources

SolidCraft's open source (MIT OR Apache-2.0), read at the 0.45.0 tag and on
`main` (November 2026), and the installed SolidCraft 0.45.0 (Flatpak
`io.tangencylabs.solidcraft`) driven through `solidcraft-cli`. Only
documentation and source were read; no program code was decompiled.

| Topic | Where |
|-------|-------|
| Plugin interface (WIT `craft:plugin@0.2.0`): `files`, `clock`, `commands` | `plugins/solidcraft/wit/plugin.wit` (SolidCraft's, unchanged) |
| Folder grants in the manifest (`permissions.files`) | `crates/craft-plugin/src/lib.rs`, `crates/engine/src/plugin/package.rs` |
| What a refused or failed host call does | `crates/craft-plugin-wasm/src/native.rs` (`fail`, `check`, `read_file`) |
| Installing with folders (`plugin.install {approve: {folders}}`) | `docs/plugins.md` |
| Control channel (port file, token, `control.subscribe`, `ui.documents`) | `docs/control-protocol.md` |
| Plugin storage, panels, listeners | `docs/plugins.md` |
| Flatpak file access (`xdg-documents` only) | `packaging/linux/flatpak/` |

## Findings

### Channels SolidCraft 0.45 offers a plugin

| Channel | Works for | Verdict |
|---------|-----------|---------|
| **Files in a granted folder** | plugin reads state the desktop app writes; plugin writes requests | **use** |
| Design properties (`document.metadata.set`) | – | no: an edit (undo step, unsaved design, stale state saved and checked in) |
| Plugin storage (`storage.*`) | desktop reads only | no: writes only inside the plugin's run, saved into the design |
| Control channel (`127.0.0.1`, token file) | desktop → SolidCraft: which designs are open, unsaved | later, optional |
| Opening a URL (`teamcraft://`) | – | no: the plugin interface has no such import |
| Panels, live badges | – | not in 0.45 (no panel; listeners from 0.46, read-only) |

The plugin has no network, so it cannot reach the server or the desktop app
directly. A **folder grant** is the one channel in both directions:

- The manifest names a folder (`permissions.files: [{name, read, write}]`);
  the user picks the real path when installing, or `plugin.install` gets it
  (`approve.folders`). It is one fixed absolute folder, not "the design's
  folder". Hidden names *below* the grant root are refused, a hidden root is
  fine.
- A file that is not there is an answer the plugin gets back (`#17899`); a
  folder that was never granted, or any other refused call, ends the run
  with SolidCraft's own message – the plugin cannot catch it.
- Files are at most 1 MiB. A read entry cannot write files.
- Under Flatpak only `~/Documents` is reachable. The desktop app's default
  working folder is `~/Documents/TeamCraft`, so the bridge folder
  `<working folder>/.teamcraft/solidcraft` is reachable from both.

### Design properties stay for identity

`teamcraft.pdm` in the design (written by *Item data* or the plugin's
*Link*) keeps the stable identity: item number and revision. They are the
key into the bridge folder (the desktop app's working-folder state knows
them, not the revision ids). Live state never goes into the design.

### Only read entries show a result

SolidCraft's entry dialog shows the result of a `read` entry; an `edit`
entry's dialog closes with *done* in the status bar
(`crates/ui-egui/src/plugin_menu.rs`, `poll_dialog`), and messages a plugin
logs go to the session log, which the window does not show. A read entry may
only call read-classified commands. `document.inspect` is one: it returns the
design's metadata as saved (`teamcraft.pdm` and SolidCraft's properties as
text keys), `dirty` (unsaved changes) and the file `path`
(`crates/engine/src/cmd/inspect.rs`; tried on 0.45.0). `document.metadata`,
`assembly.bom` and `component.links` are classed as edit commands. So the
plugin's *status*, *check* and *live* are read entries over
`document.inspect`; the BOM and link checks are an edit entry of their own,
whose findings reach the CLI, MCP and the assistant.

## Decision

A **bridge folder** `<working folder>/.teamcraft/solidcraft/` that both sides
read and write, JSON files of at most 4 KiB, versioned:

One flat folder: a write into a subfolder that does not exist ends the
plugin's run, a missing file does not. Times are Unix seconds (the plugin's
`clock.now`).

- `desktop.json` – the desktop app's heartbeat (`schema
  "teamcraft.solidcraft.desktop"`, `version 1`, `written_at`, whether the
  server is reachable), rewritten every 30 s.
- `state-<key>.json` – the state of one revision in the working folder:
  item number, revision, who has it checked out (display name, and whether it
  is me), local changes, whether the check-out was released. `<key>` is the
  item number and revision, every byte outside `A-Z a-z 0-9` written as `_XX`,
  joined by `.` – reversible and safe as a file name.
- `request-<key>.<action>.json` – an intent from the plugin: `check_out`,
  `check_in` or `open`, one file per revision and action (asking twice
  replaces the first). The desktop app deletes it when it takes it; while it
  is there, the plugin's *live* says the request waits.

Rules:

- The desktop app writes atomically (hidden temp file, rename). Unknown
  fields are ignored; a newer major `version` says "update the plugin" or
  "update TeamCraft".
- **No secrets** in the bridge: no tokens, no server credentials, display
  names only. The folder is created `0700` where the system has modes.
- Requests are **untrusted intents**: the item and revision must be one in
  the desktop app's working-folder state and match the file name, requests
  older than 10 minutes are ignored, links, non-regular and oversized files are
  refused. `check_in` only opens the desktop app's own check-in dialog; it
  never runs silently.
- The existing entries (*status*, *check*, *link*, *unlink*) never touch
  files, so they keep working without the grant. The live part comes as new
  entries.

What the user sees:

| Situation | Message |
|-----------|---------|
| No state file | TeamCraft has no live state for {item} revision {rev}. Start the TeamCraft desktop app – it shares the state of the revisions in your working folder. |
| Heartbeat older than 2 minutes | The TeamCraft desktop app is not running (last seen {age} ago). As of then: … |
| Server not reachable | The TeamCraft desktop app has no connection to the server. As of {age} ago: … |
| Fresh state | Checked out by you · latest on the server B (you have A) · In work |
| Request while the desktop app is not running | Request saved. TeamCraft does it when the desktop app starts (within 10 minutes). |

### Works with 0.45 as it is, and what needs SolidCraft

| Part | 0.45 |
|------|------|
| State and requests through the bridge folder | yes |
| State refreshed when the user runs an entry | yes |
| A badge or panel that updates by itself | needs SolidCraft panels |
| Opening the TeamCraft page from the plugin | needs a URL import |
| Recovering from a missing folder grant inside the plugin | needs SolidCraft (the run ends) |
| Bridge folders outside `~/Documents` under Flatpak | needs a wider Flatpak permission |

The control channel (which designs are open and unsaved, *save before
check-in*) is a later step on the desktop side only.
