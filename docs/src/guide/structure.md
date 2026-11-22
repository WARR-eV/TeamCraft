# BOM, references and change propagation

## Bill of materials

A revision's BOM lists child items with find numbers, quantities and notes.
The BOM page expands the structure over all levels with mass and cost
roll-up and three **revision rules**:

| Rule            | Resolves each child to                                                            |
|-----------------|-----------------------------------------------------------------------------------|
| Latest          | its newest revision, whatever the state                                           |
| Latest released | its newest released revision                                                      |
| As released     | the exact revision the parent was released with (e.g. the as-built configuration) |

The roll-up adds the entered masses. The centre of gravity and the inertia
tensor of the structure, computed from the 3D files of the parts, are on the **Mass properties** page –
see [Mass properties](mass-properties.md).

### Working with a large structure

For a big product (hundreds or thousands of lines across several project
groups) the BOM page offers:

* **Fold and unfold** – every line that has lines below it has a ▾ / ▸ button (keyboard operable, `aria-expanded`);
  *Collapse all* and *Expand all* sit
  above the table. This needs JavaScript; without it every line stays
  visible.
* **Levels** – *Levels: 1, 2, 3, 4, 5, All* (`?depth=n`) lists only the first
  n levels, also without JavaScript, and the link can be shared. A line at the
  last level shown has a `+n` marker with the number of lines hidden below it.
  Mass and cost always cover the whole structure, not just the levels shown.
  It combines with the revision rule (`?rule=`).
* **Group column** – the project group of each line, linked to the group page.
  A line you may not see (a restricted group) stays a placeholder and shows no
  group.
* **Group filter** – *Group* (`?group=CODE`, only groups that occur in the
  structure are offered) keeps the lines of that group (highlighted) and
  the path from the top to them, so they stay in context; other lines
  below a match are left out unless they lead to a match too. With a level
  limit as well, the filter applies first. An unknown or hidden code gives
  "not found".

The CSV and Excel exports and Pack and Go ignore levels and group: they
always contain every line. The desktop app's structure tab does not have
these controls yet.

Where-used shows the parents of an item; *All levels* on the revision page
opens the where-used page, which walks up through the sub-assemblies (1, 2,
3, 5 or all levels). Cycles are rejected. The BOM
exports as CSV and as Excel workbook (which the BOM import reads back), and **Pack and Go** downloads a revision with
the files of its whole structure
as ZIP (`<item>_<revision>/`, `bom.csv`, `manifest.json` with SHA-256 per
file). For every released revision the manifest also names the change order
that released it and who approved it when (`released_by`: change, title,
release time, approvals); a change order the downloader may not see is only
marked `restricted`.

## Creating a BOM from the CAD references

An assembly uploaded as a file has no BOM, although the server already knows
which part files it uses (the *References* tab). **Create BOM from CAD
references…** on the BOM card of the revision (also offered as a hint while
the BOM is empty) shows what the BOM would be, one row per child:

* item, name, **quantity** and where it comes from – *CAD instances* (NX:
  the placed instances of the component, hidden layers left out; SOLIDWORKS:
  the instances in the component tree without *exclude from BOM*; Inventor:
  the occurrences of each part, suppressed ones left out; SolidCraft: the
  placements of each linked design; STEP
  assemblies with external part files: the assembly occurrences of each
  file) or *assumed 1* (the file does not tell; the line gets the note *quantity not
  known from the CAD file – check it*);
* the CAD file that references it;
* a status: *new line* (ticked), *already in the BOM*, *differs* (BOM 2, CAD
  4 – changed only when you tick it and *Also change the quantity*), or *circular* (refused, as when adding a line by
  hand).

Only assembly files count (NX files and SolidCraft designs only when they
have component instances, STEP files only when their occurrences lead to external part
files); a drawing's link to its model stays a *drawing of* reference.
Referenced files TeamCraft does not have are listed as *not in TeamCraft* and
never created; parts in project groups you cannot see are only counted. *Add the ticked lines* needs the revision
checked out by you (the page
offers *Check out and create*) and adds all lines in one transaction. The
REST API has the same two steps: `GET …/bom/from-references` returns the
plan, `POST …/bom/from-references` with
`{"children": ["PRT-000004"], "update_quantities": false}` applies it.

### At check-in: *Update the BOM from the CAD file*

