// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – the first-start wizard *Getting started* (#422): from
// the sign-in to the working folder, the project group and the daily loop
// (Check out and open → save in CAD → Check in…).
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).
//
// The steps, which are skipped and their texts are pure rules in
// onboarding-rules.js. The wizard itself creates no file and no folder: it
// only opens the app's own confirmed actions (the folder picker, *Get project
// groups*), and stores `done` or `dismissed` in the settings.

import { $, TAURI, call, dialog, errorText, h, state, toast } from "./core.js";
import { CAD_SETUP_PREVIEW, ONBOARDING_DISMISSED, ONBOARDING_DONE, ONBOARDING_STEPS, STEP_NAMES, nextStep, shouldShow, skipReason, stepPosition, stepSummary } from "./onboarding-rules.js";
import { fixedByPolicy } from "./settings.js";
import { getProjectsDialog, gotGroups } from "./group-sync.js";
import { selectView } from "./navigator.js";
import { openWorkspaceFolder } from "./workspace.js";

/** The open wizard's dialog (one at a time). */
var wizardDialog = null;

/** What the step rules need to know, read from the session now. */
function onboardingFacts() {
  var me = state.me || {};
  var memberOf = Array.isArray(me.projects) ? me.projects : [];
  var groups = state.projects.filter(function (p) {
    return p.active && memberOf.indexOf(p.id) !== -1;
  }).map(function (p) { return p.code; });
  return {
    serverFixed: fixedByPolicy("server"),
    serverUrl: state.session && state.session.server_url,
    signedIn: !!(state.session && state.session.signed_in) || !!state.me,
    username: me.display_name || me.username || "",
    workspaceDir: state.session && state.session.workspace_dir,
    groups: groups,
    groupsOnDisk: gotGroups()
  };
}

/** Stores `done` or `dismissed` (src/settings.rs `onboarding`). */
function saveOnboarding(value) {
  return call("set_onboarding", { value: value }).then(function () {
    if (state.session) state.session.onboarding = value;
  }, function (err) { toast(errorText(err), "error", "Getting started"); });
}

/** After the shell opened: the wizard on the first start (shouldShow). */
export function maybeStartOnboarding() {
  var ctx = { signedInThisRun: state.signedInAtStart === false };
  if (!shouldShow(state.session || {}, ctx)) return;
  openOnboarding(false);
}

/** Opens the wizard; `fromHelp` is *Help → Getting started…*, which always opens it. */
export function openOnboarding(fromHelp) {
  if (!shouldShow(state.session || {}, { fromHelp: !!fromHelp, signedInThisRun: state.signedInAtStart === false })) return;
  if (wizardDialog) return;
  showStep(nextStep({ current: null }, onboardingFacts()));
}

function closeWizard() {
  if (wizardDialog) wizardDialog.close();
  wizardDialog = null;
}

/** The list of all steps on the left: done, current, skipped (with why). */
function stepList(current, facts) {
  var passed = true;
  return h("ol.onboarding-steps", { "aria-label": "Steps" }, ONBOARDING_STEPS.map(function (s) {
    if (s === current) passed = false;
    var why = skipReason(s, facts);
    // the sign-in is skipped because it is done: a tick, not a dash
    var cls = s === current ? ".current" : s === "signin" && why ? ".done" : why ? ".skipped" : passed ? ".done" : "";
    return h("li" + cls, { title: why || null, "aria-current": s === current ? "step" : null }, [
      h("span", { text: STEP_NAMES[s] }),
      why ? h("span.small.muted", { text: why }) : null
    ]);
  }));
}

/** The extra controls of a step (they only open the app's own actions). */
function stepExtras(step, facts) {
  switch (step) {
    case "folder":
      return h("div.stack", null, [
        h("code.onboarding-path", { text: facts.workspaceDir || "" }),
        h("div.row", null, [
          TAURI.dialog ? h("button", { type: "button", on: { click: function () { changeFolder(facts.workspaceDir); } } }, "Change…") : null,
          h("button", { type: "button", on: { click: function () { openWorkspaceFolder(null); } } }, "Open folder")
        ])
      ]);
    case "group":
      return h("div.row", null, [
        h("button", { type: "button", on: { click: function () { getProjectsDialog(); } } }, "Get project groups…")
      ]);
    case "cad":
      return h("ul.onboarding-cad", null, CAD_SETUP_PREVIEW.map(function (p) {
        return h("li", null, [
          h("strong", { text: p.program }), ": ",
          h("code", { text: p.file }), " – " + p.what + " ",
          h("span.small.muted", { text: "(not created)" })
        ]);
      }));
    case "loop":
      return h("div.row", null, [
        // finish here and go where the loop starts: an item of the group, or a check-out
        h("button", { type: "button", on: { click: function () { showStep(null); selectView("projects"); } } }, "Finish and show Projects"),
        h("button", { type: "button", on: { click: function () { showStep(null); selectView("worklist"); } } }, "Finish and open my worklist")
      ]);
    default:
      return null;
  }
}

/** *Change…*: the folder picker, then the same command as Settings → Workspace. */
function changeFolder(current) {
  TAURI.dialog.open({ directory: true, defaultPath: current || undefined }).then(function (dir) {
    if (!dir) return null;
    return call("set_workspace_dir", { dir: dir }).then(function () {
      state.session.workspace_dir = dir;
      var status = $("status-ws");
      if (status) status.textContent = "Workspace: " + dir;
      if (wizardDialog && wizardDialog.step === "folder") showStep("folder");
    });
  }).catch(function (err) { toast(errorText(err), "error", "Change working folder"); });
}

/** Shows `step`; null finishes the wizard (`done`). */
function showStep(step) {
  closeWizard();
  if (!step) { saveOnboarding(ONBOARDING_DONE); return; }
  var facts = onboardingFacts();
  var sum = stepSummary(step, facts);
  var back = nextStep({ current: step, back: true }, facts);
  var last = !nextStep({ current: step }, facts);
  var body = h("div.onboarding", null, [
    stepList(step, facts),
    h("div.onboarding-body.stack", null, [
      h("p.hint", { text: stepPosition(step, facts) }),
      h("h3", { text: sum.title }),
      sum.text.map(function (para) { return h("p", { text: para }); }),
      stepExtras(step, facts)
    ])
  ]);
  function go() { showStep(nextStep({ current: step }, onboardingFacts())); }
  var actions = [
    { label: "Don't show again", onClick: function () { closeWizard(); saveOnboarding(ONBOARDING_DISMISSED); } },
    back ? { label: "Back", onClick: function () { showStep(back); } } : null,
    last ? null : { label: "Skip", onClick: go },
    { label: last ? "Finish" : "Next", primary: true, submit: true, onClick: go }
  ].filter(Boolean);
  wizardDialog = dialog("Getting started", body, actions, { wide: true, modal: true });
  wizardDialog.step = step;
}
