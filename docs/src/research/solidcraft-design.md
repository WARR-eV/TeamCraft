# SolidCraft designs (`.sc`)

*2026-11-21*

## Question

What can TeamCraft read from a SolidCraft design without SolidCraft – a
picture, the linked designs, the bill of materials, the design properties –
and what does it take to keep up with a format that changes with almost
every release? (Issues #363–#371.)

## Sources

All findings come from SolidCraft's public documentation and its source
code, licensed MIT OR Apache-2.0, as of SolidCraft 0.46.1 (commit
`18c9760e3`, <https://github.com/tangency-labs/solidcraft>). They were read as
a specification; TeamCraft's readers are its own code and no SolidCraft code
is copied or vendored (see [FieldCraft and SolidCraft](../reference/interop.md)).

| Topic                                     | Source                                                     |
|-------------------------------------------|------------------------------------------------------------|
| Container, blobs, caches, limits          | `docs/design-container.md`                                 |
| Document model (components, links, parts) | `crates/doc/src/document.rs`, `crates/doc/src/assembly.rs` |
| Design and part properties                | `crates/doc/src/metadata.rs`, `crates/engine/src/cmd/metadata.rs` |
| Linked designs                            | `crates/engine/src/cmd/component_link.rs`                  |
| Bill of materials and its CSV             | `crates/doc/src/bom.rs`, `crates/engine/src/cmd/bom.rs`    |
| Lock and backup files                     | `crates/io/src/design_lock.rs`, `crates/io/src/safe_write.rs` |
| Command-line tool                         | `apps/solidcraft-cli/src/main.rs` (usage texts)            |
| Plugin runtime (no network)               | `docs/plugin-runtime.md`                                   |

## Findings

### Two encodings, one extension

* **ZIP container** (new designs): starts with `PK\x03\x04`.
  `container.json` = `{"format":"solidcraft-container/1"}` (earlier drafts:
  `solvecraft-container/1`), `document.json` = the parametric document.
* **Plain JSON**: the same document as one JSON object,
  `"format": "solidcraft/N"`. Still written by *Save As JSON*, scripts and
  MCP, and fully supported.
* The legacy extension `.solidcraft` holds either encoding. `.sc` is also used
  for command scripts, which is why `cadformat.rs` checks the content.
* The document format number `N` changes often (0.22.0 wrote 35); an older
  SolidCraft refuses a newer design. Unknown fields must be ignored by any
  reader outside SolidCraft.

### Container entries

| Entry                                       | Present                                                   | Content |
|---------------------------------------------|-----------------------------------------------------------|---------|
| `container.json`, `document.json`           | always                                                    | format marker; the document, imports' payloads replaced by blob references |
| `blobs/<sha256>.<step\|mesh\|png\|jpeg>`     | when the design imports STEP/meshes or holds images       | content-addressed inputs (SHA-256 of the uncompressed payload) |
| `cache/thumbnail.json`, `cache/thumbnail.png` | warm desktop saves with `cache: true` (the default); not in browser downloads, not with errors or an active sketch | 240 × 150 PNG (≤ 256 KiB), isometric view; key `solidcraft-thumbnail/1`, app version, SHA-256 of `document.json` and of the PNG |
| `cache/preview.json`, `cache/preview.bin`   | only saves with `preview: true` (not the default)         | display meshes, `SCMESH01`, ≤ 8 MiB / 100,000 triangles |
| `cache/model.json`, `cache/bodies.bin`      | desktop saves with `cache: true`                           | the evaluated model: native exact B-rep per body (`docs/native-brep.md`), mesh bodies as `SCMESH01` slices; keyed by a build digest |

SolidCraft's own reader limits: 256 MiB per part, 512 MiB expanded, at most
10,000 entries; ZIP64, multi-disk archives, duplicate names and path
traversal refused. Metadata is Deflate level 1, STEP/mesh blobs stored or
Deflate, PNG stored.

### Document contents TeamCraft can use

* `metadata` (design properties): text keys `project`, `part_number`,
  `revision`, `author`, `organization`, `description`, `license`; another
  tool's data as an object under a reverse-DNS key, kept untouched by
  SolidCraft. Limits: keys ≤ 128 bytes, text ≤ 4 KiB, ≤ 256 keys, ≤ 256 KiB.
* `components[]`: `id`, `name`, `parent` (0 = root), `metadata` (the same keys
  plus part-only `bom_type`, `material`, `bom_quantity`, `virtual`) and free
  `properties` (name → text).
* `occurrences[]`: `id`, `component`, `parent`, `name`, `transform`,
  `grounded`, optional `bom_type` override – the placements counted by the
  bill of materials.
* `links`: map component id → `DesignLink { path, feature, configuration?, … }`.
  `path` is relative to the assembly's folder once it is saved, `/`
  separators (absolute for an unsaved assembly). The linked bodies are also
  kept in the assembly, so a missing file is a stale link, not a broken
  assembly.
* `parts`: map component id → standard part (rebuilt from parameters, no
  file) – not a file reference; a purchased part for the BOM.

### Bill of materials CSV

`assembly.bom` writes the fixed columns `item, part_number, description,
revision, count, definition, material, quantity`, every field quoted; nested
rows numbered `1.1`, `1.1.1`. `count` is the number of placements, `quantity`
an amount (length, area …) where the definition has one.

### Files next to a design

* `.~<file>.lock`: JSON (computer, user, app version, process, opened and
  refreshed times) while the desktop app has the design open; refreshed every
  60 s, stale after 180 s, at most 4 KiB read.
* `<file>.bak`: the previous version, written on save.

Both are already skipped by the desktop app's `is_ignored`.

### Command-line tool

* `solidcraft-cli run --in design.sc --out design.step` (also `.3mf`, `.stl`,
  `.glb`; several `--out`). A failing feature fails the run and writes
  nothing unless `--allow-errors`.
* `solidcraft-cli snapshot --in design.sc --out shot.png [--view iso]`.
* `solidcraft-cli run script.json --in design.sc --save design.sc` runs
  commands such as `document.metadata.set` headless.
* Installers put it beside the app, not always on `PATH`.

### Plugins

SolidCraft's plugins are sandboxed WebAssembly components without sockets,
HTTP or fetch: an add-in inside SolidCraft cannot talk to a TeamCraft server.

## Decision

* Read `.sc` designs on the server with TeamCraft's own bounded ZIP/JSON
  reading: the thumbnail (#364), linked designs as references (#365), the
  BOM (#366) and the design properties (#367). **An unknown version, a
  missing entry or a malformed part is "nothing found", never an error** –
  the format moves daily and a file must never fail an upload.
* Parse only the fields needed, with serde defaults and unknown fields
  ignored; never depend on the format number.
* Shell out to `solidcraft-cli` only in the desktop app (#368, #369), like the
  Inventor and Solid Edge exports; conversions on the server stay pure Rust.
* The model cache (native B-rep) is not read now; a 3D preview without an
  export is a separate investigation (#371).
* Fixtures: small designs built in the tests (JSON, and ZIPs written with the
  `zip` crate) plus designs saved by SolidCraft once available.
