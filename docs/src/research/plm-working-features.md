# Popular features of commercial PLM systems

Which working features do users of an established commercial PLM system rely
on most, which of them does TeamCraft already have, and which come next? The
study looked at one widely documented enterprise system as an example.
Date: 2026-11-04.

## The kind of system studied

|               |                                                                     |
|---------------|---------------------------------------------------------------------|
| Kind          | enterprise PLM for large manufacturers (rich client and web client) |
| Core objects  | items, item revisions, datasets (files), BOM lines, change objects  |
| Typical users | design, manufacturing and change engineers                          |

## Most used features

| Feature                                                   | Commercial PLM                            | TeamCraft                                                                   |
|-----------------------------------------------------------|-------------------------------------------|-----------------------------------------------------------------------------|
| Check-out / check-in with locks                           | ✓                                        | implemented                                                                 |
| Versions and revisions, rollback                          | ✓ (revisions, dataset versions)          | implemented                                                                 |
| Lifecycle states, release workflow, reviews               | ✓ (workflows, signoffs)                  | implemented (change orders, four-eyes review)                               |
| Search by attributes and full text                        | ✓                                        | implemented                                                                 |
| **Saved searches**                                        | ✓ (saved queries)                        | **implemented now**                                                         |
| **Favorites / personal folder**                           | ✓ (favorites, home folder)               | **implemented now**                                                         |
| Inbox / worklist                                          | ✓ (worklist, tasks)                      | implemented (dashboard, notifications)                                      |
| Subscriptions / notifications                             | ✓                                        | implemented (watch, bell, e-mail, Mattermost)                               |
| Multi-level BOM, revision rules, where-used               | ✓ (structure editor)                     | implemented                                                                 |
| **BOM comparison between revisions**                      | ✓ (structure compare)                    | **implemented now**                                                         |
| **Save As (copy with files and BOM)**                     | ✓                                        | **implemented now**                                                         |
| References between CAD files (xrefs)                      | ✓                                        | implemented (reference scan, change propagation)                            |
| Impact analysis of changes                                | ✓ (change management)                    | implemented                                                                 |
| 3D viewing                                                | ✓ (JT visualisation)                     | implemented (STEP, meshes, point clouds; embedded previews of native files) |
| Export (Excel/CSV, packages)                              | ✓                                        | implemented (CSV, Pack and Go ZIP)                                          |
| Classification / custom attributes                        | ✓ (classification)                       | planned (OPEN_WORK #5)                                                      |
| Configurable numbering schemes                            | ✓                                        | planned (OPEN_WORK #4)                                                      |
| Transmittals (issue documents to externals with a record) | – (common in document management systems) | planned                                                                     |
| Markup / redlining                                        | ✓                                        | planned                                                                     |
| Effectivity, multi-site, ERP integration                  | ✓                                        | not planned for now                                                         |

## Decision

Implement the four most used features TeamCraft is missing and that fit its
data model without new concepts: **favorites**, **saved searches**, **Save As** (a new item from an existing revision:
attributes, files and BOM)
and **BOM comparison** between two revisions. Numbering schemes, custom
attributes, transmittals and redlining stay in `docs/OPEN_WORK.md`.

## Sources

The *Commercial PLM* column reflects the generally documented standard
functions of an enterprise PLM system's rich and web clients (items, datasets,
structure editor, saved queries, favorites, workflows, change management,
classification); vendor documentation itself was not read in this
investigation.
