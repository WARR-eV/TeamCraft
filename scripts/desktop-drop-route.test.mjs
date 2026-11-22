// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/drop-rules.js (where files dropped on the
// window from the file manager go – #418).
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-drop-route.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/drop-rules.js");
const src = fs.readFileSync(file, "utf8");
const { DIALOG_OPEN_TEXT, LOADING_TEXT, NO_CREATE_TEXT, dropLabel, routeDrop, skippedText } = await import(
  "data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

const paths = ["/home/a/cad/bracket.ipt", "/home/a/cad/bracket.step"];
function revTab(rev) {
  return { key: "rev:PRT-000042:A", kind: "revision", data: { rev: { item_number: "PRT-000042", code: "A", ...rev } } };
}
const editable = { ok: true, text: "Add files… copies files into your workspace; or edit the files there, then check in." };
const lockedHint = { ok: false, text: "Checked out by Ada since 1 May – files can be added after check-in." };
const releasedHint = { ok: false, text: "Released revisions are frozen – Revise to add files to a new revision." };
const reviewHint = { ok: false, text: "PRT-000042/A is in review and frozen – files can be added once the change is decided, or to a new revision." };
const viewerHint = { ok: false, text: "Only editors of the project group can add files." };

test("an empty drop does nothing", () => {
  assert.equal(routeDrop({ paths: [], tab: null, canCreate: true }), null);
  assert.equal(routeDrop({ tab: null, canCreate: true }), null);
  assert.equal(routeDrop({ paths: ["", null], tab: null, canCreate: true }), null);
  assert.equal(routeDrop(null), null);
  assert.equal(dropLabel(null), "");
});

test("no tab: New with the files", () => {
  const r = routeDrop({ paths, tab: null, canCreate: true });
  assert.deepEqual(r, { kind: "new", paths });
  assert.equal(dropLabel(r), "Drop to create a new item with 2 files");
  assert.equal(dropLabel(routeDrop({ paths: [paths[0]], tab: null, canCreate: true })), "Drop to create a new item with the file");
});

test("a change or other tab: New with the files", () => {
  const r = routeDrop({ paths, tab: { kind: "change", data: {} }, canCreate: true });
  assert.equal(r.kind, "new");
});

test("a revision checked out by me: Add files…", () => {
  const tab = revTab({ state: "in_work", checked_out_by: "me" });
  const r = routeDrop({ paths, tab, hint: editable, canCreate: true });
  assert.equal(r.kind, "add");
  assert.equal(r.tab, tab);
  assert.deepEqual(r.paths, paths);
  assert.equal(dropLabel(r), "Drop to add 2 files to PRT-000042/A");
});

test("a free revision Add files… would check out: Add files…", () => {
  const hint = { ok: true, text: "To add or replace files, check out this revision – Add files… does that for you." };
  assert.equal(routeDrop({ paths, tab: revTab({ state: "in_work" }), hint, canCreate: true }).kind, "add");
});

test("a revision checked out by someone else is refused with the reason", () => {
  const r = routeDrop({ paths, tab: revTab({ state: "in_work", checked_out_by: "ada" }), hint: lockedHint, canCreate: true });
  assert.deepEqual(r, { kind: "refuse", text: lockedHint.text });
  assert.equal(dropLabel(r), lockedHint.text);
});

test("a released revision is refused, not turned into New", () => {
  const r = routeDrop({ paths, tab: revTab({ state: "released" }), hint: releasedHint, canCreate: true });
  assert.deepEqual(r, { kind: "refuse", text: releasedHint.text });
});

test("a revision in review is refused", () => {
  const r = routeDrop({ paths, tab: revTab({ state: "in_review" }), hint: reviewHint, canCreate: true });
  assert.equal(r.kind, "refuse");
  assert.match(r.text, /in review/);
});

test("a revision without a hint is refused, never added", () => {
  const r = routeDrop({ paths, tab: revTab({ state: "in_work" }), canCreate: true });
  assert.equal(r.kind, "refuse");
});

test("a revision still loading is refused", () => {
  const r = routeDrop({ paths, tab: { kind: "revision", data: null }, canCreate: true });
  assert.deepEqual(r, { kind: "refuse", text: LOADING_TEXT });
});

test("a viewer is refused: New and on a revision", () => {
  assert.deepEqual(routeDrop({ paths, tab: null, canCreate: false }), { kind: "refuse", text: NO_CREATE_TEXT });
  const r = routeDrop({ paths, tab: revTab({ state: "in_work" }), hint: viewerHint, canCreate: false });
  assert.deepEqual(r, { kind: "refuse", text: viewerHint.text });
});

test("an open dialog refuses the drop", () => {
  const r = routeDrop({ paths, tab: null, canCreate: true, dialogOpen: true });
  assert.deepEqual(r, { kind: "refuse", text: DIALOG_OPEN_TEXT });
});

test("skipped folders are named", () => {
  assert.equal(skippedText(0), "");
  assert.match(skippedText(1), /^1 folder/);
  assert.match(skippedText(3), /^3 folders/);
});

console.log(`${n} tests passed`);
