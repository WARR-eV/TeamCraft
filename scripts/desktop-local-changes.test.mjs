// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/local-changes.js (*Local changes*, #416: the
// files of the whole working folder grouped by item, the actions per row
// and per selection). Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-local-changes.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const source = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/local-changes.js");
const {
  LOCAL_STATES, localRevKey, localRowActions, localRowKey, localSummaryText, localVersionText, planLocalChanges, revisionCheckin, selectionActions
} = await import("data:text/javascript;base64," + fs.readFileSync(source).toString("base64"));

let passed = 0;
function test(name, fn) {
  fn();
  passed++;
  console.log("ok - " + name);
}

const file = (o) => Object.assign({ name: "bracket.prt", path: "ROVER/PRT-1/bracket.prt", state: "modified",
  server_version: null, base_version: 2, writable: true, without_checkout: false }, o);
const rev = (o) => Object.assign({ project: "ROVER", item: "PRT-1", code: "A", checked_out: false, compared: true,
  inaccessible: false, error: null, dir: "/ws/ROVER/PRT-1", files: [] }, o);

test("a file modified without a check-out offers Check out (keep my file)", () => {
  const plan = planLocalChanges({ revisions: [rev({ files: [file({ without_checkout: true })] })], truncated: false, compared: 1, tracked: 1 });
  assert.equal(plan.count, 1);
  assert.equal(plan.withoutCheckout, 1);
  const row = plan.items[0].revisions[0].rows[0];
  assert.deepEqual(row.actions, ["checkout", "folder"]);
  assert.match(localSummaryText(plan), /1 modified/);
  assert.match(localSummaryText(plan), /1 changed without a check-out/);
});

test("a conflict is merged in the Workspace tab, never checked in", () => {
  const r = rev({ checked_out: true, files: [
    file({ name: "a.prt", path: "ROVER/PRT-1/a.prt", state: "conflict", server_version: 4 }),
    file({ name: "b.prt", path: "ROVER/PRT-1/b.prt", state: "modified" })
  ] });
  const plan = planLocalChanges({ revisions: [r] });
  const rows = plan.items[0].revisions[0].rows;
  assert.deepEqual(rows.map((x) => x.state), ["conflict", "modified"], "conflicts first");
  assert.deepEqual(rows[0].actions, ["merge", "folder"]);
  assert.deepEqual(rows[1].actions, ["checkin", "folder"]);
  assert.equal(localVersionText(rows[0]), "v4 on the server, yours v2");
  // ticking both: only the modified file is checked in, the conflict keeps the revision checked out
  const ticked = {};
  rows.forEach((x) => { ticked[x.key] = true; });
  const sel = selectionActions(plan, ticked);
  assert.equal(sel.checkin.changed.length, 1);
  assert.deepEqual(sel.checkin.changed[0].upload, ["b.prt"]);
  assert.deepEqual(sel.checkin.changed[0].added, []);
  assert.deepEqual(sel.checkin.changed[0].conflicts, ["a.prt"]);
  assert.deepEqual(sel.checkin.unreadable, []);
  assert.equal(sel.rows, 2);
});

test("outdated and missing files offer Get latest", () => {
  const r = rev({ files: [
    file({ name: "o.prt", path: "ROVER/PRT-1/o.prt", state: "outdated", server_version: 3 }),
    file({ name: "m.prt", path: "ROVER/PRT-1/m.prt", state: "missing", writable: false })
  ] });
  const plan = planLocalChanges({ revisions: [r] });
  const rows = plan.items[0].revisions[0].rows;
  assert.deepEqual(rows.map((x) => x.state), ["outdated", "missing"]);
  rows.forEach((x) => assert.deepEqual(x.actions, ["get", "folder"]));
  const ticked = {};
  ticked[rows[0].key] = true;
  ticked[rows[1].key] = true;
  const sel = selectionActions(plan, ticked);
  assert.deepEqual(sel.get, [{ project: "ROVER", item: "PRT-1", code: "A" }], "one Get per revision");
  assert.deepEqual(sel.checkout, []);
  assert.equal(plan.counts.outdated, 1);
  assert.equal(plan.counts.missing, 1);
});

