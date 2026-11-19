# Shared UI: working on the web UI and the desktop app together

TeamCraft has two user interfaces. The web UI renders server-side with
MiniJinja templates; the desktop app (Tauri) renders in the client from REST
JSON. They look and behave the same because of one rule: **one CSS, one
markup contract, two renderers**. Why it is built this way is recorded on
the research page [Sharing UI code between the web UI and the desktop
app](../research/shared-ui.md). This page covers the day-to-day work.

## Where does new UI code go?

| What you write                                                                      | Where                                                                                                                                                                                   |
|-------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| A colour, radius, spacing or density value                                          | a token in `shared/css/skins.css`                                                                                                                                                       |
| A component both apps show (badge, table, menu, panel …)                            | `shared/css/` plus its markup contract in `shared/README.md`; a MiniJinja macro (`templates/macros.html`) and a JS builder emit the same DOM                                             |
| A pure rule both apps need (a label, a number format, a glyph)                      | a function in `shared/js/` and, if the server renders it too, the Rust twin. Both are pinned to a vector in `shared/test-vectors/`                                                        |
| A widget that enhances markup (keyboard model, expand/collapse)                     | `shared/js/`. It takes elements and callbacks and never calls `fetch` or `invoke`                                                                                                       |
| Page layout, forms, sign-in, admin pages                                            | web only: `templates/`, `static/`                                                                                                                                                       |
| Shell, workspace, check-out, local files, Tauri commands                            | desktop only: `desktop/ui/`                                                                                                                                                             |

## What CI checks

| Check                                                                                                               | Fails when                                                                                       |
|---------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------|
| `scripts/check-desktop-modules.mjs`                                                                                 | a desktop module or `static/app.js` declares a name a `shared/js` module exports, or an import does not resolve |
| `scripts/shared-vectors.test.mjs` and `src/ui_vectors.rs`                                                           | the JS or the Rust side of a rule disagrees with `shared/test-vectors/*.json`                    |
| `assets::tests::shared_css_classes_are_not_redefined`                                                               | an app stylesheet starts a rule with a class that `shared/css/` styles                           |
| `assets::tests::no_new_component_styled_in_both_apps`                                                               | `static/style.css` and `desktop/ui/app.css` both style a class that `STYLED_TWICE` does not list, or a listed class is no longer styled twice |
| `assets::tests::shared_components_use_no_literal_colours`, `css_tokens_are_defined`                                 | a shared stylesheet uses a literal colour or an undeclared token                                 |
| `assets::tests::shared_files_are_served`, `shared_ui_files_are_mirrored`, `shared_files_carry_spdx_header`          | the server, the desktop copy or the licence header is out of step with `shared/`                |

Run them locally:

```sh
cargo test --lib -- ui_vectors assets::
node scripts/check-desktop-modules.mjs
node scripts/shared-vectors.test.mjs
```

## Changing a shared rule

1. Change or add the case in `shared/test-vectors/<rule>.json`.
2. Run both test commands above. Both fail until both sides follow.
3. Fix `shared/js/<module>.js` and the Rust twin (`templates.rs` filters,
   `util.rs`, `domain::state_glyph`, `assistant::render_markdown`).

Leave out cases whose answer depends on the platform, for example exact
rounding ties (`0.125` to two places): Rust and JavaScript round them
differently, and no user reads the difference.

## Moving a component into `shared/css/`

1. Move its rules from `static/style.css` and `desktop/ui/app.css` into a
   stylesheet in `shared/css/` (with the SPDX header), and link that
   stylesheet in `templates/base.html` and `desktop/ui/index.html`.
2. Remove its classes from `STYLED_TWICE` in `src/assets.rs`.
3. Write its markup contract into `shared/README.md`, and make the macro
   and the desktop builder emit exactly that markup.
4. Put screenshots of both apps, before and after, into the pull request.
