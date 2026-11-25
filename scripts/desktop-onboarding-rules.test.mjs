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
  groupList, groupFolder, pickAssemblies, noAssemblyMessage, lockedByMe, cadLine, groupCadText, openLine, showTitle, finishLabel, roleMessage, loopTip, loopTipAfter,
  startCard, CARD_STATUS
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
  // one I hold already would offer Open in CAD / Check in…, not Check out and open
  assert.deepEqual(picks.map((x) => x.item_number), ["PRT-2"], "an item with BOM lines counts; locked ones are left out, also my own");
});

test("no assembly to offer: the message says why", () => {
  assert.equal(noAssemblyMessage([], { editor: true }), "onboarding.open.none");
  assert.equal(noAssemblyMessage([node("PRT-1", { item_type: "part", bom_lines: 0 })], { editor: true }), "onboarding.open.none");
  // assemblies exist, but none is free and in work
  assert.equal(noAssemblyMessage([node("ASM-R", { state: "released" })], { editor: true }), "onboarding.open.none_free");
  assert.equal(noAssemblyMessage([node("ASM-L", { checked_out_by: "u-max" })]), "onboarding.open.none_free", "editor is the default");
  // a viewer sees every assembly: none at all is the only reason
  assert.equal(noAssemblyMessage([node("ASM-R", { state: "released" })], { editor: false }), "onboarding.open.none");
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

// ---------------------------------------------------------------- the card on Home

const DISMISSED = { onboarding: "dismissed" };
// nothing done yet: an editor with two groups, an empty worklist, no folder
const idle = { ...fresh, workspaceExists: false, worklist: { checkouts: [], has_checked_in: false } };
const statuses = (card) => card.steps.map((s) => s.step + ":" + s.status);
const idsOf = (list) => list.map((m) => m.id);

test("the card: only after the wizard was dismissed", () => {
  assert.ok(startCard(DISMISSED, idle), "closed early: the card");
  assert.equal(startCard({ onboarding: "done" }, idle), null, "finished, hidden or completed");
  assert.equal(startCard({ onboarding: null }, idle), null, "the wizard may still open by itself");
  assert.equal(startCard({}, idle), null);
  assert.equal(startCard({ onboarding: "later" }, idle), null, "an unknown value is no dismissal");
});

test("the card: nothing without settings or before the worklist answered", () => {
  assert.equal(startCard(null, idle), null);
  assert.equal(startCard(undefined, undefined, undefined), null);
  assert.equal(startCard(DISMISSED, { ...idle, worklist: null }), null, "no worklist yet: nothing decided");
  assert.equal(startCard(DISMISSED, { ...idle, worklist: undefined }), null);
  assert.equal(startCard(DISMISSED, {}), null);
});

test("the card: the wizard's three steps in order, nothing ticked at first", () => {
  const card = startCard(DISMISSED, idle);
  assert.deepEqual(card.steps.map((s) => s.step), ONBOARDING_STEPS);
  assert.deepEqual(card.steps.map((s) => s.name), ONBOARDING_STEPS.map((s) => STEP_NAMES[s]));
  assert.deepEqual(statuses(card), ["folder:open", "group:open", "open:open"]);
  assert.deepEqual(card.progress, { id: "onboarding.card.progress", params: { done: 0, total: 3 } });
  assert.deepEqual(idsOf(card.lead), ["onboarding.card.lead"]);
  assert.equal(card.show, true);
  assert.equal(card.complete, false);
  assert.equal(card.store, null, "an open card stores nothing");
  // what each open step needs, in the wizard's words where it has them
  assert.deepEqual(idsOf(card.steps[0].text), ["onboarding.folder.text", "onboarding.folder.missing"]);
  assert.equal(card.steps[0].text[0].params.dir, "/home/u/TeamCraft");
  assert.deepEqual(card.steps[1].text, [{ id: "onboarding.card.group.editor", params: { group: "ROVER", folder: "/home/u/TeamCraft/ROVER" } }]);
  assert.deepEqual(card.steps[2].text, [{ id: "onboarding.card.open", params: { group: "ROVER" } }]);
  for (const s of card.steps) assert.ok(CARD_STATUS[s.status], "a status word for " + s.status);
});

test("the card: the folder ticks when it is there, used or set by policy", () => {
  assert.equal(startCard(DISMISSED, { ...idle, workspaceExists: true }).steps[0].status, "done");
  assert.equal(startCard(DISMISSED, { ...idle, groupsOnDisk: ["arm"] }).steps[0].status, "done", "a Get used it");
  const policy = startCard(DISMISSED, { ...idle, workspaceRootPolicy: true }).steps[0];
  assert.equal(policy.status, "done", "policy-fixed: nothing to decide");
  assert.deepEqual(policy.text, [{ id: "onboarding.folder.line", params: { dir: "/home/u/TeamCraft" } }]);
  assert.deepEqual(policy.note, { id: "onboarding.skip.folder_policy", params: {} });
  assert.equal(startCard(DISMISSED, { ...idle, workspaceExists: true }).steps[0].note, null, "no policy note without a policy");
});

test("the card: the group ticks once one of the user's groups is in the working folder", () => {
  const got = startCard(DISMISSED, { ...idle, groupsOnDisk: ["arm"] });
  assert.deepEqual(statuses(got), ["folder:done", "group:done", "open:open"]);
  assert.deepEqual(got.steps[1].text, [{ id: "onboarding.group.one", params: { group: "ARM" } }], "the group on disk is named");
  assert.deepEqual(got.steps[1].note, { id: "onboarding.group.on_disk", params: {} });
  assert.deepEqual(got.steps[2].text, [{ id: "onboarding.card.open", params: { group: "ARM" } }]);
  assert.deepEqual(got.progress.params, { done: 2, total: 3 });
  // a group that is not the user's does not count for the group step
  const other = startCard(DISMISSED, { ...idle, groupsOnDisk: ["pump"] });
  assert.deepEqual(statuses(other), ["folder:done", "group:open", "open:open"]);
});

test("the card: an own check-out begins the last step and puts its group in the working folder", () => {
  const wl = { checkouts: [{ item_number: "ASM-1", code: "A", project_code: "arm" }], has_checked_in: false };
  const card = startCard(DISMISSED, { ...idle, worklist: wl });
  assert.deepEqual(statuses(card), ["folder:done", "group:done", "open:active"]);
  assert.equal(card.steps[1].text[0].params.group, "ARM", "the group of the check-out");
  assert.deepEqual(idsOf(card.steps[2].text), ["onboarding.tip.loop"], "the daily-loop tip: save, then Check in…");
  assert.deepEqual(card.progress.params, { done: 2, total: 3 }, "begun is not done");
  assert.equal(card.complete, false);
  // a check-out in a group the user is no member of (any more) ticks no group
  const away = startCard(DISMISSED, { ...idle, worklist: { checkouts: [{ project_code: "PUMP" }] } });
  assert.deepEqual(statuses(away), ["folder:done", "group:open", "open:active"]);
});

test("the card: the first check-in is the end of the loop – every step ticked, stored as done", () => {
  const wl = { checkouts: [], has_checked_in: true };
  const card = startCard(DISMISSED, { ...idle, worklist: wl }, { seenOpen: true });
  assert.equal(card.complete, true);
  assert.deepEqual(statuses(card), ["folder:done", "group:done", "open:done"], "earlier steps tick with it");
  assert.equal(card.store, ONBOARDING_DONE, "it does not come back");
  assert.deepEqual(card.progress.params, { done: 3, total: 3 });
  assert.deepEqual(idsOf(card.lead), ["onboarding.card.complete", "onboarding.tip.loop_done"]);
  for (const s of card.steps) {
    assert.deepEqual(s.text, [], "a complete card has no texts: " + s.step);
    assert.equal(s.note, null);
  }
  // also with a group the user lost since and a folder not on this computer
  const elsewhere = startCard(DISMISSED, { ...idle, groups: [], worklist: wl }, { seenOpen: true });
  assert.deepEqual(statuses(elsewhere), ["folder:done", "group:done", "open:done"]);
});

test("the card says it is complete once: only when it was seen open in this run", () => {
  const wl = { checkouts: [], has_checked_in: true };
  const seen = startCard(DISMISSED, { ...idle, worklist: wl }, { seenOpen: true });
  assert.equal(seen.show, true, "the user watched it tick: it says so");
  const silent = startCard(DISMISSED, { ...idle, worklist: wl }, { seenOpen: false });
  assert.equal(silent.show, false, "finished elsewhere or before: no card");
  assert.equal(silent.store, ONBOARDING_DONE, "but stored all the same");
  assert.equal(startCard(DISMISSED, { ...idle, worklist: wl }).show, false, "no ctx counts as not seen");
  // stored, the next Home has no card
  assert.equal(startCard({ onboarding: seen.store }, { ...idle, worklist: wl }, { seenOpen: true }), null);
  // an open card shows whether or not it was seen before
  assert.equal(startCard(DISMISSED, idle, { seenOpen: true }).show, true);
});

test("the card for an editor without a group: how to get one, the assembly waits for it", () => {
  const card = startCard(DISMISSED, { ...idle, groups: [] });
  assert.deepEqual(statuses(card), ["folder:open", "group:open", "open:open"]);
  assert.deepEqual(idsOf(card.steps[1].text), ["onboarding.group.none.editor"]);
  assert.deepEqual(card.steps[2].text, []);
  assert.deepEqual(card.steps[2].note, { id: "onboarding.skip.no_group", params: {} });
  assert.equal(card.complete, false);
});

test("the card for a viewer: no check-out step, complete with folder and group", () => {
  const viewer = { ...idle, editor: false };
  const card = startCard(DISMISSED, viewer);
  assert.deepEqual(statuses(card), ["folder:open", "group:open", "open:skipped"]);
  assert.deepEqual(card.steps[1].text, [{ id: "onboarding.card.group.viewer", params: { group: "ROVER", folder: "/home/u/TeamCraft/ROVER" } }]);
  assert.deepEqual(card.steps[2].note, { id: "onboarding.group.viewer", params: {} }, "checking out needs the CAD editor role");
  assert.deepEqual(card.progress.params, { done: 0, total: 2 }, "a skipped step is not counted");
  const got = startCard(DISMISSED, { ...viewer, groupsOnDisk: ["ROVER"] }, { seenOpen: true });
  assert.equal(got.complete, true, "a viewer's loop ends with the group in the working folder");
  assert.deepEqual(statuses(got), ["folder:done", "group:done", "open:skipped"]);
  assert.deepEqual(idsOf(got.lead), ["onboarding.card.complete"], "no daily-loop line for a viewer");
  assert.equal(got.store, ONBOARDING_DONE);
});

test("the card for a viewer without a group: ask a manager, with the user name", () => {
  const card = startCard(DISMISSED, { ...idle, editor: false, groups: [], workspaceExists: true });
  assert.deepEqual(statuses(card), ["folder:done", "group:open", "open:skipped"]);
  assert.deepEqual(card.steps[1].text, [{ id: "onboarding.group.none.viewer", params: { login: "erika" } }]);
  assert.deepEqual(card.progress.params, { done: 1, total: 2 });
  assert.equal(card.complete, false);
});

test("the card: an older server without has_checked_in never completes by itself", () => {
  const card = startCard(DISMISSED, { ...idle, worklist: { checkouts: [{ project_code: "ROVER" }] } });
  assert.equal(card.complete, false);
  assert.equal(card.steps[2].status, "active");
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
  // every message of the Home card, in every situation the tests above use
  answered.push(...Object.values(CARD_STATUS));
  const worklists = [{ checkouts: [] }, { checkouts: [{ project_code: "ROVER" }] }, { checkouts: [], has_checked_in: true }];
  for (const facts of [idle, { ...idle, groups: [] }, { ...idle, editor: false }, { ...idle, editor: false, groups: [] },
    { ...idle, workspaceRootPolicy: true, groupsOnDisk: ["ROVER"] }]) {
    for (const worklist of worklists) {
      const card = startCard(DISMISSED, { ...facts, worklist }, { seenOpen: true });
      answered.push(card.progress.id, ...idsOf(card.lead));
      for (const s of card.steps) {
        answered.push(s.name, CARD_STATUS[s.status], ...idsOf(s.text));
        if (s.note) answered.push(s.note.id);
      }
    }
  }
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
