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
const { changeInputs, checkInAllFailure, reportInputs, recentErrorsAfter, requestInputs, revisionInputs, failureOf, settingInputs, tabInputs, MAX_RECENT_ERRORS, MAX_REPORT_INPUTS, MAX_REPORT_INPUT } = await import(
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

test("change orders, requests and settings name their key inputs (#471)", () => {
  assert.deepEqual(reportInputs(changeInputs({ number: "CHG-000002", status: "draft" }, [["reviewer", "alex"]])),
    [["change", "CHG-000002"], ["status", "draft"], ["reviewer", "alex"]]);
  // a change known only by its number: the status is left out
  assert.deepEqual(reportInputs(changeInputs({ number: "CHG-000002" })), [["change", "CHG-000002"]]);
  assert.deepEqual(reportInputs(changeInputs(null)), []);
  assert.deepEqual(reportInputs(requestInputs({ number: "ECR-000004", status: "open" }, [["decision", "accept"]])),
    [["request", "ECR-000004"], ["status", "open"], ["decision", "accept"]]);
  assert.deepEqual(reportInputs(settingInputs("update check", true)), [["setting", "update check"], ["value", "on"]]);
  assert.deepEqual(reportInputs(settingInputs("update check", false)), [["setting", "update check"], ["value", "off"]]);
  assert.deepEqual(reportInputs(settingInputs("skin", "teamcraft-night")), [["setting", "skin"], ["value", "teamcraft-night"]]);
});

test("requests and settings without a status or value leave it out", () => {
  assert.deepEqual(reportInputs(requestInputs(null)), []);
  assert.deepEqual(reportInputs(requestInputs({ number: "ECR-000004" })), [["request", "ECR-000004"]]);
  assert.deepEqual(reportInputs(settingInputs("report address", undefined)), [["setting", "report address"]]);
  assert.deepEqual(reportInputs(settingInputs("report address", "")), [["setting", "report address"]]);
  assert.deepEqual(reportInputs(settingInputs("report address", "custom")), [["setting", "report address"], ["value", "custom"]]);
});

test("a tab that failed to open names what it shows (#471)", () => {
  assert.deepEqual(reportInputs(tabInputs({ kind: "change", number: "CHG-000002" })), [["change", "CHG-000002"]]);
  assert.deepEqual(reportInputs(tabInputs({ kind: "request", number: "ECR-000004" })), [["request", "ECR-000004"]]);
  assert.deepEqual(reportInputs(tabInputs({ kind: "locks", code: "ROVER" })), [["group", "ROVER"], ["view", "check-outs"]]);
  assert.deepEqual(reportInputs(tabInputs({ kind: "revision", number: "PRT-000042", code: "B", sub: "files" })),
    [["item", "PRT-000042"], ["revision", "B"], ["sub-tab", "files"]]);
  assert.deepEqual(reportInputs(tabInputs({ kind: "revision", number: "PRT-000042", code: "B" })),
    [["item", "PRT-000042"], ["revision", "B"]], "no sub-tab, no entry");
});

test("recent messages: 'action: text', the last 50, a redrawn error kept once (#471)", () => {
  const at = new Date("2026-11-24T10:11:12Z");
  assert.deepEqual(recentErrorsAfter(undefined, "Checking in", "locked", at), ["10:11:12 Checking in: locked"]);
  assert.deepEqual(recentErrorsAfter([], "", "plain", at), ["10:11:12 plain"]);
  // the same notice drawn again (a worklist redraw) is not kept again …
  let list = [];
  for (let i = 0; i < 20; i++) list = recentErrorsAfter(list, "Comparing", "offline", at);
  assert.equal(list.length, 1);
  // … also with another error in between
  list = recentErrorsAfter(list, "Saving", "denied", at);
  list = recentErrorsAfter(list, "Comparing", "offline", new Date("2026-11-24T10:12:00Z"));
  assert.deepEqual(list, ["10:11:12 Comparing: offline", "10:11:12 Saving: denied"]);
  // once it is no longer among the latest, it is kept again
  for (let i = 0; i < 6; i++) list = recentErrorsAfter(list, "E" + i, "x", at);
  list = recentErrorsAfter(list, "Comparing", "offline", at);
  assert.equal(list[list.length - 1], "10:11:12 Comparing: offline");
  // at most MAX_RECENT_ERRORS, the oldest go first
  let many = [];
  for (let i = 0; i < 70; i++) many = recentErrorsAfter(many, "", "error " + i, at);
  assert.equal(many.length, MAX_RECENT_ERRORS);
  assert.equal(many[0], "10:11:12 error 20");
  assert.equal(many[many.length - 1], "10:11:12 error 69");
});

test("Check in all: no failures, no report; the count matches the list (#471)", () => {
  assert.equal(checkInAllFailure([]), null);
  assert.equal(checkInAllFailure(null), null);
  assert.deepEqual(checkInAllFailure(["PRT-1 A: locked", "PRT-2 B: gone"]), {
    action: "Checking in all my check-outs",
    error: "PRT-1 A: locked; PRT-2 B: gone",
    inputs: [["failed", "2"]]
  });
});

console.log(`${n} tests passed`);
