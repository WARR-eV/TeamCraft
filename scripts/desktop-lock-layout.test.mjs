// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/lock-layout.js (the check-out graph of a
// project group, #428): the same layout as the server's
// lock_overview::graph (src/services/lock_overview.rs, its tests pin the
// same numbers). Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-lock-layout.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/lock-layout.js");
const src = fs.readFileSync(file, "utf8");
const L = await import("data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }
const now = Date.parse("2026-11-22T12:00:00Z");
const ago = (hours) => new Date(now - hours * 3600000).toISOString();
const lock = (number, hours) => ({ item_number: number, code: "A", name: "Bracket", item_type: "part", checked_out_at: ago(hours) });

test("each holder is centred on its revisions, one row per revision", () => {
  const g = L.lockGraph({ holders: [
    { name: "Erika", active: true, locks: [lock("PRT-1", 1), lock("PRT-2", 30), lock("PRT-3", 24 * 9)] },
    { name: "Gone", active: false, locks: [lock("PRT-4", 2)] }
  ] }, now);
  assert.deepEqual(g.locks.map((l) => l.y), [L.PAD, L.PAD + L.ROW, L.PAD + 2 * L.ROW, L.PAD + 3 * L.ROW]);
  assert.equal(g.holders[0].y, L.PAD + L.ROW);
  assert.equal(g.holders[0].count, 3);
  assert.equal(g.holders[1].y, L.PAD + 3 * L.ROW);
  assert.equal(g.holders[1].active, false);
  assert.deepEqual(g.locks.map((l) => l.age), ["today", "week", "old", "today"]);
  assert.ok(g.locks[0].edge.startsWith("M" + (L.HOLDER_X + L.HOLDER_W) + ","));
  assert.ok(g.locks[0].edge.endsWith(" " + L.LOCK_X + "," + (L.PAD + L.NODE_H / 2)));
  assert.equal(g.height, 2 * L.PAD + 3 * L.ROW + L.NODE_H);
  assert.equal(g.width, L.LOCK_X + L.LOCK_W + L.HOLDER_X);
  assert.equal(g.locks[0].label, "PRT-1/A Bracket");
  assert.equal(g.locks[0].number, "PRT-1");
});

test("the same sizes as the server's layout", () => {
  // src/services/lock_overview.rs: ROW 34, PAD 12, NODE_H 26, HOLDER_X 8,
  // HOLDER_W 190, LOCK_X 300, LOCK_W 330, LABEL_MAX 40
  assert.deepEqual([L.ROW, L.PAD, L.NODE_H, L.HOLDER_X, L.HOLDER_W, L.LOCK_X, L.LOCK_W, L.LABEL_MAX],
    [34, 12, 26, 8, 190, 300, 330, 40]);
  const rs = fs.readFileSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../src/services/lock_overview.rs"), "utf8");
  for (const [name, value] of [["ROW", 34], ["PAD", 12], ["NODE_H", 26], ["HOLDER_X", 8], ["HOLDER_W", 190],
                                ["LOCK_X", 300], ["LOCK_W", 330]]) {
    assert.match(rs, new RegExp("pub const " + name + ": u32 = " + value + ";"), name + " differs from the server");
  }
  assert.match(rs, /pub const LABEL_MAX: usize = 40;/);
});

test("an empty group, long labels, age limits", () => {
  const g = L.lockGraph({ holders: [] }, now);
  assert.deepEqual([g.height, g.holders.length, g.locks.length], [0, 0, 0]);
  const cut = L.cutLabel("x".repeat(60));
  assert.equal(Array.from(cut).length, L.LABEL_MAX);
  assert.ok(cut.endsWith("…"));
  assert.equal(L.cutLabel("Bracket"), "Bracket");
  assert.equal(L.ageClass(now - 23.9 * 3600000, now), "today");
  assert.equal(L.ageClass(now - 24 * 3600000, now), "week");
  assert.equal(L.ageClass(now - 24 * 7 * 3600000, now), "old");
});

console.log(n + " tests passed");
