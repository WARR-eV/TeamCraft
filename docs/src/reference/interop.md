# FieldCraft and SolidCraft

[SolidCraft](https://github.com/tangency-labs/solidcraft) (parametric CAD)
and [FieldCraft](https://github.com/tangency-labs/fieldcraft) (FEA/CFD) are
open-source engineering tools in pure Rust by Tangency Labs (MIT OR
Apache-2.0). TeamCraft manages their files like those of any other CAD
program:

| File                                                                                                                                | TeamCraft                                                                                                                        |
|-------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|
| SolidCraft design `.sc` (ZIP container with `container.json`, or JSON with `"format": "solidcraft/N"`) and the legacy `.solidcraft` | recognised as *SolidCraft design* (`.sc` is checked by content, so scripts with the same extension are not mistaken for designs); the picture SolidCraft embeds (`cache/thumbnail.png`) is the preview image when its key still names this document; an assembly's linked designs (`links`) are its file references, counted by their placements |
| SolidCraft design properties (`metadata` of the document: `project`, `part_number`, `revision`, `author`, `organization`, `description`, `license`) | read at upload and check-in (`src/scdesign.rs`, text values only, 500 characters each) and stored with the file version (`file_versions.properties`); listed with the file, searchable, compared with the item number, revision code and name on the revision page (a notice, no automatic change), and used for an empty name or description of a new item (web) and of an item from *Import a CAD folder* (description); see [Files](../guide/files.md#design-properties-of-solidcraft-designs) |
| SolidCraft lock file `.~<design>.lock` (JSON: computer, user, refresh time) | never uploaded; the desktop app shows a design that is open in SolidCraft and warns at check-in |
| FieldCraft model `.fieldcraft` (ZIP: `fieldcraft.json`, `model.json`, `mesh.bin`, `journal.json`)                                   | recognised as *FieldCraft analysis model*                                                                                        |
| `solidcraft-cli` on the user's computer | the desktop app exports `.sc` designs to STEP with it and writes the item data into a checked-out design (`part_number`, `revision`, `description`, and TeamCraft's object under `teamcraft.pdm`) (*STEP* in the Workspace tab, *Create STEP exports … first* at check-in), on Windows, macOS and Linux; the server never runs it |
| SolidCraft BOM CSV (`assembly.bom`: `item, part_number, description, revision, count, definition, material, quantity`) | *Import BOM* without mapping: nested items (`1.1`) give the levels, `definition` the name, `count` (counted in all) becomes the quantity per parent row; `revision` and `quantity` (an amount such as a length) stay extra columns |
| SolidCraft assembly `.sc` as the BOM file | *Import BOM* (web and desktop) reads the design itself (`src/scbom.rs`): one line per component placed in a parent, quantity = placements (× a plain-number `bom_quantity`), part number, description, material from the part properties; `purchased` and standard parts marked purchased and not expanded, `phantom` passes its contents up, `reference` left out; revision and linked design file as extra columns |
| STEP, 3MF (incl. SolidCraft's face-tagged 3MF), STL, OBJ exports                                                                    | 3D preview in the browser and the desktop app; STEP is searchable and scanned for references                                     |
| Abaqus input decks (`.inp`) written by FieldCraft                                                                                   | recognised as *Abaqus input file*                                                                                                |

A typical workflow: keep the `.sc` design together with its STEP or 3MF
export in one TeamCraft revision – the design for editing in SolidCraft, the
export for the preview, the BOM and downstream tools; attach the
`.fieldcraft` model to the same item, or to an analysis document that
references it.

The three projects share their approach – one Rust build, a desktop app and
a command line, open formats – but not code: TeamCraft is licensed under the
MPL-2.0 and does not include SolidCraft or FieldCraft code (the plugin for SolidCraft below contains SolidCraft's interface definition, MIT).

What TeamCraft reads from a `.sc` design, and the sources of those findings, are
recorded in the research page [SolidCraft designs](../research/solidcraft-design.md).

## TeamCraft plugin for SolidCraft

`plugins/solidcraft` is a plugin for SolidCraft's plugin runtime (a
WebAssembly component, `craft:plugin@0.2.0`, built by the workflow
*SolidCraft plugin*, see its [README](https://github.com/WARR-eV/TeamCraft/tree/main/plugins/solidcraft)).
SolidCraft's plugins have no network access, so it works on the open design
only – with the TeamCraft object the desktop app's *Item data* writes
(`teamcraft.pdm`):

* **TeamCraft status** – the item and revision of the design, the link to the
  revision page, whether part number and revision still match;
* **Check for TeamCraft** – missing part numbers, a part number or revision
  that differs from the linked item, BOM rows without a part number, one part
  number with two descriptions, quantities SolidCraft cannot add up, linked
  designs with an absolute path or the same file name twice;
* **Link to TeamCraft item** / **Unlink from TeamCraft** – write or remove the
  item data from inside SolidCraft, for designs edited without the desktop
  app.

With the desktop app running, the plugin also shows the design's live state
and sends requests to check it out, check it in or open it, through a folder
shared with the app – see
*TeamCraft inside SolidCraft* in the desktop guide
([How files appear on your computer](../guide/desktop.md#how-files-appear-on-your-computer)).

Install: copy the package folder `teamcraft` into SolidCraft's plugins folder
(*Preferences › Plugins › Show in folder*) and enable it. The plugin is
TeamCraft's own code (MPL-2.0); it contains SolidCraft's interface definition
`plugin.wit` unchanged under SolidCraft's MIT licence, which a component for
SolidCraft's runtime needs.
