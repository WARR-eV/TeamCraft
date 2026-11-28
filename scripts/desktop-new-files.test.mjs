// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/new-files.js (new items from files on this
// computer, #359: placement, duplicate findings, worklist suggestions).
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-new-files.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const source = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/new-files.js");
const {
  additionPrompt, ageText, batchSummary, companionNote, companionReasonText, folderText, isNewModelOfItsOwn, kindOf, nameParts, newItemFindings, newItemKeyText,
  newItemPlacement, newItemSummary, planNewFiles, workspacePath
} = await import("data:text/javascript;base64," + fs.readFileSync(source).toString("base64"));

let passed = 0;
function test(name, fn) {
  fn();
  passed++;
  console.log("ok - " + name);
}

const kinds = { ipt: "part", iam: "assembly", idw: "drawing", step: "exchange", stp: "exchange", pdf: "document", ipj: "project" };
const place = (o) => Object.assign({ path: "/x/" + o.name, inside: false, project: null, folder: "", sha256: "a".repeat(64), size: 1, tracked: null, pending: null }, o);

test("names and kinds", () => {
  assert.deepEqual(nameParts("Bracket.IPT"), { stem: "Bracket", ext: "ipt" });
  assert.deepEqual(nameParts("README"), { stem: "README", ext: "" });
  assert.deepEqual(nameParts(".hidden"), { stem: ".hidden", ext: "" });
  assert.equal(kindOf("a.iam", kinds), "assembly");
  assert.equal(kindOf("a.xyz", kinds), "");
  assert.equal(folderText(""), "the top of the group");
  assert.equal(folderText("Design/Struktur"), "Design/Struktur");
});

test("a file in the working folder brings its group and folder", () => {
  const places = [
    place({ name: "Bracket.idw", inside: true, project: "GRAKSLER", folder: "Zeichnungen" }),
    place({ name: "Bracket.ipt", inside: true, project: "GRAKSLER", folder: "Design/Struktur" })
  ];
  const p = newItemPlacement(places, kinds);
  assert.equal(p.inside, true);
  assert.equal(p.project, "GRAKSLER");
  assert.equal(p.folder, "Design/Struktur", "the folder of the model file, not of the first file");
  assert.deepEqual(p.targets, ["Zeichnungen", "Design/Struktur"], "every file keeps its own folder");
  assert.equal(p.main, 1, "the model file names the item");
  assert.deepEqual(p.problems, []);
});

test("a file from elsewhere joins the item's folder; none inside: the dialog decides", () => {
  const mixed = newItemPlacement([
    place({ name: "Bracket.ipt", inside: true, project: "GRAKSLER", folder: "Design" }),
    place({ name: "Bracket.step" })
  ], kinds);
  assert.deepEqual(mixed.targets, ["Design", "Design"]);
  const outside = newItemPlacement([place({ name: "Vendor.step" })], kinds);
  assert.equal(outside.inside, false);
  assert.deepEqual(outside.targets, [null]);
  assert.equal(outside.main, 0);
  assert.equal(newItemPlacement([place({ name: "a.step" }), place({ name: "a.iam", path: "/x/a2" })], kinds).main, 1);
  assert.equal(newItemPlacement([], kinds).main, -1);
  assert.deepEqual(newItemPlacement([], kinds).problems, []);
});

test("two groups or two files of one name are refused", () => {
  const two = newItemPlacement([
    place({ name: "a.ipt", inside: true, project: "GRAKSLER", folder: "" }),
    place({ name: "b.ipt", inside: true, project: "CRATER", folder: "" })
  ], kinds);
  assert.equal(two.problems.length, 1);
  assert.match(two.problems[0], /different project groups \(GRAKSLER, CRATER\)/);
  const same = newItemPlacement([place({ name: "a.ipt" }), place({ name: "A.IPT", path: "/y/A.IPT" })], kinds);
  assert.match(same.problems[0], /Two of the chosen files are called A\.IPT/);
});

test("findings: tracked and added files and the same place block, the same name asks, the same content warns", () => {
  const places = [
    place({ name: "t.ipt", tracked: "PRT-000001/A" }),
    place({ name: "p.ipt", pending: "ASM-000002/A" }),
    place({ name: "s.ipt" }),
    place({ name: "n.ipt" }),
    place({ name: "c.ipt" }),
    place({ name: "ok.ipt" })
  ];
  const targets = ["", "", "Design", "Design", "", ""];
  const check = [
    null, null,
    { same_place: { item_number: "PRT-000003", code: "B", name: "S" }, same_name: [], same_content: [] },
    { same_place: null, same_name: [{ item_number: "PRT-000004", code: "A", folder: "Normteile", filename: "n.ipt" }], same_content: [] },
    { same_place: null, same_name: [], same_content: [{ item_number: "PRT-000005", code: "A", folder: "", filename: "old.ipt" }] },
    { same_place: null, same_name: [], same_content: [] }
  ];
  const f = newItemFindings(places, targets, check);
  assert.equal(f.blocking.length, 3);
  assert.match(f.blocking[0], /t\.ipt belongs to PRT-000001\/A/);
  assert.match(f.blocking[1], /p\.ipt was added to ASM-000002\/A/);
  assert.match(f.blocking[2], /s\.ipt in Design belongs to PRT-000003\/B/);
  assert.equal(f.sameName.length, 1);
  assert.match(f.sameName[0], /n\.ipt exists in Normteile \(PRT-000004\/A\)/);
  assert.equal(f.warnings.length, 1);
  assert.match(f.warnings[0], /same content as old\.ipt in the top of the group \(PRT-000005\/A\)/);
});

