# shared/ – UI files of both the web UI and TeamCraft Desktop

Everything in this folder is used by **both** user interfaces: the
server-rendered web UI and the desktop app (Tauri). The design behind it is the
research page *Sharing UI code between the web UI and the desktop app*
([`docs/src/research/shared-ui.md`](../docs/src/research/shared-ui.md)): one CSS, one markup contract, two renderers.

```text
css/skins.css      design tokens and skins (colours, radii, spacing, density, fonts)
css/assistant.css  the "Ask TeamCraft" panel (a drawer on the web, inline in the desktop's Agent tab)
css/components.css small components of both apps (the help tip "?" next to a term, #414; the colour of selected text;
                   `[hidden] { display: none !important }` – the hidden attribute always wins over a component's display, #515)
fonts/             Montserrat, Oswald (variable WOFF2) and their SIL OFL texts
js/viewer.js       3D preview (WebGL2, TCMESH01), still a classic script
js/assistant.js    "Ask TeamCraft" sidebar: ES module, mount(host, {api, getContext, link, …}) – no fetch, no invoke
js/format.js       label, fmtNum, fmtFixed, fmtSize, fmtAge, fmtDate – the Rust filters' rules
js/states.js       glyph(state): the state glyph table (domain::state_glyph on the server)
js/markdown.js     markdown(text, link): the assistant's markdown-lite as DOM nodes
js/dom.js          el(tag, attrs, children): text-only DOM builder of the shared modules
js/i18n.js         t(id, params), negotiate, loadLanguage: the message catalogs (src/i18n.rs is the Rust twin, #379)
i18n/              en.json (the source) and de.json: message ID → sentence with {named} placeholders or plural forms
test-vectors/      JSON input → expected output, run against Rust and JS (not served, not copied)
```

## How the apps get these files

* **Server:** `build.rs` walks this folder and writes the asset table into
  `OUT_DIR`; `src/assets.rs` embeds every file and serves it at
  `/static/shared/<path>`. Stylesheets are linked with `?v=<assets version>`,
  scripts through the versioned path `/static/shared/v/<assets version>/js/…`
  (relative module imports keep the version). No list to maintain by hand.
* **Desktop:** `desktop/build.rs` mirrors this folder into
  `desktop/ui/static/shared/` (git-ignored), so the app loads the same URLs.
* Not served and not copied: this README, `*.test.js`, `test-vectors/` and
  dotfiles.
* Guard tests: `assets::tests::shared_files_are_served` (server) and
  `shared_ui_files_are_mirrored` (`cargo test --manifest-path
  desktop/Cargo.toml`) check that both apps carry exactly these files.

## Rules

1. **Licence header.** Every `.css`, `.js` and `.svg` starts with
   `/* SPDX-License-Identifier: MPL-2.0 */` (SVG: `<!-- … -->`), because the
   files end up in another program's bundle. The fonts keep their OFL texts
   next to them. Checked by `assets::tests::shared_files_carry_spdx_header`.
2. **Plain files, no build step.** No bundler, transpiler or Node.js
   packages; new scripts are plain ES modules. No inline scripts or styles,
   no `innerHTML` with data, no `eval` – both Content Security Policies
   (`src/lib.rs`, `desktop/tauri.conf.json`) enforce it.
3. **No data access.** Shared code never calls `fetch` or Tauri `invoke`; it
   takes data and callbacks (`TeamCraftViewer.mount(canvas, { load })`).
4. **Tokens, not literals.** Colours exist only in `css/skins.css`; component
   styles read the tokens. Density is a token set too: the web UI uses the
   defaults (14/20 px text, 32 px controls and rows), the desktop app sets
   `data-density="compact"` on `<html>` (13/18 px text, 28 px controls and
   rows, 24 px small controls): `--fs-body`, `--lh-body`, `--ctl-h`,
   `--ctl-h-sm`, `--row-h`.
