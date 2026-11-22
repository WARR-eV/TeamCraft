// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/empty-rules.js (the empty states of the
// desktop app name the next step, by role – #423) against the real message
// catalogs of shared/i18n/. Plain Node.js, no packages; run by the CI job
// `ui`:
//
//   node scripts/desktop-empty-rules.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const src = fs.readFileSync(path.join(root, "desktop/ui/js/empty-rules.js"), "utf8");
const { EMPTY_MESSAGE_IDS, emptyActionLabel, emptyGroupHint, noGroupsHint, noNewFilesHint, worklistHint } = await import(
  "data:text/javascript;base64," + Buffer.from(src).toString("base64"));
const catalog = (lang) => JSON.parse(fs.readFileSync(path.join(root, "shared/i18n", lang + ".json"), "utf8"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

test("no project group: editors create one, viewers ask a manager", () => {
  const editor = noGroupsHint(true);
  assert.equal(editor.text, "empty.no_groups.editor");
  assert.deepEqual(editor.actions, ["create_group"]);
  assert.equal(editor.note, null);
  const viewer = noGroupsHint(false);
  assert.equal(viewer.text, "empty.no_groups.viewer");
  assert.deepEqual(viewer.actions, [], "viewers get no button they cannot use");
  assert.equal(viewer.note, "empty.viewer_note", "and are told why");
});

test("an empty group: Import CAD folder… and New item for those who may", () => {
  const editor = emptyGroupHint(true);
  assert.equal(editor.lead, "empty.group_empty");
  assert.deepEqual(editor.actions, ["import_folder", "new_item"]);
  const viewer = emptyGroupHint(false);
  assert.equal(viewer.lead, "empty.group_empty");
  assert.equal(viewer.text, "empty.group_empty.viewer");
  assert.deepEqual(viewer.actions, []);
});

test("no new files in the working folder: Get… a group", () => {
  const hint = noNewFilesHint();
  assert.equal(hint.lead, "empty.no_new_files");
  assert.deepEqual(hint.actions, ["get"]);
});

test("worklist sections say how work arrives, viewers why they have none", () => {
  assert.equal(worklistHint("checkouts", true).text, "empty.no_checkouts");
  assert.equal(worklistHint("checkouts", false).text, "empty.no_checkouts.viewer");
  assert.equal(worklistHint("reviews", true).text, "empty.nothing_to_review");
  assert.equal(worklistHint("reviews", false).text, "empty.nothing_to_review");
  assert.equal(worklistHint("changes", true).text, "empty.no_open_changes");
  assert.equal(worklistHint("changes", false).text, "empty.no_open_changes.viewer");
  assert.equal(worklistHint("nonsense", true), null, "unknown sections have no hint");
  for (const s of ["checkouts", "reviews", "changes"]) assert.deepEqual(worklistHint(s, true).actions, []);
});

test("every action has a label, unknown ones none", () => {
  for (const a of ["create_group", "import_folder", "new_item", "get"]) assert.ok(emptyActionLabel(a), a);
  assert.equal(emptyActionLabel("delete_everything"), null);
});

test("every message the rules answer is in both catalogs", () => {
  const en = catalog("en"), de = catalog("de");
  const answered = new Set(EMPTY_MESSAGE_IDS);
  const hints = [noGroupsHint(true), noGroupsHint(false), emptyGroupHint(true), emptyGroupHint(false), noNewFilesHint()]
    .concat(["checkouts", "reviews", "changes"].flatMap((s) => [worklistHint(s, true), worklistHint(s, false)]));
  for (const h of hints) {
    for (const id of [h.lead, h.text, h.note].filter(Boolean)) assert.ok(answered.has(id), id + " is in EMPTY_MESSAGE_IDS");
    for (const a of h.actions) assert.ok(answered.has(emptyActionLabel(a)), a + " label is in EMPTY_MESSAGE_IDS");
  }
  for (const id of EMPTY_MESSAGE_IDS) {
    assert.equal(typeof en[id], "string", id + " is in en.json");
    assert.equal(typeof de[id], "string", id + " is in de.json");
  }
});

console.log(n + " tests passed");
