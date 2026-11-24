# CAD setup files of the Getting started wizard

Question: which small setup files can the desktop app's *Getting started*
wizard (#422, #478) create so that Inventor, NX and Solid Edge find the parts
of an assembly in a project group's folder of the working folder – and what
is publicly known about their formats? Date: 2026-11-24.

The rules of the wizard (from #422): nothing is ticked by default, the content
of every file is shown before anything is written, a file that exists is never
overwritten, and the files go into the folder of the chosen project group
(`<working folder>/<group>/`).

## Findings

| Program | Mechanism | Publicly documented | TeamCraft writes |
|---|---|---|---|
| Inventor | project file `.ipj` (XML) with the workspace, workgroups and libraries; the workspace is searched first for non-library references and defaults to the folder of the project file | the concepts and the API (`DesignProject.WorkspacePath`, `UsingUniqueFileNames`, `ProjectType`, `LibraryPaths`); **not** the XML schema of the file | `<group>/<group>.ipj`, minimal: root `InventorProject`, one `WorkspacePath` of `.\` |
| NX | *Assembly Load Options*: *Load* = *From Search Folders* with an ordered list of folders; a folder ending in `...` includes its subfolders; the options can be saved to `load_options.def`, which the environment variable `UGII_LOAD_OPTIONS` can name | the dialog, the search-folder list, the `...` suffix, `UGII_LOAD_OPTIONS`; **not** the line syntax of `load_options.def` (the vendor's knowledge-base articles need a sign-in) | `<group>/NX load options (TeamCraft).txt`: the search folder `<group folder>\...` and where to enter it – no `load_options.def` |
| Solid Edge | link management file `LinkMgmt.txt`: the keywords `CONTAINER`, `RELATIVE`, `ABSOLUTE` (order of the link resolution) and folders between `BEGIN SEARCH PATH` and `END SEARCH PATH`, searched recursively, used only when the other methods fail; its folder is set in *Options → File Locations → Link Management* | yes, with an example file in the Solid Edge help | `<group>/LinkMgmt.txt` with the three keywords and the group's folder as the one search path |

### Inventor `.ipj`

* Autodesk's help describes a project as a text file in XML format that
  stores the paths; file names are stored relative to the project root
  (written `.\`); the workspace is the first location searched for
  non-library references, only one workspace is allowed and it defaults to
  the project file's root folder.
* The Inventor API's `DesignProject` object names the settings
  (`WorkspacePath`, `UsingUniqueFileNames`, `LibraryPaths`, `ProjectType`,
  `OldVersionsToKeep`, …). The API documents the settings, not the file.
* An Autodesk forum question for a documentation of the XML format was
  answered with the API, not a specification: the file layout is not
  publicly documented.
* The root element `InventorProject` and the element `WorkspacePath` come from
  the maintainers' own knowledge of project files Inventor writes (and match
  the API property name). They were **not** checked with Inventor. TeamCraft
  therefore writes only those two and nothing else (no project type, no
  library paths, no unique-file-name setting); Inventor fills in its defaults
  and rewrites the file when the project is saved in *Projects*. If Inventor
  refuses the file, the same project is made in Inventor's *Projects* dialog
  (*New*, single user, workspace = the group's folder).
* Because the workspace is `.\`, the file holds no absolute path: it keeps
  working when the working folder moves. The desktop app already uses the
  nearest folder with exactly one `.ipj` as the project of a file it opens
  (`cadlaunch::project_file`, #350) – the group's `.ipj` is that file.

### NX load options

* Public pages (PC-DMIS and Windchill documentation of their NX
  integrations, Siemens knowledge-base titles, forum threads) agree: with
  *From Search Folders* NX reads an ordered list of folders; a folder written
  with a trailing `...` (`C:\projects\...`) includes all its subfolders
  (slower on deep trees); the settings can be saved as the default or to a
  file, `load_options.def`, which `UGII_LOAD_OPTIONS` points to.
* The NX Open API (`NXOpen.LoadOptions`) documents the same settings
  (`LoadMethod`: *as saved*, *from directory*, *search directories*; search
  directories with a flag for their subdirectories) – the API, not the file.
* The syntax of `load_options.def` itself was not found in a public source.
  TeamCraft does not guess it: it writes a plain text file with the search
  folder and the steps in the dialog. Writing `load_options.def` directly is
  left open until its syntax is confirmed with NX (or a public source).

### Solid Edge `LinkMgmt.txt`

* The Solid Edge help (*Managing documents*, the link management topic)
  documents the file: `CONTAINER` looks in the folder of the container
  document, `RELATIVE` relative to it as at placement or save, `ABSOLUTE` the
  exact saved path; the order of the keywords is the order of the attempts
  and keywords can be reordered or removed; the folders between `BEGIN
  SEARCH PATH` and `END SEARCH PATH` are searched (subfolders recursively,
  the last path first) when the other methods fail; changes apply to
  documents opened afterwards. By default the file is in Solid Edge's
  *Preferences* folder; *Options → File Locations → Link Management* sets
  another location.
* The help names no comment syntax, so the generated file has no comments.
  It keeps the documented default order and adds the group's folder as the
  only search path. Solid Edge reads one link management file: pointing it
  at the group's file replaces the user's own for all documents – the guide
  says so.

## Decision

* The desktop app writes the three files with a pure Rust function
  (`desktop/src/cadsetup.rs`, unit-tested: paths with spaces, umlauts and
  Windows paths, XML escaping, the refusal of existing files); the web view
  sends only the group code and the kind, never a path or content.
* Files are created with `create_new`: a file that exists – also one that
  appeared after the preview – is never overwritten; the wizard says so.
* The group's folder must exist (the group was fetched with *Get*), so no
  stray folders are made.
* The files are a **starting point**: the wizard and the guide say to check
  them in the CAD program. Not verified with Inventor, NX or Solid Edge yet
  (#478 stays open for that).

No vendor file was copied, decompiled or used as a template; the content is
written from the public sources below and the maintainers' own knowledge.

## Sources

* Autodesk Inventor help: [About Search Paths in Projects](https://help.autodesk.com/cloudhelp/2023/ENU/Inventor-Help/files/GUID-0F34FEBE-4A2A-4494-A869-FC904F15F2BE.htm),
  [About Projects](https://help.autodesk.com/cloudhelp/2020/ENU/Inventor-Help/files/GUID-FB5EA98D-E486-4DF2-AF16-C19A8A09AB69.htm)
* Inventor API: [DesignProject Object](https://help.autodesk.com/cloudhelp/2024/ENU/Inventor-API/files/DesignProject.htm)
* Autodesk forum: [ProjectFile (ipj) Documentation](https://forums.autodesk.com/t5/inventor-programming-forum/projectfile-ipj-documentation/m-p/1672774)
  (no public file specification)
* NX load options: [PC-DMIS – NX load option](https://docs.hexagonmi.com/pcdmis/2019.1/en/helpcenter/mergedProjects/settingseditor/LoadOption.htm),
  [Windchill Workgroup Manager – Assembly Load Options (NX)](https://support.ptc.com/help/windchill/r13.1.1.0/en/Windchill_Help_Center/WWGMforNX/WWGMNXAdvTechLoadOptions.html),
  [NX Open `LoadOptions`](https://docs.plm.automation.siemens.com/data_services/resources/nx/12/nx_api/custom/en_US/nxopen_python_ref/NXOpen.LoadOptions.html)
* Solid Edge help: [Managing documents – link management](https://soliddna.com/SEHelp/ST6/EN/managing_documents/openman1a.htm),
  [File Locations page (Solid Edge Options)](https://soliddna.com/SEHelp/ST5/EN/a_h/filelo1d.htm)
