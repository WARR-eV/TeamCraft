// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/files-view.js (the pure part of the Files view
// of Home). Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-files-view.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const source = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/files-view.js");
const {
  checkedOutClass, compareNames, countText, entryIconType, entryName, entryRevision, entryTitle, folderKey,
  favouriteKey, favouritePath, favouriteSuggestions, groupTotal, mergeEntries, newItemFolder, normalizeFavourite,
  isMineLock, lockChanges, lockMarks, parentFolder, relockedEntry, remaining, rowThumb, sortEntries, sortFolders,
  suggestionKey
} = await import("data:text/javascript;base64," + Buffer.from(fs.readFileSync(source, "utf8")).toString("base64"));

let passed = 0;
function test(name, fn) {
  fn();
  passed++;
  console.log("ok - " + name);
}

const file = (filename, more) => Object.assign({ file_id: "f-" + filename, filename, item_number: "PRT-000001",
  item_type: "part", name: filename.replace(/\.[^.]+$/, ""), code: "A", state: "in_work",
  format_kind: null, format_label: null, checked_out_by_name: null }, more || {});
const loose = (number, name, more) => Object.assign({ file_id: null, filename: null, item_number: number,
  item_type: "part", name, code: "A", state: "in_work", format_kind: null, format_label: null,
  checked_out_by_name: null }, more || {});

test("names sort like in a file manager: case-blind, digits by value", () => {
  const names = ["wheel.ipt", "Part10.ipt", "part2.ipt", "Axle.ipt", "Part1.ipt"];
  assert.deepEqual(names.slice().sort(compareNames), ["Axle.ipt", "Part1.ipt", "part2.ipt", "Part10.ipt", "wheel.ipt"]);
  assert.equal(compareNames("a", "a"), 0);
  assert.equal(compareNames(null, "a") < 0, true);
  // the same name in another case is a stable, different entry
  assert.notEqual(compareNames("Bracket.ipt", "bracket.ipt"), 0);
  assert.equal(compareNames("Bracket.ipt", "bracket.ipt"), -compareNames("bracket.ipt", "Bracket.ipt"));
});

test("folders by name", () => {
  const sorted = sortFolders([{ name: "wheels" }, { name: "Chassis" }, { name: "10 misc" }, { name: "2 frame" }]);
  assert.deepEqual(sorted.map((f) => f.name), ["2 frame", "10 misc", "Chassis", "wheels"]);
  assert.deepEqual(sortFolders(null), []);
});

test("by name: files first by file name, then the items without files – never by number", () => {
  const rows = [
    loose("PRT-000001", "Zinc plate"),
    file("rover.iam", { item_number: "ASM-000009" }),
    loose("PRT-000002", "bought motor"),
    file("Bracket.ipt", { item_number: "PRT-000900" }),
    file("axle.ipt", { item_number: "PRT-000500" })
  ];
  const want = ["axle.ipt", "Bracket.ipt", "rover.iam", "bought motor", "Zinc plate"];
  assert.deepEqual(sortEntries(rows, "name").map(entryName), want);
  // no order or an unknown one is the order by name
  assert.deepEqual(sortEntries(rows).map(entryName), want);
  assert.deepEqual(sortEntries(rows, "size").map(entryName), want);
  // the input is left alone
  assert.equal(entryName(rows[0]), "Zinc plate");
  // two files of the same name (two items): by number, so the order is stable
  const twins = sortEntries([file("a.ipt", { file_id: "2", item_number: "PRT-000002" }), file("a.ipt", { file_id: "1", item_number: "PRT-000001" })]);
  assert.deepEqual(twins.map((e) => e.item_number), ["PRT-000001", "PRT-000002"]);
  assert.deepEqual(sortEntries(null), []);
});

test("the other orders of Home: number, type, state – files stay before the items without files", () => {
  const rows = [
    file("c.ipt", { item_number: "PRT-000003", state: "released" }),
    file("a.iam", { item_number: "ASM-000002", item_type: "assembly", format_kind: "assembly", state: "in_work" }),
    file("b.idw", { item_number: "PRT-000001", format_kind: "drawing", state: "in_review" }),
    file("d.ipt", { item_number: "PRT-000001", state: "in_work" }),
    loose("AAA-000001", "Loose", { state: "in_work" })
  ];
  assert.deepEqual(sortEntries(rows, "number").map(entryName), ["a.iam", "b.idw", "d.ipt", "c.ipt", "Loose"]);
  assert.deepEqual(sortEntries(rows, "type").map(entryName), ["a.iam", "b.idw", "c.ipt", "d.ipt", "Loose"]);
  assert.deepEqual(sortEntries(rows, "state").map(entryName), ["b.idw", "a.iam", "d.ipt", "c.ipt", "Loose"]);
});

