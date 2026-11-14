# Live demo

A public, read-only TeamCraft that visitors try in seconds: the demo data of
the README screenshots (the openly licensed NIST CAD models, a multi-level
BOM, change orders, a change request, simulations, an as-built unit and a
restricted project group), one public account whose password the login page
shows, and a reset every night. Why it works this way: [Research: a hosted
live demo](../research/live-demo.md).

**What the server does.** With `TEAMCRAFT_DEMO_USER` (the public account) and
`TEAMCRAFT_DEMO_PASSWORD` (its password, shown on the login page) set:

* the login page names the account and fills in the user name;
* every page shows a *Live demo* banner with a link to the install;
* that account reads everything a CAD editor of its groups may read – pages,
  3D previews, BOMs, search, downloads, the REST API – but every request that
  is not `GET`/`HEAD` is refused with *This is a read-only live demo* (403),
  sign-out excepted. It cannot change its password, create API tokens, watch
  items or sign in to the desktop app;
* wrong passwords for that account count per client address only, so nobody
  can lock every visitor out;
* every other account – your administrator – works as usual.

Never set `TEAMCRAFT_DEMO_USER` on a server with real data: it makes an
account public.

## What you need

* A small Linux server with Docker – 2 CPU cores, 4 GB RAM and 20 GB disk
  are plenty; the 3D conversions are the heaviest part.
* A DNS name for it (for example `demo.<your domain>`) and a reverse proxy
  with TLS in front (the steps below use Caddy, which fetches the
  certificate itself).
* `git`, `curl` and `jq` on the server, to load the demo data.

## 1. Install TeamCraft

```sh
curl -fsSL https://raw.githubusercontent.com/rudalabs/TeamCraft/main/deploy/install.sh | bash
```

When the installer offers to create the first administrator, name it
**`admin`** (the seed script signs in with that name) and store its password
in a root-only file:

```sh
sudo sh -c 'umask 077; cat > /opt/teamcraft/secrets/admin_password'   # type the password, then Ctrl+D
```

The demo should show what `main` can do: set `TEAMCRAFT_VERSION=edge` in
`/opt/teamcraft/.env` (or a release tag, for a quieter demo).

## 2. Configure the demo

Add to `/opt/teamcraft/teamcraft.env`:

```sh
TEAMCRAFT_PUBLIC_URL=https://demo.example.org
TEAMCRAFT_INSTANCE_NAME="TeamCraft live demo"
TEAMCRAFT_DEMO_USER=demo
TEAMCRAFT_DEMO_PASSWORD=try-teamcraft
```

Leave e-mail, Mattermost, webhooks and the assistant unconfigured. Apply it:

```sh
cd /opt/teamcraft && sudo docker compose pull && sudo docker compose up -d
```

## 3. TLS in front

`/etc/caddy/Caddyfile` (see also [Reverse proxy and TLS](docker.md#reverse-proxy-and-tls)):

```text
demo.example.org {
    reverse_proxy 127.0.0.1:8080
}
```

`sudo systemctl reload caddy`, then open `https://demo.example.org/login` –
the page names the demo account.

## 4. Load the demo data

Once, against the still empty server (only `admin` exists):

```sh
git clone --depth 1 https://github.com/rudalabs/TeamCraft.git ~/teamcraft-src
cd ~/teamcraft-src
sudo BASE_URL=http://127.0.0.1:8080 \
     ADMIN_PASSWORD_FILE=/opt/teamcraft/secrets/admin_password \
     DEMO_PASSWORD=try-teamcraft \
     bash scripts/seed-demo.sh
```

`scripts/seed-demo.sh` uses the seeding of the screenshot script
(`scripts/screenshots.sh`): two editors, the project groups ROVER and LAB,
the NIST files, BOM, change orders, change request, simulations, the NX
model of the enclosure and the as-built unit; then it creates the account
`demo` (a CAD editor of ROVER, not a member of the restricted group LAB, so
the demo also shows what stays hidden) with `DEMO_PASSWORD`, which must be
the value of `TEAMCRAFT_DEMO_PASSWORD`. It takes a few minutes. The 3D
previews convert in the background: open a few models (the NX assembly takes
longest) before you announce the demo.

## 5. Snapshot and nightly reset

```sh
sudo mkdir -p /opt/teamcraft/demo
sudo cp ~/teamcraft-src/deploy/demo/demo.sh /opt/teamcraft/demo/
sudo bash /opt/teamcraft/demo/demo.sh snapshot
sudo bash /opt/teamcraft/demo/demo.sh reset      # try it once by hand
```

`snapshot` dumps the database through the database container (the TeamCraft
image has no PostgreSQL tools) to `/opt/teamcraft/demo/demo-db.dump`;
`reset` stops TeamCraft, empties the schema, restores the snapshot and starts
TeamCraft again – it also starts it again when the restore fails. No Docker
volume is deleted, and the vault stays (the demo account cannot write to it;
the cached previews stay warm). Every night at 03:15, in
`/etc/cron.d/teamcraft-demo`:

```text
15 3 * * * root bash /opt/teamcraft/demo/demo.sh reset >>/var/log/teamcraft-demo.log 2>&1
```

## 6. Check before you announce it

* `https://demo.example.org/login` names `demo` and its password; signing in
  works.
* The dashboard, *Vault*, an item page, the BOM of the sensor mast assembly,
  the 3D view of the mounting bracket and of the NX enclosure, a change
  order, the simulation results and the as-built unit open.
* *Edit* on a revision, *Check out* and *Account → API tokens* answer *This
  is a read-only live demo*.
* The restricted test rig shows *Restricted item* in its BOM.
* The next morning `/var/log/teamcraft-demo.log` ends with *reset done*.

## 7. Link it

Add the address to the top of `README.md` (next to *Download* in the badge
row, and *Try the live demo* in the first paragraph) and to the table of
`docs/src/index.md`; move the entry in `docs/OPEN_WORK.md` to done.

## Upgrades and new demo data

* **A new image** (`docker compose pull && docker compose up -d`): nothing
  else to do – the nightly reset restores the snapshot, and the server applies
  the newer migrations when it starts.
* **New demo data** (the screenshot seeding changed): empty the database, let
  TeamCraft start fresh, create `admin` again and seed again:

  ```sh
  cd /opt/teamcraft
  sudo docker compose stop teamcraft
  sudo docker compose exec -T db psql -U teamcraft -d teamcraft -c 'DROP SCHEMA public CASCADE' -c 'CREATE SCHEMA public'
  sudo docker compose start teamcraft
  sudo docker compose exec teamcraft teamcraft user create --username admin --role admin
  ```

  then steps 4 and 5 (seed, `snapshot`). The vault keeps the files of the
  previous seeding; they are content-addressed, so the same files are stored
  once.
