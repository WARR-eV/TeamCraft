# Changelog

All notable changes to TeamCraft are recorded in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the version numbers follow [Semantic Versioning](https://semver.org/).
While the version is `0.x`, any release may contain breaking changes. The
policy for supported versions is in the
[support policy](https://warr-ev.github.io/TeamCraft/admin/support-policy.html).

TeamCraft has no stable release yet. The project status is work in progress:
see the notices in the README.

## [Unreleased]

### Security

- Desktop: `cookie` 0.18.2 → 0.18.3 (RUSTSEC-2026-0594, private-cookie
  nonces can repeat after `fork()`; pulled in by Tauri). The app does not
  fork or use private cookies, but the advisory turned the weekly
  `cargo deny` check of `main` red.

### Fixed

- Web: the *Items* table of the vault explorer fits its card at 1280 and
  1440 px – the name column takes the room that is left and ends in an
  ellipsis, and the table scrolls inside the card instead of running past
  its edge (#521).
- Web: a section link (revision page, simulation results, as-built) lands
  below the sticky top bar and section nav. Anchors clear the top bar on every
  page and, below a section nav, also the nav by its measured height (app.js
  sets `--section-nav-h`, so a taller nav – larger fonts, zoom, a classic
  scrollbar under its list – is cleared too); the screenshot script scrolls
  with `scrollIntoView` and so the same way (#523).
- Web: error pages offer *Details for a bug report* only for server errors
  (5xx); a 404 – also an object hidden by need-to-know – is no bug. A
  signed-in user's 404 or 403 is shown inside the app with the top bar,
  *Back* and *Dashboard* (#524).
- Web: *Built on* of an as-built unit (card and *Built units* list) is shown
  in the viewer's date format like the other dates of the card, the same day
  in every time zone, instead of as an ISO date (#527).
- Desktop, *Get project groups* (#449): *Stop* keeps the dialog open
  (*Stopping…*) and shows the *Stopped* step instead of closing it; while the
- Desktop, *Get project groups* (#449): *Stop* keeps the dialog open and
  shows the *Stopped* step instead of closing it, once the file being copied
  has ended; until then *Close* may hide the dialog (the result comes as a
  message, and a new Get waits until the stopped one has ended); while the
  Get runs the dialog is modal, so Escape or a click beside it no longer
  hides it with the Get still running – a second Get could then run at the
  same time (#498); the revision rule and the groups are locked and the rule
  is taken once, so switching it midway no longer mixes revisions or marks
  the groups as fully fetched (#499); an earlier error of the dialog is
  cleared when a new run starts (#500); a name clash is listed once, among
  the problems, in the singular for one clash (#501); "everything was up to date" shows only when items
  were got and nothing failed or stopped, and a run where every item was
  skipped says *Nothing to get*.
- Desktop, *Report…* (#471): the worklist's "Could not compare with the
  server" is kept for the report's recent messages once per scan, not on
  every redraw (window focus, refresh), and a message equal to one of the
  last five is not kept again, so repeats no longer push older errors out;
  the 3D viewer's errors on the canvas and the problems of *Check in all*
  are kept for the recent messages too; Escape in the report dialog opened
  from a dialog's error closes only the report, not the dialog underneath
  with its input (#502).
- Desktop: follow-ups of the lock refresh (#451). The *Check-outs* tab keeps
  polling after a failed reload, an older answer (a slow poll after
  *Refresh*) no longer paints over a newer one, a tab that comes to the
  front because its neighbour was closed loads again, and a repaint keeps
  the keyboard focus on the revision it was on. *Force unlock* of someone
  else's lock clears the Home row's marker at once, and a re-marked row's
  tooltip names the new holder.
- Desktop: whether a lock is yours is decided by your account, not your
  display name – a colleague with the same name no longer shows as you, and
  a rename no longer turns your locks into someone else's.
  `GET /projects/{code}/files` and the structure endpoints send the
  holder's id (`checked_out_by`) for it (#503).
- Desktop: after signing out and in again (another user or server), Home
  no longer shows the previous user's worklist, and the first worklist of
  the new session no longer marks the previous user's locks as released
  (#504). A worklist answer or a scan of the working folder (*New on this
  computer*, *Local changes*) still on its way at the sign-out is dropped
  instead of being drawn for the next user.
- Desktop: worklist answers that arrive out of order no longer leave a
  stale worklist (rail count, *Check in all*, Home) or mark a released row
  as checked out (#505).
- Desktop update check: versions compare by SemVer precedence, so a tester
  on `0.3.0-rc.1` is told about `0.3.0-rc.2` (and `rc.10` beats `rc.9`,
  whatever GitHub's order) instead of hearing "up to date" (#529); build
  metadata with a hyphen (`0.3.0+build-1`) no longer makes a release a
  pre-release (#530).
- Desktop update window: when the offered version does not fit the
  connected server, *Update now* asks first and the release button reads
  *All releases* and opens the release list instead of the page of the
  version just warned of (#468).
- Desktop update window: *Update now* waits for every running transfer –
  also the check-ins of *Local changes*, *Check in all* and sign-out, the
  new-item upload, *Get project groups*, the folder import, *Add files…*,
  the move of the workspace and every Get or check-out (two at once each
  count, not only the shared progress bar), counted
  explicitly – and no longer refuses because a dialog shows an idle status
  line; the check is repeated before the swap, the restart and the
  installer start (#531, #532).
- Desktop update window: closing *Replace TeamCraft and restart?* or *Close
  TeamCraft and install?* with Esc or a click beside it counts as *Not now*
  instead of leaving *Update now* disabled, and Esc closes only the dialog
  on top (#534).
- Desktop update window: when the files were replaced but the new version
  does not start, the window says so and offers *Restart now* and *Close
  TeamCraft* instead of a second *Update now* that cannot work – also when
  the window is opened again; two overlapping swaps are refused (#466).
- Desktop in-place update (#466): only a folder unpacked from the portable
  zip (its `README.txt` or the new `.teamcraft-portable` marker) is updated
  in place – an MSI installed into a folder of one's choice or a
  `TeamCraft.exe` copied elsewhere gets no *Update now* – the window points to the release page (#533); a
  `README.txt`/`LICENSE.txt` of the user's own next to the app is left
  alone, an existing `*.old` is never renamed over, and the next start
  removes only the files the swap listed; each instance unpacks into its own
  staging folder, which a second instance's start no longer deletes; a
  failed AppImage copy leaves no partial `<name>.new`.
- Desktop: the highlighted next step of a revision you have just checked out
  (nothing changed yet) is *Open in CAD*, no longer *Release…*. Local
  changes, versions parked since the check-out or an unreadable working
  folder highlight *Check in…*; *Release…* is highlighted only when the
  revision is checked in, can be released and the newest upload made in this
  revision is your own – files *Revise* carried over from the previous
  revision and a cancelled check-out do not count. The rule is a pure module
  (`desktop/ui/js/next-step.js`) tested by `scripts/desktop-next-step.test.mjs`
  in the CI job `ui` (#536, UI review #513 T2).
- Desktop: a disabled primary button (e.g. *Start* in the Agent navigator
  while it starts) looked enabled – `button.primary` came after
  `button:disabled`. Disabled primary and danger buttons, and primary ones
  with `aria-disabled="true"`, are now greyed out in every skin (#516).
- Desktop change tab: an administrator who is a reviewer but not the creator
  saw *Withdraw* enabled next to *Approve change*. *Withdraw* is the
  creator's action in the toolbar; the administrator's override is
  *Withdraw (administrator)* in *More ▾* and asks for a confirmation first
  (`withdrawAction` in `release-rules.js`, tested) (#517).
- Desktop: the two *Check in all* buttons (Worklist navigator, Home card
  *Checked out by me*) had contradicting tooltips; both now use one text
  that says what the button does: check-outs without local changes are
  checked in, the ones with modified or new files are listed first to check
  them in with their files, conflicts stay checked out (#520).
- Desktop: a check-out or check-in from Home marks the row (lock colour and
  icon) at once instead of after leaving Home and coming back; the
  *Check-outs* tab of a group is loaded again each time it comes to the
  front and, while in front, every 30 seconds and when the window gets the
  focus back, so it shows the current state and other people's check-outs
  without *Refresh* (#451).
- Desktop: *Get project groups* ends with a clear result instead of a status
  line under the progress bar – *Done* with a tick and the counts, or
  *Finished with problems* / *Stopped* with every problem listed, *Open
  working folder* and *Close*; while it copies, *Get files* is disabled and
  *Close* reads *Stop* (#449).
- `desktop/README.md`: the *Ko-fi tag* row stood above the title instead of
  in the feature table, and *Revision tabs* was in the table twice with
  different details; one merged row now (#486).
- Desktop: *Check out and open* reuses a running Inventor again when the
  working folder has no Inventor project file (`.ipj`) – Inventor always has
  a project active (Default at least), so the check compared it with nothing
  and always started a second instance. Calls Inventor rejects while busy
  are retried for 10 seconds, and an Inventor that runs but cannot be asked
  (started as administrator) gives a note instead of silently starting a new
  instance (#483).
- Desktop, reusing a running Inventor (#483): an Inventor that keeps the
  script waiting past the 20-second limit gives the *did not answer* note
  too, instead of a silent new instance (the questions before the answer
  stop after 14 seconds); an Inventor of another Windows session (another
  user on a remote desktop server, fast user switching) or a hidden
  automation run no longer gives the *started as administrator* note – only
  a visible Inventor in the user's own session counts. Every call to Inventor, also
  reading the active project and looking up the open documents, is asked
  again while Inventor is busy.
- Desktop: when a running Inventor took the file but could not open it
  (locked or broken file, Inventor busy, a part already loaded as a hidden
  reference of an open assembly), the app now shows an error with the
  reason instead of nothing; a part loaded only as a reference is opened in
  its own window (#508).
- Desktop: the update window no longer tells you to install the desktop
  version you already have – when this app fits the server and the offered
  update does not, it says to keep this version until the server is
  upgraded (#468).
- Desktop usage report (#473): a report sent while you switched the report
  off is no longer written back into the settings; a newer day's report that
  became due during the send is kept instead of deleted (#509); *Show what
  is sent* says the report is switched off instead of showing an empty
  report; the error of the switch has *Report…* with its inputs like the
  other settings.
- Desktop: a setting that could not be saved (read-only or full settings
  folder) no longer stays changed in memory while the window shows the old
  value – e.g. a usage report switched back on that was counted and sent
  while the box showed it off (#510). What is already in effect is still
  remembered for the session when the save fails: the started local server
  (*Try TeamCraft on this computer*), the started agent and its model.
- Desktop: the update check never found a release – it asked GitHub for
  `releases/latest`, which leaves out pre-releases, and every 0.x release is
  published as one. It reads the list of releases now and offers the newest
  plain version (a release candidate only to an app that is one); *Release
  page* opens the offered version's page (#493).
- CI: the comment of the shared component picker named its issue as
  "#419", which the no-literal-colours guard of `shared/css/` reads as a
  hex colour; it says "issue 419" now.
- CI on `main` is green again (after #419, #420, #407, #459):
  - the BOM component picker (`.picker-*`) and `.visually-hidden` were styled
    in both app stylesheets; they are shared components now
    (`shared/css/components.css`), the web keeps its popup as a context rule;
  - the thumbnail link of the item list has an accessible name;
  - the module docs of `services::bom_references` linked a private function;
  - `bom_references::load_files` is named in the visibility guard's
    `NOT_GUARDED` list (both callers check the revision first);
  - the viewer's mouse-scheme test compared an array of the sandbox with
    `assert.deepEqual`.

### Changed

- **Getting started opens your first assembly** (desktop, #422, #478): the
  first-start wizard has one job now – bring you to a real assembly of your
  project group with *Check out and open* in front of you. At most three
  read-only steps: *Where your files go* (working folder, *Change…*, *Open
  folder* – only for a folder that exists –, the CAD programs found on this
  computer), *Your project group* (only with several groups; without one it
  says how to get one – *Create a project group* for CAD editors, *Copy user
  name* for viewers) and *Your first assembly* (what *Check out and open*
  does, naming the program for files of the group's program by the same
  rule as the chip next to the button;
  up to five assemblies of the group with their picture and **Show**
  *number* – for CAD editors only free assemblies in work (with a message
  when none is), so the revision tab opens with *Check out and open* highlighted;
  viewers get the group's assemblies in any state and no check-out promise;
  *Finish and show <group> in Home*). The server, sign-in, CAD setup and
  daily-loop pages are gone: the connection is one header line, the daily
  loop a one-time tip on the revision tab until your first check-in or
  cancelled check-out. The wizard creates no file or folder, gets and checks
  out nothing, writes nothing to the server, changes no other preference
  (the navigator shows *Files* for this run only) and has no step for any
  one CAD program; how a group sets up its CAD programs is the new admin
  page *Set up CAD programs for a project group*. It is remembered **per
  saved connection** (the old app-wide value moves onto the connection that
  was active), does not open for accounts that already have check-outs or
  have checked in before on that server (`has_checked_in`, new in
  `GET /api/v1/worklist`: a file version of theirs is stored), and the update notice,
  *Support us* tag and keychain warning wait until it is closed. Texts in
  the shared catalogs (English, German). Research page *The first start of
  the desktop app*. Rules tested by `scripts/desktop-onboarding-rules.test.mjs`.
  The nodes of `GET /projects/{code}/structure` and `…/revisions/{code}/structure`
  carry the lock holder's user id (`checked_out_by`) next to the name, so
  clients decide "checked out by me" by id, not by the display name.
- `CONTRIBUTING.md` is short now: ways to contribute, bug reports, the
  screenshots, the main code conventions, dependencies and branding, *Your own
  work only*, pull requests and the licence. New pull request policy: every
  pull request is squash-merged into one commit, its size is up to the
  contributor, CI does not have to be green for a merge but must be before a
  release, and contributions from outside the maintainers get a strict
  security review – one that tries to sneak in a vulnerability is closed
  without discussion. The development setup moved into the README's new
  *Development* section, below the features and screenshots; the checks, the
  CI table, the guard tests, the scale check and merging into the new book
  page *Checks, CI and merging* (`docs/src/dev/ci.md`); how releases are built
  into the *Release checklist*; the design rules and the translated UI into
  *Shared UI*.
- Desktop: the *Support us on Ko-fi* tag and the update notice in the status
  bar are pills now – one rounded shape around the label and its *×*, orange
  for Ko-fi, green for a new version. They sit side by side and never
  overlap; in a narrow window the workspace path shortens first, then the
  pill labels end in an ellipsis. The update pill says *TeamCraft x.y.z is
  available*; the installed version moved into its tooltip.
- README: seven more screenshots (revision page, STEP in 3D, change request,
  FEA result, notifications, review policy, desktop worklist) in a collapsed
  *More screenshots* block at the end of *Features*.
- README: fresh screenshots of the web UI and the desktop app at 0.2.13
  (the desktop change order after the script fix below).
- README: restructured in place. *Who it is for* comes first, the screenshots
  sit beside the feature groups they show instead of in one gallery at the
  end, single sign-on, offboarding, audit log, branding, monitoring, webhooks
  and backups are described once under *Enterprise management*, *Planned* is
  a section of its own, *Quick start* and *Download* no longer lead with the
  paused *with local server* installer, and the download details and the
  third-party licence table fold into collapsible blocks. No feature text
  moved out of the README.

### Fixed

- Screenshots: `scripts/screenshots.sh` clicked the old places in the desktop
  app. The worklist gained the sections *Local changes* and *New on this
  computer* and the action bar of a revision wraps to two rows at 1440 px, so
  the change order and the Graph tab were never opened and `desktop-change`,
  `desktop-graph` and `desktop-graph-nav` showed the revision tab. The click
  positions follow the new layout; the change order and the Graph tab open
  again. Still open: `desktop-graph` – the graph of the mounting bracket shows
  no card of the assembly above it to click, so it repeats `desktop-graph-nav`
  (neither image is used in the README).

- CI: the import check of the desktop modules (`scripts/check-desktop-modules.mjs`)
  reported uses that are none – a `$` inside a regular expression (`/^\d+$/`)
  as a use of `core.js`'s `$`, and the start of `else` as a use of
  `shared/js/dom.js`'s `el` – and failed the *desktop UI* job. It now skips
  regex contents and compares whole names.
- Desktop: `desktop/ui/js/revision.js` started with three stray lines (a
  copy of its data block, left by a merge), so the file did not parse – the
  Revision tab could not load, and the *desktop UI* check failed on `main`.
- CI: `tests/checkin_bom.rs` (#407) compared with one-element arrays built
  by `[x.clone()]`, which clippy refuses (`cloned_ref_to_slice_refs`); it
  uses `std::slice::from_ref` now. Server and desktop are clippy-clean on
  all targets.

### Added

- **Desktop: the version a server needs** (#468). When this app or the offered
  update does not fit the connected server, the update window says which
  desktop version the server needs (*Your server runs 0.3.0 and needs
  TeamCraft Desktop 0.3.x*) and whether the update is it; the notice after
  connecting to a server that does not fit names that version too, instead
  of "the matching desktop app version".
- **Desktop: *Report…* on the errors shown in place** (#471): a list or tab
  that could not be loaded, the new-item file check, *No BOM lines found* in
  the BOM import preview, the error on the 3D view (unreadable model,
  graphics memory) and the problems of *Check in all* now offer *Report…*
  like the toasts. Reports from change orders, change requests and settings
  name their key inputs (number and status, reviewer or decision, the
  setting and its new value).
- **Desktop: switch off the usage report** (#473): *Settings → Privacy →
  Send an anonymous usage report*. Off, nothing is counted, kept or sent and
  the counts not sent yet are deleted. The desktop policy `UsageStatistics`
  now fixes the switch either way (`0` off, `1` on, *Set by your
  organization*); without it each user chooses.
- **Release checklist: desktop update test** (#469): a manual script per install
  type (Windows `.msi`, `-setup.exe` and portable zip, macOS `.dmg`,
  AppImage, Flatpak, `.deb`/`.rpm`) for each release candidate – the notice,
  the update window, *Skip this version*, *Remind me later*, the download,
  checksum and policy cases; linked from `CONTRIBUTING.md`.
- **Desktop: the portable zip and the AppImage update themselves** (#466):
  *Update now* in the update window replaces the running copy in place after
  *Replace and restart* and starts the new version. The old files are moved
  aside and removed at the next start; a folder the app may not write into,
  or any failed step, changes nothing and offers the verified file instead.
- **Panning in the 3D viewer** (#459), in the browser and the desktop app:
  every drag only rotated before. Right or middle drag, Shift or Ctrl + drag,
  a drag with both buttons, two fingers on a touch screen and Shift + arrow
  keys move the model sideways; *Reset view* centres it again. The new
  **mouse setting** below the view chooses which button rotates and which
  pans (left, right or middle drag rotates; Shift + middle pans or rotates),
  kept per browser and app. Right and middle drags no longer open the
  context menu or start autoscroll on the canvas.

## [0.2.13] - 2026-11-22

### Fixed

- CI: the self-test of `scripts/ci-changes.sh` expected a desktop UI script
  (`desktop/ui/js/local-changes.js`, #416) to skip the desktop Rust jobs,
  but the table now runs them for every desktop UI script (#448); the
  `changes` job of every workflow failed on `main`.
- CI: the desktop tests of *Local changes* (#416) built one-element slices
  with `&[x.clone()]`, which clippy 1.100 refuses
  (`cloned_ref_to_slice_refs`); they use `std::slice::from_ref` now.

### Added

- **Desktop: "Support us on Ko-fi" tag** (#403). Once per start of the app the
  status bar shows a small tag; a click opens the Ko-fi page of the
  association that develops TeamCraft (`https://ko-fi.com/warr_ev`, fixed in
  the app) in the browser and hides it, *×* only hides it. Reconnecting,
  switching server or reloading the window does not show it again (held in
  memory only). Nothing is loaded from ko-fi.com and nothing is sent or
  counted before the click. *Settings → Support TeamCraft* switches it off
  (`donate_tag` in the settings file); the new desktop policy `ShowDonateTag`
  turns it off (`0`) or keeps it on (`1`) – in the ADMX with English and
  German ADML, the macOS profile and the Linux `policies.json`. Texts in
  English and German.

- **3D preview of E57 point clouds** (#373). Laser-scanner exports (`.e57`,
  ASTM E2807) open in the viewer like LAS: every scan is placed by its pose
  (rotation and translation), so multi-station scans line up; cartesian or
  spherical coordinates, float or scaled-integer values, colours scaled by
  `colorLimits`, otherwise the intensity as a colour-mapped field; points
  marked invalid, embedded images and unknown fields are left out; metres
  become millimetres. The CRC-32C of every 1 KiB page is checked first, so a
  damaged file is refused with a message instead of shown wrong. Own bounded
  reader in the isolated preview process (`src/mesh/e57.rs`, no new crate),
  fuzz target `e57`; research page *3D preview of E57 point clouds*.
- **Thinned point clouds say so.** A cloud with more points than the preview
  shows (3 million) is thinned evenly, and the line below the viewer now says
  *thinned for the preview from N points* – E57, LAS, PTS/XYZ and point-only
  OFF files (flag 8 of the `TCMESH01` mesh). Previews cached before keep
  their old line until the cache is rebuilt.
- **E57 and large scans:** E57 shares `TEAMCRAFT_PREVIEW_MAX_MESH_MB`
  (256 MB); the admin guide now says to raise it, and the preview memory limit
  to about the file size plus 100 MB, to preview larger scans.

### Fixed

- PTS/XYZ files whose point count was underestimated from their first lines
  failed with *point cloud not thinned out*; the reader now drops every second
  kept point and doubles its stride when it reaches the budget, so the kept
  points stay evenly spread over the whole file.
- **Desktop: one *Local changes* list for the whole working folder** (#416):
  a worklist section next to *New on this computer* lists every file of the
  working folder that is modified, outdated, in conflict, new (a companion
  such as a STEP export) or missing, grouped by item – also a file changed
  after clearing its read-only flag without a check-out, and files outdated
  because somebody checked in a newer version, without opening their item.
  Each row and each selection offers *Check out (keep my file)*, *Get latest*,
  *Check in selected* (the *Check in all* flow), *Merge…* for a conflict and
  *Folder*. The count joins the worklist badge on the rail. The scan
  (`local_changes_scan`, `desktop/src/localchanges.rs`) compares
  `state.json` with the disk first (no network; unchanged files are never
  read, at most 512 MiB hashed per scan) and then the Workspace status of at
  most 100 revisions (local changes first, then own check-outs, then a window
  over the rest that moves on with every scan, so outdated files anywhere in
  the folder are found over successive scans). The scan only compares: it
  sets no read-only flags and writes no state. A check-in from the list takes
  every changed file of the revision along (the check-in releases the
  check-out; the dialog names the unticked ones and offers *Park* instead).
  It runs when the worklist opens or is refreshed and when the window gets
  the focus back (at most every 15 s), and at once after a check-out,
  check-in, park, cancelled check-out or Get. The list rules are in
  `desktop/ui/js/local-changes.js`, tested by
  `scripts/desktop-local-changes.test.mjs`.
- **Check-in: *Update the BOM from the CAD file*** (#407): a box in the web
  check-in form and the desktop app's *Check in…*, ticked by default every
  time, shown when the revision has an assembly whose references are read (an
  assembly item, an assembly file, or an NX part, STEP file or SolidCraft
  design with counted instances – `bom_structure` in `file-status` for the
  desktop app; not for a plain part check-in). The plan of *Create BOM from
  CAD references…* is applied to the version being checked in, in the
  check-in's transaction: new children added, quantities the CAD file counts
  corrected (never from an *assumed 1*), circular children refused, no line
  removed – lines the CAD file no longer uses are kept and reported, as are
  referenced files TeamCraft does not have. The result says what changed
  (*BOM: 4 lines added, 1 quantity changed · …*). REST API: the multipart
  field `bom_from_references` on `POST …/checkin` and in the body of
  `POST /uploads/{id}/finish` (off by default), the answer adds `bom`;
  audit `bom.from_references` with `"source": "checkin"`. *Park* never
  changes the BOM. Parts still in the assembly but only excluded from its BOM
  there are not reported as *no longer in the CAD file*; the manual plan no
  longer holds a database connection while it reads the CAD files.

- CI on `main` is green again: the comment of the shared text-selection rule
  (#436) quoted two hex colours, which the no-literal-colours guard of
  `shared/css/` counts as colours in the stylesheet.

### Changed

- BOM from CAD references: SolidCraft designs with linked designs count as
  structure files (their placements are the quantities); the guides now name
  Inventor and SolidCraft as sources of instance counts too.
- Desktop: drag and drop of files from Explorer, Finder or the file manager onto the window (#418). On a revision tab you have checked out (or that *Add files…* would check out) the drop opens *Add files…*; elsewhere it opens *New item* with the files. A frame shows what the drop does; frozen revisions, revisions checked out by someone else, viewers and editors without a group are refused with the reason. Only regular files are taken (folders are skipped), with the same checks as the file dialog. Tab reordering keeps working (pointer events, not HTML5 drag and drop).
- **Getting started in the desktop app** (#422): after the first sign-in
  TeamCraft Desktop opens a short wizard – server (skipped when a policy fixes
  it), sign-in, the working folder (*Change…*, *Open folder*), *Get project
  groups…* for your groups (latest revision, also in work; skipped without
  groups or when they are already fetched), the CAD setup for information
  (the Inventor `.ipj`, NX load options and Solid Edge search paths a later
  version will create – nothing is created now) and the daily loop *Check out
  and open* → save in CAD → *Check in…*, ending in *Finish and open* with
  one of your groups: its files open in Home, ready for *Check out and open*
  on the assembly. Every step but the last can be skipped; *Finish*
  or *Don't show again* is kept in the settings (`onboarding`), and *Help →
  Getting Started…* opens it again. The wizard creates no file or folder by
  itself. Step rules in `desktop/ui/js/onboarding-rules.js`, tested by
  `scripts/desktop-onboarding-rules.test.mjs`.
- **Check out several parts of an assembly at once** (#415): `POST /api/v1/checkouts`
  takes a list of revisions (by id, or item and code) and checks them out in one
  transaction with a result per revision – the ones that can be taken are checked
  out, the rest are reported with a reason (`not_found`, `released`, `in_review`,
  `locked`, `forbidden`), so one released or locked part does not stop the others.
  The rows are locked in a fixed order, so overlapping batches cannot deadlock and
  every revision has one winner; each changed revision gets its own
  `revision.checkout` audit record. A server batch rather than one call per part:
  one round trip and one consistent moment for the whole set, with the rules of
  the single check-out in one place. TeamCraft Desktop: *Check out parts…* on an
  assembly (revision toolbar and *Structure* tab) shows the structure as a tree
  with check boxes, state and lock holder (parts that cannot be checked out are
  disabled with the reason), checks out the ticked ones in one step, makes their
  files in the working folder writable and lists what was refused.
- **BOM components picked by search** (#419, UX B5 of #408): *Add component*
  on the web BOM card lists matching items as you type part of their number,
  name or file name – with the preview image from the CAD file, latest
  revision, state, project group and the file name that matched; ↑/↓, Enter
  and Esc work, and without JavaScript the field still takes the exact item
  number. The desktop app's *Structure* tab gets *Add line…* with the same
  picker, an editable quantity and *Remove* on every first-level line of an
  own check-out. New routes: `GET /items/suggest` (web session) and
  `GET /api/v1/items/suggest`; both use the item search with the user's read
  rights, so items of restricted groups the user is not in never appear (at
  most 10 rows, nothing for fewer than 2 characters).
- **Request ids** (#381): every answer of the server carries an
  `x-request-id` header (a new UUID, or a reverse proxy's own id of 1–64
  characters `[A-Za-z0-9-]`), the request's log line records it as
  `request_id`, and the web UI's error pages show it under *Details for a bug
  report* and in *Copy details*.
- **Desktop: key inputs in problem reports** (#381): *Report…* on an error
  now names what the action worked on – project group, item, revision, file
  and format, the ticked files of a check-in, the options of a BOM or folder
  import – for check-out, check-in and park, *Get files*, opening files, the
  3D preview and STL export, STEP exports, new items, adding files, BOM and
  folder imports and request attachments. Names and short values only (a path
  is reduced to its file name, at most 12 values of 200 characters); a 3D
  preview that cannot be loaded from the server now also shows an error
  message with *Report…*.
- SolidCraft design properties (#367): part number, revision, description, project, author, organisation and licence are read from `.sc` designs at upload and check-in (bounded, text values only, migration `0078_file_properties`), listed under the file on the revision page and in the desktop app, returned as `properties`/`properties_line` with the file in the REST API (and `design_properties` in the MCP file briefs), and found by the full-text search. The revision page – web and desktop – shows a notice when they differ from the item number, revision code or name (`property_mismatches` of `GET /api/v1/items/{n}/revisions/{c}`) – worked out on every view, nothing is changed automatically. A new item created in the web UI with a design and no name takes the design's description; the desktop's *New item* (and *Create items…* of the worklist) and *Import a CAD folder* fill an empty description from it.
- **Thumbnails in lists** (#420): the picture of a revision's main model (the
  rule of the revision page, now in `services::thumbnails`; a stand-alone JT
  file after the native files) heads every row of the web item list and its
  search results, the vault explorer, and the desktop app's Home *Files*,
  *All items*, *Structure* and the search. The list answers carry
  `thumbnail_file_id` / `thumbnail_sha256` (`GET /items`, `/favorites`, the
  structure levels) or per file `thumbnail` / `sha256` (`GET
  /projects/{code}/files`), filled for the whole page in one query; the
  images load lazily (web `loading="lazy"`, desktop only for rows in view)
  and are cached by content hash. New: `GET /revisions/{id}/thumbnail` on
  web and REST API (404 without a picture or read access); thumbnail
  responses answer `If-None-Match` with 304 (not for assemblies composed from
  their parts' previews, whose picture changes with a component). Similar-part hits use the same
  picture rule.

- **Usage statistics: the desktop app counts every listed action** (#383).
  Besides check-out, check-in, park, get files, open in CAD, 3D preview and
  STEP exports, the daily report now also counts BOM imports (not the
  preview), folder imports that created items, new items, releases (a change
  created from *Release…*), cancelled check-outs, submitted searches and
  answered assistant questions – still only counts, never names, numbers or
  search terms. A test checks that every name of the fixed list has a place
  in the app that counts it.

## [0.2.12] - 2026-11-22

### Security

- Client address: `X-Forwarded-For` is read only from trusted proxies (`TEAMCRAFT_TRUSTED_PROXIES`, default loopback; the Docker install trusts the Docker networks while the port is published on loopback), walked from the right (GHSA-98v7-pm3r-5934, F20).
- OIDC: the ID token signature is checked against the provider's `jwks_uri` (RS/PS 256–512, ES256/ES384, HS256–512 with the client secret); `http://` issuers and provider endpoints are refused except on loopback (GHSA-864x-fmpf-4p73, F6).
- BOM and result imports: spreadsheets are read in the isolated `text-extract` child (`--format sheet`) with memory and time limits, ZIP containers capped at 256 MiB unpacked; CSV, STEP and designs on a blocking thread (GHSA-54vc-vgp2-8whr, F13).
- Windows: parser children get a memory limit through a job object (GHSA-7q9q-g6rr-pc58, F11).
- Malware scan: files larger than `TEAMCRAFT_CLAMD_MAX_MB` are refused (413, chunked uploads at the start) unless `TEAMCRAFT_CLAMD_OVERSIZE=store` (GHSA-qw27-m6gj-m6wm).
- Dependencies: `url` 2.5.9 in the server and the desktop app (RUSTSEC-2026-0592, stack overflow in `Url::origin()` on nested `blob:` URLs).

### Added

- **Empty states that name the next step** (#423, part of #408): no project
  group → *Create a project group* for CAD editors, "ask a manager of a group
  to add you" for viewers (vault, *Projects*, desktop *Home* and *Projects*);
  an empty project group → *New item* / *Import BOM* in the vault and
  *Import CAD folder…* / *New item* on desktop *Home* for those who may create
  items in it (CAD editors outside the group are asked to have a manager add
  them); *New on this computer* without suggestions → *Get…* with the active
  group ticked; the empty review, check-out and change lists of the dashboard,
  the desktop *Home* tab and the desktop worklist say how work arrives there;
  an empty working folder of a revision names the next step by check-out.
  Viewer accounts see a note on the dashboard, the desktop *Home* tab and atop
  the desktop worklist
  that editing needs the CAD editor role and that a server administrator
  grants it. The texts are in the i18n catalogs (`empty.*`, English and
  German); the desktop app loads its catalog before the first view draws.
  Tests: `every_template_renders` renders the dashboard and the vault as
  editor and viewer, `scripts/desktop-empty-rules.test.mjs` checks the
  desktop rules (`ui/js/empty-rules.js`).
- **Check-outs of a project group as a graph** (#428): administrators and
  the group's managers see who has what checked out right now – *Check-outs*
  on the group page (`/projects/{id}/checkouts`) and a lock button next to
  the group under *Projects* in the desktop app (a *Check-outs* tab). The
  graph puts the people on the left and the revisions they hold on the right,
  one line per check-out, coloured by how long it has been held (today,
  1–7 days, longer); a deactivated holder is marked. A list per person
  follows; a click opens the revision. `GET /api/v1/projects/{code}/checkouts`
  gives the same data, `GET /api/v1/me` names the groups a user manages
  (`managed_projects`). Web and desktop draw the same layout
  (`services::lock_overview::graph`, `desktop/ui/js/lock-layout.js`), styled
  once in `shared/css/components.css`.

- **UX for CAD users, first quick wins** (#408):
  - **Preview at the top of the revision page** (#412): the picture of the
    main model – the assembly before the part before a model file before the
    drawing – stands next to the title block and opens its 3D view.
  - **Revision and version explained in place** (#414): a **?** next to
    *Revision* and *Version* (web UI and desktop app) says in one sentence
    what each level is – on hover and on keyboard focus.
  - **Glossary** (#410): `docs/src/guide/glossary.md` explains items,
    revisions, versions, states, check out / get / park / check in, change
    requests and change orders in CAD terms.
  - **Desktop: right-click menu on items** (#417): items and files in the
    navigator, Home *Files* / *All items* / *Structure*, the worklist's
    check-outs and the Graph cards open a menu – *Check out and open* (or
    *Open in CAD* and *Check in…* for your own check-out), *Check out*,
    *Get…*, *Show in folder*, *Copy number*, *Where used*, *Request change…*,
    *Open in browser*. Same rules as the toolbar: a greyed-out entry says why
    in its tooltip. Shift+F10 and the context-menu key open it on the row
    that has the focus.
  - **Section nav on the revision page** (#421): a bar under the title block
    links every card of the page – Overview, Lifecycle, Revisions, Files,
    Structure, References, Where used, Changes, History and the cards only
    some items have – with counts; it stays under the top bar while you
    scroll and marks the section in view.
  - **Get TeamCraft Desktop** (#411): the dashboard and the revision page link
    the desktop app; `TEAMCRAFT_DESKTOP_DOWNLOAD_URL` points the links at
    your own download page (`off` hides them; default: the project's release
    page).

- **SolidCraft plugin: answers you can see.** SolidCraft shows the result of a
  plugin entry only for read entries, so *TeamCraft status*, *Check for
  TeamCraft* and *TeamCraft live status* are read entries again, over
  `document.inspect`; until now their answers never appeared in SolidCraft's
  dialog. *check* and *live* also say when the design has unsaved changes.
  The BOM and linked-design checks moved to *Check assembly for TeamCraft*
  (an edit entry: its findings go to the CLI, MCP and the assistant).
  Requests now have one file per revision and action, so asking twice does
  not ask twice, and *live* shows requests the desktop app has not taken yet.
  16 cases pass on SolidCraft 0.45.0.
- **Desktop: the problem-report tracker as a policy** (#378, #381). The
  managed-deployment policy `ReportUrl` (registry, macOS profile,
  `/etc/teamcraft/policies.json`; ADMX/ADML and examples updated) names the
  help desk's issue form *Report a problem* opens; *Settings → Report problems
  to* then shows *Set by your organization*. Only http(s) addresses count.
- **SolidCraft plugin ↔ desktop app** (research page *Talking to the desktop
  app from inside SolidCraft*). SolidCraft plugins have no network, so the
  plugin and the desktop app share `<working folder>/.teamcraft/solidcraft`:
  the app writes a heartbeat and the state of each revision it shows, the
  plugin's new entries read it and leave requests – *TeamCraft live status*,
  *Check out in TeamCraft*, *Check in with TeamCraft*, *Open in TeamCraft*.
  The app checks each request (a revision of its working folder, a known
  action, fresh, small, a regular file), asks before a check-out and opens its
  check-in dialog for a check-in. The app not running is never an error: the
  plugin says so and shows what the app last knew; a request waits 10 minutes.
  The old entries need no folder. 14 cases in `plugins/solidcraft/tests/` run
  on the real SolidCraft (`solidcraft-cli plugin test --require-cases`), and
  `build.sh` also makes the `.zip` SolidCraft installs.
- **SolidCraft designs show their picture** (#364). The preview image of a
  `.sc` design is the PNG SolidCraft embeds when saving with its model, read
  from the ZIP container with size and CRC checks and shown only when its key
  names the SHA-256 of this document and of the picture; unknown versions or
  damaged archives simply show none (`src/scdesign.rs`).
- **SolidCraft assemblies reference their linked designs** (#365). The
  designs an assembly links (*Insert Design*) are its file references, with
  the number of placements as instance count, so where-used, *Create BOM
  from CAD references* and the desktop's dependent files work for SolidCraft;
  read from the ZIP or JSON document, unknown fields ignored. New fuzz target
  `scdesign`. Run `teamcraft rescan-references --format solidcraft-design`
  once for designs stored before.
- **Desktop: designs open in SolidCraft** (#370). The Workspace tab marks a
  SolidCraft design that SolidCraft has open (its `.~<file>.lock`, stale
  locks ignored) with the computer and user, and the check-in dialog warns
  that unsaved changes there are not checked in.
- **Desktop: STEP export of SolidCraft designs** (#368). *STEP* in the
  Workspace tab and *Create STEP exports … first* at check-in export `.sc`
  designs through SolidCraft's `solidcraft-cli` on Windows, macOS and Linux
  (`SOLIDCRAFT_CLI`, `PATH` or the default install folders). A failed export
  – any program – no longer stops the check-in: the file is checked in
  without its STEP and the result says why.
- **Desktop: item data into SolidCraft designs** (#369). *Item data* in the
  Workspace tab writes the item number, revision and name into a checked-out
  SolidCraft design's properties (and TeamCraft's ids and server under
  `teamcraft.pdm`) through `solidcraft-cli`; refused while the design is
  read-only or open in SolidCraft.
- **TeamCraft plugin for SolidCraft** (`plugins/solidcraft`). A WebAssembly
  component for SolidCraft's plugin runtime: *TeamCraft status*, *Check for
  TeamCraft* (part numbers, BOM rows, linked designs), *Link to TeamCraft
  item* and *Unlink*; works offline on the open design. Built and tested by
  the new workflow *SolidCraft plugin*.
- **SolidCraft BOM CSV import** (part of #366). *Import BOM* recognises the
  CSV SolidCraft's bill of materials writes: `definition` is the name, and
  its counts – counted in all – become quantities per parent row (a count
  that does not divide evenly is kept and reported); `revision` and the
  amount column `quantity` are extra columns.
- **BOM import from a SolidCraft design** (#366). *Import BOM* (web and
  desktop) also takes a `.sc` assembly: its components and placements become
  the lines – part properties, placement counts, purchased/standard parts not
  expanded, phantoms passed through, reference components left out
  (`src/scbom.rs`, fuzzed by the `scdesign` target).
- **Translated UI, first part: English and German** (#379). Message
  catalogs in `shared/i18n/` (stable IDs, whole sentences with named
  placeholders, plural forms), read by the server (`src/i18n.rs`, `t()` in
  templates) and the desktop app (`shared/js/i18n.js`). The web header
  follows the browser's `Accept-Language` (exactly, then by base language,
  then English); the desktop app has *Settings → Language*. Guard tests check
  IDs, placeholders and plural forms. Translated so far: the web header and
  the desktop *Settings* dialog.
- **Usage statistics** (#383). The desktop apps send their own TeamCraft
  server one anonymous report per day of use (`POST /api/v1/usage`: a random
  installation id, versions, system, package, active minutes, counts of fixed
  actions and STEP exports – never content; *Settings → Privacy → Show what is
  sent*). *Admin → Usage statistics* shows installations and action counts
  and can share an aggregate (sizes in classes, features on/off) with the
  TeamCraft developers once a day – **off by default**, sent only to
  `TEAMCRAFT_USAGE_STATISTICS_URL`; `TEAMCRAFT_USAGE_STATISTICS`,
  `DO_NOT_TRACK` and `TEAMCRAFT_NO_TELEMETRY` fix it, `teamcraft local` and
  live demos never send. New page *Privacy: what TeamCraft sends*.
- **Desktop: report a problem from any error** (#381). Every error message
  (and *Help → Report a Problem…*) opens one review dialog, prefilled with
  the action, its key inputs, the error and the versions, optionally the
  session's recent error messages – redacted like crash reports. Nothing is
  sent without a click: *Open issue form* (GitHub or the tracker under
  *Settings → Report problems to*), *Copy report*, *Save report…*. Web error
  pages get *Copy details*.
- **Desktop: update window with verified downloads** (#377). The update
  notice opens a window with the release notes, *Update now* / *Download*,
  *Skip this version*, *Remind me later* (three days) and the release page.
  *Update now* downloads this installation's file (MSI, setup.exe, portable
  zip, dmg, AppImage) and checks it against the release's `SHA256SUMS.txt` –
  a mismatch or missing line deletes it; a verified Windows installer starts
  after a question and the app closes, a `.dmg` opens. Package installations
  get the package-manager way. *Help → Check for Updates…* checks at once.
- **Desktop: managed deployment policies** (#378). IT departments preset
  `ServerUrl` (+ `AllowOtherServers`), `CheckForUpdates`,
  `CrashReportPrompt`, `WorkspaceRoot`, `OrganizationName` and
  `UsageStatistics` per computer or user: Windows registry
  (`Software\Policies\TeamCraft`, HKLM wins; ADMX/ADML templates for Group
  Policy and Intune), a macOS configuration profile, or
  `/etc/teamcraft/policies.json` on Linux. Read once at start, never sent;
  wrong types count as not set; fixed settings show *Set by your
  organization*, and *Managed by …* appears in Settings and About. The
  templates ship with each release as
  `teamcraft-desktop-policies-<version>.zip`.
- **German Group Policy texts** (#378): `deploy/desktop-policies/windows/de-DE/TeamCraft.adml`
  shows the TeamCraft Desktop policies in German in the Group Policy editor
  (same string and presentation IDs as `en-US`, checked by
  `tests/architecture_guards.rs`); it ships in the policy zip.

### Fixed

- **Selected text is visible in every skin**, in the desktop app and on the
  web: a selection now takes the skin's primary colour with its text colour.
  It used the background of selected rows before, which in the dark skins is
  barely lighter than a text field – in the CachyOS skin, Ctrl+A in the
  server address of *Switch server…* selected the text but showed nothing.
  One rule in `shared/css/components.css` instead of one per app.
- **`url` 2.5.9 (RUSTSEC-2026-0592)**: `url` 2.5.8 overflowed the stack in
  `Url::origin()` on nested `blob:` URLs; `cargo deny` refuses it. Server and
  desktop lockfiles now take `url` 2.5.9 and the `form_urlencoded` 1.2.3 it
  needs (same dependencies otherwise).

- **Same words in the web UI and the desktop app** (#410): the desktop
  toolbar says *Edit attributes* like the web UI, the web UI's *More* menu
  says *Release…* like the desktop toolbar (was *Release via new change…*),
  and both search fields read *Search numbers, names, files …*.
- **Web check-out says that it copies no files** (#411): the notice and the
  button's tooltip explain that the browser only takes the lock, and that
  TeamCraft Desktop's *Check out and open* copies the files and opens them.
- **Desktop: the check-in icon no longer looks like the download icon**
  (#409): *Check in…* and *Check in all* show an arrow up out of the tray;
  *Get…* keeps the arrow down into it.
- **No SHA-256 column in the file lists** (#413): the hash moved into the
  tooltip of the version and *⋯ → Copy SHA-256* (web), into the tooltip of
  the version (desktop) – the space goes to file name and folder.
- **Stale tooltips and broken doc passages** (#426): the desktop update
  notice (it opens the update window, not the release page), the worklist's
  *Check in all* (it offers to check in local changes first), the roles table
  of *Concepts* (one *Viewer* row, renders as a table again) and a garbled
  sentence in the desktop guide.
- **Desktop: problem reports name the server version** (#381). *Report a
  problem* always left it out (nothing set it); the app now asks the server
  (`GET /api/v1`) once per run and per server switch.

- **CI on `main` is green again.** The usage statistics page (#383) gives
  its table headers a `scope` (accessibility check), its background and
  reporting functions are named in the audit guard's `NOT_AUDITED` list with
  their reasons, the web module list is in rustfmt's order, and the error
  page test expects the escaped `/` MiniJinja writes (#381).
- **SolidCraft plugin: *status* and *check* run again, and each button has
  its own label.** SolidCraft refused both with *permission denied: edit
  command document.metadata from read entry*, because it classes the
  commands they read with as edit commands, so they are now declared `edit`
  (they still change nothing). The four ribbon buttons, all labelled
  *TeamCraft* before, now read *TeamCraft status*, *Check for TeamCraft*,
  *Link to TeamCraft item* and *Unlink from TeamCraft*.
  Failures explain themselves: bad input names the field, damaged TeamCraft
  data in a design is reported with the fix, and *check* never reports
  *ready* when it could not read the BOM or the linked designs. All four
  entries were run on SolidCraft 0.45.0 (flatpak) through `solidcraft-cli`.
- **Desktop: a slow STEP export no longer holds the check-in hostage**
  (#342). With *Create STEP exports* ticked, the files are uploaded first
  (parked), then exported, then checked in with the STEP files. *Cancel*
  during an export skips it and ends the Inventor or Solid Edge the export
  started; the progress shows how long the export has taken. Calls Inventor
  rejects while busy are retried for a minute, and the whole error output of
  a failed export is kept in `export-logs/` (named in the message).

### Changed

- **Releases without the installers *with local server*** (paused, to make
  releases faster): `release.yml` no longer builds the desktop installers
  *with local server* (`TeamCraft-with-local-server_…`, job `local-server`
  switched off) nor the server builds only they needed – the Windows x64
  server archive (no longer a release download) and the macOS sidecar. The
  next releases have the Linux server archives and the regular desktop
  installers. To try TeamCraft, use `teamcraft local` or the live demo; how to
  switch the job on again is in CONTRIBUTING.md ("Releases"). An installed
  *TeamCraft Local* finds no matching file in these releases – its update
  window offers the release page instead.
- **Pull requests that still need work are opened as drafts** (CLAUDE.md,
  *Git rules*).

- **Desktop: checked-out files stand out in Home** (#362). A checked-out row
  in the *Files* view and the *Structure* tree gets one marker in the lock
  colour – a bar at its left edge and a light tint of the row – whoever holds
  the lock; the text colour is unchanged.

### Documentation

- **Research page *CAD add-ins: Inventor and Siemens NX first*** (#425):
  extension mechanism and runtime (Inventor 2025–2026 on .NET 8, 2027 on
  .NET 10 templates; NX Open journals vs signed startup applications and the
  author licence), events, UI, licences (vendor assemblies referenced, never
  redistributed), build and tests (a CAD-independent core with a fake host,
  FreeCAD as the reference add-in for CI), install and update (*Settings →
  CAD add-ins*, a `CadAddins` policy). Decision: an Inventor MVP, an NX MVP
  after the licence check, both talking to the desktop app over per-user
  local IPC (protocol `teamcraft.cad` v1, no token in the add-in, requests are
  untrusted intents). Nothing of it is built yet.

## [0.2.11] - 2026-11-21

### Added

- **New items from files on this computer** (#359). *New* in the desktop app
  takes group and folder from where a file is in the working folder
  (`<working folder>/GRAKSLER/Design/Struktur/Bracket.ipt` → group
  `GRAKSLER`, folder `Design/Struktur`; every file keeps its own folder, files
  picked from elsewhere are copied into the item's folder, never moved, and
  nothing in the working folder is overwritten), so the item lands on the
  server where the assemblies of everybody else expect it – before, the
  files of a new item went to the top of the group. Duplicates are refused
  before anything is created: a file that belongs to an item already
  (tracked, or added with *Add files…*), a file another item has at the same
  place, and – unless *Create anyway* – the same file name in another folder
  of the group; the same content is a warning. The new read-only endpoint
  `POST /api/v1/projects/{code}/file-check` answers per file with
  `same_place`, `same_name` and `same_content` (the current files – the
  latest revision of every item –, compared case-insensitively and in NFC; up
  to 200 files; a hidden group answers 404 like an unknown one). The upload's
  new flags `refuse_clash` and `refuse_same_name` (multipart fields of
  `POST …/revisions/{code}/files` and `…/checkin`, also covering the chunked
  sessions listed in `uploads`; JSON fields of `POST /api/v1/uploads`, kept
  with the session, migration `0076_upload_session_clash_guard`) let the
  server check the same again while it stores the files, under an advisory
  lock of the group: a file new in the revision is refused with 409 when
  another item of the group has it in that folder (the item's folder when the
  upload names none) – with `refuse_same_name` also under that name in any
  other folder –, and nothing of a refused request is stored, so of two
  people creating the same new file at once one gets the 409. New versions
  are never refused; without the flags nothing changes for other clients. The
  item is created with an idempotency key from its main file, so *Create*
  after a lost answer or a failed upload finds the item made the first time,
  and the uploaded files are recorded in `state.json`. The worklist gets a
  section **New on this computer**: the parts and assemblies in the working
  folder that belong to no item yet, one suggestion per model file with its
  drawing and exports of the same name, counted on the rail; a click opens
  *New* prefilled, *Create items…* creates the ticked ones after a summary of
  what is created and what is refused, *×* ignores a file
  (`.teamcraft/ignored.json`, *Show ignored*; the entries of files that are
  gone are dropped after a scan that read the whole working folder). The scan
  (`desktop/src/newfiles.rs`) skips tracked and added files, companions of
  tracked files, lock, backup and temporary files, `OldVersions` and hidden
  folders, visits at most 50 000 entries and reports at most 2 000 files, and
  runs when the worklist opens or the window gets the focus back (at most
  every 15 s); nothing is uploaded without the dialog or the summary. In
  *Check in…* a new part or assembly file next to a revision that has a model
  already is offered unticked, with *New item from this file*.
- **The app version above the logo** in the rail of the desktop app, always
  visible (#357) – which app runs is one look at a screenshot away.

- **Create an item in a folder.** An item now has a folder in its project
  group (migration `0075_item_folder`; empty = the top, as for every existing
  item). It is chosen when the item is created: the new *Folder* field of the
  web *New item* form (prefilled by `?folder=` and by *More → New item in this
  folder…* on a revision page), the *Folder* field of the desktop *New*
  dialog (prefilled with the folder last opened in Home → *Files*), the new
  **+** (*New item here*) on every folder row of the desktop *Files* view, and
  `folder` in `POST /api/v1/items`. The files uploaded with the item go
  there, later new files uploaded without a folder too, and the file browser
  (`GET /projects/{code}/files`, desktop *Files*) lists an item without files
  in its folder instead of at the top of the group – before, a file-less item
  could not be placed at all and every file had to be moved one by one.
  *Edit master data* and `PATCH /api/v1/items/{number}` change it (the files
  keep theirs); revisions and `file-status` return it as `item_folder`, so
  the desktop *Add files…* of a revision without files starts in it.

### Changed

- **Get project groups takes the latest revision by default** (#355): the
  desktop dialog preselects *Latest revision, also in work*; *Latest released
  revision* stays the second choice. In a team that releases only before
  manufacturing most items are in work, and *released* left the working
  folder almost empty and the assemblies without their parts.
- **CI: the macOS server build runs weekly and on *Run workflow* only** (#358),
  no longer per push to `main`. GitHub's hosted macOS runners are often
  unavailable; a job that never got a runner is cancelled after 15 minutes,
  and a cancelled job turned the whole run – and the commit on `main` – red
  although nothing was wrong with the code. The nightly Release run builds the
  server for macOS anyway, so a real macOS compile failure still shows within
  a day.
- **Database pool** (#317): `TEAMCRAFT_DB_MAX_CONNECTIONS` defaults to 20
  instead of 10 – requests and background jobs share the pool, and long
  imports made requests wait. A request that finds no free connection within
  10 seconds now answers **503** *The server is busy* (it was a 500 reported
  as a defect), logs a warning and counts in the new gauge
  `teamcraft_db_pool_timeouts`; `teamcraft_db_pool_max_connections` shows the
  limit. The operations guide has a section on sizing the pool.
- **Token lifetimes are limited by default** (#313).
  `TEAMCRAFT_TOKEN_MAX_DAYS` (new, default `365`, `0` = no limit) gives a
  new personal token without an expiry, or with a longer one, that many
  days – on the account page and through `POST /api/v1/tokens`; the account
  page no longer offers *never expires* while the limit is on. Tokens that
  renew on use (the desktop app's) keep renewing. `TEAMCRAFT_OIDC_TOKEN_IDLE_DAYS`
  now defaults to `90` instead of off: new tokens of single sign-on users
  expire after 90 days without use. Existing tokens keep their lifetime;
  `teamcraft user create-token` on the server is not limited. Set either
  variable to `0` for the old behaviour.

- **CI runs only the jobs a change needs.** One table,
  `scripts/ci-changes.sh`, maps the changed files to the jobs of the CI,
  Desktop app, CodeQL and Docs workflows (CONTRIBUTING.md, *What CI runs for
  a change*); pushes to `main` and merge-queue entries are compared with
  their base like pull requests. A desktop UI change no longer runs the
  server test suite or the six-platform desktop build, a change without Rust
  skips the CodeQL Rust analysis, `cargo deny` checks only the lock file that
  changed, and the book and API reference are rebuilt only when their
  sources changed. A documentation change now runs the new *docs and config
  guards* job (the configuration and documentation tests, no database),
  which docs-only pull requests used to skip. CI and the desktop workflow run
  everything once a week, so a file the table misses is caught within a
  week. Also removed: a stray line in the desktop workflow that ran an extra
  clippy pass and format check on every platform.
- **Check out and open reuses a running Inventor** (#350, Windows). If
  Inventor already runs – the installation chosen for the file, with the
  working folder's Inventor project (`.ipj`) active – the file opens there as
  a new document window instead of in a second instance. A new instance
  still starts when no Inventor runs, another version runs or another
  project is active; the message after opening says why. The running
  instance is asked through Inventor's COM interface by a short Windows
  PowerShell script (`desktop/src/cadlaunch.rs`).

### Fixed

- Desktop: *Settings → Connections* showed the active connection as *not
  signed in yet* when the sign-in was kept from an app version before the
  connections (#356). The account is now noted the first time the server
  confirms the stored sign-in at the start (`GET /me`; the settings are
  written only when it changed), and the list falls back to the signed-in
  account for the active connection.
- Desktop: *Check out and open* no longer opens the assembly after *Cancel*
  during the download, or when files could not be copied (#348) – it would
  open without the parts that were still to come. The check-out stays and
  the message says to get the files first. *Open* in the Files tab and *Get
  latest and open* follow the same rule for CAD files (`desktop/ui/js/get-rules.js`,
  tested by `scripts/desktop-get-rules.test.mjs`).
- Desktop: the worklist in the navigator updates at once after cancelling a
  check-out (or checking out, checking in, acknowledging a dependent) in a
  tab, instead of only after *Refresh* (#347).

## [0.2.10] - 2026-11-19

### Security

- E-mail delivery: `lettre` 0.11.24 fixes six SMTP advisories (RUSTSEC-2026-0560
  to 0565: STARTTLS reply injection, missing dot-stuffing after a bare LF/CR,
  wrong recipient for quoted local parts, panics on a blank `EHLO` line or a
  CR/LF in a display name, unbounded reply buffering). The unmaintained notice
  for `chrono` (RUSTSEC-2026-0543, not a vulnerability) is ignored in
  `deny.toml` until the move to `jiff` or `time` (`docs/OPEN_WORK.md`).
- **Daily quota of the built-in assistant** per user (#306, F16): at most
  `TEAMCRAFT_ASSISTANT_DAILY_RUNS` questions (default 200) and
  `TEAMCRAFT_ASSISTANT_DAILY_TOKENS` model tokens (default 2 000 000) in the
  last 24 hours, counted from the `assistant.run` audit entries; beyond that
  a question answers 429 with `Retry-After` before anything is sent to the
  provider. `0` turns a limit off. MCP requests cost no model tokens of the
  server and stay unlimited.
- Operations guide: **Data at rest** (#314) – TeamCraft does not encrypt the
  vault, the preview cache or the database; for sensitive data, disk
  encryption of the server is a requirement and backups that leave the
  server are written with `--encrypt`. `teamcraft backup` without
  `--encrypt` now prints a note saying so. **Checking the vault
  regularly** (#315) – downloads are not re-hashed, so a weekly
  `teamcraft vault check --verify-hashes` (systemd timer and cron examples)
  finds changed or damaged blobs. **When to run `vault gc --apply`** (#318) –
  in a quiet period right after a backup, with the default grace period.
- Single sign-on without `TEAMCRAFT_OIDC_VIEWER_GROUP` is pointed out
  (#311): every account of the identity provider can then sign in as a
  viewer. The server logs a warning at every start (`Config::warnings`) and
  *Admin → Users* says so; the behaviour itself is unchanged. The users
  guide recommends a viewer group for shared identity providers.
- Backups are owner-only (#307, F19): `teamcraft backup` sets the target
  folder to `0700` before writing into it and, once the backup is complete,
  every folder in it to `0700` and every file to `0600`, whatever the umask
  (vault copies kept the vault's own modes before). Only the account that
  ran the backup and root can read it. Windows keeps the folder's ACL.

### Added

- **Favourite folder** in TeamCraft Desktop's *Home* (#352): the ☆ of a
  folder in the *Files* view makes it the group's favourite folder, and Home
  starts there instead of at the top of the group – no clicking through
  sub-folders each time. A banner in the accent colour names the folder
  (`ROVER / Design / Rover`) and says it is not the top of the group;
  *Whole group* shows the top (with a way back), *Remove favourite* ends it.
  The folder of the group's Inventor project file (`.ipj`) is suggested
  (*Start there* / *No, thanks*). Kept per computer, server and group.
  `GET /projects/{code}/files` names those folders at the top of a group in
  the new field `project_folders`.

- Guard test for read permissions (#312): `tests/architecture_guards.rs`
  fails when a service function takes an actor and queries items, revisions
  or files without a visibility guard in its body – an `access::*` SQL
  fragment, `ensure_visible`, `ensure_editable` or a guarded getter. The
  twelve functions that do so on purpose (writes inside an already checked
  transaction, change propagation, notifications filtered by recipient,
  management counts) were reviewed and are listed with their reasons on
  `NOT_GUARDED`; none leaked. Row-level security in PostgreSQL as a second
  layer is not part of this.
- **Check out and open** in TeamCraft Desktop: one button on the revision
  toolbar takes the lock, gets the files and opens the main CAD file in the
  CAD program; the chip next to it names the program (e.g. *Inventor 2026*).
  The program is the project group's default CAD program in its version,
  else its newest installed version; without a default (or for a file of
  another program) the newest installed version of the program that fits the
  file type; else the system's default application. Installed programs are
  found by their install folders (Windows, SOLIDWORKS via the registry,
  macOS, FreeCAD on Linux). *Open* in the Files tab uses the same choice.
  *Import CAD folder* detects the folder's CAD program – Inventor with the
  release year read from the files' properties – and offers to set it as the
  group's default.
- **Default CAD program per project group** (*Project groups → the group →
  Default CAD program*, managers and administrators; `PUT
  /api/v1/projects/{code}/default-cad` with `{program, version?}`): the
  program and version, e.g. Autodesk Inventor 2026 or SolidWorks 2025, that
  *Check out and open* uses. Audited as `project.default_cad`; the group's JSON carries
  `default_cad` and `default_cad_version`; `GET /api/v1/formats` names each
  format's `family`. Migration 0074.
- **TeamCraft Desktop: several servers and accounts** (#322, #323).
  *Settings → Connections* lists the saved connections (server, account,
  working folder) with *Switch* and *Remove*; *Add another account…* signs in
  a second account on the same server; *Switch server…* and the connect
  screen change the server without signing out. The sign-in of the
  connection left behind is parked in the keychain (`api-token@<id>`, or a
  private `token-<id>` file without one) and returns on switching back.
  Every connection after the first gets its own working folder next to the
  default one (`TeamCraft-<server>`), so files of servers with the same item
  numbers never mix; existing installations keep their folder. Before this,
  changing the server signed out and threw the sign-in away.
- **TeamCraft Desktop on the live demo** (#325): the public demo account may
  now confirm the desktop sign-in in the browser (it was refused as a
  write). Visitors share that account, so their computer name is replaced by
  a random `demo-computer-…` before the confirmation page, the one-time code,
  the token name or the audit log see it. The app reads what the browser
  reads; every change stays refused, and the token goes with the nightly
  reset.
- **Items without a project group** are pointed out to administrators
  (#310): *Project groups* shows how many there are – everyone can read
  them and every CAD editor change them – with a link to *Vault → Without
  group*. New items and moves already require a group, so these are items
  from older versions. The permissions guide warns that visibility is a deny
  list and that sensitive data belongs in a restricted group from the start.
- The **live demo** is online: <https://teamcraft-demo.tailc7ef5b.ts.net>
  (sign in as `demo` / `try-teamcraft`, read-only, reset every night), linked
  from the top of the README and the documentation's start page. The runbook (*Admin guide → Live demo*) now also
  describes Tailscale Funnel as the TLS
  front for a server behind NAT or without an own domain.
- 3D preview of files saved by **Inventor 2027** (#271, on top of the
  Zstandard segments read since 0.2.8): typed arrays with the new element
  types (4, 8, 14, 16 for `u32` numbers, `invpart::is_u32`) are read, so
  parts give their meshes; in assemblies without occurrence titles the
  hidden components come from the active design view (`DesignViewSegment`), and flag `0x40`, which marks *Reference*
  components there, no longer hides them. Older files read as before.
  Checked by hand against 229 files saved by Inventor 2027 (137 of 137 parts
  with a body, 33 of 33 assemblies), see the research page *Inventor 2026
  and later: Zstandard streams*.
- Contribution rule **own work only – no decompiled code**: code
  decompiled, disassembled or rewritten from proprietary software, a "clean
  room" through decompilation, and leaked or incompatibly licensed code are not accepted,
  from people or from AI agents. Formats are decoded from sample files and
  public specifications, and each finding's source is recorded in its
  research page. The rule is in CONTRIBUTING.md (full), README and the book's
  start page (short), CODE_OF_CONDUCT.md (with enforcement), ADR 0005,
  CLAUDE.md and a new pull request template with a confirmation checkbox.
- Everything for a hosted **live demo** (runbook: *Live demo* in the admin
  guide). `TEAMCRAFT_DEMO_USER` and `TEAMCRAFT_DEMO_PASSWORD` make one local
  account public and read-only: the login page shows it, every page carries a *Live demo* banner, and every request of
  that account that is not
  `GET`/`HEAD` is refused (403, sign-out excepted) in the web UI and the REST
  API; wrong passwords for it count per client address only, so it cannot be
  locked. `scripts/seed-demo.sh` loads the demo data of the README
  screenshots (now reusable: `scripts/screenshots.sh` seeds in its own
  `seed_*` functions and takes an existing administrator through
  `ADMIN_PASSWORD_FILE`) and creates the account; `deploy/demo/demo.sh`
  snapshots the seeded database and resets it to the snapshot every night.

### Changed

- **Shared UI rules with drift guards.** The web UI and the desktop app now
  share their labels, number, size and age formats, state glyphs and the
  assistant's markdown-lite through `shared/js/` (`format.js`, `states.js`,
  `markdown.js`, `dom.js`). The desktop app no longer has its own copies, and
  its separate markdown parser is gone. The rules the server renders in Rust
  are pinned to the same test vectors in `shared/test-vectors/`
  (`src/ui_vectors.rs` and `scripts/shared-vectors.test.mjs`). New CI guards:
  a desktop module or `static/app.js` may not re-declare a shared function;
  an app stylesheet may not restyle a class that `shared/css/` owns; a new
  component styled in both app stylesheets fails `cargo test` (today's list of
  duplicates may only shrink); and shared stylesheets contain no literal
  colours. Visible in the desktop app: numbers have no thousands separators
  (`1234.5`, as on the web), and sizes read `1 KiB` instead of `1.0 KiB` and
  go up to TiB. On the web, the states `unchanged`, `modified`, `outdated`,
  `conflict`, `clash` and `missing` get the desktop's glyphs. See
  [Shared UI](https://warr-ev.github.io/TeamCraft/dev/shared-ui.html).
- The check-in button is called **Check in** (desktop check-in dialog, revision
  page and vault explorer of the web UI) instead of *Check in and release*: it
  ends the check-out, the revision stays in work – "release" read like the
  lifecycle *Release…* that freezes a revision for manufacturing. The tooltips
  say so too (*end your check-out*).
- The desktop check-in option **Create STEP exports with … first** is off by
  default (#342): the export through Inventor did not finish for a large
  assembly, and because the exports run before the upload, the whole check-in
  failed with it. Tick it for a check-in that should bring a 3D preview.
- The preview cache is limited to **10 GiB by default** (#309):
  `TEAMCRAFT_PREVIEW_CACHE_MAX_MB` defaults to `10240` instead of `0`, so the
  hourly housekeeping trims `<vault>/previews` (least recently used first)
  instead of letting it grow until the disk is full. Trimmed previews are
  made again when needed; `0` keeps the cache unlimited.
- CI: the macOS build (`build (macos-latest)`, on `main` only) is advisory
  and no longer part of **CI result**. GitHub's hosted macOS arm64 runners
  often stay unavailable ("The job was not acquired by Runner of type hosted
  even after multiple attempts"); the cancelled job had turned every commit
  on `main` red since 2026-11-18 although the code was fine.
- The repository moved from `rudalabs/TeamCraft` to **`WARR-eV/TeamCraft`**:
  every link, the install command, the documentation at
  <https://warr-ev.github.io/TeamCraft/> and the container image **`ghcr.io/warr-ev/teamcraft`** use the new names, and
  the release
  workflow publishes `:edge` and versions from the new repository again (its
  publish guard still named the old one). `deploy/install.sh` accepts
  compose files with either image name, so installations made before and the
  deploy files of release tags up to v0.2.9 keep working; an installation
  moves to the new image name with the first release after the move (or
  with `edge`) when the installer runs again. The
  desktop app's update check and crash reports point to the new repository.
  The app ID `io.github.rudalabs.teamcraft` stays, so settings and the
  keychain entry are kept.
- TeamCraft no longer presents itself as an alternative to a particular
  commercial product: the README tagline now says only that the project is a
  work in progress. The desktop import tutorial, the user guide and code
  comments describe features without comparing them to one vendor's PLM
  system. The research page is now *Popular features of commercial PLM
  systems* (`research/plm-working-features.md`), and the other research
  pages compare with "enterprise PLM" in general.
- File formats: JT (`jt`) now belongs to *Visualisation formats* and PLM XML (`plmxml`, label *PLM XML (product
  structure)*) to *Neutral exchange
  formats*. The separate CAD-program entry for them is gone, so the
  system id `teamcenter` no longer exists in `/formats`, the search filter
  or the API's format list. A saved web search with that filter shows all
  items; `GET /api/v1/items?system=teamcenter` answers 400 like any other
  unknown program. Format ids and stored files are unchanged, and both
  formats stay neutral (never collapsed). The comments of migrations 0007 and
  0012 were edited (`db::EDITED_BEFORE_RELEASE`, no schema change).
- Release workflow about 10 minutes shorter (29 → about 18): the macOS
  installer *with local server* compiled its server itself, but only after
  the slowest `server` build (Windows, 12 minutes) had finished, because the
  job waits for the `server` job. The `server` job now builds the macOS server
  in parallel with the others, as an artifact that is not released (there is
  still no macOS server download), and the installer job reuses it like on
  Linux and Windows.
- CI: test and dev builds with line tables only (`CARGO_PROFILE_*_DEBUG`),
  the CLI smoke test runs the binary the test run built instead of building
  it again with `cargo run`, and the disk clean-up of the Linux test job runs
  in the background while the toolchain and cache are set up.
- Guide *Try it on your computer*: the Windows `initdb` error of 0.2.8 names
  0.2.9 as the release that fixes it (confirmed on Windows) instead of "the
  next release".

### Fixed

- **Umlauts from a Mac broke Inventor assemblies on Windows.** macOS hands
  out file names decomposed (Unicode NFD, `a` + U+0308 for `ä`), Windows and
  its CAD programs write them composed (NFC); NTFS and ext4 see two different
  names. A CAD folder imported from a Mac was stored decomposed, the desktop
  app on Windows wrote the parts under those names, and an assembly asked for
  every part with an umlaut. Now every file and folder name is composed: on
  upload and check-in (server), in the folder import, the working folder and
  its state (desktop); CAD references stored decomposed are recognised too.
  Migration 0073 composes the names stored before (files, folders, references
  and the item names the folder import took from such file names). A Get or
  the Workspace tab gives a file the app wrote under a decomposed name its
  composed name, and the import log of an earlier Mac import still skips the
  items it created.
- Live demo, desktop sign-in (#325, #336): the confirmation page still held
  the visitor's real computer name – the appearance forms in the header
  repeat the page's address as their `back` target, `device=…` included. For
  the demo account they now return to the address with the random name.
- Trigram indexes of the item search could be missing without anyone
  noticing (#308): migrations 0023 and 0031 swallowed *every* error while
  creating them. Migration 0072 creates the six indexes again where they are
  missing; only an unavailable or forbidden `pg_trgm` extension is
  tolerated, any other error stops the migration. The server logs a warning
  at every start that names missing search indexes
  (`db::check_search_indexes`).

## [0.2.9] - 2026-11-15

Changes since the `v0.2.8` tag.

### Fixed

- **TeamCraft Local on Windows (0.2.8): "Try TeamCraft on this computer"
  failed** with `initdb: error: program "postgres" is needed by initdb but was
  not found` (*Das System kann den angegebenen Pfad nicht finden*). The app
  passed its resource folder as Tauri gives it on Windows, a verbatim path (`\\?\C:\…`), and PostgreSQL cannot use
  those. The app and `teamcraft
  local` now strip the prefix (`localserver::without_verbatim_prefix`). The
  installer smoke test passed a plain path and so missed it; on Windows it now
  passes the folder the way the app does.
- Logs written to a file or a service log carry no colour codes any more (they showed up as `[2m…[0m` in the app's error
  message): colours only
  when the output is a terminal.

## [0.2.8] - 2026-11-15

Changes since the `v0.2.7` tag.

### Added

- **Try TeamCraft on a laptop with one download.** The release adds desktop
  installers *with local server* (`TeamCraft-with-local-server_…`: Windows
  x64 setup, macOS `.dmg`, Linux x86_64 AppImage) – the app plus the server (as `teamcraft-server`) and PostgreSQL's
  programs. Their connect screen
  offers *Try TeamCraft on this computer*: the app starts a private
  PostgreSQL cluster and the server, both only on 127.0.0.1, opens the
  browser to create the administrator and signs in; the server stops with
  the app, also after a crash (`desktop/src/localserver.rs`). New command
  `teamcraft local [--port] [--data-dir] [--pg-bin]` does the same with an
  installed PostgreSQL: initdb with a random scram password on the first
  start, a free port and no Unix socket at every start, data in the per-user
  data directory, a second instance on the same data refused (`src/localserver.rs`, `tests/local_server.rs`). Release
  job
  `local-server` bundles the newest PostgreSQL 18 build of
  theseus-rs/postgresql-binaries (checksum-checked, with PostgreSQL's
  licence) and smoke-tests every installer's bundled server
  ([guide](https://warr-ev.github.io/TeamCraft/guide/local-server.html),
  `docs/src/research/local-test-server.md`).
- REST: `POST /api/v1/changes/{number}/cancel` cancels a draft or rejected
  change (its revisions are free for another change), and
  `DELETE /api/v1/changes/{number}/reviewers/{username}` removes a reviewer
  from a draft – what the web UI could do already (#291).
- 3D preview of PTC Creo assemblies (`.asm.N`), put together from the
  previews of their parts and sub-assemblies like NX, SOLIDWORKS, Inventor
  and CATIA assemblies. The direct components – model name, part or
  sub-assembly, axes and origin – are read from the section `NeuAsmSld`
  (`src/mesh/creo/asm.rs`) and found as `<name>.prt` / `<name>.asm`. The
  assembly's thumbnail is rendered from it. Decoded on public samples
  without a licence (kept out of the repository); the tests build
  synthetic assemblies around the NIST part ctc-01 (`docs/src/research/creo-assembly-preview.md`).

- 3D preview of CATIA V5 products (`.CATProduct`), put together from the
  previews of their parts and sub-products like NX, SOLIDWORKS and Inventor
  assemblies. A product stores no file names: its instances, part numbers
  and placements are read from its `CATProdCont` object stream (`src/catprod.rs`), and a component is the CATIA file
  whose document UUID
  it refers to (among the files of the product's revision and those found
  by name), otherwise `<part number>.CATPart` / `.CATProduct`. The
  product's thumbnail is rendered from it. Hidden components are not
  recognised yet.

- 3D preview of PTC Creo parts (`.prt.N`): the exact B-rep stored in the
  file (`VisibGeom`) is read – text table of contents, LZW sections, the
  self-describing tagged format with its compressed numbers – and
  tessellated: planes, cylinders, cones, tori, bicubic splines, rolling-ball
  fillets and ruled surfaces with their contours, edges sampled in both
  faces' parameter spaces, ear clipping per face region, welding and PN
  refinement along the analytic normals (`src/mesh/creo/`). The thumbnail is
  rendered from the preview (thumbnail cache version 8: every cached
  thumbnail is made again once). New fuzz target `creopart`
  (`docs/src/research/creo-brep-preview.md`).
- 3D preview of Autodesk Fusion designs (`.f3d`) from the display mesh
  Fusion stores in the file, refined along its exact normals like NX parts;
  the thumbnail is rendered from it (the PNG Fusion stored is the fallback).
- Offboarding (#173, step 9) – leavers removed only at the identity
  provider: **OpenID Connect Back-Channel Logout** at
  `/auth/oidc/backchannel-logout` – a logout token signed by the provider (checked against its `jwks_uri`, RS256 or
  ES256) ends every browser
  session of that user at once, audited as `user.idp_logout`;
  `TEAMCRAFT_OIDC_TOKEN_IDLE_DAYS` gives the new API and desktop tokens of
  SSO users an idle lifetime (off by default); *Users* lists **SSO users
  without sign-in** – active SSO users without a sign-in or token use for
  90 days who still hold memberships, tokens or check-outs. Migration 0071 (`users.idp_logout_at`). Automatic
  deprovisioning through SCIM is not
  built; see the research note *Deprovisioning through the identity
  provider*.
- Offboarding (#173, step 10), desktop app: *Check in all* and *Sign out*
  list the check-outs with local changes and offer to check them in first (sign-out also to park them); **Park…**
  uploads work in progress as new
  versions and keeps the check-out, so it survives a lost laptop; after a
  forced release the *Workspace* tab says *Released by X; your local changes
  are kept and need a new check-out*.
- 3D preview of Inventor parts (`.ipt`) from the display tessellation
  Inventor stores in the file, welded and refined along its exact surface
  normals like NX and SOLIDWORKS parts. Inventor assemblies (`.iam`) are
  put together from the previews of their part and sub-assembly files, with
  the placements stored in the assembly (re-posed flexible sub-assemblies
  included); hidden and suppressed components are left out. The
  thumbnails of Inventor parts and assemblies are rendered from the 3D
  preview. New fuzz target `invpart`.
- 3D preview of SketchUp models (`.skp`, SketchUp 3 to 2026): the faces are
  triangulated (earcut port, holes included), components and groups placed
  with their transforms, nested and scaled ones too, in millimetres; hidden
  faces, hidden components, hidden tags (layers) and images are left out. The
  thumbnail is rendered from the 3D preview; a file without faces keeps its
  stored picture. SketchUp 2021+ files are read by an own reader, older ones
  by the Rust port of OpenSKP (MIT), vendored unchanged in `vendor/openskp`.
  39 test files of OpenSKP in `tests/fixtures/cad/openskp`. Research:
  `docs/src/research/sketchup-preview.md`.
- BOM quantities from the CAD instance counts (#247): the reference scan
  stores how often an NX or SOLIDWORKS 2015+ assembly, and a STEP assembly
  with external part files, places each referenced file (`file_references.instances`, `GET …/references`; migrations
  0064 and
  0065 rescan the current files). The desktop *Import CAD Folder…* uses them
  as BOM quantities; *Quantities from CAD…* in the desktop app adds missing
  lines and corrects the quantity 1 of assemblies imported before.
- Desktop app: **crash reports** – a panic, an uncaught UI error or a
  session that ended unexpectedly is written locally (redacted); at the next
  start the user sees the report and opens a prefilled GitHub issue, copies
  or discards it. Nothing is sent on its own. Research:
  `docs/src/research/crash-reporting.md`.
- Desktop app, *Graph* tab: every BOM child shows a small **×n chip** – how
  many instances of it the assembly contains, summed over its BOM lines –
  and a drag that starts on a card pans the graph instead of opening it.
- JT 9.5 assemblies in the 3D preview: a stand-alone JT 9 file with several
  shapes (or a placed one) is put together through its scene graph like JT
  10 – instances, transforms, units, hidden nodes – instead of showing its
  parts piled up at the origin. Single-part JT 9 files and the JT inside NX
  parts read exactly as before. Tested with JT 9.5 files built around the
  real NIST hex nut (no openly licensed JT 9.5 assembly exists).
- 3D preview of stand-alone JT 10 files (`.jt`, ISO 14306): parts and whole
  assemblies from one file – the scene graph (LZMA via the pure-Rust
  `lzma-rs`, MIT) with transforms, units and hidden nodes, each shape's
  finest level of detail through the JT 9 topology coder plus the JT 10
  integer codecs, hash-checked and refined along its normals. The thumbnail
  is rendered from the 3D preview. Verified with the NIST box assembly (36
  placements equal to NX, sizes equal to NIST's STEP files). JT 9 files
  keep the reader of #200.
- Offboarding (#173, step 8): **Season handover** of a project group (`/projects/{id}/season`, managers and
  administrators): tick the members
  who leave – or let the page tick those inactive since a date or past their
  new *member until* date – and run one plan: make the new lead a manager,
  release check-outs with a reason, set *member until*, remove members and
  demote the old managers. Every step is checked and audited on its own.
  Migration 0070 (`project_members.expires_at`, a flag only).
- Offboarding (#173, step 7): **groups without an active manager** – or
  whose managers have neither signed in nor used a token for 90 days – are
  listed for administrators on *Project groups* and at
  `GET /api/v1/projects/manager-gaps`; the hourly housekeeping notifies the
  administrators once per gap (audited as `project.manager_gap_alert`).
- Offboarding (#173, step 6): a group manager can release the check-out of
  an **active** member with a reason (*Release with reason* on the revision
  page, `{"reason": …}` on `cancel-checkout`). The reason is audited and told
  to the holder; without one the release is still refused.
- Offboarding (#173, step 4): **Offboard and deactivate** on the admin user
  page, `POST /api/v1/users/{id}/offboard` and `teamcraft user deactivate`
  deactivate a leaver in one step – hand over the work and the manager roles
  to a successor, delete the API tokens (so a reactivation does not bring
  them back), release the check-outs. Deactivating the only active manager of
  a group without a successor needs a confirmation; every step is audited.
  Audit entries of the command line are recorded as `system` without a user.
- Desktop app, *Home* (#255): a **Files** view, the one Home starts with –
  the folders of the active project group and the files in them, as in its
  working folder: folders first, then the files by name as a file manager
  orders them (`Part2` before `Part10`), the file name first in every row.
  One folder is loaded at a time (`GET /api/v1/projects/{code}/files`: the
  sub-folders with their file counts and the files of the latest revision of
  every item, by name, 500 per request), so a group with more than 500 items
  is listed completely – *All items* shows the first 500 by number. Items
  without files follow the files at the top of the group. *New* and *Import
  CAD Folder…* start with the active group. With an older server Home shows *All items*.
- Desktop app (#255): a **Projects** page (rail, `Ctrl/Cmd+5`) to choose the
  active project group, create one, get groups into the workspace and import
  a CAD folder. **Home** becomes a file browser of the active group: names
  first with the number dimmed after them (long number prefixes no longer cut
  the file names off), sorted by name by default, or by number, type or
  state.
- Offboarding (#173, step 1): **what a user still holds**, in one place –
  check-outs with their age, open change orders they wrote, reviews waiting
  for them, change requests assigned to and raised by them, owned items,
  memberships (flagging groups where they are the only active manager) and
  API tokens. *Users → user → Everything … still holds*
  (`/users/{id}/holdings`), `GET /api/v1/users/{id}/holdings` and
  `teamcraft user holdings --username`. Filtered by the reader's rights, not
  the leaver's; group managers see the same report for the groups they
  manage (*holds…* in the member list).
- BOM quantities from Inventor assemblies (#247): the reference scan reads
  how often an `.iam` places each referenced file from the file descriptor
  table of its `UFRxDoc` stream (`src/invasm.rs`, checked against Inventor's
  own API on 32 real assemblies: 190 of 190 files), so the folder import, *Quantities from CAD…* and the
  BOM-from-references page get real
  quantities instead of 1. Suppressed occurrences are left out, as in
  Inventor's BOM (from the occurrence records in the same stream); a part
  with only suppressed occurrences counts 0 – it stays referenced but gets
  no BOM line. When the records do not add up, the counts include the
  suppressed ones instead of guessing. *Quantities from CAD…* reads the counts from the stored file, so it works
  for assemblies imported before; `teamcraft rescan-references --format
  inventor-assembly` also refreshes the stored counts of those files.

- Two optional rules of a project group's review policy (#152 step 9, both
  off by default): *Requesters do not decide their own change requests*, and *A reviewer confirms each "use as is"
  deviation* – a reviewer of the change
  order confirms every concession of an as-built revision on its own (`POST /api/v1/as-built-deviations/{id}/confirm`)
  before approving.
  Migration 0062.
- The Pack and Go manifest names, for every released revision, the change
  order that released it and its approvers (`released_by`), and
  administrators export the audit log as CSV or JSON (*Admin → Audit log*,
  `GET /api/v1/audit`); each export is recorded (#152 step 8).
- Sign-off of review decisions (#152 step 4): `TEAMCRAFT_APPROVAL_REAUTH`
  (`off` by default, `release`, `every_decision`) asks for the password – or
  a fresh single sign-on (`prompt=login`, *Sign in again*) – to decide in the
  browser. Every decision records how it was signed off and an optional
  meaning ("approved as design owner"). Migration 0063.
- The read log of need-to-know groups (#151, step 4) also records the read
  tools of MCP clients and the built-in assistant that name an item (channel
  `mcp` / `assistant`, the tool as detail) and the desktop app's Get (channel
  `desktop`, kind `workset`); the assistant's own taint comparisons are not
  logged. *The whole log, with filters* (`/projects/{id}/access-log`)
  filters by user, kind, channel, object and dates and pages through the log;
  `GET /api/v1/projects/{code}/access-log` takes the same filters and pages
  with `next` / `before`. Migration 0066.
- Periodic access review of need-to-know groups (#151, step 5): *Review the
  access every (days)* flags a group "access review due" (group list, group
  page, review page, evidence report) when no review was confirmed within the
  interval, and the hourly housekeeping reminds the managers once it is due
  and again after each further interval (audited). Migration 0067.
- Conditions on membership of need-to-know groups (#151, step 2): required
  identity-provider claims (from the last single sign-on; only the claims a
  condition names are kept), allowed networks (client address ranges) and a
  maximum sign-in age. A reader who does not meet them – administrators
  included – sees the group as "not found"; a refused member is audited once
  per session; a manager cannot set conditions that would hide the group from
  themselves; administrators limited by the policy may tighten but not
  loosen. The access review shows the conditions and whether each member met
  them. The assistant applies the request's conditions (and, for a token, the
  `api_tokens` switch) at every tool step. Migration 0068.
- 3D preview of CATIA V5 parts (`.CATPart`) from the visualisation (CGR)
  data CATIA stores in the file, refined along its exact surface normals
  like NX parts. Their thumbnail is rendered from it. Products are not
  previewed yet.

### Changed

- Release workflow: the installers *with local server* reuse the server
  binaries of the `server` job (Linux x86_64, Windows x64) instead of building
  them a second time (about 20 minutes on Windows), and the desktop jobs
  install a prebuilt Tauri CLI instead of compiling it (9 minutes without a
  cache).
- README: the *Download* section moved up, right below the work-in-progress
  notice, and starts with a link to the Releases page and which download to
  pick; a *Download TeamCraft* line sits under the badges.
- The first-start page (*Create the administrator*) continues where the
  login was going – e.g. the desktop app's sign-in confirmation – instead of
  always at the project groups (`next` kept through `POST /setup`).
- The conversion workers (`preview-mesh`, `text-extract`) also run isolated
  when the server binary is called `teamcraft-server`.
- Performance: a request with a personal access token (REST API, MCP, the
  desktop app) is authenticated with one query instead of three; the use of
  the token is still recorded once a minute.
- Performance: the 3D preview of an NX, SOLIDWORKS or Inventor assembly
  converts its parts two at a time instead of one after the other and places
  the instances without copying the part for each of them (less memory, the
  same mesh).
- Performance: thumbnails drawn from the 3D preview and assembly previews are
  made once however many requests ask for them at the same time, and at most
  two at a time; images already in the cache are answered without waiting.
- Notification delivery serves three recipients at the same time (each
  recipient's notifications in order, e-mail and chat side by side): a slow
  mail server no longer holds up the whole run or the chat messages.

### Fixed

- **Security – first-start setup behind a reverse proxy.** *Create the
  administrator* was offered to every request while no account existed and
  the server listened on 127.0.0.1 – also to requests through a reverse proxy
  on the same computer (the Linux packages bind 127.0.0.1:8080; nginx without
  explicit settings sends `Host: 127.0.0.1` and no `X-Forwarded-*`), so anyone
  reaching the proxy before the administrator could take the server over. The
  page now needs the one-time code of the setup link the server logs at start
  (`no administrator yet: open http://localhost:8080/login?setup=…`, new at
  every start); without it the login page says where to find the link.
  `teamcraft local` (and the desktop app *TeamCraft Local*) needs no code.
  `scripts/screenshots.sh` takes the code in `SETUP_CODE`.
- Desktop installers *with local server*: they install **TeamCraft Local**, an
  app of its own next to the desktop app (identifier
  `io.github.rudalabs.teamcraft.local`, own settings folder and keychain
  entry). The first test build shared the desktop app's identity, so it
  opened with the server and sign-in configured in the desktop app instead
  of the local server. With no server configured TeamCraft Local now starts
  its local server by itself.
- Desktop installers *with local server*: PostgreSQL is bundled with only
  the programs TeamCraft uses (`initdb`, `pg_ctl`, `postgres`) and the client
  tools `psql`, `pg_dump`, `pg_restore`, `pg_isready`. The Windows build no
  longer ships PL/Python's Python (Windows: `python.exe`, `Lib/`, `DLLs/`;
  Linux: `lib/python3.x`), PostgreSQL's test program and about 35 unused tools; a browser
  had reported the Windows installer as a virus. The release job fails if
  one of the needed programs or `pg_trgm` is missing.
- 3D viewer (web and desktop, #290): a preview that could not be shown could
  end as a silently black canvas – in the desktop app the status line with
  the reason sits below a canvas that fills the window, so it was often out of
  view. *Loading* and every error now appear on the canvas itself, and the
  viewer reports what it used to swallow: a shader program that did not link,
  a lost WebGL context (a model too large for the graphics process), an
  upload that ran out of graphics memory, and empty, cut-off or invalid (NaN) models. The mesh check is tested by
  `scripts/viewer-mesh.test.mjs`.
- Desktop: releasing with your own user name as reviewer left a draft change
  behind that could not be finished in the app (#291). The *Release…* dialog
  now refuses the own name before anything is created, says when nobody else
  in the project group can review (a single account can never release –
  four-eyes principle, no administrator override) and creates a draft
  instead, and cancels the half-made draft again when a revision or reviewer
  is refused – no orphaned drafts from failed attempts. The change tab
  disables *Submit for review* with the reason while the draft has no
  revision or reviewer (no more piles of error toasts) and has *Cancel
  change*, *Add reviewer* by user name and *Remove* per reviewer.
- Creo parts stored in inches (9 of the 16 NIST Creo parts, e.g. ftc-09)
  are shown in millimetres: the length unit is read from the section
  `UnitSystemDef` (they were 25.4 times too small). Turned Creo parts without
  a plane in the identity frame no longer fall apart: their face
  parametrisations are resolved from a seeded face (rocket body closed, nose
  from 102 to 9 open edges). Preview cache version 4, thumbnail cache
  version 9, shape signatures version 4.
- Documentation site: the search box in the tab bar searched for ever ("searching" never ended). mdBook 0.5 loads its
  search index only when its
  own search toggle opens the search, and the Doxygen theme hides that
  toggle; the theme now opens the search through it at the first focus, so
  the index loads and what was typed meanwhile is searched. A link with
  `?search=…` keeps its results open. The book's title and footer show the
  current version (0.2.7) instead of 0.1.0.
- Inventor 3D preview (#271): parts and assemblies whose segments are
  compressed with Zstandard instead of zlib (seen in newer Inventor files;
  every part failed with *damaged compressed data*) get their preview and
  rendered thumbnail again. The compression is told by its magic bytes; the
  Zstandard output is capped like the zlib output (pure-Rust `ruzstd`).
  Preview cache version 3, thumbnail cache version 7 and shape signature
  version 3: the failures cached before are converted again.
- Desktop, *Graph* tab (#254): a click on a sub-assembly now turns the tab
  into that sub-assembly – tab title, header, toolbar and graph are its own –
  instead of re-centring the graph inside the tab of the assembly above it;
  a part opens its overview the same way. *← Back* leads back one step per
  click, and a click on the assembly one came from is that same step. A click
  on the item in the middle opens its overview and no longer adds a step:
  each click used to add one, so the back control appeared although nothing
  had changed. `Ctrl`/`⌘`+click opens a node in a tab of its own.
- Desktop, *Graph* tab on macOS and Linux (#242): WebKit ignored the
  transforms of the SVG groups for the cards, so they landed away from their
  edges; the cards now sit in an HTML layer above the SVG with the same zoom
  and pan.
- Every web page starts with a *Skip to the main content* link (visible
  while it has the keyboard focus), and the 17 empty column headers (action
  columns, the corner of the inertia tensor) have a name for screen readers.
  The page accessibility test now requires both.
- An administrator revoking somebody else's API token is recorded in the
  token owner's history ("Revoked API token 'x' of lena"), no longer in the
  administrator's own (#173).
- CI: a unit test killed at its 1 s limit because the runner stalled no
  longer fails the run – failed unit tests get two more attempts (nextest
  `retries`); a hang still fails every attempt, a stall is reported as
  `FLAKY`. The 8-million-triangle limit test runs alone, and the vault tests (a disk flush per stored file) get 10 s on
  Windows.

## [0.2.7] - 2026-11-10

Changes since the `v0.2.6` tag.

### Added

- The *Material* field (*New item*, *Edit revision*, desktop app) suggests the
  materials of the library with their density while you type (a `<datalist>`,
  also without JavaScript); a name that is not in the library gets a hint.
  Free text stays allowed.
- Desktop: rearrange the object tabs by dragging (insertion marker, scrolling
  at the edge of the strip) or with `Ctrl+Shift+PageUp/PageDown`; switch tabs
  with `Ctrl+PageUp/PageDown` or `Ctrl+Tab`; arrow keys between tabs (WAI-ARIA
  tabs); right-click menu (move to start/end, close, close others, close tabs
  to the right).
- Desktop: a *Graph* tab on the revision view, a navigable graph of the
  item's neighbourhood (BOM children and references below, the assemblies
  that use it above). Colour shows the lifecycle state, size the item's own
  BOM complexity; every node carries the preview image and the file name.
  The view is kept while you move through the graph and it scales to large
  assemblies.
- Mattermost notifications as direct messages of a bot account, never into a
  channel (#230): `TEAMCRAFT_MATTERMOST_URL` and `TEAMCRAFT_MATTERMOST_BOT_TOKEN`
  (also `…_FILE`). TeamCraft looks the user up by username (cached), opens the
  direct channel and posts through the Mattermost REST API, with the full text
  also for restricted project groups (unless the group switched notification
  content off). An unknown or deactivated Mattermost user fails the chat
  delivery at once with a reason shown under *Notifications*
  (`chat_error` in `GET /api/v1/notifications`); there is no channel fallback.
  Outbox, retries and backoff are unchanged. The notification settings
  response has `mattermost_legacy_webhook`. No migration.

### Deprecated

- `TEAMCRAFT_MATTERMOST_WEBHOOK` and `TEAMCRAFT_MATTERMOST_DIRECT` for
  notifications (channel mentions leak personal notifications to everybody in
  the channel). They keep working when no bot is configured, log a warning at
  start-up and the account page says so; switch as described in
  [Upgrading](docs/src/admin/upgrade.md). `MATTERMOST_ERROR_WEBHOOK` is
  unchanged.

### Changed

- Mass properties (#227): the geometry file of a revision is picked
  automatically only when its 3D files are one model (same normalised name,
  e.g. `box.stl`, `box.prt`, `Box_rev-B.SLDPRT`), and the page, the exports
  and the MCP tool say so. Files of several different parts (an assembly
  attached as one item) are no longer reduced to the alphabetically first
  part: the revision gets the new source *geometry ambiguous*, adds nothing
  and flags the totals incomplete until a file is chosen. The likely whole
  assembly (root of the file references, or a name matching the item) is
  suggested with a one-click *Use …*; the file selector in *Mass settings*
  is grouped by model. REST, JSON, MCP (`geometry`, `root_geometry`) and the
  CSV (`geometry_file`, `geometry_choice`) carry the candidates and whether a
  file was chosen automatically or by hand. The preferred CAD program breaks
  ties between native formats.

## [0.2.6] - 2026-11-08

Changes since the `v0.2.5` tag.

### Added

- Mass properties of assemblies: total mass, centre of gravity and inertia
  tensor (about the CG and a reference point, principal moments) from the
  exported 3D files of the parts in the common assembly frame, the densities
  of an admin-managed material library or a per-revision override, and
  measured masses (with optional CG and inertia) that win over everything
  computed. Per-file length unit (mm, m, in) and number of units shown; open,
  non-manifold and inverted meshes are reported (fully inverted ones fixed).
  The mesh integrals run in the isolated preview process and are cached per
  content. Page per revision with CSV and JSON export,
  `GET /api/v1/items/{number}/revisions/{code}/mass-properties`,
  `GET /api/v1/materials` and the read-only MCP tool `get_mass_properties`.
  Migration 0061.
- Preferred CAD program (*Account → Preferred CAD program*): when a revision
  has files of the chosen program (e.g. NX), the native files of other CAD
  programs are listed in a collapsed group on the revision page, in the vault
  explorer and in the desktop app's Files tab. Neutral formats (STEP, JT, STL,
  PDF, …) always stay visible; display only. `GET /api/v1/me` returns
  `preferred_cad`, file formats carry `format_info.family`.
- 3D preview of Rhino models (`.3dm`, Rhino 1 to 8) from the meshes Rhino
  stores (mesh objects, render meshes of polysurfaces and extrusions, block
  instances placed); SubD objects are refined from their control net (Catmull-Clark with creases). Hidden objects and
  layers are left out. The
  thumbnail is rendered from the 3D preview; models saved without meshes
  show the picture Rhino stored.
- Branding (#184): administrators rename the header name and subtitle ("TeamCraft PLM") under *Admin → Branding*;
  effective at once in the web
  header, page titles, sign-in page, notification e-mails and the desktop
  title bar. `GET /api/v1/branding` returns `display_name` and `subtitle`,
  `PUT /api/v1/branding/name` sets them (admins). The footer, About, version
  and licence notices stay TeamCraft. `TEAMCRAFT_INSTANCE_NAME` is now only
  the default.
- Need-to-know project groups, step 5 (#151): access review of a restricted
  group (migration 0053, `project_access_reviews`). Page `/projects/{id}/access-review` for managers and
  administrators: members with role, group role, source (manual or identity
  provider), account state and last sign-in (stale after 90 days), the
  administrator policy and elevations, and a "reviewed" confirmation with an
  optional note that is dated and audited (`project.access_review`) and keeps
  the member list as it was. Evidence for auditors as CSV (page and
  `GET /api/v1/projects/{code}/access-review?format=csv`) or JSON, plus
  `POST` to confirm and `GET .../access-review/{id}` for the stored member list.
  The shorter session age for restricted groups (step 2) is not part of this.
- Data retention and cleanup: the hourly housekeeping now also removes the
  blobs of expired staged imports and upload sessions, trims the preview
  cache (`TEAMCRAFT_PREVIEW_CACHE_MAX_MB`, default no limit) and applies an
  optional audit log retention (`TEAMCRAFT_AUDIT_RETENTION_DAYS`, default
  keep for ever; one `audit.purge` entry per removal). New command
  `teamcraft vault gc` (dry run unless `--apply`) lists and removes vault
  blobs nothing refers to.
- Retention for large result files: a file can be set to *keep only the
  latest version* (`files.retention = 'latest'`, migration 0052; revision
  page menu, `PATCH /api/v1/files/{id}` `{"retention"}`, audit
  `file.retention`). Superseded versions are removed after
  `TEAMCRAFT_RETENTION_LATEST_GRACE_DAYS` (default 7) by the housekeeping and
  by `teamcraft vault purge-versions` (dry run unless `--apply`) – never the
  newest version and never anything of a revision that is not in work;
  audited as `file.purge_versions`. The default stays `all`.

- Item owner and hand-over (#152 step 7, migration 0050): `items.owner_id`
  (default: the creator) is shown on the item page and changed by the
  managers of the group and administrators (`item.owner`). Administrators
  hand over a leaving user's item ownerships, draft and rejected change
  orders, open change requests and pending reviews to another user in one
  transaction on *Users* → user → *Hand over work*; a message after the
  redirect gives the counts and the first objects that stayed (restricted
  group, already a reviewer, four-eyes); numbers the acting administrator
  may not read (need-to-know) are shown as "restricted item"; every object
  has its own audit entry.
- Reviewer suggestions show people who changed the affected revisions as *knows the item, cannot review*
  (`cannot_review` in the REST and MCP
  answers, desktop app, change page) instead of hiding them.
- Result values in the item search and on the dashboard: filters
  `result` (`failed`, `passed`, `above_limit`, `below_limit`) and
  `result_name` on the item list, `GET /api/v1/items` and the web filter bar,
  and a dashboard card *Simulations to check* (failed values or out-of-date
  basis). Restricted groups stay out of results and totals. No migration.
- Solver summary per file version, step 3 of `docs/src/research/solver-files.md`
  (migration 0049, nullable `file_versions.solver_summary`): for Fluent transcripts and report files, Nastran `.f06`
  and decks, STAR-CCM+ macros and batch logs, one line with solver, version, how
  the run ended, last iteration and FATAL/WARNING counts of the first and last
  64 KiB. Shown on the file row and the simulation page, in the REST file list and
  the MCP `get_revision` files (marked untrusted). The rules are unverified
  against real files.
- Review rounds and absent reviewers (#152, steps 5 and 6): withdrawing or
  reopening a change after decisions keeps them as an earlier round (`change_review_rounds`, `changes.review_round`,
  migration 0048). The change page and
  `GET /api/v1/changes/{number}` show earlier rounds and flag pending
  reviewers who cannot decide (deactivated, demoted, blind to an item). The
  author or an administrator replaces a pending reviewer while the change is in
  review (`POST /api/v1/changes/{number}/reviewers/{old}/replace`, audited
  `change.replace_reviewer`); other decisions are kept.
- Review policy per project group, step 2 of #152 (migration 0047): minimum approvals (default all), required reviewers
  and
  an optional role group (e.g. QA) whose member must approve. `submit` refuses a
  change that cannot meet the policy of every affected group; the review outcome
  uses it. Form on the group page, `GET`/`PUT /api/v1/projects/{code}/review-policy`.
  Nothing changes for groups without a policy.
  Review fixes: admins maintain a group hidden by need-to-know, a role group the
  reader cannot see is not named, only the parts of the policy the assigned
  reviewers can meet count after submit, a required person who is the author
  gets a specific message, ungrouped items count as "all reviewers".
- Scoped personal access tokens (threat model F2, #151 step 6, #152 step 4):
  a token gets a scope when it is created (account page, `POST /api/v1/tokens`,
  `teamcraft user create-token --scope`). `full` = read, write and approve (what every token had before; existing tokens
  and the desktop app's token
  stay `full`), `write` = read and write but no approving (reviewing a change
  order, deciding a change request, making a revision obsolete), `read_only` =
  GET/HEAD and the read-only MCP tools, `mcp_only` = only `POST /api/v1/mcp`.
  Refusals are 403 and write nothing; a token cannot mint a token with more
  rights than its own. The desktop app's new tokens now expire after 30 days
  without use (renewed on every use) instead of a fixed 90 days; existing
  desktop tokens keep their fixed expiry.
  Migration `0046`.
- The web *New item* form sends files above 32 MiB in chunks too (resumable, like the revision page); the
  upload sessions may now have no revision and only join the creation of an item. Without JavaScript the form still
  submits all files in one request.
- Need-to-know project groups, step 1 (#151): a restricted group can limit
  administrator access (`project_policies`, migration 0043): `with_reason`
  (time-limited, audited elevation with a written reason; managers are
  notified) or `none` (administrators manage but do not read). Web form on
  the group page and `GET`/`PUT /api/v1/projects/{code}/policy`,
  `POST`/`DELETE /api/v1/projects/{code}/elevations`. Nothing changes for
  groups without a policy. Migration 0044 keeps the elevation history when an
  account is deleted. While a policy is in force an administrator who is no
  manager member cannot loosen it, open the group, link an IdP group or add
  themselves; `/admin/audit` and webhooks honour the rule.
- Need-to-know project groups, steps 3 and 4 (#151; migration 0051): interface switches per restricted group
  (`api_tokens`, `mcp`,
  `assistant`, `webhooks`, `packages`; a switched-off interface sees the group
  as non-existent), a read and download log (`access_log`: views, file
  downloads, Pack and Go, BOM exports with channel and client address, one
  entry per object and user within five minutes) with a retention per group,
  `teamcraft access-log purge` (audited), `GET /api/v1/projects/{code}/access-log`
  and a form and table on the group page. `tests/interface_switches.rs` fails
  when an interface has no switch or no code applies it. Everything is allowed
  and nothing is logged by default.

- Wider separation of duties (#152, step 1): a reviewer must not have created
  the change, submitted it (an administrator acting for the author) or changed
  the revisions under review (revision creator, file uploader, logged edits).
  Checked when adding a reviewer, on submit and on every decision. The
  reviewer suggestions never offer such users. An administrator who prepared
  the draft for the author (items, reviewers, edits, submit; read from the
  audit log) cannot review it either. Migrations `0041`/`0042` add
  `changes.submitted_by` and `changes.four_eyes`. Upgrade: changes already in
  review keep working (only the author is refused); the editor and preparer
  checks apply to drafts and to every later submit.

- Company AI instead of shadow AI (data-loss prevention):
  `TEAMCRAFT_LOCAL_AGENTS=off` lets administrators turn off local AI agents (the desktop app's *Agent* view, ACP), so
  that AI use goes through the
  server-side assistant *Ask TeamCraft* with the provider and key the
  organisation configured (Anthropic, OpenAI/Azure, or a local
  OpenAI-compatible server), its per-question audit and its limits; the key
  never reaches a client. `GET /api/v1/me` reports
  `capabilities.local_agents`; the desktop app ends a running agent (it
  asks again when the Agent view opens, every minute and before every
  message) and refuses to start one, and its *Agent* view (`Ctrl/Cmd+4`) runs on the
  company AI automatically, with no local option (or, without a configured
  assistant, shows that AI is switched off by the administrator). When the
  server offers both, *Company AI* / *Local agent* in the Agent toolbar
  switches between them, remembered per server and user;
  `POST /api/v1/mcp` refuses every call with `403`, for every token (also
  the `teamcraft mcp` bridge, agents set up by hand and older desktop apps;
  the built-in assistant calls the tools inside the server and is not
  affected), and `capabilities.mcp` turns `false`. The default `allowed`
  changes nothing. The REST API stays open to a user's own token (scoped
  tokens, F2, are the complement): cooperative enforcement, not a barrier.
- **Admin → AI** (`/admin/ai`): the local-agent setting, the MCP endpoint's
  state and the configured assistant (provider, model, host, limits, whether
  a key is set – never the key).
- HTML reports of FEA/CFD tools (`.html`, `.htm`) are read as text for the
  search, `read_file_text` and `propose_result_values` (#141): table rows as
  lines with ` | ` between the cells, numbers and units as written, entities
  decoded; scripts, styles, comments, embedded images and hidden elements are
  skipped and nothing is fetched. An own bounded reader (`src/htmltext.rs`)
  runs in the isolated `teamcraft text-extract` child (`--format html`) with
  the limits of the PDF reader; new fuzz target `htmltext`.
- Files of ANSYS Fluent, STAR-CCM+ and Simcenter 3D are recognised more
  precisely: NX `.sim`/`.fem`/`.afm` by their `SPLMSSTR` container, text
  `.fem` as OptiStruct deck, `.dat` Nastran decks written by Simcenter 3D,
  `.f04`/`.pch`, Fluent meshes (`.msh`, `.msh.h5`), gzip-compressed and HDF5
  case/data/mesh files (checked by signature), projects (`.flprj`), `.cdat`,
  journals (`.jou`), transcripts (`.trn`) and report files (`report-file-*.out`), STAR-CCM+ solution histories (`.simh`)
  and macros (`.java` naming STAR-CCM+), Gmsh meshes. Transcripts, journals, macros and
  Nastran text output are indexed as text; a large plain-text file keeps its
  last 64 KiB in the search text and `read_file_text`.
- Extraction templates: a new method `table` for whitespace-separated tables
  with quoted column names (Fluent report files; migration 0040), and
  `pick = last` reads the last 2 MB of a file larger than 2 MB first. New
  fuzz targets `cadformat` and `resultscan`.
- 3D preview of SOLIDWORKS parts (`.sldprt`, 2015 and later) from the
  display tessellation SOLIDWORKS stores in the file, refined along its
  exact surface normals like NX parts. SOLIDWORKS assemblies are put
  together from the previews of their part files, with the placements of
  their component tree; suppressed and hidden components are left out. The
  thumbnails of SOLIDWORKS parts and assemblies are rendered from the 3D
  preview.
- 3D preview of standalone JT files (JT 9 with topologically compressed
  triangle LODs; other versions report their version), graphical revision
  comparison (*Compare with …* lays another version or revision over the
  model) and 3D notes pinned to a picked point of a file version (#200).
- Storage quotas per user and for the whole vault (a refused
  upload gets 413 and stores nothing), optional malware scanning with ClamAV (`TEAMCRAFT_CLAMD_ADDR`; infected files are
  refused and never stored),
  likely duplicates by name (`GET /api/v1/items/duplicates`, a hint after
  creating an item), bulk revise and obsolete (`POST /api/v1/items/bulk`, up to
  200 items, one transaction each), and a section plane and point-to-point
  measuring in the 3D viewer (web and desktop) (#199).
- One-click copy of item numbers, printable QR labels of a revision (the QR
  code of the short link `/i/<number>`), and a warning before removing a
  file that CAD files of other items still reference (#202).
- New item form: chunked, resumable uploads of large files, as on the
  revision page (#164).
- Create a BOM from the CAD references the server found (#180): a preview with
  instance counts as quantities (NX placed instances, SOLIDWORKS component
  tree; otherwise 1, flagged as assumed), unmatched names and hidden items
  counted, then the chosen lines in one audited transaction. Web UI and
  `GET`/`POST /api/v1/items/{n}/revisions/{code}/bom/from-references`.
- Desktop: a structure view of assemblies in the navigator, and checking out
  an assembly gets its parts – missing parts are downloaded read-only, parts
  changed locally are never overwritten and are listed (#183).
- Adding files to an existing item (#181): the Files card says how (check out
  and add, locked by someone else, in review, frozen – revise), offers *Check
  out and add files* and takes dropped files; the desktop app has *Add files…*
  (copies into the working folder, checks out first if needed).
- New simulation from an item (#178): the form is prefilled from the item and
  the item is linked as the basis in the same transaction; `POST /items`
  takes `basis`.
- Simulation results: *Fill with AI…* on the Results card with a box to say
  which numbers to extract and which limits apply; the assistant proposes
  values that a person accepts (#177).
- Offboarding (#173, minimum fix): managers release the check-outs of
  deactivated members (also in bulk), demote managers (at least one active
  manager stays), take over the change orders of a deactivated author; a
  change with a deactivated reviewer cannot be submitted; runbook *When
  someone leaves* in the admin guide.
- Obsolete and supersede through a change order (#152 step 3): a group
  setting `obsolete_requires_change`, change order lines with the action
  `obsolete` or `supersede` applied on release, and watchers are notified of
  a supersede.
- Need-to-know groups (#151): the switches `notification_content`,
  `external_llm` (with `TEAMCRAFT_ASSISTANT_INTERNAL_URLS`) and
  `desktop_sync`.
- Mutation testing with cargo-mutants (#170): configuration, a report-only
  check on pull requests and a weekly run; the gaps of the first
  whole-repository run are closed with unit tests (#207).

### Changed

- Desktop app, local agents: calls of TeamCraft's own read-only MCP tools (by the server's `tools/list` annotations) are
  allowed once without a
  permission prompt and marked *Allowed automatically (read-only)* in the
  tool list; the server still answers with the user's rights only. Write
  tools, other servers' tools, the agent's own tools and every request the
  app cannot identify for certain still ask. *Settings → Agents… → Ask
  before read-only TeamCraft tools* restores a prompt for every call.
- Desktop: *Ask TeamCraft* and *Agent* are one view. The company AI (the
  server's assistant, with page context, quick actions, the drafts checkbox
  and the conversation kept on the server) is a backend of the *Agent* view (`Ctrl/Cmd+4`, the *Agent* button of
  revision and change tabs); the *Ask*
  rail button, the drawer beside the tabs, *View → Ask TeamCraft*
  (`Ctrl/Cmd+5`) and the *Ask…* toolbar buttons are gone. The web UI keeps
  its sidebar.
- Extraction templates for `.html` files run on the extracted text instead of
  the raw markup.
- Text and CSV files keep their lines and decimal numbers in the stored text,
  so `read_file_text` shows the real text (only ASCII STL, OBJ and KiCad files
  still drop decimal numbers for the search). Files uploaded before keep their
  old text until a new version is uploaded.
- Admin → Attributes: the field "Order" is called "Position in forms" (#179).
- CI runs the tests with `cargo nextest`, and a unit test fails after 1 s
  unless it is slow on purpose (#209).

### Fixed

- The sign-in and error pages showed the logo twice in dark mode (#182).
- After a refused migration (changed checksum, dirty version) the next start
  no longer waits forever for the migration lock (#209).
- `POST /api/v1/change-requests` checks the 100-item limit before looking up
  the items (#163).
- Tests and CI on `main` after merges: audit guard exemptions, the
  `file_versions_solver_summary_object` check is validated,
  unique migration numbers with a guard test, rustdoc links and test
  fixtures (#196, #204, #208).

## [0.2.5] - 2026-11-07

Changes since the `v0.2.0` tag.

### Added

- The desktop app tells the user when its version does not fit the server (version handshake, see the support policy).
- The desktop settings dialog has the notification settings of the web account
  page (what to be told about, e-mail, Mattermost, send a test);
  `GET /api/v1/notifications/settings` also returns the form description.
- The desktop settings have *Change password…* (`POST /api/v1/me/password`;
  hidden for single-sign-on accounts, `GET /api/v1/me` has `auth_source`).
  Wrong current passwords of the own password change, in the web form and over
  REST, count like failed sign-ins (429 with `Retry-After`) and are audited as
  `user.password_failed`.
- Administrators can replace the wide logo (top left) and the square logo (bottom left, tab icon) under *Admin →
  Branding*; the desktop app shows them
  too. See [Branding](https://warr-ev.github.io/TeamCraft/admin/branding.html).
- `GET /metrics` in the Prometheus text format (request counts and latency,
  database pool, outbox backlogs, 3D preview queue), enabled by
  `TEAMCRAFT_METRICS_TOKEN`.
- As-built records: an item type `as_built` for one physical unit (or lot) with
  the design revision it was built to, serial number, as-built BOM and
  deviations from the design with a disposition; REST, MCP and web UI. Units
  are not recorded against obsolete design revisions; build records are not
  part of the Pack and Go ZIP.
- Sign-in, failed sign-in and sign-out are recorded in the audit log, with how
  the user signed in (password, SSO or first-start setup).
- A sign-in throttle slows down repeated failed sign-ins.
- 3D preview of NX parts (`.prt`) from the JT tessellation NX stores in the
  file, refined along its exact surface normals so round faces stay round.
  The thumbnail of an NX part is rendered from it instead of the small JPEG
  stored in the file.
- 3D preview and thumbnail of NX assemblies, put together from the previews
  of their part files (found by file name, sub-assemblies included);
  components on layers the assembly hides are left out, as in NX.
- Desktop app: an update notice. At most once a day the app asks GitHub for
  the latest release and shows a link to it in the status bar; it never
  downloads or installs anything. *Settings → Updates* switches it off.

### Changed

- Accessibility: an automated check runs on every rendered page; keyboard
  flows were completed and the Escape key closes panels.
- The MCP tools are split by area (`src/mcp/tools.rs`); no behaviour change.
- The desktop UI is split into ES modules per view (`desktop/ui/js/`, no build
  step) instead of one 5,000-line `app.js`; CI checks that every module's
  imports are complete (`scripts/check-desktop-modules.mjs`). No behaviour change.
- The service functions for requests are split into smaller modules; no
  behaviour change.
- The invariants of the remaining mutating service functions are documented.

### Fixed

- Desktop app: the assistant answer hung when a whole line looked like an item
  number to the folder import (two variables of the same name), and item
  numbers in answers were never links. Simulation numbers (`SIM-…`) are links
  now as well, in the web UI too.

- The reference scan records the names of STEP exports as a second name, and
  no longer reports noise for JT files.
- The duplicate-files report counts the files of a hidden revision correctly.
- Two administrators can no longer remove each other at the same time.
- The command line commands write their log output to standard error.

### Security

- Web requests that change data are refused when they carry the session cookie
  but neither an `Origin` nor a `Referer` header (cross-site request forgery).
- `Strict-Transport-Security` is sent when the public URL uses `https://`.
- Changing one's own password ends the other sessions of the user.
- The file readers for untrusted uploads (CAD, mesh, point-cloud and BOM
  table files) no longer panic on malformed input.
- The child processes that convert files start with a cleared environment.
- Desktop app: the sign-in token is kept in the OS keychain (Windows
  Credential Manager, macOS Keychain, Secret Service on Linux); the token file
  of earlier versions moves there at the next start. Without a keychain the
  app keeps the private file and shows a warning. Server addresses with plain
  `http://` to another computer get a warning.

[Unreleased]: https://github.com/WARR-eV/TeamCraft/compare/v0.2.13...HEAD
[0.2.13]: https://github.com/WARR-eV/TeamCraft/compare/v0.2.12...v0.2.13
[0.2.12]: https://github.com/WARR-eV/TeamCraft/compare/v0.2.11...v0.2.12
[0.2.11]: https://github.com/WARR-eV/TeamCraft/compare/v0.2.10...v0.2.11
[0.2.10]: https://github.com/WARR-eV/TeamCraft/compare/v0.2.9...v0.2.10
[0.2.9]: https://github.com/WARR-eV/TeamCraft/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/WARR-eV/TeamCraft/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/WARR-eV/TeamCraft/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/WARR-eV/TeamCraft/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/WARR-eV/TeamCraft/compare/v0.2.0...v0.2.5