5. **Tolerant of version skew.** The desktop app ships its own copy and talks
   to servers of other versions: unknown values get a generic fallback, never
   an exception.
6. **Syntax check.** CI (`.github/workflows/desktop.yml`, job `ui`) parses
   every script here with `node --input-type=module --check`; Node.js is used
   by CI only.

## Keeping the two apps in step

Each rule below has a guard that fails CI. The failure message names the rule.

7. **One source per rule.** A rule that can live here lives *only* here. App
   code imports it and never writes a second copy. Desktop modules import with
   the URL both apps serve, `import { label } from "/static/shared/js/format.js";`.
   Shared modules import their neighbours relatively (`"./dom.js"`).
   *Guard:* `scripts/check-desktop-modules.mjs` fails when a desktop module
   or `static/app.js` declares a name that a shared module exports.
8. **Rust and JS side by side, pinned to one set of vectors.** Some rules
   exist twice because the server renders without JavaScript: labels,
   numbers, sizes, ages, state glyphs and markdown-lite. Both versions run
   against the same `test-vectors/*.json`: Rust in `src/ui_vectors.rs` (in
   `cargo test --lib`), JS in `scripts/shared-vectors.test.mjs`. To change a
   rule, change the vector first; then both sides fail until both are
   fixed.
9. **Shared CSS owns its classes.** A class that starts a rule in
   `css/*.css` must not start a rule in `static/style.css` or
   `desktop/ui/app.css`. An app may still adjust it in its own context
   (`.toolbar .badge`). *Guard:* `assets::tests::shared_css_classes_are_not_redefined`.
10. **No new component written twice.** A class styled by both app
    stylesheets is a component written twice. Today's ones are listed in
    `STYLED_TWICE` (`src/assets.rs`). That list may only shrink: a new class
    for both apps goes into `css/` from the start, and a class that moves here
    leaves the list. The test fails in both directions.
    *Guard:* `assets::tests::no_new_component_styled_in_both_apps`.
11. **Colours only as tokens.** Stylesheets here, apart from `skins.css`,
    contain no literal colours. *Guard:*
    `assets::tests::shared_components_use_no_literal_colours`, plus
    `css_tokens_are_defined` for every `var(--x)`.

Where a new piece of UI goes, and how to add a vector, is described in
`docs/src/dev/shared-ui.md`.

## The assistant sidebar (`js/assistant.js`, `css/assistant.css`)

The first shared *component*: one module, one stylesheet, two shells.

* **Contract.** The shell provides an empty `<aside>` and the callbacks; the
  module builds the drawer inside it (context chip, quick actions, the
  conversation, the composer). The shell owns the icon that opens it, the
  place of the `<aside>` (web: fixed on the right below the top bar) and every
  data access: `api.thread()`, `api.send(body, signal)`, `api.reset()`,
  `getContext()`. **Inline mode** (`mount(host, { inline: true, title })`):
  the same panel fills its host instead of being a drawer – always open, no
  resize handle, no close button, no Esc. The desktop app uses it for the
  company AI inside its *Agent* tab; `onBusy(busy)` and `isBusy()` let the
  shell keep its own controls still while a question runs.
* **Page context.** The page (web: `data-tc-*` attributes on `<main>`;
  desktop: the last revision, change or request tab) declares a plain
  object – `kind`, `item`, `revision`, `change`, `request`, `project`, `view`, `rule`, `query`,
  `filters`, `selected`, `shown` – described in
  `docs/src/guide/assistant.md` (API section). It is only a hint; the server
  resolves it with the user's rights.
* **State.** Open/closed and the width are kept in `localStorage`
  (`tc.assistant.open`, `tc.assistant.width`), always inside `try/catch`.
  Inline mode stores nothing and leaves `<html>` alone (no
  `tc-assistant-open`, no `--tc-assistant-w`).
* **Answers** are markdown-lite built as DOM nodes (`markdown(text, link)`),
  never as HTML.
