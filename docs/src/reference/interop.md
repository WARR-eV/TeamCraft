# FieldCraft and SolidCraft

[SolidCraft](https://github.com/tangency-labs/solidcraft) (parametric CAD)
and [FieldCraft](https://github.com/tangency-labs/fieldcraft) (FEA/CFD) are
open-source engineering tools in pure Rust by Tangency Labs (MIT OR
Apache-2.0). TeamCraft manages their files like those of any other CAD
program:

| File                                                                                                                                | TeamCraft                                                                                                                        |
|-------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|
| SolidCraft design `.sc` (ZIP container with `container.json`, or JSON with `"format": "solidcraft/N"`) and the legacy `.solidcraft` | recognised as *SolidCraft design* (`.sc` is checked by content, so scripts with the same extension are not mistaken for designs); the picture SolidCraft embeds (`cache/thumbnail.png`) is the preview image when its key still names this document; an assembly's linked designs (`links`) are its file references, counted by their placements |
| FieldCraft model `.fieldcraft` (ZIP: `fieldcraft.json`, `model.json`, `mesh.bin`, `journal.json`)                                   | recognised as *FieldCraft analysis model*                                                                                        |
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
MPL-2.0 and does not include SolidCraft or FieldCraft code.

What TeamCraft reads from a `.sc` design, and the sources of those findings, are
recorded in the research page [SolidCraft designs](../research/solidcraft-design.md).
