# Importing existing CAD folder trees

**Question:** how do PLM/PDM systems bring an existing folder of CAD data –
often hundreds of files, with project files – into the vault, and what should
TeamCraft's import do? Investigated on 2026-11-04.

## How other systems do it

| System                     | Tool                                                                        | What it does                                                                                                                                                                                                                                                             |
|----------------------------|-----------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Autodesk Vault             | **Autoloader**, or *Check In Project* in Inventor                           | scans the folder with the Inventor project file (`.ipj`), reports missing references, uploads in batches (up to 20 000–30 000 Inventor files), keeps the folder structure, works on a copy – the source files stay untouched; DWF creation can be switched off for speed |
| SolidWorks PDM             | copy/paste of folders, scan tools, VAR import programs                      | migration guides: scan first, map folders and properties, fix broken references and duplicates before the import, decide whether files come in at their existing revision                                                                                                |
| Solid Edge with a PLM      | *Data Preparation*: *Analyze*, then add to the PLM (with dry run)           | expects a flat source folder **without duplicate file names**; *Analyze* reports duplicate names, duplicate document numbers and broken links; item IDs are generated when the document number is empty; Design Manager can flatten folders and rename duplicates        |

The common pattern:

1. **Prepare** – one project per product (Inventor: gather the files and the
   `.ipj` into one project, e.g. with *Pack and Go*), fix broken references,
   make file names unique.
2. **Analyse first (dry run)** – a report of what will be imported, duplicate
   names, broken references; nothing is changed yet.
3. **Import** – in batches, from a copy (the source stays untouched), with
   generated or property-based numbers; viewables optional.
4. **Afterwards** – check the structure, release the data through the
   normal workflow.

## Decision for TeamCraft

TeamCraft matches CAD references by **file name** – so file
names must be unique within an import. The desktop app gets an **Import folder…** wizard:

* **Scan (dry run):** walks the folder, skips lock/backup files and
  `OldVersions` folders, sorts the files into models (parts, assemblies),
  drawings, exports (STEP, STL, PDF, …), project files (`.ipj`, …) and other
  files, and reports duplicate file names. Nothing is uploaded yet.
* **Plan:** one item per model file (assembly or part); a drawing and
  exports with the **same base name** go into the same item (the item's
  datasets); project files and other documents can go into one document
  item named after the folder. Numbers are generated, or taken from the file
  names when they are valid item numbers.
* **Import:** item by item with progress and stop, from the original files (read only); every new item is checked in at
  the end, so nothing stays
  locked; the server scans the assemblies for the files they reference.
* **Structure:** optionally BOM lines from the references found in the
  assemblies, with the instance count the server read from the assembly (`instances` of `GET …/references`: SOLIDWORKS
  2015+ component tree, NX
  structure); where the format does not tell (Inventor, Solid Edge, …)
  quantity 1, to be checked – exact quantities come from the BOM export of
  the CAD program (*Import BOM…*).
* **Release** afterwards with one change order, like every other release in
  TeamCraft (no import "as released" past the lifecycle).

An in-app tutorial (desktop *Help → Import a CAD folder*) explains the
preparation (Pack and Go, project file, unique names), the wizard and the
check afterwards.

## Detecting the CAD program and its version

The scan also tells which CAD program the folder belongs to, so the wizard
can offer it as the project group's **default CAD program** (TeamCraft
Desktop's *Check out and open* opens the group's files with it):

* **Program:** the format family (`format_info.family` / `family` of
  `GET /api/v1/formats`, from `src/cadformat.rs`) with the most files, by
  file extension. Ambiguous extensions (`.prt`, `.asm`, `.drw`, …) are not
  counted.
* **Version, Inventor:** Inventor writes the release that saved a file into
  its document properties as text, `2021 (Build 250183000, 183)` – UTF-16
  in an OLE property set (the OLE property set format is public:
  [MS-OLEPS](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oleps/);
  the property is the one Inventor shows as *Last Updated With* in its
  iProperties). Found in the two NIST Inventor 2021 parts of
  `tests/fixtures/cad` (`INV_nist_ctc_01_asme1_2021.ipt`,
  `INV_nist_ftc_11_asme1_2021.ipt`) with a plain `strings -el` text search –
  no program was run or analysed. The scan reads the first 4 MiB of at most
  40 Inventor files (assemblies first) and takes the newest year it finds in
  each (a file created with an older release carries that year as well);
  the newest year of all files is offered, because an older release cannot
  open files saved by a newer one (`desktop/src/folderimport.rs`
  `inventor_year`).
* **Version, other programs:** not read. SOLIDWORKS 2015+ keeps its
  metadata in compressed sections and the NIST samples show no plain
  version text; the user types the version on the group page if wanted.

The installed programs on the user's computer are found by their install
folders (Windows *Program Files*, macOS `/Applications`) and, for
SOLIDWORKS, the registry keys its installer writes – see
`desktop/src/cadlaunch.rs`; these are the installers' usual default
locations, not tested against real installations here.

## Sources

* Autoloader and Inventor projects:
  [Vault help – Autoloader](https://help.autodesk.com/cloudhelp/2025/ENU/Vault-General/files/GUID-EA9B2552-30B9-4789-9A5E-6580B8648981.htm),
  [Check in Project with Inventor](https://autodesk.com/support/technical/article/caas/sfdcarticles/sfdcarticles/CHS/How-To-Check-in-Project-using-Autodesk-Inventor-and-Autodesk-Vault.html),
  [Modify an Inventor project](https://help.autodesk.com/cloudhelp/2026/ENU/Vault-General/files/GUID-DB018A2B-8B78-401D-B45C-B0AEE3613D30.htm)
* SolidWorks PDM migration:
  [TriMech – data migration best practices](https://trimech.com/solidworks-pdm-data-migration-best-practices-guide/),
  [GoEngineer – manually importing CAD data](https://www.goengineer.com/blog/manually-importing-cad-data-into-solidworks-pdm-overview),
  [Javelin – how to prep for a PDM migration](https://www.javelin-tech.com/blog/2022/05/how-to-prep-for-a-solidworks-pdm-migration/)
* Solid Edge with a PLM:
  [Data Preparation tools](https://support.sw.siemens.com/en-US/okba/PL8547626/Using-the-SEEC-Data-Preparation-Tools-simplified-workflow/index.html),
  [Analyze](https://soliddna.com/SEHelp/ST5/EN/a_h/analyze2ad.htm),
  [Design Manager – flatten folder structure](https://support.sw.siemens.com/en-US/okba/PL8015842/Using-Design-Manager-to-flatten-folder-structure/index.html)

## Status

* **Implemented** in the desktop app: *Tools → Import CAD Folder…*
  (`desktop/src/folderimport.rs` for the scan, the plan and the import in
  `desktop/ui/app.js`) and the tutorial *Help → How to Import a CAD Folder*.
  Imported items are remembered per folder (browser storage of the app), so
  an interrupted import continues where it stopped.
* Tested: the folder scan (unit test) and the planning with a sample tree (items, duplicates, Creo iterations, project
  files). **Not yet tried with a
  real Inventor or Solid Edge project of hundreds of files** – the BOM lines
  depend on the heuristic reference scan of the server.
