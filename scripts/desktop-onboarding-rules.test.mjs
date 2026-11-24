// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/onboarding-rules.js (the steps of the
// first-start wizard *Getting started* – #422) against the real message
// catalogs of shared/i18n/.
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-onboarding-rules.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const src = fs.readFileSync(path.join(root, "desktop/ui/js/onboarding-rules.js"), "utf8");
const R = await import("data:text/javascript;base64," + Buffer.from(src).toString("base64"));
const {
  ONBOARDING_STEPS, ONBOARDING_DONE, ONBOARDING_DISMISSED, ONBOARDING_MESSAGE_IDS, STEP_NAMES, MAX_ASSEMBLIES,
  shouldShow, skipReason, visibleSteps, nextStep, isFinal, stepPosition, stepSummary, startGroup, chosenGroup,
  groupList, groupFolder, pickAssemblies, lockedByMe, cadLine, groupCadText, openLine, showTitle, finishLabel, roleMessage, loopTip, loopTipAfter
} = R;
const catalog = (lang) => JSON.parse(fs.readFileSync(path.join(root, "shared/i18n", lang + ".json"), "utf8"));
const placeholders = (text) => new Set([...String(text).matchAll(/\{([A-Za-z0-9_]+)\}/g)].map((m) => m[1]));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

const ROVER = { code: "ROVER", name: "Rover sensor mast", cad: "inventor", cadVersion: "2026" };
const ARM = { code: "ARM", name: "Robot arm", cad: "", cadVersion: "" };
// a user right after the first sign-in: two groups, nothing on disk yet
const fresh = { groups: [ROVER, ARM], groupsOnDisk: [], workspaceDir: "/home/u/TeamCraft", editor: true, login: "erika" };

test("the three steps and their names", () => {
  assert.deepEqual(ONBOARDING_STEPS, ["folder", "group", "open"]);
  for (const s of ONBOARDING_STEPS) assert.ok(STEP_NAMES[s], "a name for " + s);
});

test("the stored values match the Rust side", () => {
  assert.equal(ONBOARDING_DONE, "done");
  assert.equal(ONBOARDING_DISMISSED, "dismissed");
});

test("shown on the first start after the sign-in", () => {
  assert.equal(shouldShow({}, { signedInThisRun: true }), true);
  assert.equal(shouldShow({ onboarding: null }, { signedInThisRun: true }), true);
  assert.equal(shouldShow({ onboarding: "later" }, { signedInThisRun: true }), true, "an unknown value counts as not done");
});

test("never again on this connection once done or dismissed", () => {
  assert.equal(shouldShow({ onboarding: "done" }, { signedInThisRun: true }), false);
  assert.equal(shouldShow({ onboarding: "dismissed" }, { signedInThisRun: true }), false);
});

test("not for an app that starts signed in (an update, a reload)", () => {
  assert.equal(shouldShow({}, { signedInThisRun: false }), false);
  assert.equal(shouldShow({}, {}), false);
  assert.equal(shouldShow(null, undefined), false);
});

test("not for returning users: own check-outs or an own check-in on this server", () => {
  assert.equal(shouldShow({}, { signedInThisRun: true, hasCheckouts: true }), false);
  assert.equal(shouldShow({}, { signedInThisRun: true, hasCheckedIn: true }), false);
  assert.equal(shouldShow({}, { signedInThisRun: true, hasCheckouts: false, hasCheckedIn: false }), true);
});

test("Help → Getting started always opens it", () => {
  assert.equal(shouldShow({ onboarding: "done" }, { fromHelp: true }), true);
  assert.equal(shouldShow({ onboarding: "dismissed" }, { fromHelp: true, hasCheckouts: true }), true);
  assert.equal(shouldShow(null, { fromHelp: true }), true);
});

test("groups: objects or plain codes, empty ones dropped", () => {
  assert.deepEqual(groupList(["ROVER", "", null, 7, { code: " ARM " }]).map((g) => g.code), ["ROVER", "ARM"]);
  assert.deepEqual(groupList(undefined), []);
});

