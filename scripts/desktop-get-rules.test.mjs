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
const { addDownload, groupsGetResult, incompleteGet, mergeGotGroups, notOpenedText, rememberGroups } = await import(
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
  const again = groupsGetResult(tally({ items: 1, files: 0, kept: 1, skipped: 2 }), false, ["A", "B"]);
  assert.equal(again.title, "Done: A, B are in your working folder");
  assert.deepEqual(again.lines, ["1 item got, 0 files copied (everything was up to date)", "1 locally modified file kept",
    "2 items without a released revision skipped"]);
  // a failed item is a problem, listed in full
  const bad = groupsGetResult(tally({ items: 3, files: 3, errors: ["PRT-1: denied", "PRT-2: gone"] }), false, ["DRV"]);
  assert.equal(bad.outcome, "problems");
  assert.equal(bad.title, "Finished with problems: DRV");
  assert.deepEqual(bad.problems, ["PRT-1: denied", "PRT-2: gone"]);
  // stopped wins over problems
  assert.equal(groupsGetResult(tally({ errors: ["x"] }), true, ["DRV"]).outcome, "stopped");
});

const tally = (more) => Object.assign({ items: 0, files: 0, kept: 0, skipped: 0, clashes: 0, errors: [] }, more);

test("'everything was up to date' only when the items got were unchanged (#449)", () => {
  // every download failed (server offline)
  const failed = groupsGetResult(tally({ errors: ["PRT-1: offline", "PRT-2: offline"] }), false, ["DRV"]);
  assert.equal(failed.outcome, "problems");
  assert.deepEqual(failed.lines, ["0 items got, 0 files copied"]);
  // some items up to date, others failed
  assert.deepEqual(groupsGetResult(tally({ items: 2, errors: ["PRT-3: denied"] }), false, ["DRV"]).lines,
    ["2 items got, 0 files copied"]);
  // stopped before the first download
  const stopped = groupsGetResult(tally(), true, ["A", "B"]);
  assert.equal(stopped.title, "Stopped – the working folder has only part of A, B");
  assert.deepEqual(stopped.lines, ["0 items got, 0 files copied"]);
  // stopped after unchanged items: still not "up to date"
  assert.deepEqual(groupsGetResult(tally({ items: 3 }), true, ["A"]).lines, ["3 items got, 0 files copied"]);
});

test("every item skipped: nothing to get, not 'done' with up to date (#449)", () => {
  const r = groupsGetResult(tally({ skipped: 4 }), false, ["DRV"]);
  assert.equal(r.outcome, "done");
  assert.equal(r.title, "Nothing to get: no item in DRV has a released revision");
  assert.deepEqual(r.lines, ["0 items got, 0 files copied", "4 items without a released revision skipped"]);
});

test("a name clash is listed once, with the problems (#449)", () => {
  const done = tally({ items: 2, files: 1 });
  addDownload(done, "PRT-1", { downloaded: ["x.ipt"], kept: [], failed: [], clashes: [] });
  addDownload(done, "PRT-2", { downloaded: [], kept: [], failed: [], clashes: [{ path: "PRT-2", file: "x.ipt", with: "PRT-1" }] });
  const r = groupsGetResult(done, false, ["A", "B"]);
  assert.equal(r.outcome, "problems");
  assert.equal(r.title, "Finished with problems: A, B");
  assert.deepEqual(r.problems, ["PRT-2: x.ipt – taken by PRT-1"]);
  assert.deepEqual(r.lines, ["4 items got, 2 files copied", "The name clash is listed below – nothing was overwritten"]);
  assert.equal(r.lines.some((l) => /1 name clash/.test(l)), false, "the clash is not counted again");
  addDownload(done, "PRT-3", { downloaded: [], kept: [], failed: [], clashes: [{ path: "PRT-3", file: "x.ipt", with: "PRT-1" }] });
  assert.equal(groupsGetResult(done, false, ["A", "B"]).lines[1], "The name clashes are listed below – nothing was overwritten");
});

test("the result tolerates missing codes and errors, and copies the problems", () => {
  assert.equal(groupsGetResult(tally({ items: 1, files: 1 }), false, null).outcome, "done");
  assert.equal(groupsGetResult(tally({ items: 1, files: 1 }), false, undefined).outcome, "done");
  const noErrors = { items: 1, files: 1, kept: 0, skipped: 0, clashes: 0 };
  assert.deepEqual(groupsGetResult(noErrors, false, ["A"]).problems, []);
  const done = tally({ errors: ["a"] });
  const r = groupsGetResult(done, false, ["A"]);
  r.problems.push("b");
  assert.deepEqual(done.errors, ["a"], "changing the result leaves the tally alone");
});

test("addDownload counts a download and names its problems", () => {
  const done = tally();
  const cancelled = addDownload(done, "PRT-7", {
    downloaded: ["a.ipt", "b.ipt"], kept: ["c.ipt"], failed: ["d.ipt: timeout"],
    clashes: [{ path: "DRV/x", file: "e.ipt", with: "PRT-8" }], cancelled: false
  });
  assert.equal(cancelled, false);
  assert.deepEqual(done, { items: 1, files: 2, kept: 1, skipped: 0, clashes: 1,
    errors: ["PRT-7: d.ipt: timeout", "DRV/x: e.ipt – taken by PRT-8"] });
  assert.equal(addDownload(done, "PRT-9", { downloaded: [], kept: [], failed: [], clashes: [], cancelled: true }), true);
  assert.equal(done.items, 2);
  // missing lists count as empty
  assert.equal(addDownload(done, "PRT-10", {}), false);
  assert.equal(done.items, 3);
});

test("only a complete 'latest' Get remembers its groups", () => {
  assert.equal(rememberGroups(tally({ items: 3, files: 3 }), false, "latest"), true);
  assert.equal(rememberGroups(tally({ items: 3 }), true, "latest"), false, "stopped");
  assert.equal(rememberGroups(tally({ errors: ["x"] }), false, "latest"), false, "a failed item");
  assert.equal(rememberGroups(tally({ clashes: 1 }), false, "latest"), false, "a name clash");
  assert.equal(rememberGroups(tally({ items: 3 }), false, "released"), false, "released revisions only");
});

test("the got groups merge without duplicates, strings only, the last 200", () => {
  assert.deepEqual(mergeGotGroups(["A", "B"], ["B", "C", "A"]), ["A", "B", "C"]);
  assert.deepEqual(mergeGotGroups(["A", 3, null, { x: 1 }], [7, "B"]), ["A", "B"]);
  assert.deepEqual(mergeGotGroups("A,B", ["C"]), ["C"], "a stored value that is no list counts as empty");
  assert.deepEqual(mergeGotGroups(null, null), []);
  const many = Array.from({ length: 250 }, (_, i) => "G" + i);
  const kept = mergeGotGroups(many, ["NEW"]);
  assert.equal(kept.length, 200);
  assert.equal(kept[0], "G51");
  assert.equal(kept[199], "NEW");
});

console.log(`${n} tests passed`);
