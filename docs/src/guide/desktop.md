# Desktop app

TeamCraft Desktop (`desktop/`, Tauri 2) is a rich client for Linux, macOS and
Windows: a navigator tree of the project groups, object tabs (overview,
files, structure, references, where used, changes, history – in the web UI's order – then graph, 3D viewer and workspace), a worklist with
check-outs and reviews, and a **local working folder**:

* **Home** is the pinned first tab (#513): it cannot be closed or moved, and
  a click on it (or closing the last tab) brings it back. It shows what the
  navigator beside it cannot – the worklist as one line of counts (*1
  check-out · 2 reviews waiting · 1 dependent to check*) that opens the
  Worklist, the product counts, **Recently opened** (the revisions, changes
  and requests you opened on this computer, per server) and the server's
  **Recent activity** (`GET /api/v1/activity`, the same entries as on the web
  dashboard). The rail entry for the files of the active group is **Files**
  (`Ctrl/Cmd+1`, *View → Files*);
* the **Worklist** navigator lists the sections with something in them
  first; an empty section is one line (*My open changes 0 ⓘ*) whose
  explanation – and its action, such as *Get…* – opens on a click (#513);
* the **status bar** names the connection in a word before the address:
  *Connected*, *Unencrypted* (plain `http://` to another computer) or
  *Offline* (the last poll did not reach the server). The bell shows a dot
  for unread notifications (the number is in its tooltip) and stays pressed
  while the notifications are in the navigator; the number on the
  *Worklist* rail button counts work to do, its tooltip says what (#513);
* *New* (`Ctrl/Cmd+N`) creates an item together with its files: choose the
  CAD files first, name and type follow from the first one. Files in the
  working folder bring their place (#359): a part saved in the CAD program as
  `<working folder>/GRAKSLER/Design/Struktur/Bracket.ipt` becomes an item of
  the group `GRAKSLER` in the folder `Design/Struktur`, every file keeps its
  own folder (a drawing in `Zeichnungen/`, a STEP export next to the model),
  and group and folder are shown, not typed – to put the item elsewhere, save
  its files there. Files picked from elsewhere are **copied** into the item's
  folder (the originals stay untouched; an existing file is never
  overwritten). Before anything is created the dialog refuses duplicates – a
  file that belongs to an item already (tracked, or added with *Add files…*),
  a file another item has at the same place in the group – and asks before a
  file name that exists in another folder of the group (CAD programs find
  parts by file name; *Create anyway* for the rare intended case, such as
  standard parts in two languages); the same content as a stored file is a
  warning. A summary says what will happen (*Creates a new item in GRAKSLER /
  Design/Struktur, uploads 2 files – it stays checked out to you*). The
  server checks the same group, folder and name again while it stores the
  files, so two people creating the same new file at the same moment cannot
  both succeed, and *Create* after a lost answer or a failed upload finds the
  item made the first time (an idempotency key from the main file) instead of
  creating a second one. The uploaded files are recorded in the working
  folder, so no other check-out and no worklist suggestion offers them again;
* *Check out* takes the lock, copies the files into the working folder and
  makes them writable. For an assembly it also downloads every part of its
  structure that is missing (latest version, read-only) – only the assembly is
  locked; parts you changed locally, or whose place is taken by another file,
  are never overwritten and are listed in the report (*Settings → Workspace*
  switches this off);
* *Check out parts…* on an assembly (revision toolbar: the drop-down of *Check out and open*, or group **Lock** while checked out; and
  the *Structure* tab) checks out several of its parts at once, to change
  them in the context of the assembly – see
  [Check out parts of an assembly](#check-out-parts-of-an-assembly);
* *Files / All items / Structure* in the navigator head: *Files* shows the
  folders and files of the project group as in its working folder (see
  below); *Structure* shows, per project
  group, the assemblies and the parts no BOM uses; an assembly expands into its
  parts one level at a time with quantity, state, lock owner and ⧉ N for a part
  used in several assemblies;
* *Get…* on a revision copies the item **with everything it consists of** –
  its BOM resolved with a revision rule and the parts its CAD files
  reference – so the assembly opens in the CAD program (see below);
* the download icon next to *Projects* in the navigator (*Tools → Get Project Groups…*) copies the files of every item
  of
  chosen project groups into the working folder – for new team members. It
  takes the **latest revision, also in work**, unless *Latest released
  revision* is chosen (#355): most items of a team that releases only before
  manufacturing are in work, and *released* would leave the working folder
  almost empty and the assemblies without their parts;
* the folder icon next to it (*Tools → Import CAD Folder…*) brings an existing CAD folder in:
  scan first (nothing is uploaded), then one item per part or assembly with
  its drawings and exports; every file keeps its folder, so the working
  folder later looks like the original tree. An interrupted import continues
  where it stopped – an item whose files did not arrive is completed, none is
  created twice; the app remembers per project group what it imported, so
  the same folder can also be imported into another group. BOM lines come
  from the references in the assembly files – for NX also from `.prt` files (such an assembly is imported as a part
  item, NX does not tell parts and
  assemblies apart by the file type). Their quantity is how often the
  assembly places the part where the file tells it (SOLIDWORKS 2015+, NX,
  Inventor and STEP assemblies with external part files; instances excluded
  from the BOM and suppressed Inventor occurrences do not count, a part that
  is only placed that way gets no BOM line but stays referenced); for other
  formats (Solid Edge, …) it is 1
  with the note *check the quantity* – set the real quantities with *Import
  BOM…* from the CAD program's BOM export. *Help → How to
  Import a CAD Folder* is the step-by-step tutorial (Pack and Go, unique file
  names, check and release);
* *Import BOM…* (in the *Structure* tab and the *More* menu of a revision you
  have checked out, or *Tools → Import BOM as New Assembly…*) reads a BOM
  exported by the CAD program – CSV, Excel, ODS, a STEP assembly or a SolidCraft
  design (`.sc`, its components and part properties) – with the
  same column recognition as the web import: the preview shows the lines,
  how many items are new or existing and the notes (for example a quantity
  that is not a number); *Import* adds the lines in one step, creating the
  missing items. *Part numbers from the file* or *New numbers* (the group's
  numbering scheme), and whether items with the same number are reused, are
  chosen in the dialog;
* *Quantities from CAD…* (next to *Import BOM…*) compares the BOM with the
  instances counted in the revision's CAD files (SOLIDWORKS 2015+, NX,
  Inventor, SolidCraft, STEP assemblies with external part files): missing lines and lines whose
  quantity differs – such as the quantity 1 of an assembly imported before
  the counts existed – are ticked; *Apply* adds and corrects them in one step (the same plan as *Create BOM from CAD
  references…* in the web UI). A
  quantity the CAD file does not tell never replaces an existing one;
* *Add line…* (also in the *Structure* tab of a revision you have checked
  out) adds a single component: type part of its number, name or file name
  and choose one of the matching items – shown with the preview image from
  the CAD file, revision, state and project group (↑/↓ and Enter work too, Esc closes the list);
  enter quantity, find number (empty = next free one) and notes, *Add*. An
  exact item number works without choosing. Only items you may read are
  offered (restricted groups you are not in stay hidden). Under your
  check-out the quantity of every first-level line is an input – a change is
  saved when you leave it or press Enter (Esc restores it) – and *Remove*
  deletes a line after a confirmation; the tab reloads after each change.
  These use the same REST routes as the web UI (`GET /items/suggest`,
  `POST …/bom`, `PATCH`/`DELETE /bom-lines/{id}`); an older server without
  the suggest route is searched through `GET /items?q=`;
* the *Graph* tab draws a navigable reference and BOM graph around the
  revision: the focused item in the middle, its BOM children and references
  below (*ref*), the assemblies whose BOM uses it and the items that reference
  it above (*uses*). Every node is a
  card with the item's preview image, file name and number. Node colour is
  the lifecycle state (the legend names them),
  node size (and the ⚙ badge) the item's own BOM complexity; the small ×n in
  the top-left corner of a child is how many instances of it the assembly
  contains (summed over its BOM lines, ×1 included). Click a node to go to it: the tab
  becomes that item – a sub-assembly shows its own graph, anything else its
  overview – and *← Back*, always in the same place (left of the tab row),
  leads back the way you came, one step per click; clicking the assembly you
  came from is the same step back. A click on the item in the middle opens
  its own overview. `Ctrl`+click (`⌘`+click on macOS) opens a node in a tab
  of its own instead. The mouse wheel zooms on the cursor, dragging pans –
  also when the drag starts on a card, which then does not open – and a
  double-click on the background fits the graph again; while previews are
  still loading the view fits itself until you zoom or pan. Large
  assemblies (more than 8 neighbours on a side) are laid out in rows, and
  only the first 48 neighbours get a preview and their complexity (one
  request each), the rest keep a basic card. The graph is drawn by the app
  itself (SVG with an HTML layer for the cards, no external engine), in the
  colours of the chosen skin. While the assemblies that use the item are
  still loading, a line above the graph says so; when nothing is around the
  item, a note in the same place says that;
* the *Workspace* tab shows per file where it is, whether it is up to date,
  modified, outdated, in conflict, new, missing or a name clash, whether it
  is read-only, and who holds the lock for how long; opening an outdated
  file or one somebody else has checked out asks first;
* *Check in…* uploads the selected files as new versions and releases the
  lock – based on the versions they were copied from, so a version somebody
  else checked in meanwhile is never overwritten (see
  [Concurrency](vault.md#concurrency-nothing-is-overwritten-silently)) –
  and makes the files read-only again; files above 32 MiB are sent in
  resumable chunks – a dropped connection resumes where it stopped (see
  [Large files](vault.md#large-files-chunked-resumable-uploads)). The dialog
  stays open with the progress while the check-in runs, and **Cancel** stops
  it at any point until the last byte is sent: between two chunks of a large file (the chunks on the server are
  discarded – a cancelled upload starts over next time) or half-way through
  the request with the small files. Nothing is stored then, the revision
  stays checked out to you and your local files are untouched. Once every
  byte is sent the server finishes the check-in on its own; the dialog says
  so (*too late to cancel*). A new file that is a part or assembly while the
  revision has a model already (a new `Bracket.iam` next to the checked-out
  `Bracket.ipt` – the same name stem makes it a candidate) is offered
  **unticked**, with *New item from this file*: it is usually an item of its
  own (#359);
* **Update the BOM from the CAD file** (#407) – a box in *Check in…*,
  **ticked by default** every time (the choice is not remembered) and shown
  when the revision has an assembly whose references are read: the server
  knows a structure file of it (`bom_structure` of `file-status` – an
  assembly, or an NX part, STEP file or SolidCraft design with counted
  instances), the item is an assembly, or a file is an assembly (Inventor
  `.iam`, SOLIDWORKS, Creo `.asm`) or a SolidCraft `.sc` / `.solidcraft`
  design. A plain part check-in (an NX or Creo part, its STEP export) shows
  no box. The
  server applies the plan of *Quantities from CAD…* to the version being
  checked in, in the same transaction: missing children are added, a
  quantity the CAD file counts (*CAD instances*) is corrected – an *assumed
  1* never replaces an existing quantity –, circular children are refused,
  and **no line is removed**: lines whose part the assembly no longer uses
  (glue, cable ties, bought parts) are kept and reported. The result says
  what changed, e.g. *BOM: 4 lines added, 1 quantity changed · not in
  TeamCraft: Lager.ipt · no longer in the CAD file: PRT-000012 (kept)*;
  nothing changed – no line. Unticking it keeps the BOM as it is for that
  one check-in; *Park* never changes the BOM;
* the worklist in the navigator follows what you do in the tabs: checking
  out, cancelling a check-out, checking in or acknowledging a dependent
  updates it at once, while it stays open on the left;
* **New on this computer** (worklist, #359) lists the parts and assemblies in
  the working folder that belong to no item yet – typically the ones just
  created in the CAD program – one suggestion per model file with its
  drawing and exports of the same name in the same folder, newest first, the
  count is part of the worklist count on the rail. A click opens *New*
  prefilled; *Create items…* creates one item for each ticked suggestion in
  its folder, after a summary that names what is refused and why (a refused
  file can still be created one by one with *New*, where a duplicate name can
  be confirmed); *×* ignores a file for good (`.teamcraft/ignored.json`,
  *Show ignored* brings it back; the entries of files that are gone –
  deleted, moved or part of an item meanwhile – are dropped after a scan that
  read the whole working folder). Left out: files that are tracked, added
  with *Add files…*, a companion of a tracked file (the STEP export next to
  a tracked model goes into that item's check-in), lock, backup and
  temporary files, `OldVersions` and hidden folders, files directly in the
  working folder and other files than CAD models (under *Other new files*).
  The folder is looked at when the worklist opens or is refreshed and when
  the app window gets the focus back (at most every 15 s); one scan visits at
  most 50 000 entries and reports at most 2 000 files, so a huge folder never
  blocks the app. Nothing is uploaded from the worklist without the dialog or
  the confirmed summary. Viewers do not see the section;
* **Local changes** (worklist, #416) lists every file of the working folder
  that is not the same as on the server, across all project groups and
  grouped by item: **modified**, **outdated** (somebody checked in a newer
  version), **conflict** (changed here and on the server), **new** (a file
  next to a tracked one, such as a STEP export) and **missing**. A file you
  changed after clearing its read-only flag shows up too, marked *not checked
  out* – you do not have to open its item first. Each row has its actions,
  and the ticked rows take the ones above the list: **Check out** (keep my
  file) for a file changed without a check-out – your file stays as it is and
  the revision becomes yours to check in; **Get latest** for outdated and
  missing files (a changed file is never overwritten); **Check in selected**
  for the changed files of your own check-outs (a comment, then the same flow
  as *Check in all*; a revision with a conflict stays checked out). A
  check-in releases the check-out, so it always takes **every** changed file
  of the revision along – the dialog names the ones you did not tick, and
  **Park** uploads them instead and keeps the check-out;
  **Merge…** opens the Workspace tab of a conflict; **Folder** opens the
  folder. A click on a revision (a button, also by keyboard) opens its Workspace tab. The count is part of
  the worklist count on the rail. Files that belong to no item yet stay in
  *New on this computer* (the link under the list jumps there). The list is
  made in two steps: the working folder is compared with what TeamCraft
  copied there (no network; unchanged files are not read again), then up to
  100 revisions per scan are compared with the server – the ones with local
  changes first, then your check-outs, then the others in turns, so in a
  larger working folder every revision is checked for outdated files over a
  few scans. Only that second step knows *outdated* and *conflict*; the note
  under the list says how many revisions were compared on this computer only
  this time. The scan only compares – it changes no read-only flags and no
  files. It runs when the worklist opens or is refreshed, when the app window
  gets the focus back (at most every 15 s) and right after a check-out,
  check-in, park, cancelled check-out or Get. Viewers see the list with *Get latest* and *Folder* only;
* *Check in all* in the worklist first lists the check-outs with local
  changes (modified or new files) and offers to **check them in first**
  with their files; *Only the others* checks in just the check-outs without
  local changes. Check-outs with a conflict always stay checked out – merge
  them in their Workspace tab. *Sign out* (and *Change server*) asks the
  same before it signs you out: *Check in and sign out*, *Park and sign
  out* or *Sign out anyway*;
* **Park…** (Workspace tab, *More ▾*, and a button of the check-in dialog)
  uploads the ticked files as new versions but **keeps your check-out** –
  work in progress is on the server even if your computer is lost, and
  nobody else can change the revision meanwhile. Same conflict check as a
  check-in; an empty comment is stored as *Work in progress (parked, still
  checked out)*;
* when somebody else released your check-out (an administrator, or a
  manager of the group with a reason), the Workspace tab says *Released by
  X; your local changes are kept and need a new check-out*, with the date
  and the reason – until the revision is checked out again;
* *Dependents to check* (worklist) shows one line per dependent revision
  with the number of changes it has to be checked against; the arrow lists
  them (a click opens the item that changed). The revision tab of a
  dependent shows its checks with **Acknowledge all…** (one note for all of
  them, see [Change propagation](structure.md#change-propagation)) and an
  **Acknowledge…** per change;
* *Request change…* on a revision tab and *Request…* on a file (Files and
  Workspace tabs) raise a [change request](changes.md) with the item,
  revision and file name filled in and an optional screenshot or PDF; the
  worklist lists the requests assigned to you and your open ones;
* on Windows with Inventor or Solid Edge installed, *STEP* (workspace) and
  the check-in option (off by default) create STEP exports for the 3D preview with that
  program; the Files tab shows the preview image stored in native files. At
  check-in only parts and small assemblies are exported: an assembly file
  (`.iam`, Solid Edge `.asm`) larger than 5 MB is skipped and named in the
  dialog and the result – its 3D preview is composed from the parts (#342).
  The *STEP* button of the Workspace tab still exports any file on request.
* on every system with SolidCraft installed, the same *STEP* button and
  check-in option export SolidCraft designs (`.sc`) through SolidCraft's
  command-line tool `solidcraft-cli` (set `SOLIDCRAFT_CLI` to its path if it
  is not found); a design with a failing feature is not exported. An export
  that fails or is not possible never stops the check-in – the file is
  checked in without its STEP and the result names the reason.
* with the check-in option ticked, the ticked files are **uploaded first**
  (parked – the revision stays checked out), then the exports run one after
  the other with the time each has taken so far, then the check-in follows
  with the STEP files (#342). An export of a large assembly can take many
  minutes: **Cancel** during an export skips it (and the remaining ones) and
  ends an Inventor or Solid Edge the export started – your files are already
  on the server. Calls Inventor rejects while it is busy are tried again for
  up to a minute. The whole error output of a failed export is kept in
  `export-logs/` in the app's configuration folder (the newest 20, on this
  computer only); the error message names the log.
* for a SolidCraft design of your own check-out, *Item data* (Workspace tab)
  writes the item number, the revision and the item name into the design's
  properties (*Part number*, *Revision*, *Description* in SolidCraft), plus
  TeamCraft's ids and server under the key `teamcraft.pdm`, through
  `solidcraft-cli`. Refused while the design is read-only or open in
  SolidCraft. The design changes and is checked in like any other change.
  The Files tab of a revision lists the design properties TeamCraft read
  from a design under its name (*Part number … · Revision … · Description …*),
  with the notice *Design properties differ from this revision* when they
  do not match the item; *New item* from local files and *Import a CAD
  folder* fill an empty description from them, the name stays the file name
  (see [Files](files.md#design-properties-of-solidcraft-designs)).
* with a *Preferred CAD program* set on your account page in the web UI, the
  Files tab of a revision collapses the native files of other CAD programs
  (see [Files](files.md#preferred-cad-program)); the app reads the setting at
  sign-in.
* *Agent* (rail, *View → Agent*, `Ctrl/Cmd+4`, or *Agent* in a revision or
  change tab) is the app's one place for AI. It runs on one of two
  backends, decided by the server's policy and your choice:
    * **Company AI** – the server's assistant, configured by the
      administrator (the key stays on the server). It is the
      [Ask TeamCraft](assistant.md) conversation of the web UI, inline in the
      Agent tab: it knows the last revision (and its section, BOM with
      revision rule), change order or change request tab you opened, offers
      quick actions that fit it (*Draft release notes*, *Check before
      release*, *Brief me for this review* …), has the *Allow drafting*
      checkbox, *Cancel* (shown only while an answer is on its way) and
      *New conversation*, and keeps one conversation on the server that
      survives restarts. Numbered lists in its answers keep their numbers,
      also when the items are separated by blank lines.
    * **Local agent** – an AI agent on your computer that you set up under *Settings → Agents…*, for example Claude Code
      through its ACP adapter
      or Gemini CLI. None is set up by default. The app starts the agent in
      the workspace folder and gives it TeamCraft's tools with your rights.
      The conversation streams in the tab, with tool calls, the agent's plan,
      permission prompts (*Allow once* / *Reject*), *Cancel* and quick
      prompts (release notes, release check, change review, worklist triage).
      Through the app the agent may only read text files in the workspace
      folder; it gets no write access and no terminal.

  When the server offers both, *Company AI* / *Local agent* in the Agent
  toolbar switches between them (each keeps its own conversation); the app
  remembers the choice per server and user, and starts with the local agent
  if you have set one up, otherwise with the company AI. If the
  administrator turned local agents off (`TEAMCRAFT_LOCAL_AGENTS=off`), the
  Agent view runs on the company AI only, the app ends a running agent and
  refuses to start one; with no company AI configured either, the view
  says that AI is switched off by the administrator. The app asks the
  server again when the Agent view opens, every minute and before every
  message to a local agent, so a changed setting takes effect without a
  restart. See
  [ACP agents](assistant.md#acp-agents-in-the-desktop-app).

Sign-in runs through the browser (single sign-on or password, confirmed on a
TeamCraft page) or with a personal access token. The app keeps the token in
the system keychain (Windows Credential Manager, macOS Keychain, GNOME Keyring
or KWallet on Linux). Where there is none, it uses a file only your user
account can read and says so at the start and in *Settings*. An address with
plain `http://` to another computer gets a warning when you connect: the token
and all data would cross the network unencrypted – use the `https://` address
of the server. On a hosted live demo the app signs in to the public
account under a random computer name and can look at everything but change
nothing. The app uses the same skins
and 3D viewer as the web interface; the models are converted by the server,
and *Loading* or the reason a model cannot be shown appears on the canvas of
the *3D viewer* tab (see [Files](files.md), also for rotating, panning and
the mouse setting below the view); point clouds (PTS, XYZ, LAS, E57) and simulation results with a result field have no
*Export STL*, and a thinned cloud says so below the view. *Settings → Appearance*: *TeamCraft* (the
default) follows the light or dark
appearance of the operating system and switches with it, *TeamCraft Light*
and *TeamCraft Dark* stay fixed, the distribution skins are dark. *Settings → Language*: *As the system* (the
default: your operating system's language if TeamCraft has it, else English),
*English* or *Deutsch* – from the next start; the header of the web UI
follows your browser's language (#379, more views follow). Your data – item
names, comments, file names – is never translated. *Settings → Change password…* changes the password of
a local account (it
asks for the current one; wrong ones count like failed sign-ins, and your web
sessions end); single-sign-on accounts do not see it.

**Several servers and accounts.** *Settings → Connections* lists every
server and account the app keeps a sign-in for, with its working folder.
*Switch* changes to another one, and the window reloads; *Add another
account…* signs in a second account on the same server; *Switch server…*
(also the connect screen) goes to another server **without signing you
out** – the sign-in you leave stays in the keychain and is back when you
switch again. Each connection after the first gets a **working folder of its
own** next to the default one (`TeamCraft-plm.example.org`, numbered for a
second account there), never inside another one, because two servers can use
the same item numbers and project codes; the first connection keeps the
folder it had. *Remove* forgets a connection and its sign-in; its working
folder stays on your disk. Check-outs belong to the connection they were made
in – switch back to check them in. The list names the account of every
connection; a sign-in kept from an app version before the connections gets
its account the first time the server confirms it at the start (#356).

The window is built for daily work: object tabs on top, under them the
toolbar of the open object with its one next step filled (see
[The toolbar](#the-toolbar)) and the rare tools under *More*. The navigator shows a state as its symbol (a check:
released, a ring: in review, a half-filled ring: in work, – obsolete, the
name in the tooltip); the search field is `Ctrl+F` (`⌘F`). Above the logo
at the bottom of the rail the app shows its own version (`0.2.10`), always –
which app runs is one look at a screenshot away (#357); the desktop app
should match the server's version.

**Moving tabs:** drag an object tab to a new place (a bar shows where it
lands; the tab keeps its state, and a plain click still just selects it).
Without a mouse: `Ctrl+Shift+PageUp` / `PageDown` move the active tab left or
right, `Ctrl+PageUp` / `PageDown` (or `Ctrl+Tab` / `Ctrl+Shift+Tab`) switch
to the previous or next tab, and the arrow keys move between the tabs when
the tab strip has the focus. The right-click menu of a tab offers *Move to
start*, *Move to end*, *Close*, *Close others* and *Close tabs to the right*.

## The toolbar

The toolbar sits right below the tab strip and acts on the active tab. Its
buttons are grouped, and they keep their place whatever state the object is
in; a button that does not apply right now is greyed out, and its tooltip
says why (for example *Check out first* or *Already in change order
CHG-000002*). The **one highlighted button is the most likely next step**:

| Revision                                                                                                                   | Highlighted          |
|----------------------------------------------------------------------------------------------------------------------------|----------------------|
| checked out by you, no local changes yet                                                                                   | *Open in CAD*        |
| checked out by you, with local changes, versions parked since the check-out, or the workspace could not be read            | *Check in…*          |
| checked out by you, no native CAD file to open                                                                             | *Check in…*          |
| in work, not checked out, the newest file uploaded in this revision is yours and it can be released (no open change order) | *Release…*           |
| in work, not checked out, otherwise                                                                                        | *Check out and open* |
| in review, obsolete, or checked out by somebody else                                                                       | nothing              |
| released                                                                                                                   | *Revise*             |

*Release…* is never the highlighted step while the revision is checked out:
an unchanged revision is not sent to review – the CAD step comes first. The
working folder of your check-out is looked at again whenever the tab is
shown or the app window gets the focus back – save in the CAD program, switch
back, and the highlight moves from *Open in CAD* to *Check in…*. After the
check-in, *Release…* is highlighted when the revision can be released. Only
files uploaded in this revision count: the files *Revise* takes over from the
previous revision keep their old uploader and time, so a revision you revised
and left unchanged – also after a cancelled check-out – highlights *Check out
and open*, not *Release…*.

The revision toolbar fits in one row at 1440 px (#513):

* revision tab: **Lock** (*Check in…* and *Cancel check-out* / *Force
  unlock* while checked out; *Check out parts…* on an assembly) · **Open**:
  one split button – *Check out and open ▾* with *Check out without opening*,
  *Check out parts…* (assembly) and the CAD program in its drop-down, or
  *Open in CAD ▾* for your own check-out, whose drop-down names the program
  that will open the file (*Opens with Inventor 2025*) and leads to the
  group's default CAD program – see [Check out and open](#check-out-and-open)
  · **Work** (*Edit attributes*, *Get…*, *Show in folder*) · **Lifecycle**
  (*Release…*, *Revise*) · **More ▾**, starting with *Request change…*,
  *Watch* and *Agent*;
* change tab: **Review** (*Approve change* highlighted, *Reject*) while your
  review is pending – the same two buttons are under the comment in *Your
  review*, with *Meaning of your sign-off* as on the web; the comment is
  required for a rejection · **Lifecycle** (*Submit for review*, highlighted
  on your draft – disabled with the reason as tooltip while the draft has no
  revision or no reviewer –, *Withdraw* on your change in review, *Cancel
  change* for a draft or rejected change, at once) · **Collaborate**. An
  administrator who did not create the change finds *Withdraw
  (administrator)* in **More ▾** instead – never next to *Approve change* –
  and confirms it first. On your draft the *Reviewers* panel has *Add
  reviewer* (user names, any CAD editor or administrator – not you), *Remove*
  per reviewer and the suggested reviewers;
* change request tab: **Triage** (*Start triage* or *Create change order*
  highlighted, *Take up again*) · **Requester** (*Attach…*, *Withdraw*) · **Collaborate**.

*Release…* creates a change order for the revision – with its unreleased
dependencies by default – assigns the reviewers you tick (the other CAD
editors of the project group) or type under *Other reviewers* (user names of
any CAD editor or administrator) and submits it. Your own user name is
refused before anything is created (four-eyes principle). When nobody else in
the group can review, the dialog says so and creates a draft instead of
submitting – a release always needs a second person, so a server with a
single account cannot release (see [Separation of
duties](changes.md#separation-of-duties)). If a revision or a reviewer is
refused, the half-made draft is cancelled again and nothing is left behind;
only a refused submit (for example an unreleased dependency) keeps the
complete draft and opens it.

*Check in…* (and *Check in all* in the worklist) shows an arrow **up out of
the tray** – your files go to the server; *Get…* keeps the arrow down into
it (#409). A **?** next to *Revision* (Overview) and *Version* (Files tab)
explains the two levels in one sentence, on hover and on keyboard focus
(#414; the words are in the [Glossary](glossary.md)); the SHA-256 of a file
is in the tooltip of its version.

**Right-click menu** (#417): a right-click on an item or file – in the
navigator, the *Files* view (*Files* / *All items* / *Structure*), the worklist's
check-outs and the cards of the *Graph* tab – opens *Check out and open*
(for your own check-out *Open in CAD* and *Check in…*), *Check out*, *Get…*,
*Show in folder*, *Copy number*, *Where used*, *Request change…* and *Open in
browser*, without opening the item first. The entries follow the toolbar's
rules: one that does not apply is greyed out and its tooltip says why
(*Checked out by Mia*, *no native CAD file*). Shift+F10 or the context-menu
key opens it on the row that has the focus; the arrow keys move, `Esc`
closes.

Rare actions are in **More ▾** at the end: on a revision *Request change…*, *Watch* and *Agent* first, then *Pack and Go…*, *Add files…*, *Park…*, *Import BOM…*, *Quantities from CAD…*, *Open in browser*, *Refresh* (`F5`) and *Mark obsolete…*. The
menu works with the keyboard:
`Enter`, `Space` or `↓` opens it, `↑`/`↓`, `Home`/`End` move, `Enter` runs
the action, `Esc` closes it and returns to the button; a click outside closes
it too.

The **Overview** of a revision shows *At a glance* next to the properties:
the preview image stored in an Inventor, Solid Edge, SolidWorks or NX file (with the way into the 3D viewer) and the
number of files, BOM lines, where
used, references, change orders and open change requests – a click opens the
matching sub-tab.

**Projects** (the rail button below *Agent*, `Ctrl/Cmd+5`) is where you
choose the **active project group**: your groups first, then the others.
Administrators and the managers of a group see a lock button next to it:
*Check-outs* opens a tab with the graph of who has what checked out in that
group – the people on the left, their revisions on the right, the line
coloured by how long the lock is held – and a list per person; a click on a
revision opens it (#428, the same overview as on the group page in the web
UI, *Open in browser*). The tab is loaded again each time it comes to the
front, and while it is in front it looks again every 30 seconds and when the
window gets the focus back, so a check-out by someone else shows up without
*Refresh*. A failed look is tried again at the next one; the keyboard focus
stays on the revision it was on when the overview is painted again.
CAD editors create a new group there (*New* – you become its manager; tick *Restricted* to keep it to its members), and
the page offers *Get…* (copy
groups into your workspace) and *Import CAD folder…*. While *Get…* copies,
its dialog shows the progress, the groups and the revision rule cannot be
changed, and the dialog stays open (Escape does not close it); *Stop* cancels
the copy once the file being copied has ended, and the dialog then shows
*Stopped* – meanwhile *Close* hides the dialog and the result comes as a
message (a new Get starts only once the stopped one has ended). At the
end the dialog turns into a result: **Done** with a tick and the counts
(items, files copied, local changes kept, items skipped – *Nothing to get*
when no item has a released revision), or *Finished with problems* /
*Stopped* with every problem listed (a name clash among them, nothing
overwritten), and *Open working folder* next to *Close*. The choice of the
group is kept on this computer.

**Files** (the rail entry, `Ctrl/Cmd+1`; called *Home* before #513) is a file browser of the active group, in three views:

* *Files* (the view it starts with): the **folders** of the group and the **files** in them, as in its working
  folder – folders first, then the files
  by name the way a file manager orders them (without regard to case, `Part2`
  before `Part10`), each with the state and the lock of its item. A checked-out file is marked
  with a bar in the lock colour at the left edge of its row and a light tint
  of the row (the same for your check-outs and other people's; also in
  *Structure*). Your own check-out or check-in, and a lock of someone else you
  release (*Force unlock*), show at once, in place, with the row's tooltip; other
  people's locks show the next time the list is loaded (*Refresh*). Whether a
  lock is yours is decided by your account, not your display name, so a
  colleague with the same name is never shown as you. The file
  name comes first and the number and revision after it, so a narrow navigator
  cuts the number, not the name. A folder opens its own level – a large group
  is never loaded as a whole, and a folder with more than 500 files offers *show … more*. The files are those of the
  latest revision of every item;
  items without files are listed below the files of their own folder (the top
  of the group unless the item was given a folder).
  The number next to the order is the rows of the whole group. A click on a
  file opens its item. The **+** on a folder row (shown when the pointer is on
  it) is **New item here**: the *New* dialog opens with the group and that
  folder filled in;
* *All items*: every item in one list, the name first (the first 500 by
  number);
* *Structure*: the assemblies with their parts.

**Pictures in the rows** (#420). In all three views and in the search, a
row shows a small picture of its model in place of the type icon: in
*Files* the preview image of the file itself, in *All items*, *Structure*
and the search the picture of the item's main model (the assembly before the
part before the drawing, as at the top of the revision page). The server
names the picture in the list, and the app fetches it only when the row
scrolls into view, once per file content; rows without a picture keep the
type icon.

**Favourite folder.** When your work is several folders down, make that
folder the group's favourite: point at the folder in *Files* and click its
☆. *Files* then starts in that folder instead of at the top of the group. A
banner in the accent colour above the list shows where you are
(`ROVER / Design / Rover`) and that it is *not the top of the group*;
*Whole group* shows the top (*Back to the favourite folder* returns), and
*Remove favourite* (or the filled ★ of the folder) lets *Files* start at the
top again. If the group holds an Inventor project file (`.ipj`) in a
folder, *Files* suggests that folder – *Start there*, or *No, thanks* to stop
the suggestion for this group. The favourite is kept on this computer, for
each server and group; the count next to the order then counts the rows
below the favourite folder.

*Files* and *All items* are sorted by name (or by number, type or state – the
choice is kept). *Change…* goes back to *Projects*. *New* and *Import CAD
Folder…* start with the active group. *New* (and `Ctrl/Cmd+N`) also starts
with the folder you last opened or clicked in in *Files*; its *Folder* field
(e.g. `chassis/brackets`, empty = the top) is where the item and the files
chosen in the dialog go – type a new path to start a new folder. With a
server older than the *Files* view, *Files* shows *All items* instead.

*Files*, *Projects*, *Refresh* and the dialogs that offer project groups (*New*, *Get Project Groups…*, *Import CAD
Folder…*) load the groups and your
memberships from the server again, so a group created or joined in the web UI
shows without restarting the app.

Keyboard: `Ctrl/Cmd+N` new item, `Ctrl/Cmd+F` search, `F5` refresh,
`Ctrl/Cmd+1/2/3/4/5` Files, Worklist, Changes, Agent, Projects, `Ctrl/Cmd+W` close the tab,
`Ctrl/Cmd` + `+` / `-` / `0` (or `Ctrl` + mouse wheel) zoom.

## Check out parts of an assembly

CAD users change parts **in the context of the assembly**. *Check out
parts…* (revision toolbar, group **Lock**, on an assembly, and the
*Structure* tab) shows the assembly's structure as a tree with check boxes:
every part with its state and who holds it. Parts that cannot be checked
out are greyed out with the reason – *Not a member of project group …*,
*Released – revise it first*, *In review*, *Checked out by …*, or
*Already checked out by you* (ticked). A
part used several times is offered once; parts the CAD files reference
without a BOM line follow at the end. *Tick all* / *Tick none* help with
large assemblies.

*Check out* takes the locks of all ticked parts in **one step** on the
server (`POST /api/v1/checkouts`) and makes their files in the working
folder writable – missing or outdated files are downloaded, an unchanged
copy only loses its read-only flag, a copy you changed is kept – so the CAD
program can save them. What the server refused (someone took a part a
moment earlier, a part was released meanwhile, you are not a member of its
project group) is listed with the reason; the rest is checked out anyway.
The other parts stay read-only.

## Check out and open

*Check out and open* (revision toolbar, group **Open**) takes the lock,
copies the files into your working folder and opens the main CAD file in the
CAD program – in one click. For your own check-out the button is *Open in
CAD*. The chip right next to the button names the program, for example
*Inventor 2026*; its tooltip says why, and a click shows the project group's
setting in the web UI.

* **Which file:** a native CAD file of the revision (one CAD program's
  part, assembly or drawing), of the group's default program when there is
  one – the assembly before the part before the drawing.
* **Which program:** the project group's **default CAD program** (set by
  its managers on the group page in the web UI, see
  [Concepts](concepts.md#default-cad-program)) in its version; is that
  version not installed, the newest installed version of the program. A
  group without a default, and a file of another program, open in the
  **newest installed version of the program that fits the file type**.
  Without such a program – and for neutral files such as STEP or PDF – the
  operating system opens the file with its default application. A dashed
  chip means the program asked for is not the one used.
* **Where the programs are looked for:** on Windows the install folders
  below *Program Files* (`Autodesk\Inventor 2026`, `Siemens\Solid Edge
  2025`, `Siemens\NX2312`, `PTC\Creo 10.0.0.0`, `Dassault Systemes\B33`,
  `FreeCAD 1.0`, `Rhino 8`, `SketchUp\SketchUp 2024`) and for SOLIDWORKS the
  versions its installer registers; on macOS the apps in `/Applications`
  (FreeCAD, Rhino, SketchUp, Fusion); on Linux FreeCAD on the `PATH`. The
  app looks once per start – restart it after installing a CAD program.
* **A running Inventor is reused** (Windows): when Inventor is already
  running – the installation chosen above – the file opens there as a new
  document window, and Inventor comes to the front, instead of starting a
  second instance. If the working folder has an Inventor project file
  (`.ipj`: the one in the nearest folder from the file's own up to the
  working folder root; a folder with several makes it unknown), the running
  Inventor must have that project active. Without one it is reused whatever
  project it has – a new instance would start with that same, last active
  project anyway (#483). A new instance starts when no Inventor runs, when
  another version runs, when the running one has another project active, or
  when it does not answer (started as administrator, or busy with a dialog –
  the app waits at most 20 seconds); the message after opening says which
  (*Inventor is running with the project Default, not Graksler 4.3 – opened
  in a new instance*). Only a visible Inventor in your own Windows session
  counts: another user's on the same computer (remote desktop, fast user
  switching) or a hidden one of a STEP export is not asked. Inventor is
  asked through its COM interface by a short Windows PowerShell script, as
  for the STEP export. The app does not wait for a large assembly to load:
  if the running Inventor then cannot open the file (locked or broken file,
  Inventor busy), an error with the reason appears (#508). A part already
  loaded as a hidden reference of an open assembly opens in its own window.
* **Only a complete Get opens:** if you press *Cancel* next to the progress
  bar while the files are copied, or files could not be copied, the file is
  not opened – an assembly would open without the parts that were still to
  come. The check-out stays; use *Get…* and then *Open in CAD*. The same
  applies to *Open* in the Files tab and *Get latest and open* for CAD files
  (other files, such as a PDF, still open).
* *Open* in the Files tab uses the same choice for CAD files.

*Import CAD folder* shows the CAD program the folder belongs to – for
Inventor with the version read from the files (*Detected CAD program:
Autodesk Inventor 2021*) – and offers to make it the group's default
program (managers of the group and administrators; the import report says
whether it was set).

## How files appear on your computer

There is **one working folder per project group**, and inside it the files
sit in the **same folders as in the original CAD tree** – one copy of each
file. The revision is information TeamCraft keeps about the file, not a
folder level:

```text
Documents/TeamCraft/                     ← the workspace (Settings → Workspace)
├── ROVER/                               ← project group
│   ├── chassis/
│   │   ├── chassis.asm                  ← ASM-000001 rev. B, checked out by you (writable)
│   │   └── brackets/
│   │       ├── bracket.prt              ← PRT-000001 rev. A (read-only)
│   │       └── bracket.step             ← new STEP export next to it
│   └── drawings/
│       └── chassis.drw
├── LANDER/
│   └── …
└── .teamcraft/
    ├── state.json                       ← per file: item, revision, file id, version, SHA-256, checked out
    └── migration.log                    ← what the move from the old layout did
```

The folder of a file is stored with it on the server (*Folder* on the
revision page, `folder` in the API). The folder import sets it from the
scanned tree; uploads without a folder put new files into the item's folder
(chosen in *New*, the top of the group by default), and a file keeps its
folder when new versions arrive. While a revision is checked out, the
revision page can move a file to another folder.

**Umlauts and accents.** macOS hands out file names *decomposed* (Unicode
NFD: `a` followed by a combining diaeresis for `ä`), Windows and the CAD
programs there write them *composed* (NFC). The two look the same, but on
Windows and Linux they are different files, and an assembly saved on Windows
does not find a part whose name came through a Mac. TeamCraft therefore keeps
every file and folder name composed: the server composes what it receives,
the folder import and the working folder use composed names on every system,
and a file an older app version wrote under a decomposed name is renamed at
the next Get or when the Workspace tab looks at it.

**Files open in SolidCraft.** While SolidCraft has a design of the working
folder open, the Workspace tab marks it *Open in SolidCraft* and names the
computer and user (from the lock file SolidCraft keeps next to the design;
locks not refreshed for three minutes are ignored). The check-in dialog warns
about such files: changes not saved in SolidCraft are not checked in. You can
check in anyway.

**TeamCraft inside SolidCraft.** The TeamCraft plugin for SolidCraft
(`plugins/solidcraft`) shows a design's TeamCraft item, checks it before
check-in and links it to an item – offline, from the design itself. With the
desktop app running it also shows the live state (*TeamCraft live status*:
checked out by whom, local changes, whether the app reaches the server) and
sends requests to the app: *Check out in TeamCraft*, *Check in with
TeamCraft*, *Open in TeamCraft*. SolidCraft plugins have no network, so the
two share the folder `.teamcraft/solidcraft` in your working folder; grant it
to the plugin as `teamcraft` (reading and writing) when you install it. The
app asks before a check-out the plugin requests and opens its usual check-in
dialog for a check-in – a request never acts on its own. When the app is not
running, the plugin says so and shows what the app knew when it last ran; a
request waits 10 minutes for the app to start.

**Read-only rule.** Files you have **not** checked out are read-only on disk (the read-only attribute on Windows, no
write permission on Linux and macOS).
CAD programs then open them read-only, and nobody edits a file that cannot be
checked in. *Check out* makes the files of the revision writable; *Check in*
uploads the changed ones and makes them read-only again; *Cancel check-out*
does the same without uploading. Files you changed are never made read-only
or overwritten by TeamCraft.

**Check-out of an assembly.** The lock is taken on the assembly only; its
structure is then fetched like *Get* does under the *latest* rule, in the same
progress dialog. Per file: missing – downloaded read-only; up to date –
skipped; an unchanged older copy – updated; changed here, checked out by you,
or a different file at the same place – kept untouched and listed (*kept – your
local changes*, *name clash*). Cancelling leaves no partial file. **Add files.** *Add files…* (Files section and *More
▾* of a revision) takes
files from a file picker, checks the revision out first if you have not,
and **copies** them into its working folder – the originals are never moved
or changed. New files go into the revision's main folder (for a revision
without files yet: the item's folder) or a sub-folder of it. A file with the name of an existing file becomes its next version and
asks first (unticked files are not added). The copies appear as *new*
files in *Check in…* – also files with an unrelated name, which TeamCraft
remembers in `.teamcraft/added.json` until they are checked in – or tick *Check in now*. The Files section explains why
the button is disabled:
checked out by someone else (add after check-in), in review, or released (frozen: *Revise* and add the files to the new
revision).

**Drop files.** Drag CAD files from Explorer, Finder or your file manager
onto the window. A frame shows what the drop does before you let go:

* on an open revision tab that you have checked out, or that *Add files…*
  would check out for you, the files go into *Add files…* (with the same
  dialog, sub-folder and confirmations as picked files);
* anywhere else (the start page, a change, a request) the drop opens
  *New item* with the files already chosen – name, type, group and folder
  follow from them like from picked files, and the duplicate check runs;
* a revision that takes no files (released, in review, checked out by
  someone else) refuses the drop with the same reason the Files section
  gives, and so does the app for viewers and for editors without a project
  group to create items in. While a dialog is open, close it first.

Only files are taken: dropped folders are skipped, with a note. The files
are copied, never moved, and nothing in the working folder is overwritten
without asking – a drop has exactly the checks of the file dialog. Tabs are
still reordered by dragging them.

**Get.** *Get…* on a revision resolves its structure on the server – the
multi-level BOM with the revision rule you choose (*latest*: the newest
revision of every part, also in work; *released*: the newest released
revision) plus the items whose files its CAD files reference (an assembly
names its parts, a drawing its model) – and downloads **only what is
missing or outdated**, compared by SHA-256. Every file is checked against
its SHA-256 after the download and recorded at once, so a Get that was
cancelled or interrupted continues where it stopped – also inside a large file:
the bytes already received stay next to the file (`<name>.<sha>.teamcraft-download`, ignored by the working folder), the
next
Get asks the server only for the rest and checks the SHA-256 of the whole
file before it puts it in place. A file that changed on the server
meanwhile is downloaded completely again. The status bar shows
the progress with a *Cancel* button.

**Nothing is overwritten.**

* A file you changed locally is kept (and listed) when the server has a
  newer version – *conflict* in the Workspace tab.
* Two items with a file of the same folder and name (`chassis/bolt.prt` in
  two items) cannot share one place: the second is reported as a **name
  clash** and not downloaded. Give one of them another folder on the
  revision page.
* A file in the way that did not come from TeamCraft is reported the same
  way.

**Why not the browser?** A browser download saves single files into the
Downloads folder: no folder structure, so the CAD program cannot find the
parts of an assembly; no record of which version a file is, so changes
cannot be checked in safely; and no read-only protection. Browser downloads
are fine for one file or for looking at something – CAD work goes through
the desktop working folder. *Pack and Go* (ZIP) is for sending a structure
to someone outside TeamCraft: after the save dialog a progress dialog shows
how much has arrived, and **Cancel** stops it – also while the server is
still packing. The ZIP is written under a temporary name next to the target
and renamed only when it is complete, so a cancelled or failed Pack and Go
never leaves a half ZIP behind.

**Moving an older workspace.** Earlier versions of the app kept one folder
per revision, `<group>/<item>/<revision>/` (and before that
`<item>/<revision>/`). After the sign-in the app finds these folders and
offers to move the files to their places in the new layout. Files whose
place is already taken by a different file, and older revisions of an item
whose newer revision takes the place, stay where they are; files you
changed are moved first. Identical copies are removed, emptied old folders
too, and every step is written to `.teamcraft/migration.log`.

## Getting started

After the **first sign-in** the app opens a short wizard, *Getting
started*; *Help → Getting Started…* opens it again at any time. It has one
job: bring you to a real assembly of your project group, with *Check out
and open* in front of you. A line at the top says which server and account
you are connected with (change them in *Settings → Account*). It has at
most three steps:

1. **Where your files go** – your CAD files live in the working folder,
   each project group in its own folder
   (`<working folder>/<group>/<folder>/<file>`). *Change…* picks another
   folder (the same setting as *Settings → Workspace*), *Open folder* shows
   it in the file manager – only once the folder exists; before that the
   button is off and a line says that the first *Get* or check-out creates
   it. Below, one line names the CAD programs found on
   this computer – or says that none was found and you can still open files
   yourself from the working folder. Skipped when your organization set
   where working folders go ([Managed computers](#managed-computers)) or a
   *Get* already used the folder; the header then shows the folder.
2. **Your project group** – with several groups: *Which group do you work in
   first?*, one row per group with its default CAD program and *already in
   your working folder* where that is so. Skipped with exactly one group.
   **Without a group** this is the last step: CAD editors get *Create a
   project group* (the wizard closes and *Projects → New* opens), viewers
   *Copy user name* to send to a group manager who adds them.
3. **Your first assembly** – what *Check out and open* will do: lock the
   assembly, copy it with its parts into `<working folder>/<group>/` and
   open it in the group's CAD program – for files of that program the same
   rule as the chip next to *Check out and open* (a STEP or other neutral
   file opens as the chip of its tab says): the group's version when it is installed,
   else the newest installed version (the line then says that the group's
   version is missing), else *not installed on this computer*. Below, up
   to five assemblies of the group – top-level ones that are **in work** and
   free, the largest first – with their picture and **Show** *number*. A
   released, in-review or checked-out assembly is not offered (one you hold
   already is in your worklist); when none is free and in work, the step
   says so – open one in *Files* and revise it to work on it. An empty group shows *Import CAD folder…* and
   *New item* (see [Empty views](#empty-views)); a viewer reads that
   *Get…* copies an item with its parts and gets the group's assemblies in
   any state, with no check-out promise. A link leads to
   how a group sets up its CAD programs
   ([Set up CAD programs](../admin/cad-setup.md)).

**Show** *number* makes that group your active one, closes the wizard and
opens the revision tab of the assembly, where – for a CAD editor – **Check
out and open** is the highlighted next step: your next click is real work. *Finish and show
<group> in Files* instead makes the group active and shows its files in the
navigator (*Files*). The group you pick in step 2 only counts with one of
these clicks: *Back* and *Don't show again* change nothing.

The wizard **creates no file or folder**, gets and checks out nothing and
writes nothing to the server; it is the same whichever CAD program you use.
It stores only that you finished it (or chose *Don't show again*) – **per
saved connection**, so a second server or account offers it once more – the
working folder if you pick one with *Change…*, and the active group. Other
preferences stay as they are: *Finish and show <group> in Files* switches the
navigator to *Files* for this run only. It does not open by itself for an
app that starts already signed in (an update, a reload), nor for an account
that already has check-outs or has checked in (or uploaded) a file on that
server before (a new computer, an expired sign-in). The update
notice, the *Support us* tag and the keychain warning wait until it is
closed; the question to move a workspace of the old layout comes first.

**The daily loop, where it happens.** While a revision's next step is
*Check out and open*, *Open in CAD* or *Check in…*, its tab shows one tip: a
check-out only locks it – *Cancel check-out* undoes it without a new
version; save in your CAD program as usual, then *Check in…* there or in
your worklist. After your first check-in or cancelled check-out the tip
goes, and one line says *This is your daily loop* (once per computer).

## Empty views

An empty list says what to do next, and the answer depends on your role
(#423):

* **No project group yet** – CAD editors get *Create a project group* (you
  become its manager and add the others); viewers are asked to have a
  manager of a group add them.
* **An empty project group** in *Files* (*Files*, *All items* and
  *Structure*) – *Import CAD folder…* and *New item*, for CAD editors who may
  create items in the group; CAD editors who are not members of it are asked
  to have a manager of the group add them; viewers learn that the group's CAD
  editors add the items.
* **New on this computer** in the worklist without suggestions – *Get…*
  copies the files of a project group into your working folder; the dialog
  starts with the active group ticked.
* **Checked out by me**, **Waiting for my review** and **My open changes**
  say in one line how work gets there – in the worklist, where an empty
  section is one line that opens this explanation (#513).
* A **viewer account** sees a note at the top of the worklist and of the
  *Home* tab: editing –
  items, check-outs, changes – needs the CAD editor role; ask an
  administrator of the server.

An empty working folder on a revision tells the holder of the check-out to
save the CAD files there and check them in, tells who may check the revision
out to do so, and everyone else only that it has no files yet.

The texts follow *Settings → Language*.

## Install or run it

The installers of every version are on the
[Releases page](https://github.com/WARR-eV/TeamCraft/releases) (see *Download* in the README for the file of each
system). From source:

```sh
cd desktop
cargo run            # Linux needs the WebKitGTK development packages
```

The platform guides list the system packages.

## Try TeamCraft on this computer

The downloads **with local server** (Windows x64 `-setup.exe`, macOS `.dmg`,
Linux `.AppImage`, named `TeamCraft-with-local-server_…`) also contain the
server, as `teamcraft-server`, and PostgreSQL's programs. They install **TeamCraft Local**, an app of its own
(identifier
`io.github.rudalabs.teamcraft.local`, own settings folder and keychain entry)
next to the desktop app, which keeps its own server and sign-in. With no
server configured – its first start – TeamCraft Local starts the local server
by itself; later its connect screen offers **Try TeamCraft on this
computer** below the server address:

1. The app starts `teamcraft-server local` – a private PostgreSQL cluster and
   the server, both only on `127.0.0.1` – and waits until it is ready (the
   first start creates the database).
2. The server's address `http://127.0.0.1:<port>` becomes the app's server,
   and the browser sign-in opens. On the first start the browser shows *Create the administrator*; afterwards it
   continues to the confirmation of
   the app's sign-in.
3. While the local server is the configured server, the app starts it again
   at every launch (the splash says *Starting the local TeamCraft server …*).
   It keeps the port, so the address and the token stay the same; if another
   program took the port meanwhile, the app picks a free one and follows.

The server stops when the app ends – also when it crashes: the app keeps a
pipe to the server's standard input open, and the server shuts itself and
PostgreSQL down when it closes. Its output goes to `local-server.log` in the
app's settings folder; the data is in the per-user data folder shared with
`teamcraft local` – see [Try it on your computer](local-server.md). The
regular installers do not show the button.

## Update notice

Once a day at most, the app asks GitHub for the latest TeamCraft release. If
it is newer than your version, the status bar shows a green pill *TeamCraft
x.y.z is available* (the tooltip names the version you have). A click opens
the **update window** with the release notes and:

* **What your server needs:** when this app or the offered version does not
  fit the server you are connected to (before 1.0 app and server must share
  the first two numbers of the version, 0.3.x with 0.3.x), the window names
  the desktop version the server needs – *Your server runs 0.3.0 and needs
  TeamCraft Desktop 0.3.x* – and whether the update is it. When this app
  fits the server and the update does not, it says so and to keep this
  version until the administrator upgrades the server. The same version
  is named in the notice after connecting to a server that does not fit.

* **Update now** (Windows installers and portable zip, macOS, AppImage):
  downloads the file for your installation into your Downloads
  folder and checks it against the release's published checksums
  (`SHA256SUMS.txt`). A file that does not match is deleted and nothing
  changes. A Windows installer then starts after you confirm – TeamCraft
  closes, your working folder, sign-in and settings stay; on macOS the disk
  image opens and you drag TeamCraft to *Applications*. The Windows portable
  zip and the AppImage replace your copy in place after you confirm
  (*Replace and restart*): the new version goes into the app's folder, the
  old files are removed at the next start, and TeamCraft restarts (#466).
  Only a folder unpacked from the portable zip is updated in place (it holds
  the zip's `README.txt` or `.teamcraft-portable`); for a `TeamCraft.exe`
  copied somewhere else, or an MSI installed into a folder of your choice,
  the window has no *Update now* – it says to download the new version from
  the release page (*Release page*). A `README.txt` or `LICENSE.txt`
  of your own next to the app is left alone, and only the files the update
  moved aside are removed at the next start. A
  folder the app may not write into, or any other failure, changes nothing –
  the window says why and *Show the file* shows the verified download. If
  the files were replaced but the new version does not start, the window
  says so and offers *Restart now* and *Close TeamCraft* – also when you
  open it again – never a second download. Installed with Flatpak or a
  `.deb`/`.rpm` package, the window names the package-manager way instead.
  Not while a check-in, park, upload, Get or check-out, *Get project
  groups*, folder import, *Add files…*, the move of the workspace or Pack
  and Go runs – also one started from *Local changes* or *Check in all*.
* When the offered version does not fit your server, *Update now* asks
  first (*Update anyway?*), and the release button reads **All releases**:
  it opens the list of releases, where you find the version your server
  needs.
* **Skip this version** (or *×* in the status bar): no notice until a newer
  version.
* **Remind me later**: the notice comes back in three days.
* **Release page**: the release on GitHub in your browser.

Closing a confirmation (*Replace and restart?*, *Close TeamCraft and
install?*) with *Esc* or a click beside it counts as *Not now*; *Esc* closes
only the dialog on top.

*Help → Check for Updates…* asks at once – also when the daily check is off –
and says whether you are up to date. A tester's release candidate is told
about the next candidate of the same version (0.3.0-rc.1 → 0.3.0-rc.2);
versions are compared by Semantic Versioning, build metadata (`+…`) never
counts. *Settings → Updates* switches the daily
check off (for example on computers without internet access).

## Support us on Ko-fi

TeamCraft is free software, developed by a non-profit association. Once per
start of the app, the status bar shows a small orange **Support us on Ko-fi**
pill. When the update notice shows too, the two pills sit side by side; in a
narrow window the workspace path shortens first, then the pill labels end in
an ellipsis – they never overlap.
A click opens the association's Ko-fi page (`https://ko-fi.com/warr_ev`) in
your browser and hides the pill; *×* only hides it. It does not come back
until the app starts again – reconnecting or switching server does not show
it again. Nothing is loaded from ko-fi.com inside the app and nothing is
sent or counted; only your click opens the page.

*Settings → Support TeamCraft* switches the tag off for good on this
computer. On a managed computer your organization may have switched it off
(or on) with the `ShowDonateTag` policy; the switch then shows *Set by your
organization*.

## Report a problem

Every error message has a **Report…** link – the messages at the bottom,
the errors in a dialog, and the errors shown in place: a list or tab that
could not be loaded, the new-item file check, *No BOM lines found* in the BOM
import preview, the error on the 3D view (also when the model cannot be read
or the graphics memory runs out) and the problems of *Check in all*. These
errors are also among the session's error messages a later report can add –
each once, even when the list showing it is drawn again. Escape in the report
dialog closes only the report, not the dialog it was opened from. *Help →
Report a Problem…* opens the same dialog without one. It shows the report before anything
leaves the app: a title (what you did and the error), the action, its key
inputs, the error and the versions; add what
you were trying to do (left out if empty) and, if you like, the error
messages of this session. Your home folder, server addresses and tokens are
removed. Edit the text as you like, then:

* *Open issue form* opens a new issue at the TeamCraft repository on GitHub –
  or your help desk's form, if *Settings → Report problems to* names one (or
  your organization set one with the `ReportUrl` policy). A
  long report is cut in the link and copied to the clipboard in full;
* *Copy report* puts it on the clipboard;
* *Save report…* writes it to a file (both work without a network).

The key inputs are what the action worked on, for example the project group,
item number and revision of a check-out or check-in, the names of the ticked
files, the file and format of a 3D preview, or the file name and options of a
BOM or folder import, the number and status of a change order or change
request with the reviewer or decision, or the name and new value of a setting
that could not be saved. They are names and short values only: a path is
reduced to the file name, and file contents, item names, descriptions and
BOM lines are never included.

The web UI's pages for server errors have *Details for a bug report* with
*Copy details* (status, message, page, time, version and the request id); a
page that was not found has none – a missing or hidden object is no bug. The
request id is also in the server log, so your administrator can find the
error there.

## Usage report

Once a day of use, the app sends your TeamCraft server an anonymous report of
the day before: a random id of this installation, the app version, system,
package type, the minutes the window was in front, and how often fixed
actions were used (check-out, check-in, cancel check-out, get files, open in
CAD, 3D preview, BOM and folder imports, new items, releases, searches and
assistant questions, STEP exports per CAD program) – never names,
numbers, paths, search terms or anything you type, and not who you are.
*Settings → Privacy → Show what is sent* shows the exact report, and *Send an
anonymous usage report* switches it off: nothing is then counted, kept or
sent, the counts not sent yet are deleted, and *Show what is sent* says that
the report is switched off (#473). If your organisation
set the `UsageStatistics` policy, the switch shows *Set by your organization*.
The report stays with your organisation; your administrator decides whether the server
shares a summary with the TeamCraft developers (off unless they turn it on) –
see [Privacy](../admin/privacy.md).

## Crash reports

When the app crashes – a panic in the app, an uncaught error in its user
interface, or a session that ended without closing normally (for example the
web view crashed, the app hung and was force-quit, or the computer lost
power) – it writes a report into the `crash-reports` folder of its settings
directory. Before the report is written, the home folder (and with it your
user name), server addresses and sign-in tokens are removed. The report holds
no item data and no files.

At the next start a dialog shows the whole report:

* *Report on GitHub…* opens a new issue at the TeamCraft repository in your
  browser, prefilled with the report. You read it, add what you were doing,
  and submit it yourself; a GitHub account is needed. The full text is also
  copied to the clipboard, in case the link had to shorten it.
* *Copy* puts the report on the clipboard, for example for an e-mail.
* *Delete report* removes it without sharing.

Nothing is ever sent without that choice. Closing the dialog keeps the report
for the next start. *Don't ask again* (or *Settings → Crash reports*) stops
the question; the reports then stay on the computer only. Background and the
alternatives that were considered: [Crash reports from the desktop
app](../research/crash-reporting.md).

## Managed computers

On a computer your IT department manages, some settings can be fixed by
*policies* (see [Deploying the desktop app](../admin/desktop-deployment.md)):
the server, the update check, the question after a crash, the default
working folder and the *Support us on Ko-fi* tag. *Settings* then shows *Managed by …* at the top, and a fixed
setting is marked **Set by your organization** and cannot be changed. With a
fixed server, the connect screen shows that server only, *Switch server…* is
hidden and saved connections to other servers cannot be switched to.
