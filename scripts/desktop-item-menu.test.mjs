// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/item-menu-rules.js (the right-click menu of an
// item, #417): the entries follow the revision toolbar's abilities, and a
// disabled entry says why. Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-item-menu.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/item-menu-rules.js");
const src = fs.readFileSync(file, "utf8");
const { itemMenuEntries } = await import("data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }
const byId = (entries) => Object.fromEntries(entries.filter(Boolean).map((e) => [e.id, e]));
const ids = (entries) => entries.map((e) => (e ? e.id : "|")).join(" ");

const free = { checkout: true, checkin: false };
const why = { checkout: "Checked out by Mia", checkin: "Check out first" };

test("a free revision in work: check out and open first, then check out", () => {
  const e = itemMenuEntries({ number: "PRT-000001", can: free, why, mine: false, cadFile: "Bracket.ipt" });
  assert.equal(ids(e), "checkout-open checkout get | folder copy | whereused request browser");
  const m = byId(e);
  assert.equal(m["checkout-open"].off, false);
  assert.match(m["checkout-open"].title, /Bracket\.ipt/);
  assert.equal(m.checkout.off, false);
  assert.match(m.copy.title, /PRT-000001/);
});

test("locked by someone else or released: check-out entries off with the toolbar's reason", () => {
  const e = byId(itemMenuEntries({ number: "PRT-000001", can: { checkout: false }, why, mine: false, cadFile: "Bracket.ipt" }));
  assert.equal(e["checkout-open"].off, true);
  assert.equal(e["checkout-open"].title, "Checked out by Mia");
  assert.equal(e.checkout.off, true);
  assert.equal(e.checkout.title, "Checked out by Mia");
  // reading stays possible
  assert.equal(e.get.off, false);
  assert.equal(e.whereused.off, false);
  assert.equal(e.request.off, false);
});

test("no native CAD file: check out and open is off and says so, plain check out stays", () => {
  const e = byId(itemMenuEntries({ number: "DOC-000001", can: free, why, mine: false, cadFile: null }));
  assert.equal(e["checkout-open"].off, true);
  assert.match(e["checkout-open"].title, /no native CAD file/);
  assert.equal(e.checkout.off, false);
});

test("my own check-out: open in CAD and check in instead of the check-out entries", () => {
  const e = itemMenuEntries({ number: "ASM-000002", can: { checkout: false, checkin: true }, why, mine: true, cadFile: "Frame.iam" });
  assert.equal(ids(e), "open-cad checkin get | folder copy | whereused request browser");
  const m = byId(e);
  assert.equal(m["open-cad"].off, false);
  assert.match(m["open-cad"].title, /Frame\.iam/);
  assert.equal(m.checkin.off, false);
  const noCad = byId(itemMenuEntries({ number: "ASM-000002", can: { checkin: true }, why, mine: true, cadFile: null }));
  assert.equal(noCad["open-cad"].off, true);
});

test("every entry has a text and a tooltip", () => {
  for (const mine of [false, true]) {
    for (const e of itemMenuEntries({ number: "X-1", can: {}, why, mine, cadFile: null }).filter(Boolean)) {
      assert.ok(e.text && e.title, JSON.stringify(e));
    }
  }
});

console.log(n + " tests passed");
