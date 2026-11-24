// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/get-rules.js (whether a Get left the working
// folder complete enough to open the main CAD file – #348).
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-get-rules.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/get-rules.js");
const src = fs.readFileSync(file, "utf8");
const { groupsGetResult, incompleteGet, notOpenedText } = await import(
  "data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

const done = { cancelled: false, failed: [], kept: [], clashes: [], unresolved: [], missing_references: [] };

test("a complete Get opens", () => {
  assert.equal(incompleteGet(done), null);
});

test("a stopped Get does not open (#348)", () => {
  const why = incompleteGet({ ...done, cancelled: true, downloaded: ["a.iam"] });
  assert.match(why, /stopped/);
});

test("failed files do not open, counted", () => {
  assert.equal(incompleteGet({ ...done, failed: ["a.ipt: timeout"] }), "1 file could not be copied");
  assert.equal(incompleteGet({ ...done, failed: ["a", "b"] }), "2 files could not be copied");
});

test("the stop is named before failures", () => {
  assert.match(incompleteGet({ ...done, cancelled: true, failed: ["a"] }), /stopped/);
});

test("kept files, clashes and missing references still open", () => {
  assert.equal(incompleteGet({ ...done, kept: ["a.ipt"], clashes: [{ path: "x" }], missing_references: ["y.ipt"] }), null);
});

test("no result (an open without a Get) opens", () => {
  assert.equal(incompleteGet(null), null);
  assert.equal(incompleteGet(undefined), null);
  assert.equal(incompleteGet({}), null);
});

test("the note names the file and what to do", () => {
  const t = notOpenedText("Rover.iam", "1 file could not be copied");
  assert.equal(t, "Rover.iam was not opened: 1 file could not be copied. Get the files again, then open it.");
});

test("the end of Get project groups says plainly that it is done (#449)", () => {
  const tally = (more) => Object.assign({ items: 0, files: 0, kept: 0, skipped: 0, clashes: 0, errors: [] }, more);
  const ok = groupsGetResult(tally({ items: 12, files: 30 }), false, ["DRV"]);
  assert.equal(ok.outcome, "done");
  assert.equal(ok.title, "Done: DRV is in your working folder");
  assert.deepEqual(ok.lines, ["12 items got, 30 files copied"]);
  assert.deepEqual(ok.problems, []);
  // two groups, nothing new to copy, with the counts worth naming
  const again = groupsGetResult(tally({ items: 1, files: 0, kept: 1, skipped: 2, clashes: 1 }), false, ["A", "B"]);
  assert.equal(again.title, "Done: A, B are in your working folder");
  assert.deepEqual(again.lines, ["1 item got, 0 files copied (everything was up to date)", "1 locally modified file kept",
    "2 items without a released revision skipped", "1 name clash – nothing was overwritten"]);
  // a failed item is a problem, listed in full
  const bad = groupsGetResult(tally({ items: 3, files: 3, errors: ["PRT-1: denied", "PRT-2: gone"] }), false, ["DRV"]);
  assert.equal(bad.outcome, "problems");
  assert.equal(bad.title, "Finished with problems: DRV");
  assert.deepEqual(bad.problems, ["PRT-1: denied", "PRT-2: gone"]);
  // stopped wins over problems
  assert.equal(groupsGetResult(tally({ errors: ["x"] }), true, ["DRV"]).outcome, "stopped");
});

console.log(`${n} tests passed`);
