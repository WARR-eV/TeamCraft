# Desktop app

TeamCraft Desktop (`desktop/`, Tauri 2) is a rich client for Linux, macOS and
Windows: a navigator tree of the project groups, object tabs (overview,
files, structure, where used, references, graph, changes, history, 3D viewer), a worklist with
check-outs and reviews, and a **local working folder**:

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
* the *Graph* tab draws a navigable reference and BOM graph around the
  revision: the focused item in the middle, its BOM children and references
  below (*ref*), the assemblies that use it above (*uses*). Every node is a
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
  colours of the chosen skin;
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
  when the revision has an assembly whose references are read (Inventor
  `.iam`, SOLIDWORKS, an NX `.prt`, a STEP file, a SolidCraft `.sc`). The
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
* *Dependents to check* (worklist and home) shows one line per dependent
  revision with the number of changes it has to be checked against; the
  arrow lists them (a click opens the item that changed). On the home page
  every dependent has **Open**, **Revise** and **Acknowledge all…** (one note
  for all of them, see [Change propagation](structure.md#change-propagation))
  and an **Acknowledge…** per change;
* *Request change…* on a revision tab and *Request…* on a file (Files and
  Workspace tabs) raise a [change request](changes.md) with the item,
  revision and file name filled in and an optional screenshot or PDF; the
  worklist lists the requests assigned to you and your open ones;
* on Windows with Inventor or Solid Edge installed, *STEP* (workspace) and
  the check-in option (off by default) create STEP exports for the 3D preview with that
  program; the Files tab shows the preview image stored in native files.
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
      checkbox, *Cancel* and *New conversation*, and keeps one conversation on
      the server that survives restarts.
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
the *3D viewer* tab (see [Files](files.md)). *Settings → Appearance*: *TeamCraft* (the
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

| Revision                                                                    | Highlighted                                                 |
|-----------------------------------------------------------------------------|-------------------------------------------------------------|
| in work, not checked out                                                    | *Check out and open*                                        |
| checked out by you, with local changes (or the workspace could not be read) | *Check in…*                                                 |
| checked out by you, no local changes                                        | *Release…* (when it can be released, otherwise *Check in…*) |
| in review, obsolete, or checked out by somebody else                        | nothing                                                     |
| released                                                                    | *Revise*                                                    |

The working folder of your check-out is looked at again whenever the tab is
shown or the app window gets the focus back – save in the CAD program, switch
back, and *Check in…* is highlighted.

* revision tab: **Lock** (*Check out*, or *Check in…* and *Cancel check-out*
  / *Force unlock* while checked out) · **Open** (*Check out and open*, or *Open in CAD* for your own
  check-out, with the CAD program that will open the file next to it – see
  [Check out and open](#check-out-and-open)) · **Work** (*Edit attributes*, *Get…*, *Open
  folder*) · **Lifecycle** (*Release…*, *Revise*) · **Collaborate**
  (*Request change…*, *Watch*, *Agent*);
* change tab: **Review** (*Approve change* highlighted, *Reject* – the comment field
  of *Your review* goes with it, required for a rejection) while your review
  is pending · **Lifecycle** (*Submit for review*, highlighted on your draft –
  disabled with the reason as tooltip while the draft has no revision or no
  reviewer –, *Withdraw*, *Cancel change* for a draft or rejected change, at
  once) · **Collaborate**. On your draft the *Reviewers* panel has *Add
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
navigator, Home *Files* / *All items* / *Structure*, the worklist's
check-outs and the cards of the *Graph* tab – opens *Check out and open*
(for your own check-out *Open in CAD* and *Check in…*), *Check out*, *Get…*,
*Show in folder*, *Copy number*, *Where used*, *Request change…* and *Open in
browser*, without opening the item first. The entries follow the toolbar's
rules: one that does not apply is greyed out and its tooltip says why
(*Checked out by Mia*, *no native CAD file*). Shift+F10 or the context-menu
key opens it on the row that has the focus; the arrow keys move, `Esc`
closes.

Rare actions are in **More ▾** at the end: *Pack and Go…*, *Open in browser*, *Refresh* (`F5`) and *Mark obsolete…*. The
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
UI, *Open in browser*).
CAD editors create a new group there (*New* – you become its manager; tick *Restricted* to keep it to its members), and
the page offers *Get…* (copy
groups into your workspace) and *Import CAD folder…*. The choice is kept on
this computer.

**Home** is a file browser of the active group, in three views:

* *Files* (the view Home starts with): the **folders** of the group and the **files** in them, as in its working
  folder – folders first, then the files
  by name the way a file manager orders them (without regard to case, `Part2`
  before `Part10`), each with the state and the lock of its item. A checked-out file is marked
  with a bar in the lock colour at the left edge of its row and a light tint
  of the row (the same for your check-outs and other people's; also in
  *Structure*). The file
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

**Favourite folder.** When your work is several folders down, make that
folder the group's favourite: point at the folder in *Files* and click its
☆. Home then starts in that folder instead of at the top of the group. A
banner in the accent colour above the list shows where you are
(`ROVER / Design / Rover`) and that it is *not the top of the group*;
*Whole group* shows the top (*Back to the favourite folder* returns), and
*Remove favourite* (or the filled ★ of the folder) lets Home start at the
top again. If the group holds an Inventor project file (`.ipj`) in a
folder, Home suggests that folder – *Start there*, or *No, thanks* to stop
the suggestion for this group. The favourite is kept on this computer, for
each server and group; the count next to the order then counts the rows
below the favourite folder.

*Files* and *All items* are sorted by name (or by number, type or state – the
choice is kept). *Change…* goes back to *Projects*. *New* and *Import CAD
Folder…* start with the active group. *New* (and `Ctrl/Cmd+N`) also starts
with the folder you last opened or clicked in in *Files*; its *Folder* field
(e.g. `chassis/brackets`, empty = the top) is where the item and the files
chosen in the dialog go – type a new path to start a new folder. With a
server older than the *Files* view, Home shows *All items* instead.

*Home*, *Projects*, *Refresh* and the dialogs that offer project groups (*New*, *Get Project Groups…*, *Import CAD
Folder…*) load the groups and your
memberships from the server again, so a group created or joined in the web UI
shows without restarting the app.

Keyboard: `Ctrl/Cmd+N` new item, `Ctrl/Cmd+F` search, `F5` refresh,
`Ctrl/Cmd+1/2/3/4/5` Home, Worklist, Changes, Agent, Projects, `Ctrl/Cmd+W` close the tab,
`Ctrl/Cmd` + `+` / `-` / `0` (or `Ctrl` + mouse wheel) zoom.

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
  running – the installation chosen above, with the working folder's
  Inventor project active – the file opens there as a new document window,
  and Inventor comes to the front, instead of starting a second instance.
  The working folder's project is the Inventor project file (`.ipj`) in the
  nearest folder from the file's own up to the working folder root; a folder
  with several `.ipj` files makes it unknown. A new instance starts when no
  Inventor runs, when another version runs, or when the running one has
  another project active (or the working folder has no `.ipj`); the message
  after opening says which (*Inventor is running with the project Default,
  not Graksler 4.3 – opened in a new instance*). Inventor is asked through
  its COM interface by a short Windows PowerShell script, as for the STEP
  export.
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
revision). Dropping files
onto the window is not supported yet.

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
it is newer than your version, the status bar shows *TeamCraft x.y.z is
available*. A click opens the **update window** with the release notes and:

* **Update now** (Windows installers, macOS) or **Download** (portable zip,
  AppImage): downloads the file for your installation into your Downloads
  folder and checks it against the release's published checksums
  (`SHA256SUMS.txt`). A file that does not match is deleted and nothing
  changes. A Windows installer then starts after you confirm – TeamCraft
  closes, your working folder, sign-in and settings stay; on macOS the disk
  image opens and you drag TeamCraft to *Applications*; a zip or AppImage is
  shown in its folder to replace your copy. Installed with Flatpak or a
  `.deb`/`.rpm` package, the window names the package-manager way instead.
  Not while a check-in or Get runs.
* **Skip this version** (or *×* in the status bar): no notice until a newer
  version.
* **Remind me later**: the notice comes back in three days.
* **Release page**: the release on GitHub in your browser.

*Help → Check for Updates…* asks at once – also when the daily check is off –
and says whether you are up to date. *Settings → Updates* switches the daily
check off (for example on computers without internet access).

## Report a problem

Every error message has a **Report…** link, and *Help → Report a Problem…*
opens the same dialog without one. It shows the report before anything
leaves the app: a title (what you did and the error), the action, its key
inputs (no file contents or item data), the error and the versions; add what
you were trying to do (left out if empty) and, if you like, the error
messages of this session. Your home folder, server addresses and tokens are
removed. Edit the text as you like, then:

* *Open issue form* opens a new issue at the TeamCraft repository on GitHub –
  or your help desk's form, if *Settings → Report problems to* names one (or
  your organization set one with the `ReportUrl` policy). A
  long report is cut in the link and copied to the clipboard in full;
* *Copy report* puts it on the clipboard;
* *Save report…* writes it to a file (both work without a network).

The error pages of the web UI have *Details for a bug report* with *Copy
details* (status, message, page, time and version).

## Usage report

Once a day of use, the app sends your TeamCraft server an anonymous report of
the day before: a random id of this installation, the app version, system,
package type, the minutes the window was in front, and how often fixed
actions were used (check-out, check-in, 3D preview …) – never names,
numbers, paths, search terms or anything you type, and not who you are.
*Settings → Privacy → Show what is sent* shows the exact report. The report
stays with your organisation; your administrator decides whether the server
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
the server, the update check, the question after a crash and the default
working folder. *Settings* then shows *Managed by …* at the top, and a fixed
setting is marked **Set by your organization** and cannot be changed. With a
fixed server, the connect screen shows that server only, *Switch server…* is
hidden and saved connections to other servers cannot be switched to.
