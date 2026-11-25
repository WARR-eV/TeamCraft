// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/next-step.js (the one highlighted button of the
// revision toolbar – #536, #513 T2).
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-next-step.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/next-step.js");
const src = fs.readFileSync(file, "utf8");
const { revisionNextStep, parkedSinceCheckout, lastCheckinMine } = await import(
  "data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

const ME = "u-me", OTHER = "u-other";
// the revision was created at CREATED; BEFORE and AFTER are uploads made in it
const CREATED = "2026-11-24T08:00:00Z";
const CHECKED_OUT_AT = "2026-11-24T10:00:00Z";
const BEFORE = "2026-11-24T09:00:00Z", AFTER = "2026-11-24T11:00:00Z";
// uploads of the previous revision, carried over by *Revise* with their old
// uploaded_by / uploaded_at (services::revisions::revise)
const CARRIED = "2026-11-01T12:00:00Z";
// what revisionAbilities gives an editor of the group on a revision in work
const EDITOR = { checkout: true, release: true, revise: false };

function mine(extra) {
  return Object.assign({
    rev: { state: "in_work", checked_out_by: ME, checked_out_at: CHECKED_OUT_AT, created_at: CREATED },
    files: [{ uploaded_by: OTHER, uploaded_at: BEFORE }],
    me: ME, localChanges: false, cadFile: true,
    can: { checkout: false, release: true, revise: false }
  }, extra);
}

function free(files, can, created) {
  return {
    rev: { state: "in_work", checked_out_by: null, checked_out_at: null, created_at: created === undefined ? CREATED : created },
    files: files, me: ME, localChanges: null, cadFile: true, can: can || EDITOR
  };
}

test("a fresh own check-out without local changes: Open in CAD, never Release… (#513 T2)", () => {
  assert.equal(revisionNextStep(mine()), "open");
  // also when my own upload is older than the check-out
  assert.equal(revisionNextStep(mine({ files: [{ uploaded_by: ME, uploaded_at: BEFORE }] })), "open");
});

test("local changes in the working folder: Check in…", () => {
  assert.equal(revisionNextStep(mine({ localChanges: true })), "checkin");
});

test("a working folder that could not be read: Check in… (the safe side)", () => {
  assert.equal(revisionNextStep(mine({ localChanges: null })), "checkin");
});

test("versions parked since the check-out: Check in…, not Open in CAD", () => {
  const parked = mine({ files: [{ uploaded_by: ME, uploaded_at: AFTER }] });
  assert.equal(parkedSinceCheckout(parked.rev, parked.files, ME), true);
  assert.equal(revisionNextStep(parked), "checkin");
  // somebody else's upload after my check-out is no parking of mine
  const other = mine({ files: [{ uploaded_by: OTHER, uploaded_at: AFTER }] });
  assert.equal(parkedSinceCheckout(other.rev, other.files, ME), false);
  assert.equal(revisionNextStep(other), "open");
});

test("own check-out without a CAD file to open: Check in…", () => {
  assert.equal(revisionNextStep(mine({ cadFile: false })), "checkin");
});

test("a check-out never highlights Release…, whatever can.release says", () => {
  for (const lc of [true, false, null]) {
    for (const cad of [true, false]) {
      assert.notEqual(revisionNextStep(mine({ localChanges: lc, cadFile: cad })), "release");
    }
  }
});

test("checked out by somebody else: nothing", () => {
  const f = mine({ rev: { state: "in_work", checked_out_by: OTHER, checked_out_at: CHECKED_OUT_AT } });
  assert.equal(revisionNextStep(f), null);
});

test("checked in by me last and releasable: Release…", () => {
  const files = [{ uploaded_by: OTHER, uploaded_at: BEFORE }, { uploaded_by: ME, uploaded_at: AFTER }];
  assert.equal(lastCheckinMine(free(files).rev, files, ME), true);
  assert.equal(revisionNextStep(free(files)), "release");
});

test("a revised revision with only carried-over files of mine: Check out and open, not Release… (#537 review)", () => {
  // I made the last upload to A; *Revise* copied it into B with its old time
  const carried = [{ uploaded_by: OTHER, uploaded_at: "2026-10-30T08:00:00Z" }, { uploaded_by: ME, uploaded_at: CARRIED }];
  const revisedAt = "2026-11-20T09:00:00Z";
  // B right after *Revise*: checked out to me at its creation, nothing changed
  const fresh = {
    rev: { state: "in_work", checked_out_by: ME, checked_out_at: revisedAt, created_at: revisedAt },
    files: carried, me: ME, localChanges: false, cadFile: true, can: { checkout: false, release: true, revise: false }
  };
  assert.equal(parkedSinceCheckout(fresh.rev, carried, ME), false);
  assert.equal(revisionNextStep(fresh), "open");
  // … and after *Cancel check-out* (no upload): the carried files say nothing
  const cancelled = free(carried, EDITOR, revisedAt);
  assert.equal(lastCheckinMine(cancelled.rev, carried, ME), false);
  assert.equal(revisionNextStep(cancelled), "checkout");
  // once I check in a change to B, Release… is the next step
  const checkedIn = carried.concat([{ uploaded_by: ME, uploaded_at: "2026-11-21T15:00:00Z" }]);
  assert.equal(revisionNextStep(free(checkedIn, EDITOR, revisedAt)), "release");
  // a colleague's check-in to B after mine: Check out and open
  const colleague = checkedIn.concat([{ uploaded_by: OTHER, uploaded_at: "2026-11-22T15:00:00Z" }]);
  assert.equal(revisionNextStep(free(colleague, EDITOR, revisedAt)), "checkout");
});

test("a cancelled check-out does not make a revision ready for Release…", () => {
  // checked out and cancelled without any upload: the files are as before
  const theirs = [{ uploaded_by: OTHER, uploaded_at: BEFORE }];
  assert.equal(revisionNextStep(mine({ files: theirs })), "open");
  assert.equal(revisionNextStep(free(theirs)), "checkout");
  // a revision my own earlier check-in left ready stays ready after a cancelled check-out
  const myCheckin = [{ uploaded_by: ME, uploaded_at: BEFORE }];
  assert.equal(revisionNextStep(free(myCheckin)), "release");
});

test("no creation time in the payload: no guess on a releasable free revision", () => {
  const files = [{ uploaded_by: ME, uploaded_at: AFTER }];
  assert.equal(lastCheckinMine({ created_at: null }, files, ME), null);
  assert.equal(lastCheckinMine(null, files, ME), null);
  assert.equal(revisionNextStep(free(files, EDITOR, null)), null);
  // without the right to release, Check out and open needs no creation time
  assert.equal(revisionNextStep(free(files, { checkout: true, release: false, revise: false }, null)), "checkout");
});

test("an upload in the same transaction as the revision counts as made in it", () => {
  const files = [{ uploaded_by: ME, uploaded_at: CREATED }];
  assert.equal(revisionNextStep(free(files)), "release");
});

test("checked in, but not releasable (open change order): Check out and open", () => {
  const files = [{ uploaded_by: ME, uploaded_at: AFTER }];
  assert.equal(revisionNextStep(free(files, { checkout: true, release: false, revise: false })), "checkout");
});

test("a free revision last changed by somebody else or without files: Check out and open", () => {
  assert.equal(revisionNextStep(free([{ uploaded_by: ME, uploaded_at: BEFORE }, { uploaded_by: OTHER, uploaded_at: AFTER }])), "checkout");
  assert.equal(revisionNextStep(free([])), "checkout");
  assert.equal(lastCheckinMine({ created_at: CREATED }, [], ME), false);
});

test("not an editor of the group: nothing on a free revision", () => {
  assert.equal(revisionNextStep(free([{ uploaded_by: ME, uploaded_at: AFTER }], { checkout: false, release: false, revise: false })), null);
});

test("released: Revise when allowed; in review and obsolete: nothing", () => {
  const rel = { rev: { state: "released", checked_out_by: null }, files: [], me: ME, localChanges: null, cadFile: true, can: { revise: true } };
  assert.equal(revisionNextStep(rel), "revise");
  assert.equal(revisionNextStep(Object.assign({}, rel, { can: { revise: false } })), null);
  for (const state of ["in_review", "obsolete"]) {
    assert.equal(revisionNextStep({ rev: { state: state, checked_out_by: null }, files: [{ uploaded_by: ME, uploaded_at: AFTER }], me: ME, can: { release: true, checkout: true, revise: true } }), null);
  }
});

test("parking needs a check-out time", () => {
  assert.equal(parkedSinceCheckout({ checked_out_at: null }, [{ uploaded_by: ME, uploaded_at: AFTER }], ME), false);
});

console.log(n + " tests passed");
