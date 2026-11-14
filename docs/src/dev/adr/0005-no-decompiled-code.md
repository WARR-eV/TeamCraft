# ADR 0005 – Own work only: no decompiled code

**Status:** accepted (2026-11-14) – a contribution rule of the project,
stated in full in
[CONTRIBUTING.md](https://github.com/rudalabs/TeamCraft/blob/main/CONTRIBUTING.md#your-own-work-only--no-decompiled-code).

## Context

TeamCraft reads the native files of many CAD programs (NX, SOLIDWORKS,
Inventor, CATIA V5, Creo, Fusion, Rhino, SketchUp, JT) to show 3D previews,
references and assembly structures. Most of these formats are not published.
In 2026 AI agents made it cheap to decompile a program and have the output
"rewritten" into another language. Open-source communities discussed projects
that did this and still called themselves clean-room re-implementations. The
points raised there:

* only the copyright holder can license code, and a translated or rewritten
  decompilation is still a derivative work;
* a clean room also requires that nobody involved has seen the vendor's code
  or is bound by an NDA;
* with AI agents nobody can show that the separation between "the team that
  read the code" and "the team that wrote the new code" held.

A single such contribution would expose the project and everyone who
distributes it to copyright and licence claims, and would force a removal and
a rewrite of the Git history. It would also undo the contributions built on
top of it.

## Decision

* Every contribution is the contributor's own work, written from their own
  skills and knowledge or from material whose licence allows it. The same
  holds for code an AI agent writes: from its general knowledge, the
  repository and permitted material.
* **Not accepted, from people or from AI agents:** code, pseudocode, tables or
  algorithms decompiled, disassembled or otherwise extracted from proprietary
  software (for example Siemens Teamcenter, VistaPoint, CAD programs, their
  viewers and SDKs), including translated or rewritten versions of it. Also
  not accepted: the two-team "clean room" through decompilation, leaked or
  confidential vendor material, and code ported from projects with an
  incompatible licence.
* Native formats are decoded from **files** (openly licensed samples such as
  the NIST models in `tests/fixtures/cad`, own files), **public
  specifications** (ISO 10303, ISO 14306) and **openly licensed code** under a
  compatible licence that is credited (`vendor/openskp`). The research page of
  each format records the source of its findings.

## Consequences

* A reader for a format can only decode what the samples and specifications
  show. Features that would need a program's code stay unread: the research
  pages name them (for example "the ShapeManager B-rep stays unread" for
  Fusion, "the Parasolid block later or never" for NX).
* Format work needs openly licensed sample files first. This is why
  [Openly licensed CAD files for end-to-end tests](../../research/cad-test-files.md)
  came before the readers.
* Reviewers may ask where a format finding came from; a contribution that
  cannot say is not merged.
* Someone who has seen a vendor's source code or is under an NDA about a
  program does not work on that program's formats.

## Alternatives turned down

* **A clean room with a decompiling team and a writing team.** It is a
  recognised method between people, but the specification still comes from
  the program's code, and with AI agents the separation cannot be shown.
* **Accepting decompilation for interoperability where a law allows it.**
  Such exceptions are narrow and do not allow publishing the result. Most
  licence agreements forbid decompiling anyway.

## Where it is held

| What                                    | Where                                                                                                   |
|-----------------------------------------|---------------------------------------------------------------------------------------------------------|
| The rule, what is allowed, why          | `CONTRIBUTING.md` (*Your own work only – no decompiled code*, *Licence of contributions*)               |
| Short hint                              | `README.md` (*Contributing*), `docs/src/index.md`                                                        |
| Conduct and enforcement                 | `CODE_OF_CONDUCT.md`                                                                                    |
| Confirmation per pull request           | `.github/pull_request_template.md`                                                                      |
| Rule for AI agents in this repository   | `CLAUDE.md` (*Hard rules*)                                                                              |
| Origin of the format knowledge          | the format pages in `docs/src/research/` (index note)                                                    |
