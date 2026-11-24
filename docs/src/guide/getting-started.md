# Getting started

## Try it on your laptop

> **Paused for the next releases** (since 2026-11-22, to make releases faster): the
> installers *with local server* and the Windows x64 server download are not built.
> Until they are back, use `teamcraft local` ([Try it on your computer](local-server.md)) or the
> live demo.

Download the desktop app **with local server** from the
[Releases page](https://github.com/WARR-eV/TeamCraft/releases) (Windows x64,
macOS Apple silicon, Linux x86_64) and start *TeamCraft Local*, an app of its
own next to the regular desktop app: it starts a private server with its own database on
your computer and opens the browser, where you create the administrator. With
PostgreSQL installed, `teamcraft local` does the same on the command line.
Details: [Try it on your computer](local-server.md).

## Install on Linux with Docker

```sh
curl -fsSL https://raw.githubusercontent.com/WARR-eV/TeamCraft/main/deploy/install.sh | bash
```

TeamCraft and PostgreSQL run as containers; the script offers to create the
administrator at the end. See [Install with Docker](../admin/docker.md).

## Build and run

You need Rust (stable, via [rustup](https://rustup.rs)) and PostgreSQL 16 or newer.
The repository's `docker-compose.yml` starts a development database that
needs no password:

```sh
git clone https://github.com/WARR-eV/TeamCraft.git && cd TeamCraft
docker compose up -d db
cargo run --release
```

TeamCraft applies its database migrations on start and listens on
<http://localhost:8080>. The platform guides cover native PostgreSQL
installations and the desktop app:
[Linux](https://github.com/WARR-eV/TeamCraft/blob/main/platforms/linux.md) ·
[macOS](https://github.com/WARR-eV/TeamCraft/blob/main/platforms/macos.md) ·
[Windows](https://github.com/WARR-eV/TeamCraft/blob/main/platforms/windows.md).

## First start

1. Open the **setup link** the server writes to its log when it starts
   (`no administrator yet: open http://localhost:8080/login?setup=…`; with
   systemd: `journalctl -u teamcraft`). A server without accounts that listens
   on localhost then offers **Create the administrator** – pick a username and
   a password (10+ characters). The form disappears afterwards. The link's
   one-time code keeps out anyone who reaches the server through a reverse
   proxy before you; it changes at every restart. Instead of the link:
   `teamcraft user create --role admin` on the server. `teamcraft local` and
   the desktop app *TeamCraft Local* need no link.
2. **Projects → New project group**: every item belongs to one. The creator
   manages the group and adds the team members.
3. **Admin → Users** (admins): create accounts for your team, or connect single
   sign-on (see [Users, roles and SSO](../admin/users.md)).
4. **Items → New item**: choose the CAD file (s) first – the model, its
   drawing and a STEP export for the 3D preview. Name and type are taken from
   the first file (an assembly for `.iam`, `.asm`, `.sldasm`, …). *Folder*
   (optional, e.g. `chassis/brackets`) is where the item and its files go in
   the project group's folder tree. The item is created together with its
   files; its first revision *A* is *in work* and checked out to you.
5. **Check in**, then release it with a change order.

**Desktop app, first start:** after you connect to the server and sign in,
TeamCraft Desktop opens *Getting started* – at most three read-only steps:
where your files go (the working folder and the CAD programs found on the
computer), your project group (only when you have several, or none yet) and
your first assembly – up to five of the group's assemblies, each with
**Show**, which opens its revision tab with *Check out and open* as the next
step. The wizard creates no file or folder and works the same for every CAD
program; it is remembered per server and account, and *Help → Getting
Started…* opens it again. See [Desktop app](desktop.md#getting-started).
How a group's manager sets up the CAD programs to find the parts of its
assemblies: [Set up CAD programs](../admin/cad-setup.md).

**Empty pages name the next step** (#423): with no project group yet, the
vault and *Projects* offer *Create a project group* to CAD editors and tell
viewers to ask a manager of a group to add them; an empty project group in
the vault offers *New item* and *Import BOM* to those who may create items
in it (its CAD editor members and administrators) and asks other CAD editors
to have a manager of the group add them; the dashboard's empty cards
(*Waiting for my review*, *Checked out by me*, *My open changes*) say how work
arrives there. A **viewer account** sees a note on the dashboard that editing
needs the CAD editor role and that an administrator of the server grants it –
so the missing buttons are not a mystery. These texts come from the message
catalogs and follow the browser's language (English, German).

**Appearance:** TeamCraft follows the light or dark appearance of your
operating system. The menu under your name (top right) and *Account →
Appearance* switch between *System*, *Light* and *Dark*; choose *Dark* to keep
the dark look of earlier versions. The distribution skins (Arch, CachyOS,
EndeavourOS, KDE, Kubuntu, openSUSE, Ubuntu) are dark. The choice is personal
and also styles the login page in that browser.

**Preferred CAD program:** if your parts exist in several CAD formats, *Account → Preferred CAD program* shows the files
of your program (and all
neutral formats such as STEP) first and collapses the native files of other
programs – see [Files](files.md#preferred-cad-program). The setting is
personal and applies in the browser and the desktop app.

**Keyboard:** `/` or `Ctrl+K` (`⌘K`) jumps to the search field. The single-key
`/` can be switched off under *Account → Appearance → Keyboard* (per browser).

Keyboard: `/` jumps to the search field (on the vault explorer: to its
filter); in menus, `Esc` closes and the arrow keys move.

On a server that is reachable from other machines, create the first
administrator from the command line instead:

```sh
teamcraft user create --username admin --role admin      # prompts for the password
```

## Command line

| Command                                                                            | Purpose                                      |
|------------------------------------------------------------------------------------|----------------------------------------------|
| `teamcraft` / `teamcraft serve`                                                    | run the web server (applies migrations)      |
| `teamcraft migrate`                                                                | apply migrations and exit                    |
| `teamcraft user create --username U --role viewer\|editor\|admin [--project CODE]` | local account                                |
| `teamcraft user set-password --username U`                                         | reset a password                             |
| `teamcraft user list`                                                              | all accounts                                 |
| `teamcraft vault check [--verify-hashes]`                                          | every stored file present (and intact)?      |
| `teamcraft rescan-references [--format id]`                                        | read stored CAD file references again        |
| `teamcraft vault signatures [--convert]`                                           | shape signatures for the similar part search |
| `teamcraft healthcheck`                                                            | exit 0 when the server answers on `/healthz` |
