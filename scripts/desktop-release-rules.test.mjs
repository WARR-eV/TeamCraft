// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/release-rules.js (the reviewers of the release
// dialog, why a draft change cannot be submitted yet – #291, and where
// Withdraw goes – #517).
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
const { reviewerProblem, draftBlocker, withdrawAction } = await import(
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

const inReview = { status: "in_review", created_by: 7, created_by_name: "Mia Chen" };

test("the creator withdraws from the toolbar, at once, while in review (#517)", () => {
  const w = withdrawAction(inReview, { id: 7, role: "editor" }, true, "In Review");
  assert.deepEqual([w.place, w.enabled, w.label, w.confirm], ["toolbar", true, "Withdraw", false]);
  const draft = withdrawAction({ ...inReview, status: "draft" }, { id: 7, role: "editor" }, true, "Draft");
  assert.equal(draft.enabled, false);
  assert.match(draft.title, /only a change in review/i);
  assert.match(draft.title, /Draft$/);
});

test("an administrator who is the creator is the creator", () => {
  const w = withdrawAction(inReview, { id: 7, role: "admin" }, true);
  assert.deepEqual([w.place, w.confirm, w.label], ["toolbar", false, "Withdraw"]);
});

test("an administrator's override is in More, labelled and confirmed – never next to Approve (#517)", () => {
  const w = withdrawAction(inReview, { id: 1, role: "admin" }, true, "In Review");
  assert.deepEqual([w.place, w.enabled, w.confirm], ["more", true, true]);
  assert.match(w.label, /\(administrator\)/);
  assert.match(w.title, /Mia Chen/);
  const closed = withdrawAction({ ...inReview, status: "released" }, { id: 1, role: "admin" }, true, "Released");
  assert.deepEqual([closed.place, closed.enabled], ["more", false]);
});

test("everybody else sees it disabled with the reason", () => {
  const reviewer = withdrawAction(inReview, { id: 3, role: "editor" }, true);
  assert.deepEqual([reviewer.place, reviewer.enabled], ["toolbar", false]);
  assert.match(reviewer.title, /Only Mia Chen \(who created it\) or an administrator/);
  // without edit rights nobody withdraws
  assert.equal(withdrawAction(inReview, { id: 7, role: "viewer" }, false).enabled, false);
  assert.equal(withdrawAction(inReview, { id: 1, role: "admin" }, false).place, "toolbar");
  assert.equal(withdrawAction(null, null, true).enabled, false);
});

console.log(n + " tests passed");