The check-in form of the web UI and the desktop app's *Check in…* have a
box **Update the BOM from the CAD file**, ticked by default every time
(#407). It is shown when the revision has an assembly whose references are
read: an assembly item, a file of an assembly format, or an NX part, STEP
file or SolidCraft design whose stored references carry instance counts (in
the web UI also a revision whose CAD files lead to other items; in the
desktop app also a file of an assembly format or a SolidCraft `.sc` /
`.solidcraft` design in the working folder, a part or an assembly by
content). Any other plain part check-in shows no box. With the box ticked, the plan above is applied
to the version being checked in, **in the check-in's transaction** – the
files and the BOM change together, or neither (a refused upload, 409, leaves
the BOM as it was):

* children with the status *new* are added;
* a quantity that *differs* is corrected only where the CAD file counts it
  (*CAD instances*); an *assumed 1* never replaces an existing quantity;
* *circular* children are refused, as above;
* lines whose part the CAD files no longer reference are **kept** (a BOM
  carries lines without CAD geometry: glue, cable ties, bought parts) and
  reported as *no longer in the CAD file*; a part still in the assembly but
  only excluded from its BOM there (SOLIDWORKS *exclude from BOM*, suppressed
  Inventor occurrences) is kept without that report;
* referenced files TeamCraft does not have are reported as *not in
  TeamCraft*, never created.

The check-in message says what changed – *BOM: 4 lines added, 1 quantity
changed · not in TeamCraft: Lager.ipt · no longer in the CAD file:
PRT-000012 (kept)* – and says nothing when nothing changed. The permission
is that of the check-in (the holder of the check-out); the audit log gets
`bom.add` / `bom.update` per line and one `bom.from_references` with
`"source": "checkin"`. The counts are the ones the reference scan stored
with the new version. Unticking the box keeps the BOM as it is for that one
check-in; a park (upload without check-in) never changes it. On the REST API
the field `bom_from_references` is off by default, so existing clients
behave as before.

## Comparing BOMs

*Compare…* on the BOM page lists the lines that were **added**, **removed**
or **changed** (quantity, find number, notes) between two revisions – by
default the previous revision of the same item; any other revision can be
chosen, and the API also compares by code (`GET …/revisions/B/bom/compare?with=A`).

*Levels: All levels* compares the whole structure: every child is resolved
with the chosen revision rule, the children of two matched lines are
compared the same way, and an added or removed sub-assembly brings its
whole sub-tree. A child that now resolves to another revision counts as
changed. When one of the two revisions is released, the default rule is *As released*: a released parent uses the child
revisions it was released
with, an in-work one the newest released children, so comparing two
revisions shows exactly which parts went into the new one; otherwise the
default is *Latest*. *Latest* and *Latest released* resolve a child by its
item alone – both sides then get the same child revision and sub-tree, and
only the direct lines can differ. In the API: `depth=all` and
`rule=latest|released|as_released` (same default). The Excel
download contains the level and the revisions too.

## Alternates, supersession and replacing a part

The card *Alternates and replacement* on the revision page holds two
relations of the item itself (not of one revision – they stay true for old
assemblies too). Changing them needs edit rights in the item's project group.

- **Alternates** are items that may be built in instead of this one in every
  assembly – same form, fit and function, e.g. a second source of a purchased
  part. *Two-way* (the default) also makes this item an alternate of the other
  one; that needs edit rights in the other item's group too, otherwise add it
  one-way. The other item then lists it under *may be replaced by*.
- **Supersession** says: do not use this item for new designs any more, use
  its successor. An item has at most one successor, and a chain of successors
  never leads back to the item. The revision page of a superseded item shows a
  notice, BOM lines that use a superseded item are marked *Superseded by …*,
  and adding one to a BOM still works but says so.
  Superseding changes no BOM and no lifecycle state; making the old revisions
  obsolete stays a change order's job.

**Replace in assemblies** on the where-used page puts another item (by
default the successor, otherwise an alternate) in place of this one. The
preview lists every assembly whose *latest* revision uses the item directly
and what will happen to it:

| Latest revision of the assembly                | Result                                                       |
|------------------------------------------------|--------------------------------------------------------------|
| in work, not checked out or checked out by you | the line is replaced (quantity, find number and notes stay)  |
| in work, checked out by somebody else          | left alone – ask them, or replace it there yourself later    |
| in work, in a group you cannot edit            | left alone                                                   |
| in review                                      | left alone until the review is finished or rejected          |
| released                                       | needs a new revision – revise it (usually in a change order) |
| would contain itself through the replacement   | left alone (circular BOM)                                    |

A revision that was not checked out is changed directly, like check out,
edit, check in; one checked out by you stays checked out. Older revisions keep
their BOM. Every replaced line is recorded in the history of the assembly (`bom.replace`). Assemblies in groups you
cannot see are only counted.

## References

Besides the BOM, revisions reference other items: *drawing of*, *specification for*, *references*. CAD files are scanned
for the files they
reference (assemblies, drawings, STEP, KiCad; ASCII and UTF-16 paths as
written by SolidWorks, Inventor, Solid Edge, CATIA, …; NX parts,
SOLIDWORKS 2015+ files and SolidCraft assemblies (their linked designs)
through their structure, see
[Formats](../reference/formats.md)) and matched against
the vault: referenced files that are *not in TeamCraft* are flagged and the
missing BOM lines are suggested. The other direction warns before damage:
removing a file that CAD files of other items still reference names them (up to three, then a count) in the
confirmation, so a part still used in an
assembly or drawing is not deleted by accident. Deleting an item is refused
while it is used in a BOM.

## Change propagation

When a change releases a new revision (or a revision becomes obsolete), every
dependent – via BOM, references, CAD file references or as the basis of a
[simulation](simulation.md) – is flagged **Check needed**. A CAD editor acknowledges it with
a note (*form, fit and
function unchanged*) or revises and releases the dependent, which resolves
the check and propagates one level further. Every change shows this **impact analysis** before it is approved.

*Dependents to check* on the dashboard (and in the desktop app) groups the
open checks by the dependent revision: one header per assembly, drawing or
simulation (*ASM-000002/A Electronics enclosure – 4 changes to check*) with **Open**, **Revise** and **Acknowledge
all…**, and one compact row per source
below it – the item that changed, *released* or *obsolete* via which change,
and the link (BOM line, CAD file reference, drawing, specification, reference,
simulation basis) – each with its own **Acknowledge…**. **Acknowledge all…**
resolves exactly the checks shown with one note, in one step; checks resolved
by someone else in the meantime are skipped, and every acknowledged check is
recorded in the item's history. The revision page shows its open checks the
same way.