test("an older server without the check: only the local findings", () => {
  const f = newItemFindings([place({ name: "a.ipt" }), place({ name: "b.ipt", tracked: "PRT-1/A" })], ["", ""], null);
  assert.deepEqual(f.sameName, []);
  assert.equal(f.blocking.length, 1);
});

test("the idempotency key text follows group, folder, name (case-blind) and content", () => {
  const a = newItemKeyText("GRAKSLER", "Design", "Bracket.ipt", "ab");
  assert.equal(a, newItemKeyText("GRAKSLER", "Design", "BRACKET.IPT", "ab"));
  assert.notEqual(a, newItemKeyText("GRAKSLER", "Design", "Bracket.ipt", "cd"));
  assert.notEqual(a, newItemKeyText("GRAKSLER", "Other", "Bracket.ipt", "ab"));
  assert.notEqual(newItemKeyText("A", "b", "c", "d"), newItemKeyText("A", "", "b\u0000c", "d"));
});

test("the summary says where, how many files and that it stays checked out", () => {
  assert.equal(newItemSummary("GRAKSLER", "Design/Struktur", 2, 0),
    "Creates a new item in GRAKSLER / Design/Struktur, uploads 2 files – it stays checked out to you.");
  assert.match(newItemSummary("GRAKSLER", "", 1, 1), /in GRAKSLER, uploads 1 file \(1 copy into the working folder first\)/);
  assert.match(newItemSummary("GRAKSLER", "", 0, 0), /without files/);
});

test("worklist suggestions: one item per model with its drawing and export, newest first", () => {
  const files = [
    { rel: "GRAKSLER/Design/Bracket.ipt", project: "GRAKSLER", folder: "Design", name: "Bracket.ipt", modified_ms: 100 },
    { rel: "GRAKSLER/Design/Bracket.step", project: "GRAKSLER", folder: "Design", name: "Bracket.step", modified_ms: 300 },
    { rel: "GRAKSLER/Design/bracket.idw", project: "GRAKSLER", folder: "Design", name: "bracket.idw", modified_ms: 50 },
    { rel: "GRAKSLER/Other/Bracket.ipt", project: "GRAKSLER", folder: "Other", name: "Bracket.ipt", modified_ms: 200 },
    { rel: "GRAKSLER/Asm.iam", project: "GRAKSLER", folder: "", name: "Asm.iam", modified_ms: 400 },
    { rel: "GRAKSLER/Lonely.idw", project: "GRAKSLER", folder: "", name: "Lonely.idw", modified_ms: 500 },
    { rel: "GRAKSLER/notes.pdf", project: "GRAKSLER", folder: "", name: "notes.pdf", modified_ms: 600 },
    { rel: "CRATER/x.ipt", project: "CRATER", folder: "", name: "x.ipt", modified_ms: 700 }
  ];
  const plan = planNewFiles(files, kinds, (code) => code === "GRAKSLER");
  assert.deepEqual(plan.suggestions.map((s) => [s.project, s.folder, s.name, s.type, s.files.length]), [
    ["GRAKSLER", "", "Asm", "assembly", 1],
    ["GRAKSLER", "Design", "Bracket", "part", 3],
    ["GRAKSLER", "Other", "Bracket", "part", 1]
  ]);
  assert.equal(plan.suggestions[1].files[0].name, "Bracket.ipt", "the model file first");
  assert.equal(plan.suggestions[1].modified_ms, 300, "the newest file of the group counts");
  assert.deepEqual(plan.other.map((f) => f.rel), ["GRAKSLER/Lonely.idw", "GRAKSLER/notes.pdf"],
    "a drawing without its model and other files are not suggested");
  assert.equal(plan.suggestions.some((s) => s.project === "CRATER"), false, "only groups one may create items in");
});

test("the batch summary counts per group and folder and names the refused", () => {
  assert.equal(batchSummary([{ project: "G", folder: "A" }, { project: "G", folder: "A" }, { project: "G", folder: "" }], []),
    "Creates 3 items in G (1), G / A (2).");
  assert.match(batchSummary([{ project: "G", folder: "" }], ["x.ipt: exists in Normteile"]), /1 item in G \(1\)\. 1 refused: x\.ipt: exists in Normteile/);
  assert.equal(batchSummary([], []), "Creates 0 items.");
});

