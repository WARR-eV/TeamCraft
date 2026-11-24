// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/lock-rules.js (refreshing lock data, #451):
// when the Check-outs tab polls, which answer may still be applied, which
// tabs load again. Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-lock-rules.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/lock-rules.js");
const R = await import("data:text/javascript;base64," + Buffer.from(fs.readFileSync(file, "utf8")).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }
const me = { id: "u-1", display_name: "Erika" };
const overview = (...refs) => ({ project_code: "ROVER", total: refs.length, holders: [{ name: "Kim", locks: refs }] });

test("only a Check-outs tab with data polls, and only while someone is signed in", () => {
  assert.equal(R.shouldPoll({ kind: "locks", data: overview("PRT-1/A") }, me), true);
  assert.equal(R.shouldPoll({ kind: "revision", data: {} }, me), false);
  assert.equal(R.shouldPoll({ kind: "locks", data: null }, me), false, "still loading");
  assert.equal(R.shouldPoll({ kind: "locks", data: overview() }, null), false, "signed out");
  assert.equal(R.shouldPoll(null, me), false, "no tab in front");
});

test("the overview is painted again only when it changed", () => {
  assert.equal(R.locksDiffer(overview("PRT-1/A"), overview("PRT-1/A")), false);
  assert.equal(R.locksDiffer(overview("PRT-1/A"), overview("PRT-1/A", "PRT-2/A")), true);
  assert.equal(R.locksDiffer(null, overview()), true);
});

test("an answer older than one already applied is dropped", () => {
  const tab = {};
  const poll = R.takeSeq(tab);    // the 30 s poll starts
  const refresh = R.takeSeq(tab); // Refresh starts while it runs
  assert.ok(refresh > poll);
  // Refresh answers first, the slow poll after it
  assert.equal(R.acceptSeq(tab, refresh), true);
  assert.equal(R.acceptSeq(tab, poll), false, "the older poll must not paint over Refresh");
  // in order, both apply
  const a = R.takeSeq(tab), b = R.takeSeq(tab);
  assert.equal(R.acceptSeq(tab, a), true);
  assert.equal(R.acceptSeq(tab, b), true);
  // a fresh counter accepts its first answer
  const w = {};
  assert.equal(R.acceptSeq(w, R.takeSeq(w)), true);
});

test("forgetting the worklist drops the answers still on their way", () => {
  // shell.js forgetWorklist (sign-out) calls dropPending on the worklist counter
  const w = {};
  const older = R.takeSeq(w);
  const late = R.takeSeq(w);
  assert.equal(R.acceptSeq(w, older), true, "an answer before the sign-out applies");
  R.dropPending(w);
  assert.equal(R.acceptSeq(w, late), false, "the previous user's answer is dropped");
  assert.equal(R.acceptSeq(w, R.takeSeq(w)), true, "the next user's request applies");
  // a fresh counter (nothing asked yet) can be reset too
  const fresh = {};
  R.dropPending(fresh);
  assert.equal(R.acceptSeq(fresh, R.takeSeq(fresh)), true);
});

test("a dropped answer is never handed to the caller", () => {
  const theirs = { checkouts: [{ item_number: "PRT-1", code: "A" }] };
  // a newer answer was applied meanwhile: the caller gets that one
  assert.equal(R.droppedAnswer(theirs), theirs);
  // the worklist was forgotten (sign-out): nothing to draw
  assert.equal(R.droppedAnswer(null), null);
  assert.equal(R.droppedAnswer(undefined), null);
});

test("own check-outs changed: hidden Check-outs tabs forget their data, the one in front keeps it", () => {
  const front = { kind: "locks", code: "ROVER", data: overview("PRT-1/A") };
  const hidden = { kind: "locks", code: "PUMP", data: overview("PMP-1/A") };
  const rev = { kind: "revision", data: { rev: {} } };
  R.forgetHiddenLocks([front, hidden, rev], front);
  assert.equal(hidden.data, null, "loads again when it comes to the front");
  assert.deepEqual(rev.data, { rev: {} }, "other tabs are not touched");
  // the one in front stays pollable even if the fetch that follows fails
  assert.equal(R.shouldPoll(front, me), true);
  // nothing in front: every Check-outs tab forgets
  R.forgetHiddenLocks([front], null);
  assert.equal(front.data, null);
  R.forgetHiddenLocks(null, null);
});

console.log(n + " tests passed");