test("the folder step: skipped when a policy sets the root or the folder is in use", () => {
  assert.equal(skipReason("folder", fresh), null);
  assert.equal(skipReason("folder", { ...fresh, workspaceRootPolicy: true }), "onboarding.skip.folder_policy");
  assert.equal(skipReason("folder", { ...fresh, groupsOnDisk: ["rover"] }), "onboarding.skip.folder_used");
});

test("the group step: shown with several groups and with none, skipped with one", () => {
  assert.equal(skipReason("group", fresh), null);
  assert.equal(skipReason("group", { ...fresh, groups: [ROVER] }), "onboarding.skip.one_group");
  assert.equal(skipReason("group", { ...fresh, groups: [] }), null);
});

test("the assembly step: never skipped while the user has a group", () => {
  assert.equal(skipReason("open", fresh), null);
  assert.equal(skipReason("open", { ...fresh, groups: [ROVER] }), null);
  assert.equal(skipReason("open", { ...fresh, groups: [] }), "onboarding.skip.no_group");
});

test("an unknown step is skipped", () => {
  assert.equal(skipReason("server", fresh), "onboarding.skip.unknown");
  assert.equal(skipReason("cad", fresh), "onboarding.skip.unknown");
});

test("visible steps per situation", () => {
  assert.deepEqual(visibleSteps(fresh), ["folder", "group", "open"]);
  assert.deepEqual(visibleSteps({ ...fresh, groups: [ROVER] }), ["folder", "open"]);
  assert.deepEqual(visibleSteps({ ...fresh, groups: [ROVER], groupsOnDisk: ["ROVER"] }), ["open"]);
  assert.deepEqual(visibleSteps({ ...fresh, groups: [] }), ["folder", "group"]);
  assert.deepEqual(visibleSteps({ ...fresh, groups: [], workspaceRootPolicy: true }), ["group"]);
});

test("without a group the group step is the last one", () => {
  const none = { ...fresh, groups: [] };
  assert.equal(isFinal("group", none), true);
  assert.equal(isFinal("folder", none), false);
  assert.equal(isFinal("open", fresh), true);
  assert.equal(isFinal("group", fresh), false);
  assert.equal(isFinal("open", none), false, "a skipped step is never the final one");
});

test("next and back walk the shown steps", () => {
  const order = [];
  let s = nextStep({ current: null }, fresh);
  while (s) { order.push(s); s = nextStep({ current: s }, fresh); }
  assert.deepEqual(order, ["folder", "group", "open"]);
  assert.equal(nextStep({ current: "open", back: true }, fresh), "group");
  assert.equal(nextStep({ current: "open", back: true }, { ...fresh, groups: [ROVER] }), "folder");
  assert.equal(nextStep({ current: "folder", back: true }, fresh), null);
  assert.equal(nextStep({ current: "nope" }, fresh), "folder");
  assert.equal(nextStep({}, { ...fresh, workspaceRootPolicy: true }), "group");
});

test("step positions count shown steps only", () => {
  assert.deepEqual(stepPosition("folder", fresh), { id: "onboarding.position", params: { at: 1, total: 3 } });
  assert.deepEqual(stepPosition("open", { ...fresh, groups: [ROVER] }), { id: "onboarding.position", params: { at: 2, total: 2 } });
  assert.equal(stepPosition("group", { ...fresh, groups: [ROVER] }), null);
});

test("the start group: one already on disk first, else the first", () => {
  assert.equal(startGroup(fresh), "ROVER");
  assert.equal(startGroup({ ...fresh, groupsOnDisk: ["arm"] }), "ARM");
  assert.equal(startGroup({ groups: [] }), null);
  assert.equal(startGroup({}), null);
});

test("the chosen group: the user's pick when it is theirs, else the start group", () => {
  assert.equal(chosenGroup(fresh, "arm").code, "ARM");
  assert.equal(chosenGroup(fresh, "NOPE").code, "ROVER");
  assert.equal(chosenGroup(fresh, null).code, "ROVER");
  assert.equal(chosenGroup({ groups: [] }, "ARM"), null);
});

test("the group folder keeps the separator of the working folder", () => {
  assert.equal(groupFolder("/home/u/TeamCraft", "ROVER"), "/home/u/TeamCraft/ROVER");
  assert.equal(groupFolder("/home/u/TeamCraft/", "ROVER"), "/home/u/TeamCraft/ROVER");
  assert.equal(groupFolder("C:\\Users\\u\\Documents\\TeamCraft", "ROVER"), "C:\\Users\\u\\Documents\\TeamCraft\\ROVER");
  assert.equal(groupFolder("", "ROVER"), "ROVER");
});

