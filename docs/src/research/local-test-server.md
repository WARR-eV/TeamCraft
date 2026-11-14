# One download to try TeamCraft on a laptop

Question: how can somebody who is not an administrator – a student, an
engineer who wants to evaluate TeamCraft – try it on their own laptop with one
download and no set-up? The release pipeline already builds server archives
and desktop installers, but the server needs PostgreSQL, and the only
one-command installation is Docker on Linux. Date: 2026-11-14.

## Requirements

- One installer per common platform (Windows x64, macOS Apple silicon, Linux
  x86_64) that brings the desktop app, the server and the database.
- Nothing to configure: no database URL, no password, no port.
- Nothing reachable from the network; nothing left running after the app
  ends; no data loss when the app crashes.
- No new runtime dependency for normal server installations, no change to
  the regular desktop installers (size, behaviour).
- Testable in CI on every shipped platform.

## Options

| Option | Pros | Cons |
|---|---|---|
| **Docker install** (exists) | production set-up, backups, upgrades | needs Docker (Desktop on Windows/macOS, licences for companies); a shell command; too much for a first look |
| **SQLite instead of PostgreSQL for local use** | one file, no server process | the schema and queries are PostgreSQL (`FOR UPDATE SKIP LOCKED`, `CHECK`, full-text search, `jsonb`); a second database backend for every migration and query – not maintainable |
| **`postgresql_embedded` crate** (MIT/Apache-2.0) in the server | downloads or bundles PostgreSQL, manages `initdb`/`pg_ctl` | its `bundled` feature downloads the archive in `build.rs` (network access during every build); otherwise it downloads PostgreSQL from GitHub at the first run; brings an archive/HTTP stack into the server for a feature only test installations use |
| **Per-platform installer scripts** (NSIS, WiX, `.pkg`, deb `postinst`) that install PostgreSQL as a service | a "real" installation | administrator rights, a service that keeps running, five scripting languages, hard to test and to uninstall cleanly |
| **Bundle PostgreSQL's programs; `teamcraft local` runs them** | no new crate; the release job downloads a pinned major version with a checksum; the same command works with any installed PostgreSQL; easy to test end to end | the installers get larger (PostgreSQL ≈ 30–60 MB uncompressed after pruning); macOS needs every Mach-O file signed (ad hoc) |

## Decision

Take the last option, in three parts:

1. **`teamcraft local`** (`src/localserver.rs`). The server binary gets a
   subcommand that runs a private cluster and the server:
   - `initdb` on the first start: role `teamcraft`, `scram-sha-256` with a
     random 32-character password kept in `<data>/db-password` (0600),
     UTF-8 without locale;
   - at every start a free port, `listen_addresses = '127.0.0.1'`,
     `unix_socket_directories = ''` (written to `teamcraft-local.conf`,
     included from `postgresql.conf`), `pg_ctl start --wait`;
   - the server's configuration from an overlay that wins over the
     environment (bind address, public URL, database, vault); everything
     else – an assistant key, OIDC – still comes from the environment;
   - `pg_ctl stop --mode=fast` after the server stopped; a cluster left
     running by a killed instance is stopped at the next start; a second
     instance on the same data refuses (its port file answers `/readyz`);
   - `--exit-with-stdin`: the desktop app keeps a pipe open and never writes
     to it. When the app ends – also by a crash or kill – the pipe closes and
     the server shuts down cleanly with PostgreSQL. A plain thread reads
     stdin, so the runtime never waits for it.

   The data lives in the per-user data directory (XDG on Linux,
   `Application Support` on macOS, `%LOCALAPPDATA%` on Windows), shared by
   the app and the command line.
2. **Desktop app** (`desktop/src/localserver.rs`). When the server binary is
   next to the app, the connect screen offers *Try TeamCraft on this
   computer*: start `teamcraft-server local` with the bundled
   `postgresql/bin`, wait for `/readyz`, use `http://127.0.0.1:<port>` as the
   server and open the normal browser sign-in. The first-start page creates
   the administrator and now returns to the app's authorize page (`next`
   kept through `/setup`). The port is kept in the settings so that the
   address and the token stay valid; while the local server is the
   configured server the app starts it at every launch.

   *Addendum 2026-11-15:* the installers with local server are an app of
   their own, *TeamCraft Local* (identifier
   `io.github.rudalabs.teamcraft.local`, own settings folder, the token's
   keychain service follows the identifier). The first test build shared the
   desktop app's identity and opened with the server configured there. With
   no server configured, TeamCraft Local starts its local server by itself.
3. **Release job `local-server`** (`.github/workflows/release.yml`): builds
   the server, takes the newest patch release of PostgreSQL 18 from
   [theseus-rs/postgresql-binaries](https://github.com/theseus-rs/postgresql-binaries)
   (relocatable builds for Linux, macOS and Windows, PostgreSQL Licence),
   checks its `.sha256`, prunes headers, docs and static libraries, keeps
   only the programs `initdb`, `pg_ctl`, `postgres` and the client tools
   `psql`, `pg_dump`, `pg_restore`, `pg_isready` (the Windows build also
   brings PL/Python's Python interpreter, which is dropped – an unsigned
   installer with an embedded `python.exe` was reported as a virus by a
   browser; addendum 2026-11-15), adds the
   `COPYRIGHT` of that PostgreSQL version, signs the macOS files ad hoc and
   bundles everything with `desktop/tauri.local-server.conf.json`
   (`externalBin` + `resources`). Every installer is then started where an
   installation puts it (AppImage extracted, the macOS `.app`, the Windows
   setup installed silently) and its bundled server runs a first start, the
   first-start page and a clean stop.

### Details that decided the shape

- **The binary's name.** The app's own executable is `TeamCraft` on Windows
  and macOS, whose file systems ignore case: a sidecar called `teamcraft`
  would be the same file. The server ships as `teamcraft-server`; its
  isolated conversion workers (`preview-mesh`, `text-extract`) recognise that
  name too, so CAD conversions stay in a child process with limits.
- **Separate installers.** Only Windows x64, macOS Apple silicon and Linux
  x86_64 (AppImage) get a *with local server* variant – the platforms of most
  testers; the regular installers keep their size. Windows ARM64 runs the x64
  build under emulation.
- **No new crate**, no download at run time, nothing in a normal server
  build changes.

## Limits

- Not for production: one user's computer, no backups, no TLS, no e-mail.
- Not code-signed or notarised (like every TeamCraft download so far). On
  macOS the quarantine flag may also have to be removed from the app.
- Data from the local server moves to a real installation with `teamcraft
  backup`/`restore` only by hand (database settings from `db-password`).

## Sources

- PostgreSQL: [`initdb`](https://www.postgresql.org/docs/current/app-initdb.html),
  [`pg_ctl`](https://www.postgresql.org/docs/current/app-pg-ctl.html),
  [connection settings](https://www.postgresql.org/docs/current/runtime-config-connection.html),
  [licence](https://www.postgresql.org/about/licence/).
- [theseus-rs/postgresql-binaries](https://github.com/theseus-rs/postgresql-binaries)
  and [postgresql_embedded](https://crates.io/crates/postgresql_embedded)
  (read for comparison, not used).
- Tauri 2: [embedding external binaries](https://v2.tauri.app/develop/sidecar/),
  [resources](https://v2.tauri.app/develop/resources/).
