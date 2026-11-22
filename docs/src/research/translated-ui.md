# Translated UI: message catalogs for web and desktop

Issue #379: the web UI and the desktop app are English only. Many teams that
use TeamCraft work in German first. This page compares ways to keep message
catalogs that **both** apps read – the server's MiniJinja templates and Rust
code, and the plain ES modules of the desktop app and of `shared/` (no build
step, no Node.js at runtime) – and records the choice.

## Requirements (from the issue)

* one catalog per language, stable message IDs, English fallback, embedded
  in the binary and the desktop app (works offline);
* whole sentences with **named placeholders** (`{item}`, `{count}`) a
  translator can reorder; **plural forms** per language; inserted values are
  escaped and never read as placeholders;
* one source for web and desktop (`shared/`), so a term is never translated
  twice in two ways;
* guard tests: every ID used exists in English; every catalog has only known
  IDs and the same placeholders as English; tests run in English.

## Options

| Option | Format | Plurals | Rust | JavaScript | Notes |
|---|---|---|---|---|---|
| Project Fluent | `.ftl` | CLDR categories, selectors | `fluent-bundle` (Apache-2.0/MIT) | `@fluent/bundle` (Apache-2.0) – a dependency to vendor into `shared/` | expressive (selectors, terms), but two runtimes to keep in step, and a new syntax for translators |
| GNU gettext | `.po`/`.mo` | `Plural-Forms` C expression | `gettext-rs` (C library) or pure-Rust parsers | own `.po` parser needed | English text as key: a changed English sentence orphans translations; plural formula is code |
| ICU MessageFormat | strings in JSON | `{n, plural, one {…} other {…}}` | `icu_*` crates (Unicode-3.0) | `Intl.MessageFormat` not built in; libraries | powerful but heavy for both sides |
| **JSON catalogs with CLDR plural categories** | `shared/i18n/<lang>.json`: ID → text, or `{ "one": …, "other": … }` | category from `Intl.PluralRules` (JS) and a small table per language in Rust | `serde_json` (already a dependency), `include_str!` | `fetch` of the same file, `Intl.PluralRules`, `Intl.NumberFormat` (built into every WebView) | no new dependency; the same file is read by both apps; placeholders are `{name}` only |

## Choice

**JSON catalogs in `shared/i18n/`** (`en.json` is the source, `de.json` the
first translation):

* a message is a string, or an object of CLDR plural categories (`zero`,
  `one`, `two`, `few`, `many`, `other`; English and German use `one` and
  `other`) selected by the `count` parameter;
* placeholders are `{name}` (letters, digits, `_`); values are inserted as
  text after the lookup, so a value that contains `{x}` is never expanded;
  the web templates escape them like every other value, the desktop inserts
  them with `textContent`;
* a missing message falls back to English, never to the ID; an unknown ID in
  English shows the ID (and the guard test fails before that ships);
* the language: a saved choice, else the browser's `Accept-Language` / the
  OS locale, matched exactly, then by base language, then English;
* dates and numbers through `Intl` (desktop) and the language's decimal
  separator (server), units unchanged.

Rust plural rules are a table for the languages TeamCraft ships (English,
German); a new language adds its CLDR rule there and a test vector in
`shared/test-vectors/`, so the Rust and JS twins stay pinned like the other
shared rules.

Fluent stays the upgrade path if a language needs grammatical cases or
gender selectors: the JSON IDs map one to one to Fluent message IDs.

## Sources

* Unicode CLDR, *Language Plural Rules* (public specification).
* MDN, `Intl.PluralRules`, `Intl.NumberFormat`, `Intl.DateTimeFormat`.
* RFC 9110 §12.5.4 `Accept-Language`; RFC 4647 *Matching of Language Tags*
  (lookup by base language).
* Project Fluent syntax guide; GNU gettext manual, *Plural forms*.