test("a row is known by its file name; the revision follows", () => {
  assert.equal(entryName(file("Rover.iam")), "Rover.iam");
  assert.equal(entryName(loose("PRT-000007", "Bought motor")), "Bought motor");
  assert.equal(entryName({ item_number: "PRT-000007" }), "PRT-000007");
  assert.equal(entryRevision(file("x.ipt", { item_number: "PRT-000042", code: "B" })), "PRT-000042/B");
  assert.equal(entryRevision({ item_number: "PRT-000042" }), "PRT-000042");
});

test("the icon follows what the file contains, else the item type", () => {
  assert.equal(entryIconType(file("a.idw", { format_kind: "drawing", item_type: "part" })), "document");
  assert.equal(entryIconType(file("a.iam", { format_kind: "assembly", item_type: "part" })), "assembly");
  assert.equal(entryIconType(file("a.ipt", { format_kind: "part", item_type: "assembly" })), "part");
  assert.equal(entryIconType(file("a.step", { format_kind: "exchange", item_type: "assembly" })), "assembly");
  assert.equal(entryIconType(file("a.bin", { item_type: "software" })), "software");
  assert.equal(entryIconType(loose("DOC-000001", "Plan", { item_type: "document" })), "document");
});

test("the tooltip carries what the row leaves out", () => {
  const label = (s) => ({ in_work: "WIP", released: "Released" })[s];
  assert.equal(entryTitle(file("Bracket.ipt", { name: "Bracket", format_label: "Inventor part", state: "released" }), label),
    "PRT-000001/A – Bracket · Inventor part · Released");
  assert.equal(entryTitle(file("notes.txt", { name: "Notes", checked_out_by_name: "Lena" }), label),
    "PRT-000001/A – Notes · file · WIP · checked out by Lena");
  assert.equal(entryTitle(loose("PRT-000007", "Bought motor")), "PRT-000007/A – Bought motor · no files yet · in_work");
});

test("a further page is added once", () => {
  const a = file("a.ipt"), b = file("b.ipt"), c = file("c.ipt"), l = loose("PRT-000009", "Loose");
  assert.deepEqual(mergeEntries([a, b], [b, c, l]).map(entryName), ["a.ipt", "b.ipt", "c.ipt", "Loose"]);
  assert.deepEqual(mergeEntries([l], [l]).length, 1);
  assert.deepEqual(mergeEntries(null, [a]).length, 1);
  assert.equal(remaining(120, 50), 70);
  assert.equal(remaining(3, 5), 0);
  assert.equal(remaining(undefined, 0), 0);
});

test("the rows of a group: its top entries and the files in its folders", () => {
  assert.equal(groupTotal({ folders: [{ files: 521 }, { files: 3 }], total: 7 }), 531);
  assert.equal(groupTotal({ folders: [], total: 0 }), 0);
  assert.equal(groupTotal({}), 0);
});

test("folder keys keep the groups apart", () => {
  assert.equal(folderKey("ROVER", "chassis/brackets"), "dir:ROVER:chassis/brackets");
  assert.equal(folderKey("ROVER", ""), "dir:ROVER:");
  assert.notEqual(folderKey("ROVER", "a"), folderKey("ROCKET", "a"));
});

test("counts read naturally", () => {
  assert.equal(countText(1, "file", "files"), "1 file");
  assert.equal(countText(0, "file", "files"), "0 files");
  assert.equal(countText(12, "file", "files"), "12 files");
});

test("New item starts in the folder last opened in the group", () => {
  assert.equal(parentFolder("chassis/brackets"), "chassis");
  assert.equal(parentFolder("chassis"), "");
  assert.equal(parentFolder(""), "");
  const remembered = { ROVER: "chassis/brackets" };
  assert.equal(newItemFolder(remembered, "ROVER"), "chassis/brackets");
  assert.equal(newItemFolder(remembered, "ROCKET"), "");
  assert.equal(newItemFolder(undefined, "ROVER"), "");
});

