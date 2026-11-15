# Try it on your computer

You can try TeamCraft on your own laptop without a server, Docker or a
database administrator. Two ways lead to the same result: a private
PostgreSQL database and the TeamCraft server, both running only on your
computer, with all data in one folder of your user account.

This is for trying TeamCraft out. It is **not** a way to run TeamCraft for a
team: nobody else can reach the server, there are no backups and no TLS. For
a team, use the [Docker install](../admin/docker.md).

## With the desktop app (no set-up)

1. Download the desktop app **with local server** from the
   [Releases page](https://github.com/rudalabs/TeamCraft/releases):

   | System                         | File                                              |
   |--------------------------------|---------------------------------------------------|
   | Windows 10 / 11 (x64)          | `TeamCraft-with-local-server_<version>_x64-setup.exe` |
   | macOS (Apple silicon)          | `TeamCraft-with-local-server_<version>_aarch64.dmg`   |
   | Linux (x86_64)                 | `TeamCraft-with-local-server_<version>_amd64.AppImage` |

   The installers are not code-signed yet: on Windows, SmartScreen shows
   *More info → Run anyway*; on macOS, open the app the first time with
   *right-click → Open*; on Linux, make the AppImage executable
   (`chmod +x`).
2. Start **TeamCraft Local**. On its first start it starts the local server
   by itself (no address to type); creating the database takes a few
   seconds, at most about a minute on a slow disk.
3. Your browser opens. Create the **administrator** – pick a username and a
   password (10+ characters) – and confirm the sign-in of the app. You are
   now signed in, in the browser and in the app.
4. Continue with [Getting started](getting-started.md#first-start): create a
   project group, then an item with a STEP file, and open it in 3D.

The app starts the local server again each time you open it while it is the
configured server, and stops it when you quit the app. *change* next to the
server name on the sign-in screen (or *File → Sign Out*) connects the same
app to any other TeamCraft server; the local data stays where it is. The
connect screen then offers **Try TeamCraft on this computer** to come back.

The installer *with local server* is the regular desktop app plus two
things: the server program (`teamcraft-server`) and PostgreSQL's programs
(`postgresql/`: `initdb`, `pg_ctl`, `postgres`, the client tools `psql`,
`pg_dump`, `pg_restore`, `pg_isready` and their libraries, with PostgreSQL's
licence in `COPYRIGHT.txt`).

**An app of its own.** It installs as *TeamCraft Local* (app identifier
`io.github.rudalabs.teamcraft.local`) next to the regular desktop app
*TeamCraft*, with its own start-menu entry, settings folder and keychain
entry for the sign-in. A desktop app that is connected to your team's server
stays connected, and neither app ever sends its token to the other's
server. The regular installers stay small and do not show the button.

## On the command line

With PostgreSQL 16 or newer installed (its server programs `initdb`,
`pg_ctl` and `postgres`, not only the client), the server archive from the
Releases page does the same:

```sh
teamcraft local
```

Open <http://127.0.0.1:8080> and create the administrator. `Ctrl+C` stops
the server and PostgreSQL.

| Option              | Default                                   | Meaning                                                                                   |
|---------------------|-------------------------------------------|-------------------------------------------------------------------------------------------|
| `--port <port>`     | `8080`                                    | port of the web server on 127.0.0.1                                                       |
| `--data-dir <dir>`  | per-user data folder (below)              | database, vault and logs                                                                  |
| `--pg-bin <dir>`    | searched                                  | folder with `initdb`, `pg_ctl` and `postgres`                                             |
| `--exit-with-stdin` | off                                       | stop when standard input closes (the desktop app keeps a pipe open while it runs)        |

Without `--pg-bin`, TeamCraft looks for PostgreSQL's programs in this order:
`postgresql/bin` next to the `teamcraft` program (the desktop download
*with local server*), `PATH`, `/usr/lib/postgresql/<version>/bin` (Debian,
Ubuntu), Homebrew (`postgresql@<version>`), Postgres.app, and
`C:\Program Files\PostgreSQL\<version>\bin` on Windows – the newest version
first. PostgreSQL refuses to run as root (`sudo`): start `teamcraft local`
as your normal user.

## Where the data is

The desktop app and `teamcraft local` use the same folder, so both see the
same data:

| System  | Folder                                                    |
|---------|-----------------------------------------------------------|
| Linux   | `~/.local/share/teamcraft/local-server` (or `$XDG_DATA_HOME/teamcraft/local-server`) |
| macOS   | `~/Library/Application Support/TeamCraft/local-server`    |
| Windows | `%LOCALAPPDATA%\TeamCraft\local-server`                   |

Inside it:

```text
postgres/         the database cluster
postgres.log      PostgreSQL's log
pg_ctl.log        output of the last `pg_ctl start` (why PostgreSQL did not start)
db-password       the generated database password (readable by you only)
vault/            the stored files
```

The desktop app writes the server's own log to `local-server.log` in its
settings folder. To start over, quit the app (or stop `teamcraft local`) and
delete the folder. Nothing in it is backed up: for data you want to keep, use
the [Docker install](../admin/docker.md) with its backups.

## How it is protected

- PostgreSQL and the server listen only on `127.0.0.1`; other computers
  cannot connect. PostgreSQL gets a free port at every start and no Unix
  socket.
- The database password is random (32 characters), stored in `db-password`
  (mode `0600` on Linux and macOS), and checked with `scram-sha-256`, so other
  accounts on the same computer cannot use the database.
- The page that creates the administrator is only offered while no account
  exists and the server listens on this computer. Unlike a normal server,
  `teamcraft local` needs no setup link from its log: its port is new and
  private, and the desktop app opens the page right after the start.
- A second `teamcraft local` on the same folder refuses to start instead of
  stopping the running database.

## When something goes wrong

- **The button is missing** – this is the regular desktop app; download the
  one *with local server*.
- **It opens with your other server instead of the local one** – an early
  test build of the installer *with local server* was still the desktop app
  itself and took over its settings and sign-in. Install the current one
  (*TeamCraft Local*); if the desktop app now shows *Try TeamCraft on this
  computer*, reinstall the regular desktop app over it.
- **"The local server stopped" / "did not start"** – the message shows the
  end of the server's log. `postgres.log` in the data folder has PostgreSQL's
  side. The app replaces a port that another program took with a free one;
  `teamcraft local` names the port in use – choose another with `--port`.
- **macOS blocks the server or PostgreSQL** – the app is not notarised yet.
  If *right-click → Open* on the app was not enough, remove the quarantine
  flag once: `xattr -dr com.apple.quarantine "/Applications/TeamCraft Local.app"`.
- **"TeamCraft already runs on …"** – another `teamcraft local` (or the app in
  another window) serves the same folder; use that one, or stop it first.
- **Windows, 0.2.8: `initdb: error: program "postgres" is needed by initdb
  but was not found`** – a bug of the 0.2.8 installer (the app passed its folder
  as a `\\?\C:\…` path, which PostgreSQL cannot use), fixed in 0.2.9.
  Install 0.2.9 or newer over it; nothing on your computer needs to be
  cleaned up.
- **The browser or Windows Defender reports a virus** – the installers are
  not code-signed yet, and an unsigned installer full of programs is what
  heuristic scanners distrust. Download only from the
  [Releases page](https://github.com/rudalabs/TeamCraft/releases) and compare
  the file with `SHA256SUMS.txt` of the same release (PowerShell:
  `Get-FileHash <file>`). Please
  [open an issue](https://github.com/rudalabs/TeamCraft/issues) with the name
  of the detection, so that it can be reported to the scanner's vendor as a
  false positive. Do not turn off your virus scanner.
