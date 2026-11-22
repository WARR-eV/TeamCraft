// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/onboarding-rules.js (the steps of the
// first-start wizard *Getting started* – #422).
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

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/onboarding-rules.js");
const src = fs.readFileSync(file, "utf8");
const {
  ONBOARDING_STEPS, ONBOARDING_DONE, ONBOARDING_DISMISSED, CAD_SETUP_PREVIEW, STEP_NAMES,
  shouldShow, skipReason, visibleSteps, nextStep, stepPosition, stepSummary, startGroup
} = await import("data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

// a user right after the first sign-in: server not fixed, one group not on disk yet
const fresh = { serverFixed: false, signedIn: true, groups: ["ROVER"], groupsOnDisk: [], serverUrl: "https://plm.example.org", workspaceDir: "/home/u/TeamCraft" };

test("the steps and their order", () => {
  assert.deepEqual(ONBOARDING_STEPS, ["server", "signin", "folder", "group", "cad", "loop"]);
  for (const s of ONBOARDING_STEPS) assert.ok(STEP_NAMES[s], "a name for " + s);
});

test("the stored values match the Rust side", () => {
  assert.equal(ONBOARDING_DONE, "done");
  assert.equal(ONBOARDING_DISMISSED, "dismissed");
});

test("shown on the first start after the sign-in", () => {
  assert.equal(shouldShow({}, { signedInThisRun: true }), true);
  assert.equal(shouldShow({ onboarding: null }, { signedInThisRun: true }), true);
});

test("never shown again once done or dismissed", () => {
  assert.equal(shouldShow({ onboarding: "done" }, { signedInThisRun: true }), false);
  assert.equal(shouldShow({ onboarding: "dismissed" }, { signedInThisRun: true }), false);
});

test("not shown when the app starts signed in (an update, a reload)", () => {
  assert.equal(shouldShow({}, { signedInThisRun: false }), false);
  assert.equal(shouldShow({}, {}), false);
  assert.equal(shouldShow(null, undefined), false);
});

test("Help → Getting started always opens it", () => {
  assert.equal(shouldShow({ onboarding: "done" }, { fromHelp: true }), true);
  assert.equal(shouldShow({ onboarding: "dismissed" }, { fromHelp: true }), true);
  assert.equal(shouldShow(null, { fromHelp: true }), true);
});

test("an unknown stored value counts as not done", () => {
  assert.equal(shouldShow({ onboarding: "later" }, { signedInThisRun: true }), true);
});

test("a server fixed by policy skips the server step (#378)", () => {
  assert.equal(skipReason("server", { ...fresh, serverFixed: true }), "Set by your organization");
  assert.equal(skipReason("server", fresh), null);
  assert.ok(!visibleSteps({ ...fresh, serverFixed: true }).includes("server"));
});

test("a sign-in skips the sign-in step", () => {
  assert.equal(skipReason("signin", fresh), "Signed in");
  assert.equal(skipReason("signin", { ...fresh, signedIn: false }), null);
});

test("no groups skips the group step", () => {
  assert.match(skipReason("group", { ...fresh, groups: [] }), /not a member/);
  assert.match(skipReason("group", { ...fresh, groups: undefined }), /not a member/);
  assert.match(skipReason("group", { ...fresh, groups: ["", null, 7] }), /not a member/);
});

test("groups already on disk skip the group step, ignoring case", () => {
  assert.match(skipReason("group", { ...fresh, groupsOnDisk: ["rover"] }), /already/);
  assert.equal(skipReason("group", { ...fresh, groups: ["ROVER", "SAT"], groupsOnDisk: ["ROVER"] }), null);
});

test("folder, CAD setup and the daily loop are always shown", () => {
  for (const s of ["folder", "cad", "loop"]) assert.equal(skipReason(s, {}), null);
});

test("an unknown step is skipped", () => {
  assert.equal(skipReason("nope", fresh), "Unknown step");
});

test("visible steps of a fresh user", () => {
  assert.deepEqual(visibleSteps(fresh), ["server", "folder", "group", "cad", "loop"]);
  assert.deepEqual(visibleSteps({ ...fresh, serverFixed: true, groups: [] }), ["folder", "cad", "loop"]);
  assert.deepEqual(visibleSteps({}), ["server", "signin", "folder", "cad", "loop"]);
});

test("next step walks the shown steps and ends with null", () => {
  const order = [];
  let s = nextStep({ current: null }, fresh);
  while (s) { order.push(s); s = nextStep({ current: s }, fresh); }
  assert.deepEqual(order, visibleSteps(fresh));
});

test("next step skips the server when it is fixed", () => {
  assert.equal(nextStep({}, { ...fresh, serverFixed: true }), "folder");
  assert.equal(nextStep(undefined, { ...fresh, serverFixed: true }), "folder");
});

test("back goes to the shown step before, null at the start", () => {
  assert.equal(nextStep({ current: "cad", back: true }, fresh), "group");
  assert.equal(nextStep({ current: "cad", back: true }, { ...fresh, groups: [] }), "folder");
  assert.equal(nextStep({ current: "server", back: true }, fresh), null);
});

test("a step skipped meanwhile goes on after it (the group was fetched)", () => {
  const got = { ...fresh, groupsOnDisk: ["ROVER"] };
  assert.equal(nextStep({ current: "group" }, got), "cad");
  assert.equal(nextStep({ current: "loop" }, got), null);
});

test("an unknown current step starts at the beginning", () => {
  assert.equal(nextStep({ current: "nope" }, fresh), "server");
});

test("step positions count shown steps only", () => {
  assert.equal(stepPosition("server", fresh), "Step 1 of 5");
  assert.equal(stepPosition("loop", fresh), "Step 5 of 5");
  assert.equal(stepPosition("signin", fresh), "");
});

test("every step has a title and text", () => {
  for (const s of ONBOARDING_STEPS) {
    const sum = stepSummary(s, fresh);
    assert.ok(sum.title, "title of " + s);
    assert.ok(sum.text.length > 0, "text of " + s);
  }
  assert.deepEqual(stepSummary("nope", fresh), { title: "", text: [] });
});

test("the folder step names the working folder", () => {
  assert.match(stepSummary("folder", fresh).text.join(" "), /\/home\/u\/TeamCraft/);
  assert.match(stepSummary("folder", {}).text.join(" "), /the default folder/);
});

test("the group step names the groups", () => {
  assert.match(stepSummary("group", fresh).text[0], /project group ROVER/);
  assert.match(stepSummary("group", { groups: ["A", "B"] }).text[0], /2 project groups: A, B/);
  assert.match(stepSummary("group", {}).text[0], /not a member/);
});

test("the server step says when the organization set it", () => {
  assert.match(stepSummary("server", { ...fresh, serverFixed: true }).text.join(" "), /organization set this server/);
  assert.match(stepSummary("server", fresh).text.join(" "), /Settings/);
});

test("the CAD step promises nothing is created now", () => {
  assert.match(stepSummary("cad", fresh).text.join(" "), /Nothing is created now/);
  assert.ok(CAD_SETUP_PREVIEW.length >= 3);
  for (const p of CAD_SETUP_PREVIEW) assert.ok(p.program && p.file && p.what);
});

test("the loop step has the three steps", () => {
  const text = stepSummary("loop", fresh).text;
  assert.equal(text.length, 4);
  assert.match(text[0], /Check out and open/);
  assert.match(text[2], /Check in…/);
  // the next action: open the group, then Check out and open on the assembly
  assert.match(text[3], /Finish and open ROVER/);
  assert.match(text[3], /Check out and open/);
  assert.match(stepSummary("loop", { ...fresh, groups: [] }).text[3], /Finish and show Projects/);
});

test("the group the last step opens: a fetched one first, none without groups", () => {
  assert.equal(startGroup(fresh), "ROVER");
  assert.equal(startGroup({ groups: ["ROVER", "ARM"], groupsOnDisk: ["arm"] }), "ARM");
  assert.equal(startGroup({ groups: ["ROVER", "ARM"], groupsOnDisk: [] }), "ROVER");
  assert.equal(startGroup({ groups: [], groupsOnDisk: ["arm"] }), null);
  assert.equal(startGroup({ groups: ["", 3] }), null);
  assert.equal(startGroup({}), null);
});

console.log(`${n} tests passed`);
