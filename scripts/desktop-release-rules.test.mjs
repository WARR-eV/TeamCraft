// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/release-rules.js (the reviewers of the release
// dialog, and why a draft change cannot be submitted yet – #291).
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-release-rules.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/release-rules.js");
const src = fs.readFileSync(file, "utf8");
const { reviewerProblem, draftBlocker } = await import(
  "data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

test("ticked members and typed names are the reviewers, without duplicates", () => {
  const r = reviewerProblem(["mchen"], " jlopez, MCHEN;@sara  rolf ", "admin", true);
  assert.equal(r.error, null);
  assert.deepEqual(r.reviewers, ["mchen", "jlopez", "sara", "rolf"]);
});

test("the own user name is refused, also typed in another case (#291)", () => {
  const r = reviewerProblem([], "Admin", "admin", true);
  assert.match(r.error, /cannot review your own change/);
  assert.match(r.error, /untick/, "submitting: the draft is offered");
  assert.deepEqual(r.reviewers, []);
  const draft = reviewerProblem(["mchen"], "admin", "admin", false);
  assert.match(draft.error, /four-eyes/);
  assert.doesNotMatch(draft.error, /untick/, "a draft needs no hint about the draft");
});

test("submitting needs a reviewer, a draft does not", () => {
  assert.match(reviewerProblem([], "  ", "admin", true).error, /at least one reviewer/);
  assert.deepEqual(reviewerProblem([], "", "admin", false), { error: null, reviewers: [] });
});

test("unknown users are left to the server", () => {
  assert.deepEqual(reviewerProblem([], "nobody", "admin", true).reviewers, ["nobody"]);
});

test("a draft is submitted with a revision and a reviewer", () => {
  assert.match(draftBlocker(0, 1), /revisions/);
  assert.match(draftBlocker(2, 0), /reviewer/);
  assert.match(draftBlocker(0, 0), /revisions/, "the revisions come first");
  assert.equal(draftBlocker(1, 1), null);
});

console.log(n + " tests passed");