const node = (number, extra) => ({ item_number: number, code: "A", name: number, item_type: "assembly", state: "in_work", bom_lines: 3, checked_out_by: null, checked_out_by_name: null, restricted: false, ...extra });
const ME = { id: "u-erika", name: "Erika" };

test("assemblies: the most BOM lines first, then the number", () => {
  const picks = pickAssemblies([
    node("ASM-3", { bom_lines: 2 }),
    node("ASM-2", { bom_lines: 9 }),
    node("ASM-0", { bom_lines: 2 })
  ], { me: ME, editor: true });
  assert.deepEqual(picks.map((x) => x.item_number), ["ASM-2", "ASM-0", "ASM-3"]);
});

test("assemblies for an editor: only in work – released, in review and obsolete are left out", () => {
  const picks = pickAssemblies([
    node("ASM-W"),
    node("ASM-R", { state: "released", bom_lines: 9 }),
    node("ASM-V", { state: "in_review", bom_lines: 9 }),
    node("ASM-O", { state: "obsolete", bom_lines: 9 })
  ], { me: ME, editor: true });
  assert.deepEqual(picks.map((x) => x.item_number), ["ASM-W"], "Check out and open works on these");
  assert.equal(pickAssemblies([node("ASM-R", { state: "released" })], { me: ME }).length, 0, "editor is the default");
});

test("assemblies for a viewer: any state, locks do not matter", () => {
  const picks = pickAssemblies([
    node("ASM-W"),
    node("ASM-R", { state: "released", bom_lines: 9 }),
    node("ASM-L", { checked_out_by: "u-max", checked_out_by_name: "Max", bom_lines: 1 })
  ], { me: ME, editor: false });
  assert.deepEqual(picks.map((x) => x.item_number), ["ASM-R", "ASM-W", "ASM-L"]);
});

test("assemblies: parts, placeholders, no revision and locked by others are left out", () => {
  const picks = pickAssemblies([
    node("PRT-1", { item_type: "part", bom_lines: 0 }),
    node("PRT-2", { item_type: "part", bom_lines: 2 }),
    node("ASM-R", { restricted: true }),
    node("ASM-N", { code: null }),
    node("ASM-O", { checked_out_by: "u-max", checked_out_by_name: "Max" }),
    node("ASM-M", { checked_out_by: "u-erika", checked_out_by_name: "Erika" })
  ], { me: ME, editor: true });
  assert.deepEqual(picks.map((x) => x.item_number), ["ASM-M", "PRT-2"], "an item with BOM lines counts; my own lock does not hide it");
});

test("mine is decided by user id; the display name only without one", () => {
  assert.equal(lockedByMe(node("A", { checked_out_by: "u-other", checked_out_by_name: "Erika" }), ME), false, "another Erika");
  assert.equal(lockedByMe(node("A", { checked_out_by: "u-erika", checked_out_by_name: "Erika M." }), ME), true);
  assert.equal(lockedByMe(node("A", { checked_out_by_name: "Erika" }), ME), true, "an older server without the id");
  assert.equal(lockedByMe(node("A", { checked_out_by_name: "Max" }), ME), false);
  assert.equal(lockedByMe(node("A"), ME), false, "not checked out");
  const picks = pickAssemblies([node("ASM-T", { checked_out_by: "u-other", checked_out_by_name: "Erika" })], { me: ME, editor: true });
  assert.deepEqual(picks, [], "a namesake's lock is not mine");
});

test("assemblies: at most five, an empty group none", () => {
  const many = Array.from({ length: 9 }, (_, i) => node("ASM-" + i));
  assert.equal(MAX_ASSEMBLIES, 5);
  assert.equal(pickAssemblies(many).length, 5);
  assert.equal(pickAssemblies(many, { n: 2 }).length, 2);
  assert.deepEqual(pickAssemblies([]), []);
  assert.deepEqual(pickAssemblies(undefined), []);
});