test("the favourite folder is kept per server and group (#352)", () => {
  const a = favouriteKey("https://plm.example.org", "ROVER");
  assert.equal(a, 'teamcraft.home-root:["https://plm.example.org","ROVER"]');
  assert.notEqual(a, favouriteKey("https://plm.example.org", "ROCKET"));
  assert.notEqual(a, favouriteKey("http://127.0.0.1:8080", "ROVER"));
  // no code can be read as another server's: the parts are quoted
  assert.notEqual(favouriteKey("a", "b:c"), favouriteKey("a:b", "c"));
  assert.notEqual(a, suggestionKey("https://plm.example.org", "ROVER"));
  assert.equal(favouriteKey(null, undefined), 'teamcraft.home-root:["",""]');
});

test("a stored favourite is a folder as the server writes it, or the top", () => {
  assert.equal(normalizeFavourite("Design/Rover"), "Design/Rover");
  assert.equal(normalizeFavourite("\\Design\\Rover\\"), "Design/Rover");
  assert.equal(normalizeFavourite("/Design//./Rover/"), "Design/Rover");
  assert.equal(normalizeFavourite("100%/a_b"), "100%/a_b");
  for (const bad of [null, undefined, 42, {}, "", "/", "..", "Design/../outside"]) assert.equal(normalizeFavourite(bad), "", String(bad));
});

test("where the favourite folder is in its group", () => {
  assert.equal(favouritePath("ROVER", "Design/Rover"), "ROVER / Design / Rover");
  assert.equal(favouritePath("ROVER", ""), "ROVER");
});

test("the folders with an Inventor project file are suggested – at most 3, never twice", () => {
  const top = { project_folders: ["Design/arm", "Design/Rover", "Design/Rover", "", "Design/x", "Design/y"] };
  assert.deepEqual(favouriteSuggestions(top, "", false), ["Design/arm", "Design/Rover", "Design/x"]);
  // none while the group has a favourite or the suggestion was declined
  assert.deepEqual(favouriteSuggestions(top, "Design/arm", false), []);
  assert.deepEqual(favouriteSuggestions(top, "", true), []);
  // an older server does not name them; nothing loaded yet
  assert.deepEqual(favouriteSuggestions({ folders: [] }, "", false), []);
  assert.deepEqual(favouriteSuggestions({ project_folders: "Design" }, "", false), []);
  assert.deepEqual(favouriteSuggestions(null, "", false), []);
});

test("a checked-out row gets the one marker class, whoever holds the lock (#362)", () => {
  assert.equal(checkedOutClass(file("a.ipt", { checked_out_by_name: "Ada" })), ".checked-out");
  assert.equal(checkedOutClass(file("a.ipt", { checked_out_by_name: "You" })), ".checked-out");
  assert.equal(checkedOutClass(file("a.ipt")), "");
  assert.equal(checkedOutClass(null), "");
});

test("a row names its preview image when the server gave one (#420)", () => {
  // Files rows: the file's own picture, when its format has one
  assert.deepEqual(rowThumb(file("a.ipt", { thumbnail: true, sha256: "ab" })), { id: "f-a.ipt", sha: "ab" });
  assert.equal(rowThumb(file("a.step", { thumbnail: false, sha256: "cd" })), null);
  assert.equal(rowThumb(file("a.ipt", { thumbnail: true, sha256: null })), null);
  assert.equal(rowThumb(loose("PRT-000002", "Bolt", { thumbnail: true })), null);
  // All items, search, structure: the main thumbnail of the revision
  assert.deepEqual(rowThumb({ number: "ASM-000001", thumbnail_file_id: "f1", thumbnail_sha256: "ef" }), { id: "f1", sha: "ef" });
  assert.equal(rowThumb({ number: "ASM-000001", thumbnail_file_id: null, thumbnail_sha256: null }), null);
  // an older server sends neither; a restricted structure line neither
  assert.equal(rowThumb({ number: "PRT-000003" }), null);
  assert.equal(rowThumb({ restricted: true }), null);
  assert.equal(rowThumb(null), null);
});

