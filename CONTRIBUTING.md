# Contributing to TeamCraft

Thank you for your interest in TeamCraft! TeamCraft is an open-source product
lifecycle management (PLM) system written in pure Rust. It is a **work in
progress** with **no stable release yet**, so it helps a lot when people
try it out and say what works and what doesn't.

How to build, run and test TeamCraft from source is in the README under
[Development](README.md#development).

## Commits and pull requests

1. Fork the repository, create a branch from `main` and open the pull request
   against `main`. Describe what changed and how you tested it, and write
   `Closes #N` for an issue it finishes. The pull request template asks you
   to confirm that the contribution is your own work.
2. **Every pull request is squash-merged:** it lands on `main` as one clean
   commit with the pull request's title, so the history of your branch does
   not matter – commit as often as you like.
3. **Any size is fine:** a pull request can be a one-line fix, or a set of
   features and fixes that work together or depend on one another.
4. **CI does not have to be green for a merge.** A failing check can be fixed
   later in its own commit. Before a release, though, CI on `main` must pass,
   so many red commits delay the next release – if you want your change in
   the next release quickly, make CI green.
   Releases follow the
   [release checklist](docs/src/dev/release-checklist.md), including the
   manual [desktop update test](docs/src/dev/release-checklist.md#desktop-update-test)
   per install type for each release candidate.
5. **Security is the strictest part.** Pull requests from people outside the
   maintainers get a strict security and vulnerability review. A pull
   request that tries to sneak a vulnerability into TeamCraft is closed
   without discussion.

**Never commit secrets:** no passwords, tokens, private keys, `.env` files,
database dumps or vault data. If it happens anyway, tell the maintainers
right away so the secret can be rotated. Deleting it in a later commit is
not enough, because it stays in the history.

## Your own work only – no decompiled code

Every contribution – code, tests, fixtures, documentation – must be the
contributor's own work, written from their own skills and knowledge, or come
from a source whose licence allows it (see above) and be credited as such.
This applies in exactly the same way to code that an **AI agent** writes for
you: it has to come from the agent's general knowledge and from material it
may use, never from someone else's program code.

**Not allowed, in no form and by nobody – neither people nor AI agents:**

* **Decompiling, disassembling or otherwise extracting the program code of
  proprietary software** – for example a commercial PLM or PDM system, a CAD
  program, its viewers, SDKs, plug-ins, DLLs, .NET assemblies, Java archives
  or firmware – and contributing anything that comes out of it: decompiled or
  disassembled code, pseudocode, code translated into Rust, "cleaned up",
  renamed, restructured or otherwise rewritten from it (by hand or by an AI
  agent), and tables, constants or algorithms lifted from it. Rewriting code
  that was decompiled does not make it your own work; even slightly changed
  copies can infringe the original's copyright.
* **Handing decompiled output to an AI agent** to "rewrite" or "re-implement"
  it, asking an agent to decompile a program for this repository, or pointing
  reverse-engineering tool kits (decompilers, disassemblers, agent tools built
  on them) at proprietary programs for it.
* **"Clean room" by decompilation.** TeamCraft does not accept the two-team
  method either, where one person or agent decompiles a program and writes a
  specification and another writes the code from it: the specification still
  comes from the program's code, and with AI agents nobody can show that the
  separation held.
* **Leaked or confidential source code and documents** – vendor source code,
  SDK headers, samples or manuals under a licence or NDA that does not allow
  this use – and code copied, ported or translated from other projects whose
  licence is not compatible with the MPL-2.0. If you have seen a vendor's
  source code (for example as an employee) or are bound by an NDA about a
  program, do not work on reading that program's files.

**Why.** Most end-user licence agreements forbid decompiling the program,
and copyright protects the program code itself. Where a law permits
decompilation at all – narrowly, for example to achieve interoperability –
it does not permit publishing the result or using it to write a similar
program. A single such contribution could expose TeamCraft and everyone who
distributes it to copyright and licence claims, and would force the project
to remove the feature and rewrite the Git history.

**What is allowed – this is how TeamCraft's readers of native formats are
built:**

* studying **files**: sample files that may be used (the openly licensed NIST
  models in `tests/fixtures/cad`, files you made yourself with a program you
  are licensed for) and what a program writes into them – a file format is
  analysed from its data, never from the program's code;
* **public specifications and standards** (STEP / ISO 10303, JT / ISO 14306,
  published format documentation) and openly licensed code under a compatible
  licence, credited in the README's licence table (as `vendor/openskp`);
* comparing the **documented behaviour or output** of a program you use
  legally – for example an export of the same model – without looking at its
  code.

Write down how a format was decoded in its research page in
`docs/src/research/` – for each finding its source (which sample file, which
section of which specification, which experiment) and whether it is
confirmed or still a guess – so anyone can check the clean origin instead of
taking it on faith.

**AI agents** working on this repository follow the same rules: write the
code from general knowledge, the repository's own code and documentation,
the sample files and public specifications; refuse to decompile or
disassemble proprietary programs or to use their output; and say so when a
request would need it. If an agent produces something that looks copied –
long verbatim passages, another code base's identifiers or comments, vendor
names in internal symbols – do not submit it.

**When in doubt, ask** in the issue before you write the code. A contribution
that turns out to contain decompiled, leaked or incompatibly licensed code is
removed – with a rewrite of the history if needed – and its author may be
excluded from the project (see the [Code of Conduct](CODE_OF_CONDUCT.md)).
Report a suspected case to the maintainers in an issue, or privately as
described in [SECURITY.md](SECURITY.md).

## Ways to contribute

* **Try it and report what you find.** Follow the quick start in the
  [README](README.md) or the guide for [Linux](platforms/linux.md),
  [macOS](platforms/macos.md) or [Windows](platforms/windows.md), then
  [open an issue](https://github.com/WARR-eV/TeamCraft/issues) for each bug,
  confusing behaviour or gap in the documentation you run into.
* **Test with real CAD data.** Several parts were built from public format
  descriptions without the CAD programs at hand, and they need real files:
  the 3D preview of STEP exports, the scan for references inside CAD files,
  the BOM table import and the embedded preview images of Inventor, Solid
  Edge and SolidWorks files. Tell us the program, its version and what
  happened. Attach a file only if you are allowed to share it.
* **Improve the documentation**, either the book in [`docs/`](docs) (mdBook)
  or the build guides in [`platforms/`](platforms).
* **Write code.** Pick an entry from [docs/OPEN_WORK.md](docs/OPEN_WORK.md) or
  an open issue – preferably from its sections *Proofing* and *Structure*: tests,
  fuzzing and refactoring come before new features for now (see *Direction*
  there). For anything bigger than a bug fix, open an issue first and
  describe your plan. That way we can agree on the approach before you put
  time into it.

Security problems are the exception: **do not open a public issue**. Follow
[SECURITY.md](SECURITY.md) instead.

## Reporting bugs

A good bug report contains:

* the TeamCraft version and commit (`teamcraft --version`,
  `git rev-parse --short HEAD`);
* the operating system, the PostgreSQL version, and the browser or desktop app
  you used;
* what you did, what you expected and what happened instead;
* the relevant lines of the server log, or a screenshot.

**Remove secrets before you post.** That means passwords, API tokens (`tc_pat_…`), session cookies (`tc_sess_…`), OIDC
client secrets, webhook URLs
and private keys.

## Updating the README screenshots

The screenshots in `media/screenshots/` are made by
[`scripts/screenshots.sh`](scripts/screenshots.sh) from the same demo data,
never by hand:

1. Run the **Screenshots** workflow (*Actions → Screenshots → Run workflow*);
   a pull request that changes the script or the workflow runs it too.
2. Download its `screenshots` artifact, look at every image and copy the ones
   the README uses into `media/screenshots/`. The workflow never commits
   anything itself.
3. Commit the images in the same pull request as the README lines that show
   them.

A new page for the README is a `shot` line in the script's `capture` function
plus an image in the README with alt text that says what it shows. New demo
data goes into a `seed_*` function of the script, which
`scripts/seed-demo.sh` shares with the [live demo](docs/src/admin/live-demo.md).

## Code conventions

[Architecture](docs/src/reference/architecture.md) gives an overview of the
code base. The rules that matter most – guard tests catch most of the others
and say what to change
([what they check](docs/src/dev/ci.md#what-the-guard-tests-check)):

* **All writes go through `src/services/`.** A service function checks the
  permissions, locks the rows it changes (`SELECT … FOR UPDATE`), validates
  lifecycle transitions and writes the audit entry (`audit::record`) – all in
  one transaction. The web (`src/web/`), API (`src/api/`) and MCP (`src/mcp/`)
  handlers stay thin and call the same service functions.
* **Reads take the actor.** [Restricted project groups](docs/src/admin/permissions.md)
  hide their data from non-members, so every read service takes `&Actor` and
  answers "not found" for hidden objects; the SQL guards come from
  `services::access`.
* **SQL** uses runtime queries (`sqlx::query_as::<_, T>` with `FromRow`), not
  the `query!` macros. Schema changes go into a new numbered file in
  `migrations/`; never edit a migration that is already on `main`.
* **Web UI:** forms follow POST → redirect → GET; new templates are registered
  in `TEMPLATES` (`src/templates.rs`) and the `every_template_renders` test;
  no inline JavaScript or styles (the Content Security Policy blocks them);
  colours only as tokens from `shared/css/skins.css`. What both the web UI and
  the desktop app use lives once in `shared/`
  ([Shared UI](docs/src/dev/shared-ui.md)).
* **Pure Rust:** everything builds with `cargo` alone – no Node.js, no
  downloaded engines and no C/C++ CAD kernels.
* **Configuration:** a new setting goes into `src/config.rs` and
  `docs/src/admin/configuration.md`, and it stays optional.
* **Tests and documentation:** new behaviour comes with tests (unit tests next
  to the code, end-to-end tests in `tests/`). The documentation and an entry
  under *Unreleased* in `CHANGELOG.md` change in the same pull request.

## Dependencies, research and branding

TeamCraft is licensed under the MPL-2.0, so new dependencies must be
compatible with it:

* **Allowed:** MIT, Apache-2.0, BSD, ISC, Zlib, Unlicense, MPL-2.0.
* **LGPL:** only for a component that is loaded separately.
* **Not allowed:** GPL, AGPL, SSPL and proprietary code.

Add every new third-party component to the licence table in the README.

Before you build a larger feature, look at how existing open-source projects
solve the problem. If you research how other systems handle something, write
the findings down in [`docs/src/research/`](docs/src/research): one page per
topic, listed in its `index.md` and in `docs/src/SUMMARY.md`.

Do not add logos, artwork or branding of other products, and nothing that
imitates the look of another vendor's software. Product names may be used to
describe compatibility, for example "reads SolidWorks files".

## Licence of contributions

TeamCraft has no contributor licence agreement. By submitting a contribution,
you confirm that you wrote it (or your AI agent did, from its own knowledge)
or have the right to submit it, and that it contains no code decompiled,
disassembled or rewritten from proprietary software and no leaked or
incompatibly licensed code (see [Your own work only](#your-own-work-only--no-decompiled-code)).
You also agree that it is licensed under the [Mozilla Public License 2.0](LICENSE),
like the rest of the project.
