// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/home-rules.js (the pinned Home tab – #513 T1,
// T9: recently opened objects, the worklist as one line of counts).
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-home-rules.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/home-rules.js");
const src = fs.readFileSync(file, "utf8");
const { rememberRecent, validRecent, worklistSummary, RECENT_MAX } = await import(
  "data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

const rev = (number, code) => ({ key: "rev:" + number + ":" + code, kind: "revision", number, code, title: number + "/" + code });

test("an opened revision goes first, its older entry is dropped", () => {
  let list = rememberRecent([], rev("PRT-000001", "A"));
  list = rememberRecent(list, { key: "chg:CHG-000002", kind: "change", number: "CHG-000002", title: "CHG-000002" });
  list = rememberRecent(list, rev("PRT-000001", "A"));
  assert.deepEqual(list.map((e) => e.key), ["rev:PRT-000001:A", "chg:CHG-000002"]);
  assert.equal(list[1].code, null, "a change has no revision code");
});

test("two revisions of one item are two entries", () => {
  const list = rememberRecent(rememberRecent([], rev("PRT-000001", "A")), rev("PRT-000001", "B"));
  assert.deepEqual(list.map((e) => e.key), ["rev:PRT-000001:B", "rev:PRT-000001:A"]);
});

test("the list keeps at most RECENT_MAX entries", () => {
  let list = [];
  for (let i = 0; i < RECENT_MAX + 3; i++) list = rememberRecent(list, rev("PRT-" + String(i).padStart(6, "0"), "A"));
  assert.equal(list.length, RECENT_MAX);
  assert.equal(list[0].number, "PRT-" + String(RECENT_MAX + 2).padStart(6, "0"));
});

test("the Agent and Check-outs tabs are not remembered", () => {
  const start = [rev("PRT-000001", "A")];
  assert.deepEqual(rememberRecent(start, { key: "agent", kind: "agent", title: "Agent" }), start);
  assert.deepEqual(rememberRecent(start, { key: "locks:ROVER", kind: "locks", code: "ROVER", title: "Check-outs ROVER" }), start);
});

test("broken entries read back from storage are dropped", () => {
  assert.equal(validRecent(null), false);
  assert.equal(validRecent({ key: "rev:X:A", kind: "revision", number: "X", title: "X/A" }), false, "no code");
  assert.equal(validRecent({ key: "x", kind: "agent", number: "x", title: "x" }), false);
  const list = rememberRecent([{ bad: true }, "text", rev("PRT-000001", "A")], null);
  assert.deepEqual(list.map((e) => e.key), ["rev:PRT-000001:A"]);
  assert.deepEqual(rememberRecent("not a list", null), []);
});

test("the worklist summary: own check-outs first, empty sections left out", () => {
  const wl = { checkouts: [{}], reviews: [{}, {}], impact_groups: [], requests_assigned: [{}] };
  assert.deepEqual(worklistSummary(wl).map((p) => p.text),
    ["1 check-out", "2 reviews waiting", "1 request assigned to you"]);
  assert.deepEqual(worklistSummary({ checkouts: [], reviews: [] }), []);
  assert.deepEqual(worklistSummary(null), []);
  assert.deepEqual(worklistSummary({ impact_groups: [{}, {}] }).map((p) => p.text), ["2 dependents to check"]);
});

console.log(`${n} tests passed`);