test("the own check-outs that changed between two worklists (#451)", () => {
  const wl = (...refs) => ({ checkouts: refs.map((r) => ({ item_number: r.split("/")[0], code: r.split("/")[1], name: "x" })) });
  assert.deepEqual(lockChanges(wl("PRT-1/A"), wl("PRT-1/A", "PRT-2/B")), { taken: [{ item: "PRT-2", code: "B" }], released: [] });
  assert.deepEqual(lockChanges(wl("PRT-1/A", "PRT-2/B"), wl("PRT-2/B")), { taken: [], released: [{ item: "PRT-1", code: "A" }] });
  // a new revision of the same item is another lock
  assert.deepEqual(lockChanges(wl("PRT-1/A"), wl("PRT-1/B")), { taken: [{ item: "PRT-1", code: "B" }], released: [{ item: "PRT-1", code: "A" }] });
  assert.deepEqual(lockChanges(wl("PRT-1/A"), wl("PRT-1/A")), { taken: [], released: [] });
  // the first worklist of a session changes nothing; a missing list is empty
  assert.deepEqual(lockChanges(null, wl("PRT-1/A")), { taken: [], released: [] });
  assert.deepEqual(lockChanges({}, wl("PRT-1/A")), { taken: [{ item: "PRT-1", code: "A" }], released: [] });
});

test("the marks of Home after the own check-outs changed: taken by me, released by nobody", () => {
  const wl = (...refs) => ({ checkouts: refs.map((r) => ({ item_number: r.split("/")[0], code: r.split("/")[1], name: "x" })) });
  const me = { id: "u-1", display_name: "Alex Meyer" };
  assert.deepEqual(lockMarks(wl("PRT-1/A"), wl("PRT-2/B"), me), [
    { item: "PRT-2", code: "B", holder: "Alex Meyer", holderId: "u-1" },
    { item: "PRT-1", code: "A", holder: null, holderId: null }
  ]);
  // signed out in between: nobody to name
  assert.deepEqual(lockMarks(wl(), wl("PRT-2/B"), null), [{ item: "PRT-2", code: "B", holder: null, holderId: null }]);
  // the first worklist of a session (or after a sign-out) marks nothing
  assert.deepEqual(lockMarks(null, wl("PRT-2/B"), me), []);
  assert.deepEqual(lockMarks(wl("PRT-1/A"), wl("PRT-1/A"), me), []);
});

test("a lock is mine by the holder's id, not by a display name another user may share", () => {
  const me = { id: "u-1", display_name: "Alex Meyer" };
  assert.equal(isMineLock({ checked_out_by: "u-1", checked_out_by_name: "Alex Meyer" }, me), true);
  // a namesake is someone else
  assert.equal(isMineLock({ checked_out_by: "u-2", checked_out_by_name: "Alex Meyer" }, me), false);
  // renamed by an administrator after GET /me: still mine
  assert.equal(isMineLock({ checked_out_by: "u-1", checked_out_by_name: "Alex M. Meyer" }, me), true);
  assert.equal(isMineLock({ checked_out_by: null, checked_out_by_name: null }, me), false);
  assert.equal(isMineLock({ checked_out_by: "u-1", checked_out_by_name: "Alex Meyer" }, null), false);
  assert.equal(isMineLock(null, me), false);
  // an older server sends only the name
  assert.equal(isMineLock({ checked_out_by_name: "Alex Meyer" }, me), true);
  assert.equal(isMineLock({ checked_out_by_name: "Kim" }, me), false);
});

test("a row's tooltip follows its lock: a check-out adds the holder, a check-in removes it", () => {
  const row = file("bracket.ipt", { name: "Bracket" });
  const taken = relockedEntry(row, "Alex Meyer", "u-1");
  assert.match(entryTitle(taken), /checked out by Alex Meyer$/);
  assert.equal(taken.checked_out_by, "u-1");
  assert.equal(row.checked_out_by_name, null, "the row's own entry is not changed");
  const released = relockedEntry(taken, null, null);
  assert.doesNotMatch(entryTitle(released), /checked out/);
  assert.equal(released.checked_out_by, null);
  // marked twice: the text is there once
  const again = relockedEntry(relockedEntry(taken, "Alex Meyer", "u-1"), "Alex Meyer", "u-1");
  assert.equal(entryTitle(again).split("checked out by").length, 2);
});

console.log(passed + " tests passed");
