// SPDX-License-Identifier: MPL-2.0
// Unit tests of shared/js/i18n.js (#379) with the real catalogs of
// shared/i18n/. Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/shared-i18n.test.mjs
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = fs.readFileSync(path.join(root, "shared/js/i18n.js"), "utf8");
const { fill, negotiate, supported, t, useCatalogs } =
  await import("data:text/javascript;base64," + Buffer.from(source).toString("base64"));
const catalog = (lang) => JSON.parse(fs.readFileSync(path.join(root, "shared/i18n", lang + ".json"), "utf8"));

let passed = 0;
function test(name, fn) { fn(); passed++; console.log("ok - " + name); }

test("languages are matched exactly, then by base, then English", () => {
  assert.equal(supported("de-AT"), "de");
  assert.equal(negotiate(["fr-FR", "de-CH", "en"]), "de");
  assert.equal(negotiate(["fr"]), "en");
  assert.equal(negotiate([]), "en");
});

test("messages fall back to English, then to the ID; plurals by count", () => {
  useCatalogs({ en: catalog("en"), de: catalog("de") }, "de");
  assert.equal(t("nav.items"), "Artikel");
  assert.equal(t("nav.notifications.unread", { count: 1 }), "1 ungelesene Benachrichtigung");
  assert.equal(t("nav.notifications.unread", { count: 4 }), "4 ungelesene Benachrichtigungen");
  assert.equal(t("no.such.message"), "no.such.message");
  useCatalogs({ en: catalog("en"), de: {} }, "de");
  assert.equal(t("nav.items"), "Items", "English fallback");
});

test("inserted values are never expanded", () => {
  assert.equal(fill("x {a} {b} {c}", { a: "{b}", b: "B" }), "x {b} B {c}");
});

test("the same answers as the Rust twin (src/i18n.rs)", () => {
  // the vectors of src/i18n.rs: negotiation and plural forms
  assert.equal(negotiate(["de-DE", "de", "en"]), "de");
  useCatalogs({ en: catalog("en"), de: catalog("de") }, "en");
  assert.equal(t("nav.notifications.unread", { count: 3 }), "3 unread notifications");
});

test("every ID the desktop scripts use exists in English, every catalog only known IDs", () => {
  const en = catalog("en");
  const dir = path.join(root, "desktop/ui/js");
  let used = 0;
  for (const name of fs.readdirSync(dir).filter((n) => n.endsWith(".js"))) {
    const text = fs.readFileSync(path.join(dir, name), "utf8");
    for (const m of text.matchAll(/(?<![A-Za-z0-9_.$])t\("([A-Za-z0-9_]+\.[A-Za-z0-9_.]+)"/g)) {
      used++;
      assert.ok(Object.hasOwn(en, m[1]), name + ": " + m[1] + " is not in shared/i18n/en.json");
    }
  }
  assert.ok(used > 0, "the desktop scripts use t(\"…\")");
  for (const id of Object.keys(catalog("de"))) assert.ok(Object.hasOwn(en, id), "de: " + id + " is not in en.json");
});

console.log(passed + " tests passed");