test("ages", () => {
  const now = 1_700_000_000_000;
  assert.equal(ageText(0, now), "");
  assert.equal(ageText(now - 5_000, now), "just now");
  assert.equal(ageText(now - 120_000, now), "2 min");
  assert.equal(ageText(now - 7_200_000, now), "2 h");
  assert.equal(ageText(now - 3 * 86_400_000, now), "3 d");
  assert.equal(ageText(now + 60_000, now), "just now", "a clock ahead is not negative");
});

test("check-in: a second model file is a new item of its own, drawings and exports are not", () => {
  const ipt = { name: "Bracket.ipt", state: "modified" };
  const iam = { name: "Bracket.iam", state: "new" };
  const step = { name: "Bracket.step", state: "new" };
  assert.equal(isNewModelOfItsOwn(iam, [ipt, iam, step], kinds), true);
  assert.equal(isNewModelOfItsOwn(step, [ipt, iam, step], kinds), false);
  assert.equal(isNewModelOfItsOwn(ipt, [ipt, iam], kinds), false, "not new");
  // a revision without a model (a document item getting its first model) keeps it ticked
  assert.equal(isNewModelOfItsOwn(iam, [{ name: "Bracket.pdf", state: "up_to_date" }, iam], kinds), false);
  assert.equal(isNewModelOfItsOwn(null, [], kinds), false);
});

test("working-folder paths use the folder's separator", () => {
  assert.equal(workspacePath("C:\\TeamCraft\\", "GRAKSLER/Design/a.ipt"), "C:\\TeamCraft\\GRAKSLER\\Design\\a.ipt");
  assert.equal(workspacePath("/home/u/TeamCraft", "GRAKSLER/a.ipt"), "/home/u/TeamCraft/GRAKSLER/a.ipt");
  assert.equal(workspacePath("/ws/", "/G//a.ipt"), "/ws/G/a.ipt");
});

test("#564: a supplier download joins the new part saved from it, in another folder", () => {
  const plan = planNewFiles([
    { rel: "G/Buy/ISWB25-64.step", project: "G", folder: "Buy", name: "ISWB25-64.step", modified_ms: 2,
      companion_of: "G/Buy/ISWB25-64/ISWB25-64.ipt", reason: "saved_from" },
    { rel: "G/Buy/ISWB25-64/ISWB25-64.ipt", project: "G", folder: "Buy/ISWB25-64", name: "ISWB25-64.ipt", modified_ms: 1 },
    { rel: "G/Buy/orphan.step", project: "G", folder: "Buy", name: "orphan.step", companion_of: "G/gone.ipt", reason: "same_name" }
  ], kinds);
  assert.equal(plan.suggestions.length, 1);
  const s = plan.suggestions[0];
  assert.deepEqual(s.files.map((f) => f.name), ["ISWB25-64.ipt", "ISWB25-64.step"], "the model first, the download along");
  assert.equal(s.folder, "Buy/ISWB25-64");
  assert.equal(companionNote(s), "ISWB25-64.step goes along – the part was saved from the download (the part names it)");
  assert.deepEqual(plan.other.map((f) => f.name), ["orphan.step"], "a companion of a part that is not new stays an other file");
  assert.deepEqual(plan.additions, []);
});

test("#564: a supplier download of a tracked part is offered as Add to item, explained", () => {
  const owner = { project: "G", item: "PRT-387", revision: "A", file: "G/Buy/ISWB25-64/ISWB25-64.ipt" };
  const plan = planNewFiles([
    { rel: "G/Buy/ISWB25-64.step", project: "G", folder: "Buy", name: "ISWB25-64.step", belongs_to: owner, reason: "same_name_below" }
  ], kinds);
  assert.equal(plan.suggestions.length, 0);
  assert.equal(plan.other.length, 0);
  assert.equal(plan.additions.length, 1);
  const p = additionPrompt(plan.additions[0]);
  assert.equal(p.title, "Add ISWB25-64.step to PRT-387/A?");
  assert.equal(p.button, "Add to PRT-387");
  assert.match(p.lines[0], /belongs to ISWB25-64\.ipt of PRT-387\/A: same name – the part is in a folder below/);
  assert.match(p.lines.join(" "), /in its folder \(G \/ Buy\)/);
  assert.match(p.lines.join(" "), /checks it out for this and checks it in again/);
  // groups the user may not edit are left out, as for suggestions
  assert.equal(planNewFiles([{ rel: "X/a.step", project: "X", folder: "", name: "a.step", belongs_to: owner }], kinds, () => false).additions.length, 0);
  assert.equal(companionReasonText("unknown"), "it belongs to the part");
});

console.log(passed + " tests passed");