test("the CAD line names what was found, neutrally", () => {
  assert.deepEqual(cadLine([{ family: "inventor", label: "Inventor 2026" }, { family: "solidworks", label: "SolidWorks 2024" }, { family: "inventor", label: "Inventor 2026" }]),
    { id: "onboarding.cad.found", params: { programs: "Inventor 2026, SolidWorks 2024" } });
  assert.deepEqual(cadLine([]), { id: "onboarding.cad.none", params: {} });
  assert.deepEqual(cadLine(undefined), { id: "onboarding.cad.none", params: {} });
});

test("the group's default CAD program as text", () => {
  assert.equal(groupCadText(ROVER, { inventor: "Autodesk Inventor" }), "Autodesk Inventor 2026");
  assert.equal(groupCadText({ ...ROVER, cadVersion: "" }, {}), "inventor");
  assert.equal(groupCadText(ARM, {}), "");
  assert.equal(groupCadText(null, {}), "");
});

test("what Check out and open does follows the chip's plan (cad_open_plan)", () => {
  const opts = { dir: "/home/u/TeamCraft", names: { inventor: "Autodesk Inventor" }, editor: true };
  const folder = "/home/u/TeamCraft/ROVER";
  const exact = { program: { family: "inventor", version: "2026" }, label: "Inventor 2026", fallback: false };
  assert.deepEqual(openLine(ROVER, { ...opts, plan: exact }), { id: "onboarding.open.program", params: { folder, program: "Inventor 2026" } });
  // the group wants 2026, the plan took the newest installed one: both named
  const other = { program: { family: "inventor", version: "2027" }, label: "Inventor 2027", fallback: true };
  assert.deepEqual(openLine(ROVER, { ...opts, plan: other }),
    { id: "onboarding.open.program_other", params: { folder, program: "Inventor 2027", wanted: "Autodesk Inventor 2026" } });
  // no version wanted: the plan's (newest) program, never the first found
  const newest = { program: { family: "inventor", version: "2027" }, label: "Inventor 2027", fallback: false };
  assert.equal(openLine({ ...ROVER, cadVersion: "" }, { ...opts, plan: newest }).params.program, "Inventor 2027");
  const none = { program: null, label: "Default app", fallback: true };
  assert.deepEqual(openLine(ROVER, { ...opts, plan: none }),
    { id: "onboarding.open.program_missing", params: { folder, program: "Autodesk Inventor 2026" } });
  assert.equal(openLine(ROVER, { ...opts, plan: null }).id, "onboarding.open.any", "no plan (not asked or failed)");
  assert.equal(openLine(ARM, { ...opts, plan: exact }).id, "onboarding.open.any");
  assert.deepEqual(openLine(ROVER, { ...opts, plan: exact, editor: false }), { id: "onboarding.open.viewer", params: { folder } });
});

test("Show promises Check out and open only to an editor", () => {
  assert.deepEqual(showTitle("ROVER", "ASM-1", true), { id: "onboarding.open.show.title", params: { group: "ROVER", number: "ASM-1" } });
  assert.deepEqual(showTitle("ROVER", "ASM-1", false), { id: "onboarding.open.show.viewer_title", params: { group: "ROVER", number: "ASM-1" } });
  assert.ok(!/check out/i.test(catalog("en")["onboarding.open.show.viewer_title"]));
});

test("step texts by role and situation", () => {
  for (const s of ONBOARDING_STEPS) {
    const sum = stepSummary(s, fresh);
    assert.ok(sum.title, "title of " + s);
    assert.ok(sum.text.length > 0, "text of " + s);
  }
  assert.deepEqual(stepSummary("nope", fresh), { title: "", text: [] });
  assert.equal(stepSummary("folder", fresh).text[0].params.dir, "/home/u/TeamCraft");
  assert.deepEqual(stepSummary("group", fresh).text.map((m) => m.id), ["onboarding.group.question"]);
  assert.deepEqual(stepSummary("group", { ...fresh, editor: false }).text.map((m) => m.id), ["onboarding.group.question", "onboarding.group.viewer"]);
  assert.deepEqual(stepSummary("group", { ...fresh, groups: [] }).text.map((m) => m.id), ["onboarding.group.none.editor"]);
  const viewer = stepSummary("group", { ...fresh, groups: [], editor: false }).text[0];
  assert.equal(viewer.id, "onboarding.group.none.viewer");
  assert.equal(viewer.params.login, "erika");
});

