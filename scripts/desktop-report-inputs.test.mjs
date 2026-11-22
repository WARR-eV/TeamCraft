// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/report-inputs.js (the key inputs of a failed
// action that *Report…* puts into the problem report, #381).
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-report-inputs.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const file = path.resolve(here, "../desktop/ui/js/report-inputs.js");
const src = fs.readFileSync(file, "utf8");
const { reportInputs, revisionInputs, failureOf, MAX_REPORT_INPUTS, MAX_REPORT_INPUT } = await import(
  "data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

test("the inputs of run() reach the failure of the report dialog", () => {
  const f = failureOf("Checking in PRT-000042", "file is locked", [["item", "PRT-000042"], ["file", "bracket.ipt"]]);
  assert.deepEqual(f, {
    action: "Checking in PRT-000042",
    error: "file is locked",
    inputs: [["item", "PRT-000042"], ["file", "bracket.ipt"]]
  });
});
test("no inputs give an empty list, not undefined", () => {
  assert.deepEqual(failureOf("Saving", "x").inputs, []);
  assert.deepEqual(failureOf(undefined, undefined, null), { action: "", error: "", inputs: [] });
  assert.deepEqual(reportInputs("item=1"), []);
});
test("empty names and values are left out, others become trimmed strings", () => {
  assert.deepEqual(reportInputs([["item", ""], ["revision", null], ["", "x"], [" group ", " ROVER "], ["count", 3], ["bad"]]),
    [["group", "ROVER"], ["count", "3"]]);
});
test("paths are reduced to the file name", () => {
  assert.deepEqual(reportInputs([
    ["file", "C:\\Users\\max\\work\\bracket.ipt"],
    ["target", "/home/max/vault/pump.step"],
    ["folder", "~/work/rover/"],
    ["note", "a/b is fine"]
  ]), [["file", "bracket.ipt"], ["target", "pump.step"], ["folder", "rover"], ["note", "a/b is fine"]]);
});
test("long values are cut and at most MAX_REPORT_INPUTS are kept", () => {
  const long = reportInputs([["name", "x".repeat(500)]])[0][1];
  assert.equal(long.length, MAX_REPORT_INPUT);
  assert.ok(long.endsWith("…"));
  const many = Array.from({ length: 30 }, (_, i) => ["k" + i, "v" + i]);
  assert.equal(reportInputs(many).length, MAX_REPORT_INPUTS);
});
test("a revision names its group, item and revision, then the extra inputs", () => {
  const rev = { project_code: "ROVER", item_number: "PRT-000042", code: "B", name: "Bracket (item data)" };
  assert.deepEqual(reportInputs(revisionInputs(rev, [["file", "/home/max/work/bracket.ipt"]])),
    [["group", "ROVER"], ["item", "PRT-000042"], ["revision", "B"], ["file", "bracket.ipt"]]);
  // no item name or other item data; a missing revision gives no inputs
  assert.equal(JSON.stringify(revisionInputs(rev)).includes("Bracket"), false);
  assert.deepEqual(reportInputs(revisionInputs(undefined)), []);
});
test("the caps match desktop/src/report.rs", () => {
  const rs = fs.readFileSync(path.resolve(here, "../desktop/src/report.rs"), "utf8");
  assert.match(rs, new RegExp("const MAX_INPUTS: usize = " + MAX_REPORT_INPUTS + ";"));
  assert.match(rs, new RegExp("const MAX_INPUT: usize = " + MAX_REPORT_INPUT + ";"));
});

console.log(`${n} tests passed`);
