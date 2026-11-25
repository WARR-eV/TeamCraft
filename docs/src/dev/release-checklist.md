# Release checklist

This is the list a maintainer follows to cut a release. How the workflow
builds and publishes is described [below](#how-releases-are-built); the
release rules for agents are in
[CLAUDE.md](https://github.com/WARR-eV/TeamCraft/blob/main/CLAUDE.md). The
workflow is `.github/workflows/release.yml`. Nothing is published before the
maintainer pushes the tag. CI on `main` must be green before a release
([Merging](ci.md#merging)).

Version numbers follow [Semantic Versioning](https://semver.org). While the
version is `0.x`, a release may contain breaking changes. From `1.0` on, the
rules of the [support policy](../admin/support-policy.md) apply.

## 1. Before the release candidate

- [ ] `main` is green: the CI workflow (`ci.yml`, fmt, clippy, tests on
  PostgreSQL 16 and 18, macOS and Windows builds), `docs.yml` and
  `release.yml` (build only; it runs nightly, so start it with *Run workflow*
  on `main` when the commit is newer than the last nightly build) all pass on
  the commit to release.
- [ ] The latest run of the scale check (`scale.yml`, manual or weekly) passed
  on this commit or a later one, or a failure is known and written down in
  the release notes.
- [ ] `CHANGELOG.md` has every change since the last tag under *Unreleased*,
  grouped as *Added*, *Changed*, *Fixed* and *Security*. Each entry is
  taken from a merged change; nothing is added from memory.
- [ ] The threat model (`docs/src/dev/threat-model.md`) and
  `docs/OPEN_WORK.md` show no open finding that the release claims to fix.
- [ ] The work-in-progress and no-warranty notices are still in the README,
  `docs/src/index.md`, the web footer and the desktop *About* dialog, unless
  the maintainer decides that this release is the first stable one.

## 2. Version bump and release candidate

The version must be the same in three files. The release workflow refuses a
mismatch.

- [ ] `Cargo.toml` (`version`)
- [ ] `desktop/Cargo.toml` (`version`)
- [ ] `desktop/tauri.conf.json` (`version`)
- [ ] Also `docs/theme/doxygen.js` (`version`, shown in the title and footer
  of the book); the release workflow does not check this one.
- [ ] Run `cargo check` and `cargo check --manifest-path desktop/Cargo.toml`
  so that both `Cargo.lock` files follow, then commit the bump on `main`
  through a pull request.
- [ ] Move the *Unreleased* entries of `CHANGELOG.md` under the new version
  heading, with the date, and leave an empty *Unreleased* section.
- [ ] Tag a release candidate: `git tag -a v1.0.0-rc.1 -m "TeamCraft 1.0.0-rc.1"`
  and `git push origin v1.0.0-rc.1`. A suffixed tag is a pre-release. It
  gets no `:latest` image tag and no `:<major>.<minor>` update.

## 3. Soak time

- [ ] Let the release candidate run for the soak period before the final tag.
  The project's proposed minimum is **7 days**, with no new open bug
  rated as blocking. A fix during the soak needs a new candidate (`-rc.2`) and the soak starts again.
- [ ] Test the candidate: the install from the container image with
  `deploy/install.sh`, an upgrade from the previous release with data, and
  one desktop installer per platform. With one installer *with local
  server*, on a computer without PostgreSQL and with the desktop app
  connected to another server: TeamCraft Local installs next to it, starts
  its local server by itself, the browser offers to create the administrator,
  the app signs in, quitting it stops the server, and the desktop app still
  shows its own server.
- [ ] Run the [desktop update test](#desktop-update-test) for each install
  type.
- [ ] Desktop first use (*Getting started*, #422), with a fresh settings
  folder: (1) a new **viewer without a group** signs in – the wizard ends on
  the group step with *Copy user name*, nothing is created in the working
  folder; (2) a new **CAD editor in a group with an assembly** signs in –
  *Show* opens the revision tab with *Check out and open* highlighted and the
  loop tip, *Check out and open* opens the assembly with its parts; after
  *Cancel check-out* the tip gives way to *This is your daily loop*;
  (3) *Settings → Connections → Add another account* and sign in – the
  wizard opens once more for that account, not again for the first one.
- [ ] Desktop: a running Inventor is reused (#350, #483), on Windows with
  Inventor started by hand and a part of a group without a default CAD
  version: (1) *Check out and open* an Inventor part – it opens in that
  Inventor, its window comes to the front and the toast ends with "(the
  instance that was already running)"; (2) with two Inventor versions
  installed and the **older** one running, the same – the toast names the
  older version; (3) a group whose default CAD program pins the **other**
  version – a new instance of that version starts and the toast says which
  one is running; (4) Inventor started *as administrator* – a new instance
  starts with the note that the running one did not answer. Paste the toast
  of any step that differs into #483.
- [ ] Note what was found in the soak in the changelog or in the release
  notes.

## 4. Final tag

- [ ] Tag the same commit as the last candidate, without a suffix:
  `git tag -a v1.0.0 -m "TeamCraft 1.0.0"` and `git push origin v1.0.0`.
  Do not move a tag that was pushed; use a new version instead.
- [ ] Watch `release.yml` for the tag. It builds everything again from the tag,
  runs the install smoke test against the new image, and creates the GitHub
  Release. A `-rc` or `0.x` version is marked as a pre-release.

## 5. Verify the assets

- [ ] The GitHub Release lists the server archives (Linux x86_64 and ARM64),
  the desktop installers (AppImage, `.deb`, `.rpm`,
  Flatpak, `.dmg`, `.msi`, NSIS setup and portable `.zip`) and `SHA256SUMS.txt`.
  *Paused since 2026-11-22:* the Windows x64 server archive and the three
  installers *with local server* (`TeamCraft-with-local-server_…`) – when they
  are switched on again ([How releases are built](#how-releases-are-built)),
  check them here too.
- [ ] Download `SHA256SUMS.txt` and at least one archive and check the hash
  with `sha256sum -c SHA256SUMS.txt --ignore-missing`. A mismatch means
  the release is withdrawn, not patched.
- [ ] The release notes are the generated notes, with the changelog section
  added when it is more complete.

## 6. Container image tags

The image `ghcr.io/warr-ev/teamcraft` is published by CI only.

- [ ] The workflow sets `:<version>`, `:<major>.<minor>` and, for a version
  without a suffix, `:latest` (also for `0.x`). For `v1.0.0` that is `1.0.0`,
  `1.0` and `latest`. There is no `:<major>` tag.
- [ ] For a suffixed candidate such as `v1.0.0-rc.1`, `:latest` must not move.
  Check the tags on the registry after the run and report any mismatch.
- [ ] The `:edge` tag comes from `main`; it is not a release.
- [ ] Pull the image by its release tag and check `/readyz` on a test
  instance. Do not build or push an image by hand.

## 7. Announcement

- [ ] Publish the GitHub Release (it is created by the workflow; check that it
  is not a draft).
- [ ] Update the install examples that name a version (`deploy/install.sh`
  help text and the Docker guide) through a pull request, if they still
  name an older release.
- [ ] Post the announcement with the link to the release and the changelog
  section. Say what changed for operators first: configuration, migrations
  and the upgrade path.
- [ ] Report security fixes in the release through the advisory that was
  prepared under [SECURITY.md](https://github.com/WARR-eV/TeamCraft/blob/main/SECURITY.md),
  and publish it on the release day.

## Desktop update test

Run this for each release candidate and once more after the final tag, on
one computer per install type (#469). It checks the update notice and the
update window (#377) against the **published** release, so it starts from the
previous release installed the normal way. Releases before the fix of #493
(0.2.13 and older) never find an update; start from a later one.

The app offers a release candidate (`-rc.N`) only to an app that is a
candidate itself. To test a candidate, start from the previous candidate. A
first candidate is checked after its final tag.

**Before every install type**

- [ ] Install the previous release (or candidate) and sign in to a test
  server. *Settings → Updates → Tell me when a new version is out* is on.
- [ ] *Help → Check for Updates…* names the new version. The green pill
  *TeamCraft x.y.z is available* is in the status bar, and its tooltip names
  the installed version. Both pills (with the Ko-fi one) fit side by side
  when the window is narrow.
- [ ] A click on the pill opens the update window: release notes of the new
  version, *Update now* (or the package-manager hint), *Skip
  this version*, *Remind me later* and *Release page*.
- [ ] *Release page* opens `github.com/WARR-eV/TeamCraft/releases/tag/v<new
  version>` in the browser.
- [ ] Connected to a server of another minor version (0.x), the window also
  says *Your server runs … and needs TeamCraft Desktop …* (#468).

**Skip and remind**

- [ ] *Skip this version*: the pill goes and stays away after a restart.
  *Help → Check for Updates…* still offers the version (asked on purpose).
- [ ] *Remind me later* (after undoing the skip: delete `update_dismissed` in
  `settings.json` while the app is closed): the pill goes. After a restart
  it stays away – the reminder runs three days. *Help → Check for Updates…*
  ends the reminder and opens the update window with the pill.
- [ ] *×* on the pill is the same as *Skip this version*.

**Per install type**

| Install | Expected after *Update now* |
|---|---|
| Windows `.msi` (per machine, under *Program Files*) | the `.msi` lands in *Downloads*, *Verified: …* is shown, *Close TeamCraft and install?* → *Close and install* ends the app and starts the installer; after it, the app starts in the new version with the same sign-in, working folder and settings |
| Windows NSIS `-setup.exe` (per user, under `AppData\Local`) | as for the `.msi`, with the `-setup.exe` |
| Windows portable `.zip` | *Update now* → *Replace and restart?* → *Replace and restart*: the files in the app's folder are replaced, TeamCraft restarts in the new version and the `*.old` files are gone after that start; *Not now* changes nothing; a read-only folder gives an error and *Show the file* (#466). The zip holds `.teamcraft-portable`; a `TeamCraft.exe` copied alone into another folder offers no *Update now*, only the line to download from the release page and *Release page* (#533); after a good swap whose restart failed, the window opened again shows *Restart now* / *Close TeamCraft* |
| macOS `.dmg` | *Update now*: the `.dmg` opens; drag TeamCraft to *Applications* and replace the old one |
| Linux AppImage | *Update now* → *Replace and restart*: the AppImage file is replaced (still executable) and TeamCraft restarts in the new version; a read-only folder gives an error and *Show the file* (#466) |
| Flatpak | no download: the window names `flatpak update` |
| `.deb` / `.rpm` | no download: the window names the package manager |

For each row also check:

- [ ] *Not now* in *Close TeamCraft and install?* leaves the app running.
- [ ] During a check-in or a *Get*, *Update now* refuses with *A check-in or
  download is running – update when it has finished*.
- [ ] *Help → About* shows the new version after the update. The status bar
  pill is gone.

**Checksum mismatch.** The app checks the downloaded file against
`SHA256SUMS.txt` of the same release. A file that does not match is deleted
and nothing changes. This cannot be provoked against a real release without
changing a published asset – never do that. It is covered by the unit tests
in `desktop/src/update.rs` (`verify_digest`, the checksum list parser).
Manually, check only that a broken network during the download ends in an
error message with *Report…* and leaves no partial file in *Downloads*.

**Policies.** On one Windows computer with the ADMX template
([Deploying the desktop app](../admin/desktop-deployment.md)):
`CheckForUpdates = 0` disables the switch (*Set by your organization*), shows
no pill and refuses *Help → Check for Updates…*.

Write down what failed in the release notes or as an issue before the final
tag.

## What is not part of a release

- No package registries. Nothing is published to crates.io, npm or a similar
  registry.
- No code signing yet. Installers are not signed (see
  [below](#how-releases-are-built)); the release notes say so.
- No backport branch unless the support policy requires one.

## How releases are built

`main` is the development branch. Version numbers follow
[Semantic Versioning](https://semver.org); while the version is `0.x`, any
release may contain breaking changes.

**Paused for the next releases** (since 2026-11-22, to make releases faster):
the job `local-server` is switched off (`if: false && …`) and the `server`
job builds only Linux – the Windows x64 server and the macOS sidecar were
needed only by the installers *with local server*. To bring them back: drop
`false &&`, uncomment the two server matrix rows and add `local-server` to
the `needs` of `publish` (the top of `release.yml` says the same). The
description below is the full build.

**Build artifacts.** `.github/workflows/release.yml` builds the server
binaries (Linux x86_64/ARM64, Windows x64) and the desktop installers
(AppImage, `.deb`, `.rpm` and Flatpak bundles for x86_64/aarch64, `.dmg`,
and for Windows x64 and ARM64 `.msi`, NSIS `-setup.exe` and a portable
`.zip`), and the desktop installers *with local server* (job `local-server`:
the app plus the server as `teamcraft-server` and PostgreSQL's programs from
theseus-rs/postgresql-binaries, checksum-checked, for Windows x64, macOS
Apple silicon and Linux x86_64; each smoke-tested with its bundled server –
it takes the server binaries of the `server` job, for macOS an unreleased
`sidecar-macos-aarch64` artifact; the Tauri CLI comes prebuilt through
`taiki-e/install-action`). The artifacts are kept for 14 days. The workflow
builds `main` every night (01:07 UTC) when `main` has changed outside the
documentation since its last successful build – not on every push, where
each push cancelled the build of the one before. *Run workflow* builds any
branch at once. A pull request that changes the workflow, `packaging/` or
`desktop/tauri.conf.json` (or `desktop/tauri.local-server.conf.json`) builds
everything before it is merged.

**Container image.** The same workflow builds `ghcr.io/warr-ev/teamcraft`
from the `Dockerfile` on an amd64 and an arm64 runner and publishes it:
`:edge` from `main` (nightly, or *Run workflow* on `main`), `:<version>`,
`:<major>.<minor>` and `:latest` for a tag (`latest` also for 0.x, not for
suffixed versions such as `-rc.1`). A pull request that changes the
`Dockerfile` or `deploy/` builds the image without publishing it. Before
anything is published, a smoke test runs `deploy/install.sh` with the fresh
image against PostgreSQL and checks `/readyz`, an administrator, the health
check, an upgrade run and the uninstall. Images are built by CI only.
`deploy/install.sh` is checked with `bash -n` and `shellcheck` in CI.

**Cutting a release** (the short form of the list above):

1. Set the same version in `Cargo.toml`, `desktop/Cargo.toml` and
   `desktop/tauri.conf.json` (the workflow refuses a mismatch), run
   `cargo check` and `cargo check --manifest-path desktop/Cargo.toml` so both
   `Cargo.lock` files follow, and merge it.
2. Tag the merge commit and push: `git tag -a v0.2.0 -m "TeamCraft 0.2.0"`
   and `git push origin v0.2.0`.
3. The workflow builds everything again from the tag, writes
   `SHA256SUMS.txt` and creates the GitHub Release with generated notes – as a
   **pre-release** for `0.x` and suffixed versions (`1.0.0-rc.1`). The desktop
   app's *Update now* (#377) picks its file by these names
   (`TeamCraft_<version>_x64_en-US.msi`, `_x64-setup.exe`, `_x64_portable.zip`,
   `_aarch64.dmg`, `_amd64.AppImage`, `TeamCraft-with-local-server_…`) and
   checks it against `SHA256SUMS.txt` – rename an asset only together with
   `desktop/src/update.rs` `pick_asset`.

The installers are not code-signed yet (macOS: ad-hoc signature, Windows:
unsigned); signing needs certificates as repository secrets and is a
follow-up. Nothing else is published (no package registries).