test("finish labels and roles", () => {
  assert.deepEqual(finishLabel("ROVER"), { id: "onboarding.finish_home", params: { group: "ROVER" } });
  assert.deepEqual(finishLabel(null), { id: "onboarding.finish", params: {} });
  assert.equal(roleMessage("admin"), "onboarding.role.admin");
  assert.equal(roleMessage("editor"), "onboarding.role.editor");
  assert.equal(roleMessage("viewer"), "onboarding.role.viewer");
  assert.equal(roleMessage(undefined), "onboarding.role.viewer");
});

test("the daily-loop tip: while the next step is in the loop, until the first check-in or cancel", () => {
  for (const next of ["checkout", "checkout-open", "open", "checkin"]) {
    assert.deepEqual(loopTip(null, next), { show: "onboarding.tip.loop", store: null }, next);
  }
  for (const next of ["release", "revise", null]) assert.deepEqual(loopTip(null, next), { show: null, store: null });
  assert.equal(loopTipAfter(null, "park"), null, "parking keeps the tip");
  assert.equal(loopTipAfter(null, "checkin"), "learned");
  assert.equal(loopTipAfter(null, "cancel"), "learned");
  assert.deepEqual(loopTip("learned", "release"), { show: "onboarding.tip.loop_done", store: "done" }, "one closing line");
  assert.deepEqual(loopTip("done", "checkout-open"), { show: null, store: null });
  assert.equal(loopTipAfter("done", "checkin"), "done");
});

test("every message ID is in both catalogs with the placeholders of English", () => {
  const en = catalog("en"), de = catalog("de");
  const ids = new Set(ONBOARDING_MESSAGE_IDS);
  // everything the rules answer is listed
  const answered = [];
  for (const facts of [fresh, { ...fresh, groups: [] }, { ...fresh, groups: [], editor: false }, { ...fresh, editor: false }]) {
    for (const s of ONBOARDING_STEPS) {
      const sum = stepSummary(s, facts);
      answered.push(sum.title, ...sum.text.map((m) => m.id));
      const why = skipReason(s, facts);
      if (why) answered.push(why);
      const pos = stepPosition(s, facts);
      if (pos) answered.push(pos.id);
    }
  }
  answered.push(skipReason("folder", { workspaceRootPolicy: true }), skipReason("folder", { groupsOnDisk: ["x"] }), skipReason("group", { groups: ["x"] }), skipReason("x", {}));
  answered.push(cadLine([]).id, cadLine([{ label: "X" }]).id, finishLabel("X").id, finishLabel(null).id);
  answered.push(...["admin", "editor", "viewer"].map(roleMessage));
  answered.push(...["any", "program", "program_missing", "viewer"].map((k) => "onboarding.open." + k));
  answered.push(loopTip(null, "open").show, loopTip("learned", null).show);
  for (const id of answered) assert.ok(ids.has(id), id + " is in ONBOARDING_MESSAGE_IDS");
  for (const id of ONBOARDING_MESSAGE_IDS) {
    assert.equal(typeof en[id], "string", id + " is in en.json");
    assert.equal(typeof de[id], "string", id + " is in de.json");
    assert.deepEqual(placeholders(de[id]), placeholders(en[id]), id + ": the same placeholders");
  }
});

test("every message onboarding.js uses is listed", () => {
  const ui = fs.readFileSync(path.join(root, "desktop/ui/js/onboarding.js"), "utf8");
  const used = [...ui.matchAll(/t\("(onboarding\.[a-z0-9_.]+)"/g)].map((m) => m[1]);
  assert.ok(used.length > 5);
  for (const id of used) assert.ok(ONBOARDING_MESSAGE_IDS.includes(id), id);
});

test("nothing in the rules is shaped around one CAD program", () => {
  // the wizard names what the computer and the group have; no program,
  // project-file or search-path rule of its own (maintainer decision, #478)
  for (const word of ["ipj", "inventor", "load_options", "LinkMgmt", "solid-edge"]) {
    assert.ok(!src.toLowerCase().includes(word.toLowerCase()), "onboarding-rules.js mentions " + word);
  }
});

console.log(`${n} tests passed`);