test("own check-outs: modified and new files are checked in", () => {
  const r = rev({ checked_out: true, files: [file({}), file({ name: "bracket.step", path: "ROVER/PRT-1/bracket.step", state: "new" })] });
  const plan = planLocalChanges({ revisions: [r] });
  const rows = plan.items[0].revisions[0].rows;
  assert.deepEqual(rows.map((x) => x.state), ["modified", "new"]);
  rows.forEach((x) => assert.deepEqual(x.actions, ["checkin", "folder"]));
  const ticked = {};
  ticked[rows[0].key] = true;
  ticked[rows[1].key] = true;
  const sel = selectionActions(plan, ticked);
  assert.deepEqual(sel.checkin.changed, [{ project: "ROVER", item: "PRT-1", code: "A", upload: ["bracket.prt", "bracket.step"], added: [], conflicts: [] }]);
  assert.deepEqual(selectionActions(plan, {}).checkin.changed, [], "nothing ticked, nothing to do");
});

test("a partial selection checks in every changed file of the revision and names the unticked ones", () => {
  const r = rev({ checked_out: true, files: [file({}), file({ name: "bracket.step", path: "ROVER/PRT-1/bracket.step", state: "new" }),
    file({ name: "old.prt", path: "ROVER/PRT-1/old.prt", state: "outdated", server_version: 3 })] });
  const plan = planLocalChanges({ revisions: [r] });
  const rows = plan.items[0].revisions[0].rows;
  const step = rows.find((x) => x.name === "bracket.step");
  const ticked = {};
  ticked[step.key] = true;
  const sel = selectionActions(plan, ticked);
  assert.equal(sel.checkin.changed.length, 1);
  assert.deepEqual(sel.checkin.changed[0].upload, ["bracket.prt", "bracket.step"], "the check-in releases the check-out: nothing changed stays behind");
  assert.deepEqual(sel.checkin.changed[0].added, ["bracket.prt"]);
  // the row action does the same
  const one = revisionCheckin(plan.items[0].revisions[0], [step]);
  assert.deepEqual(one.upload, ["bracket.prt", "bracket.step"]);
  assert.deepEqual(one.added, ["bracket.prt"]);
  // an outdated file alone has nothing to check in
  assert.equal(revisionCheckin(plan.items[0].revisions[0], rows.filter((x) => x.state === "outdated")), null);
});

test("grouping by item, sorted; empty revisions and other states are left out", () => {
  const plan = planLocalChanges({
    revisions: [
      rev({ item: "PRT-9", files: [file({ path: "ROVER/PRT-9/x.prt" })] }),
      rev({ item: "PRT-1", code: "A", files: [file({})] }),
      rev({ item: "PRT-1", code: "B", files: [file({ path: "ROVER/PRT-1/B/bracket.prt" })] }),
      rev({ item: "PRT-5", files: [file({ state: "up_to_date" })] }),
      rev({ item: "PRT-7", files: [], error: "server unreachable" })
    ],
    truncated: true, compared: 3, tracked: 9
  });
  assert.deepEqual(plan.items.map((i) => i.item), ["PRT-1", "PRT-9"]);
  assert.deepEqual(plan.items[0].revisions.map((r) => r.code), ["A", "B"]);
  assert.equal(plan.count, 3);
  assert.deepEqual(plan.errors, ["PRT-7/A: server unreachable"]);
  assert.equal(plan.truncated, true);
  assert.equal(plan.compared, 3);
  assert.equal(plan.tracked, 9);
  assert.notEqual(plan.items[0].revisions[0].rows[0].key, plan.items[0].revisions[1].rows[0].key);
});

test("an inaccessible revision only opens its folder", () => {
  assert.deepEqual(localRowActions(rev({ inaccessible: true }), file({ without_checkout: false })), ["folder"]);
});

test("keys ignore case and an empty scan plans nothing", () => {
  assert.equal(localRowKey(rev({ project: "Rover" }), file({})), localRowKey(rev({ project: "ROVER" }), file({ path: "rover/prt-1/BRACKET.prt" })));
  assert.equal(localRevKey(rev({ item: "prt-1" })), localRevKey(rev({ item: "PRT-1" })));
  const plan = planLocalChanges(null);
  assert.equal(plan.count, 0);
  assert.deepEqual(plan.items, []);
  assert.equal(localSummaryText(plan), "");
  assert.deepEqual(LOCAL_STATES, ["conflict", "modified", "new", "outdated", "missing"]);
});

console.log(passed + " tests passed");
