# Set up CAD programs for a project group

An assembly file names its parts; the CAD program has to find them when it
opens the assembly. TeamCraft puts every file of a project group into one
folder tree on each computer – `<working folder>/<group>/<folder>/<file>`,
the same tree for everyone – and *Check out and open* or *Get…* copies an
assembly together with its parts into it. What is left is telling the CAD
program to look there. That is set up **once per project group**, by its
manager or a lead, in the CAD program itself.

TeamCraft creates no CAD settings or project files of its own: the desktop
app's *Getting started* only links this page, and nothing in the app is
shaped around one CAD program
([research: the first start of the desktop app](../research/first-start.md)).

## The rules for every program

* **Settings the team shares are vaulted files of the group.** A project or
  configuration file of a CAD program that the whole group uses is made in
  that program, then added to the group like any other file (*Add files…*,
  or *Import CAD folder…* for an existing tree) – checked in, revisioned,
  reviewed. Everyone gets it with their first *Check out and open* or
  *Get…*, at the same place relative to the group's folder. A change to it
  is a new version that everyone gets with their next *Get…*.
* **Keep paths relative to the group's folder.** Working folders sit in
  different places on different computers (`Documents/TeamCraft`, a policy
  folder, a second connection's own folder). A shared file that holds an
  absolute path works on one computer only.
* **One shared file per folder tree.** Two project files for the same tree
  compete; programs and TeamCraft's own launcher then cannot tell which one
  applies.
* **Personal and machine settings stay personal.** Search paths that a
  program keeps per user or per computer are set in that program's own
  dialogs, by each user once – described below.
* **Managed computers** configure CAD programs with their own deployment
  tools (the same way they roll out the program itself). TeamCraft ships no
  generator for CAD settings; see [Deploying the desktop app](desktop-deployment.md)
  for the policies of TeamCraft Desktop, e.g. `WorkspaceRoot` for where
  working folders go.

## Per program

Each section answers the same three questions: how the program finds the
parts of an assembly, what the group keeps in TeamCraft, and what each user
sets once. The sources are the programs' public documentation.

### Autodesk Inventor

* **How it finds parts:** through the active *project* (`.ipj`). Its
  workspace is searched first for non-library references and defaults to
  the folder of the project file; paths in it are stored relative to it.
* **What the group keeps:** one `.ipj` made in Inventor (*Projects → New*)
  with its workspace at the group's folder (`.\` relative to the file), saved
  at the top of the group's folder and added to the group. TeamCraft Desktop
  opens a file with the nearest folder that holds exactly one `.ipj` and
  reuses a running Inventor with that project.
* **What each user sets:** nothing beyond activating that project the first
  time (Inventor asks, or *Projects*).
* Sources: Inventor help,
  [About Projects](https://help.autodesk.com/cloudhelp/2020/ENU/Inventor-Help/files/GUID-FB5EA98D-E486-4DF2-AF16-C19A8A09AB69.htm),
  [About Search Paths in Projects](https://help.autodesk.com/cloudhelp/2023/ENU/Inventor-Help/files/GUID-0F34FEBE-4A2A-4494-A869-FC904F15F2BE.htm).

### SOLIDWORKS

* **How it finds parts:** when a referenced document is not at its saved
  path, SOLIDWORKS looks in open documents, then in the folders of
  *Tools → Options → System Options → File Locations → Referenced
  Documents*, then in its own search routine.
* **What the group keeps:** nothing program-specific – the assembly and its
  parts in the group's folder tree; keep references inside the tree.
* **What each user sets:** add the group's folder to *Referenced Documents*
  (with *Include sub-folders*) and switch on *Search file locations for
  external references* (*External References* options); without that
  option the list is ignored.
* Sources: SOLIDWORKS help,
  [Search Routine for Referenced Documents](https://help.solidworks.com/2025/English/SolidWorks/Sldworks/c_Search_Routine_for_Referenced_Documents.htm),
  [File Locations Options](https://help.solidworks.com/2025/English/SolidWorks/Sldworks/HIDD_Options_External_Folders.htm).

### Solid Edge

* **How it finds parts:** by the link management file (`LinkMgmt.txt`):
  container folder, relative path, absolute path in the order of its
  keywords, then the folders between `BEGIN SEARCH PATH` and `END SEARCH
  PATH` (searched with their subfolders) when those fail.
* **What the group keeps:** nothing program-specific; the assembly and its
  parts in the group's folder tree, so the container and relative methods
  find them.
* **What each user sets:** if links still break, the group's folder as a
  search path in the link management file (*Options → File Locations → Link
  Management* shows which file is used). Solid Edge reads one such file for
  all documents, so it is each user's own file, not a vaulted one.
* Sources: Solid Edge help (online copies),
  [Managing documents – link management](https://soliddna.com/SEHelp/ST6/EN/managing_documents/openman1a.htm),
  [File Locations page](https://soliddna.com/SEHelp/ST5/EN/a_h/filelo1d.htm).

### Siemens NX

* **How it finds parts:** by the *Assembly Load Options*. With *Load* set to
  *From Search Folders* NX looks in an ordered list of folders; a folder
  written with a trailing `...` includes its subfolders.
* **What the group keeps:** nothing program-specific; the assembly and its
  parts in the group's folder tree.
* **What each user sets:** *Assembly Load Options → From Search Folders*
  with the group's folder followed by `...`, saved as the default (or in a
  load options file the user or the computer's deployment points to).
* Sources: NX Open API,
  [`NXOpen.LoadOptions`](https://docs.plm.automation.siemens.com/data_services/resources/nx/12/nx_api/custom/en_US/nxopen_python_ref/NXOpen.LoadOptions.html);
  a measurement program's public documentation of the same dialog,
  [NX load option](https://docs.hexagonmi.com/pcdmis/2019.1/en/helpcenter/mergedProjects/settingseditor/LoadOption.htm).

### PTC Creo

* **How it finds parts:** in the session, the working directory and then
  the directories of its search path – the `search_path` option, or a
  `search.pro` file named by `search_path_file` – in the listed order.
* **What the group keeps:** nothing program-specific; the assembly and its
  parts in the group's folder tree.
* **What each user sets:** the group's folder as the working directory when
  opening its assemblies, or as a `search_path` entry in the user's
  configuration; deeper folder trees list their folders in a `search.pro`.
* Sources: Creo Parametric help,
  [About Searching For and Retrieving Models](https://support.ptc.com/help/creo/creo_lcd/usascii/lcd/layout/fund_two_sub/About_Searchi_For_and_Retrieving_Models.html),
  [To Load a search.pro File](https://support.ptc.com/help/creo/creo_lcd/usascii/lcd/layout/fund_two_sub/To_Load_a_search_pro_File.html).

### CATIA V5

* **How it finds parts:** by *Linked Documents Localization*
  (*Tools → Options → General → Document*): an ordered list of strategies –
  the folder of the pointing document, the relative folder, the folder of
  the link, other folders.
* **What the group keeps:** nothing program-specific; the product and its
  parts in the group's folder tree, so the relative strategies find them.
* **What each user sets:** the relative strategies first in the search order,
  and – if needed – the group's folder under *Other folders*. The list can
  be exported to a text file and imported on another computer.
* Sources: CATIA V5 help, *Linked Documents Localization* settings; the
  default search order as listed in
  [IBM support note HD50583](https://www.ibm.com/support/pages/apar/HD50583).

### Other programs

Programs whose assemblies keep their parts inside one file (or that do not
open assemblies from a folder tree) need no setup. For programs not listed
here, the same rules apply: what the team shares goes into the group as a
vaulted file, what the program keeps per user is set in its own options.
