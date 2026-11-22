# Glossary

The words TeamCraft uses, in CAD terms. The web UI and the desktop app use
the same word for the same action (#410).

## Things

| Word | Meaning |
|------|---------|
| **Item** | A part, assembly, document, software component, simulation or as-built record with a unique number (`PRT-000042`). Its CAD files, attributes and BOM belong to its revisions. |
| **Revision** (A, B …) | A state of the item that gets released. Released revisions never change – *Revise* starts the next one. The web UI and the desktop app explain it with the **?** next to *Revision*. |
| **Version** (v1, v2 …) | Every *Check in* or *Park* stores a new version of a file – the history inside a revision. Shown in the *Version* column of the files. |
| **Project group** | A product, machine, customer project or vehicle; every item belongs to one. Each group has its own folder in the working folder. |
| **Working folder** | The folder on your computer where TeamCraft Desktop keeps one copy of each file, in the same folders as the original CAD tree (`Documents/TeamCraft/<group>/…`). Files you have not checked out are read-only. |
| **BOM** | Bill of materials: the lines of an assembly revision (component, quantity, find number). |
| **References** | Files a CAD file points to (the parts of an assembly, the model of a drawing), found by reading the files. |
| **Where used** | The assemblies whose BOM or CAD files use an item. |

## States

| State | Meaning |
|-------|---------|
| **WIP** (in work) | Being designed; only this state changes, and only while checked out. |
| **In Review** | Part of a submitted change order; waits for the reviewers. |
| **Released** | Approved; never changes again. |
| **Obsolete** | Not to be used any more (final). |

## Actions

| Action | What it does |
|--------|--------------|
| **Check out** | Takes the edit lock of a revision. In the desktop app it also copies the files into your working folder and makes them writable; in the browser it only takes the lock – no files are copied. |
| **Check out and open** | Desktop app: check out, then open the main CAD file in the CAD program chosen for the project group. |
| **Get** | Copies files into the working folder without a lock (read-only) – an assembly with everything it consists of. |
| **Check in** | Uploads the changed files as new versions and ends your check-out; the revision stays in work. |
| **Park** | Uploads the changed files as new versions but keeps your check-out. |
| **Cancel check-out** | Ends your check-out without uploading anything. |
| **Edit attributes** | Changes name, description, material, mass and the other attributes of a revision you have checked out. |
| **Release…** | Creates a change order for the revision, with reviewers, and submits it. Released when every reviewer approved – never by the author alone. |
| **Revise** | Starts the next revision (B after A) from a released one. |
| **Pack and Go** | One ZIP with a revision and every file of its structure. |

## Changes

| Word | Meaning |
|------|---------|
| **Change request** | "Something is wrong or could be better" – anybody can raise one, also viewers. It is triaged and may lead to a change order. |
| **Change order** | The revisions to release (or make obsolete) together, with the reviewers who approve them. *Release…* creates one. |
| **Dependents to check** | Assemblies and documents that use something which was released in a new revision or made obsolete – acknowledge what needs no change, or revise them. |

See [Concepts](concepts.md) for the rules behind these words and
[Desktop app](desktop.md) for the daily work with CAD files.
