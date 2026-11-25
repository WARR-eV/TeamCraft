# Checks, CI and merging

What the maintainers run and what CI checks. The setup and the basic
commands are in the README under
[Development](https://github.com/WARR-eV/TeamCraft#development), the rules
for contributors in
[CONTRIBUTING.md](https://github.com/WARR-eV/TeamCraft/blob/main/CONTRIBUTING.md);
this page is the detail behind both.

## Checks before a pull request

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test                  # unit tests and end-to-end tests
```

The end-to-end tests in `tests/` create and drop their own temporary
`teamcraft_test_*` databases on the configured server, or on
`TEAMCRAFT_TEST_DATABASE_URL`. Without a reachable database they are
skipped; with `TEAMCRAFT_REQUIRE_DB_TESTS=1` that is a failure. `cargo test
--lib` runs only the unit tests and needs no database. CI runs the tests with
`cargo nextest run`, which enforces the limits in `.config/nextest.toml`: a
unit test fails after **1 second** (slow tests on purpose are listed there
with a reason; database tests get a minute) and is retried twice, so a test
caught by a stalled runner shows up as `FLAKY`. `tests/backup_restore.rs`
also needs `pg_dump` / `pg_restore` on `PATH` (required in CI with
`TEAMCRAFT_REQUIRE_PG_TOOLS=1`).

Each test binary keeps at most 6 test databases open (5 connections each;
`TEAMCRAFT_TEST_DB_SLOTS`); several test runs against one server can still
reach PostgreSQL's `max_connections`, and the harness then waits. A server
whose `lc_messages` is not English sends errors sqlx cannot read –
`lc_messages = 'C'` helps. A failing test prints the server's warnings and
errors (`RUST_LOG` selects more).

| If you changed …                       | also run                                                                                                                         |
|----------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|
| the desktop app                        | `cargo fmt/clippy/test --manifest-path desktop/Cargo.toml`                                                                       |
| the documentation                      | `mdbook build docs` (mdBook 0.5) and `scripts/check-doc-links.sh`                                                                |
| the web UI                             | check the changed pages in a browser (no CSP errors in the console)                                                              |
| a pure module in `.cargo/mutants.toml` | `cargo mutants -F '^src/<module>\.rs:'` – every missed mutant gets a unit test or a reason ([mutation testing](mutation-testing.md)) |

## What the guard tests check

Several tests read the sources instead of running them, and fail with the
rule that was broken:

* **Architecture guards** (`tests/architecture_guards.rs`, no database):
  `sqlx` in `src/web`, `src/api` or `src/mcp`; a public service function
  that writes without an audit entry (itself or through the services it
  calls – personal data, caches and housekeeping go on its `NOT_AUDITED`
  list with a reason); a service function that takes an actor and queries
  items, revisions or files without a visibility guard in its body (an
  `access::*` fragment, `ensure_visible`, `ensure_editable` or a guarded
  getter such as `revisions::get` – reviewed exceptions are on `NOT_GUARDED`
  with a reason); a template that is not in `TEMPLATES` or not rendered by
  `every_template_renders`; a `TEAMCRAFT_*` variable missing from
  [Configuration](../admin/configuration.md); a language file of the desktop
  Group Policy template whose IDs differ from `en-US`.
* **Hidden data:** `tests/no_unchecked_in_handlers.rs` fails when a
  `*_unchecked` getter (for background and system code) is called from
  `src/web`, `src/api` or `src/mcp`. The leak test `tests/read_permissions.rs`
  reads the router sources and exercises every route and MCP tool; a route
  with a new kind of path parameter needs a placeholder rule in its
  `fill()`.
* **One transaction:** `tests/atomic_processes.rs` puts a trigger on a table
  that makes one step of a multi-step mutation fail (an error or a lost
  connection), and the snapshot before and after must be equal – add a case
  for a new multi-step mutation.
* **Migrations:** `tests/migrations.rs` upgrades a v0.1.0 database with data
  (`tests/fixtures/migrations/v0.1.0-seed.sql`) and fails when a migration
  loses rows, changes old values or narrows a `CHECK` list that old rows
  still use – a new enum value is fine, removing one needs a data migration.
* **Shared UI and design tokens:** see [Shared UI](shared-ui.md#what-ci-checks).

## What CI runs for a change

A job runs only when a file it compiles, tests or reads changed. One table
decides it for the workflows CI, Desktop app, CodeQL and Docs:
`scripts/ci-changes.sh`. Each workflow's `changes` job runs its self-test
first, then compares the pull request with its base (a push to `main` with
the previous `main`, a merge-queue entry with its base) and starts only the
jobs that need it.

| Changed files                                                                                                                                                                                               | Jobs that run                                                                                  |
|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------|
| server: `src/`, `tests/`, `migrations/`, `templates/`, `static/`, `shared/`, `vendor/`, `media/logos/`, `media/icons/`, `fuzz/seeds/`, `Cargo.toml`/`Cargo.lock`, `build.rs`, lint and nextest configuration | CI: fmt and clippy, tests on PostgreSQL 16 and 18, Windows build (weekly: also macOS)          |
| the desktop files server tests compare with: `desktop/ui/app.css`, `desktop/ui/index.html`, `desktop/ui/js/{core,settings,shell}.js`, `desktop/src/compat.rs`                                               | the server jobs as well                                                                        |
| `docs/src/`, `deploy/teamcraft.env.example`, `deploy/docker-compose.yml`, `deploy/desktop-policies/windows/`, `packaging/linux/teamcraft.env`                                                               | CI: *docs and config guards* – `config::tests` and `tests/architecture_guards.rs`, no database |
| `Cargo.toml`/`Cargo.lock`, `desktop/Cargo.*`, `deny.toml`, `vendor/`                                                                                                                                        | CI: `cargo deny` for the lock file concerned                                                   |
| the desktop app's Rust code and configuration, `shared/`, `media/logos/`, `media/icons/`, `tests/fixtures/cad/nist/`                                                                                        | Desktop app: fmt, clippy, tests, build and start-up test on every platform                    |
| `desktop/ui/`, `static/`, `shared/`, `scripts/*.mjs`                                                                                                                                                        | Desktop app: *desktop UI* (syntax, imports, the Node tests)                                    |
| `*.rs` and Cargo files / `*.js`, `*.mjs`, `*.html` / workflows                                                                                                                                              | CodeQL: the Rust / JavaScript / Actions analysis                                               |
| `docs/`, `src/`, the Cargo files                                                                                                                                                                            | Docs: build of the book and the API reference (on `main`: deploy); the links check runs always |

A change of a workflow runs its own jobs; a change of `scripts/ci-changes.sh`
runs everything. Scheduled runs (CI and Desktop app weekly, CodeQL weekly)
and *Run workflow* run everything, so a file the table misses is caught
within a week. **When a test starts to read a file outside its area, add the
file to the table** (and a line to the self-test). To see what a branch
would run:

```sh
git diff --name-only origin/main... | bash scripts/ci-changes.sh --files
bash scripts/ci-changes.sh --self-test
```

Mutation testing, the release builds and the screenshots keep their own path
filters (`.github/workflows/mutants.yml`, `release.yml`, `screenshots.yml`);
fuzzing and the scale check run on a schedule only.

## Scale check

After changing queries, indexes or pages that list many rows, run the scale
check: an ignored test that seeds a large synthetic vault (~100k items, ~1M
BOM lines, 2M audit rows; see
[Operations → Scale check](../admin/operations.md#scale-check)) and times the
hot paths through the router.

```sh
docker compose up -d db
cargo test --release --test scale -- --ignored --nocapture
```

The seed (a few minutes, ~1.6 GB) stays in the database `teamcraft_scale`;
`DROP DATABASE teamcraft_scale` starts over. The report lands in
`target/scale-report/` (`report.md`, `plans.md` with the `EXPLAIN (ANALYZE,
BUFFERS)` plans, `results.json`); `TEAMCRAFT_SCALE_BASELINE=<earlier
results.json>` adds a before/after column. On GitHub, run the **Scale check**
workflow by hand (`gh workflow run scale.yml --ref <branch>`) and download the
`scale-report` artifact.

## Merging

* Pull requests are **squash-merged**: each lands on `main` as one commit
  with the pull request's title, whatever its branch history looked like.
* **CI does not have to be green for a merge**; a fix can follow in its own
  pull request. Before a **release**, though, CI on `main` must pass – so
  red commits on `main` delay the next release (see the
  [Release checklist](release-checklist.md)).
* **CI result** is the one check that sums up the CI workflow: green when
  every job passed or was skipped on purpose (a job runs only for the files
  it checks, see [above](#what-ci-runs-for-a-change)), red when one failed or
  was cancelled. The macOS server build runs only weekly and on *Run
  workflow* and is advisory: GitHub's macOS runners are often unavailable,
  and a job that never got a runner is cancelled – no fault of the code
  (#358); the nightly Release run builds the server for macOS anyway. CI
  also runs on `merge_group`, so a merge queue (*Settings → Rules*) tests
  each pull request against the `main` it lands on.
* Security review is strict for contributions from outside the maintainers;
  a pull request that tries to sneak in a vulnerability is closed without
  discussion (CONTRIBUTING.md, *Commits and pull requests*).
