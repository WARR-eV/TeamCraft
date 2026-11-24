# The first start of the desktop app

Question: what should TeamCraft Desktop show a CAD user right after the
first sign-in, so that the next click is real work and not reading – and
what must such a first-start wizard never do? Issues #422 (*Getting
started*) and #478 (CAD project setup). Date: 2026-11-24.

The first version of the wizard (#422) walked through six steps: server,
sign-in, working folder, project group with a whole-group *Get*, a *CAD
setup* page that listed files a later version would create, and a text page
on the daily loop. A follow-up (#478) tried to make the CAD page create
those files. This page collects the generic guidance behind the redesign.
It cites public design guidelines only; no product is taken as a model.

## Findings

| Pattern | What the guidelines say | Consequence for the wizard |
|---|---|---|
| One wizard, one task | A wizard is for a task the user does rarely and needs guidance for; pages that need no decision (welcome pages, pages that only inform) do not belong in it; the commit button names what it commits. First-use tasks are better presented in context than all at once. | One job: get the user to a real assembly with *Check out and open* in front of them. No welcome page, no server or sign-in page (the connect screen did both), no text-only page. The commit button says *Show* plus the item number. |
| Pull, not push | Up-front tutorials are skipped or forgotten; help shown at the moment of need – when the user is about to do the thing – is read and remembered. | The daily loop (check out → save in CAD → check in) is a one-time tip under *Check out and open* in the revision tab, not a wizard page. |
| Detect before asking; show only relevant steps | Ask only what the app cannot find out itself; hide steps that the current state already settles; start each step with a verb or the outcome. | The wizard reads what the app already knows: the working folder, the project groups, the CAD programs found on this computer. A step whose answer is settled (one group, a folder already in use or set by the organization) is skipped and shown as a line in the header. |
| Never touch user-owned data without a specific yes | Changes to the user's files or to machine-wide settings need an explicit, specific confirmation; silent changes are for administrators' deployment tools. | The wizard writes no file and no folder. CAD project files and search paths belong to the group and to each CAD program's own settings; managed computers configure CAD programs with their own deployment tools. |
| Hand over into real work | The end of onboarding is the first real task, with one concrete action, not a summary page. Empty states name the next step. | The wizard ends in the real revision tab of a real assembly of the group, with *Check out and open* as the highlighted next step (#536). Users without a group end where the role-aware empty states (#423) take over. |
| Let it be reopened and dismissed | Onboarding must be skippable at every point and findable later from Help; it must not come back for people who already use the product. | *Don't show again* on every step, *Help → Getting Started…* reopens it. It opens by itself only after a sign-in in this run of the app, never for a user who already has check-outs or a stored file version (a check-in or an upload) on that server, and it is remembered per saved connection: a second server or account offers it once more. |

## Decision

* Three read-only steps: **Where your files go** (working folder, CAD
  programs found on this computer), **Your project group** (only with
  several groups, or none), **Your first assembly** (up to five assemblies of
  the group, each with *Show*). The pure rules are in
  `desktop/ui/js/onboarding-rules.js`; the texts come from the shared
  message catalogs (#379).
* The wizard writes only the app's own settings: `done` or `dismissed` on the
  active connection (`desktop/src/profiles.rs`), the working folder when the
  user picks one with *Change…*, and the active project group on the final
  click. No *Get*, no check-out, no server write.
* Nothing in the wizard depends on one CAD program. How a project group sets
  up its CAD programs is an administrator page
  ([Set up CAD programs for a project group](../admin/cad-setup.md)), linked
  from step 3.
* Follow-ups, not in the wizard: a self-ticking *Getting started* card on
  Home for users who closed the wizard early, the size of a whole-group *Get*
  before it starts, a read-only view of a group's CAD setup for managers.

## Sources

* Microsoft, Windows app design guidelines (Win32 UX guide):
  [Wizards](https://learn.microsoft.com/en-us/windows/win32/uxguide/win-wizards),
  [First experience](https://learn.microsoft.com/en-us/windows/win32/uxguide/exper-first-exper)
* Nielsen Norman Group:
  [Onboarding Tutorials vs. Contextual Help](https://www.nngroup.com/articles/onboarding-tutorials/)
* Apple Human Interface Guidelines:
  [Onboarding](https://developer.apple.com/design/human-interface-guidelines/onboarding)
* Visual Studio Code extension UX guidelines:
  [Walkthroughs](https://code.visualstudio.com/api/ux-guidelines/walkthroughs)
  (steps shown only when relevant, titles that start with a verb)
