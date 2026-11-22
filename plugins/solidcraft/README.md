# TeamCraft plugin for SolidCraft

A plugin for [SolidCraft](https://github.com/tangency-labs/solidcraft)
(open-source parametric CAD) that shows and checks a design's TeamCraft
data inside SolidCraft. It is a WebAssembly component for SolidCraft's
experimental plugin runtime (`craft:plugin@0.2.0`, capability
`commands@0.2`).

SolidCraft's plugins have no network access, so the plugin never talks to a
TeamCraft server. It works with what is in the open design: its properties,
the TeamCraft object the desktop app writes (*Item data*, key
`teamcraft.pdm`: item number, revision, item and revision ids, server), the
bill of materials and the linked designs. Check-out, check-in and the
working folder stay in the TeamCraft desktop app.

## Entries

Each has its own button in the ribbon's Plugins group (labelled as in the
table) and a row in the Plugins menu.

| Entry | Effect | What it does |
|-------|--------|--------------|
| **status** – *TeamCraft status* | read | The TeamCraft item and revision the design belongs to, the page of the revision on the server, and whether the design's part number and revision still say the same. |
| **check** – *Check for TeamCraft* | read | What keeps the design from going into TeamCraft cleanly: no part number, a part number or revision that differs from the linked item, unsaved changes. `ready` is true when only notes are left. |
| **check-assembly** – *Check assembly for TeamCraft* | edit¹ | The bill of materials and the linked designs: BOM rows without a part number, one part number with two descriptions, quantities SolidCraft cannot add up, linked designs with an absolute path or two linked designs with the same file name. |
| **link** – *Link to TeamCraft item* | edit | Writes item number, revision and (optional) name into the design properties (*Part number*, *Revision*, *Description*) and TeamCraft's object – for designs edited without the desktop app. Ids written by the desktop app are kept while the item stays the same. One undo step; save the design afterwards. |
| **unlink** – *Unlink from TeamCraft* | edit | Removes TeamCraft's object; part number and revision stay. |
| **live** – *TeamCraft live status* | read | What the TeamCraft desktop app last shared about this revision: who has it checked out, local changes, requests it has not taken yet, unsaved changes – and whether the desktop app runs and reaches the server. Needs the folder (below). |
| **check-out** – *Check out in TeamCraft* | edit | Asks the desktop app to check out this revision; it asks you before it does. Needs the folder. |
| **check-in** – *Check in with TeamCraft* | edit | Asks the desktop app to open its check-in dialog for this revision. Save the design first. Needs the folder. |
| **open** – *Open in TeamCraft* | edit | Asks the desktop app to show this revision. Needs the folder. |

SolidCraft shows the result of a **read** entry in its dialog; an **edit**
entry's dialog just closes with *done*. *status*, *check* and *live* are read
entries over `document.inspect`, so their answers are shown.

¹ *check-assembly* only reads, but SolidCraft classes `assembly.bom` and
`component.links` as edit commands and refuses them from a read entry, so it
is an edit entry: its findings reach the CLI, MCP and the assistant, while
SolidCraft's dialog only says *done*.

SolidCraft commands it calls (declared in `plugin.json`, none of them reads
or writes files): `document.inspect`, `document.metadata.set`,
`document.metadata.remove`, `assembly.bom`, `component.links`.

## The folder shared with the desktop app

SolidCraft plugins have no network, so *live*, *check-out*, *check-in* and
*open* talk to the TeamCraft desktop app through one folder:
`<working folder>/.teamcraft/solidcraft` (by default
`~/Documents/TeamCraft/.teamcraft/solidcraft`). The desktop app creates it
while it runs, writes its heartbeat and the state of each revision it shows
there, and picks up the plugin's requests every few seconds
(`desktop/src/scbridge.rs`; format and reasons in
`docs/src/research/solidcraft-bridge.md`).

When installing the plugin, SolidCraft asks for the folder `teamcraft`: pick
that folder, with reading and writing. Without it, *status*, *check*, *link*
and *unlink* work as before; the four others end with SolidCraft's message
that the folder `teamcraft` is not granted.

The desktop app not running is never an error: *live* says so and shows what
it knew when it last ran, and a request is kept for 10 minutes and carried out
when the desktop app starts. Nothing secret is in the folder (display names
and states), and the desktop app treats each request as a wish: it asks before
a check-out and opens its own dialog for a check-in.

## When something fails

The plugin needs neither a TeamCraft server nor the desktop app: TeamCraft
not running never makes it fail. What it says when something does go wrong:

- **Bad input** to *Link to TeamCraft item* names the field and changes
  nothing (*Item number is required. Nothing was changed.*).
- **A design without TeamCraft data** is *not linked*, with how to link it.
  **Damaged TeamCraft data** (not an object) is reported, with *Link* or
  *Unlink* as the fix.
- **A SolidCraft command that fails** ends the run: SolidCraft shows its own
  message, prefixed *TeamCraft:*, and keeps the design as it was. The plugin
  cannot catch it (SolidCraft 0.45). So the plugin only calls commands that
  answer on any design – a part without components gives an empty BOM and no
  links – and checks its input before it writes. On a host that hands the
  error back, the plugin adds what to do, and *check-assembly* reports a BOM or link
  list it could not read as a warning instead of *ready*.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-tools
./build.sh            # → target/package/teamcraft/ (plugin.json, plugin.wasm, SHA256SUMS) and teamcraft.zip
cargo test            # the entries against a fake design, on this computer
# the entries on the real SolidCraft, with the cases in tests/:
cp -r tests target/package/teamcraft/ && solidcraft-cli plugin test target/package/teamcraft --require-cases
```

`build.sh` checks that the component imports nothing but `craft:plugin@0.2.0`
(no WASI). The GitHub workflow *SolidCraft plugin* builds and tests it and
keeps the package as an artifact.

## Install

Copy the folder `teamcraft` (the package; the folder name must be the plugin
id) into SolidCraft's plugins folder – *Preferences › Plugins › Show in
folder* – enable it there and review the approval SolidCraft shows. Installing
or enabling never runs plugin code; SolidCraft asks before the first run.

## Licence and sources

The plugin is TeamCraft's own code under the MPL-2.0. `wit/plugin.wit` is
SolidCraft's interface definition (`crates/craft-plugin/wit/plugin.wit` at
SolidCraft commit `94096b84f`), unchanged, under SolidCraft's MIT licence
(`wit/LICENSE-MIT`); it is needed to build a component for SolidCraft's
plugin runtime. Background: `docs/src/research/solidcraft-design.md`.
