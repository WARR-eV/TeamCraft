// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – the first-start wizard *Getting started* (#422): from
// the sign-in to the working folder, the project group and the daily loop
// (Check out and open → save in CAD → Check in…).
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).
//
// The steps, which are skipped and their texts are pure rules in
// onboarding-rules.js. The wizard opens the app's own confirmed actions (the
// folder picker, *Get project groups*) and stores `done` or `dismissed` in
// the settings. The only files it writes are the CAD setup files (#478):
// nothing ticked at the start, each file's content shown first, written only
// on *Create ticked files*, never over an existing file (src/cadsetup.rs).

import { $, TAURI, call, dialog, errorText, h, state, toast } from "./core.js";
import { CAD_SETUP_FILES, ONBOARDING_DISMISSED, ONBOARDING_DONE, ONBOARDING_STEPS, STEP_NAMES, cadSetupGroups, cadSetupReport, cadSetupRows, cadSetupToCreate, initialCadPicks, nextStep, shouldShow, skipReason, startGroup, stepPosition, stepSummary } from "./onboarding-rules.js";
import { fixedByPolicy } from "./settings.js";
import { getProjectsDialog, gotGroups } from "./group-sync.js";
import { selectView, setActiveProject } from "./navigator.js";
import { openWorkspaceFolder } from "./workspace.js";

/** The open wizard's dialog (one at a time). */
var wizardDialog = null;

/** Steps the user passed with *Skip* in this run of the wizard (the step
 *  list marks them skipped, not done). */
var skippedSteps = new Set();

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
  skippedSteps = new Set();
  showStep(nextStep({ current: null }, onboardingFacts()));
}

function closeWizard() {
  if (wizardDialog) wizardDialog.close();
  wizardDialog = null;
}

/** The list of all steps on the left: done, current, skipped (with why).
 *  The status is also spoken (a visually hidden text), not only the CSS
 *  tick or dash. */
function stepList(current, facts) {
  var passed = true;
  return h("ol.onboarding-steps", { "aria-label": "Steps" }, ONBOARDING_STEPS.map(function (s) {
    if (s === current) passed = false;
    var why = skipReason(s, facts);
    // the sign-in is skipped because it is done: a tick, not a dash;
    // a step passed with *Skip* is skipped, not done
    var status = s === current ? "current" : s === "signin" && why ? "done" : why ? "skipped"
      : passed && skippedSteps.has(s) ? "skipped" : passed ? "done" : "";
    return h("li" + (status ? "." + status : ""), { title: why || null, "aria-current": s === current ? "step" : null }, [
      h("span", { text: STEP_NAMES[s] }),
      status === "done" || status === "skipped" ? h("span.visually-hidden", { text: " (" + status + ")" }) : null,
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
      return cadSetup(facts);
    case "loop": {
      // finish here and go where the loop starts: the files of the group in
      // Home (as choosing it on the Projects page does), or the group picker
      // for a user without groups
      var start = startGroup(facts);
      return h("div.row", null, [
        start
          ? h("button", { type: "button", on: { click: function () { showStep(null); setActiveProject(start); selectView("home"); } } }, "Finish and open " + start)
          : h("button", { type: "button", on: { click: function () { showStep(null); selectView("projects"); } } }, "Finish and show Projects"),
        h("button", { type: "button", on: { click: function () { showStep(null); selectView("worklist"); } } }, "Finish and open my worklist")
      ]);
    }
    default:
      return null;
  }
}

/**
 * The CAD step's files (#478): a group choice, one row per file with its
 * tick box (none ticked at the start), target path and content preview, and
 * *Create ticked files*. The previews come from the app (`cad_setup_preview`),
 * so the text shown is the text written; the web view sends only the group
 * code and the kind.
 */
function cadSetup(facts) {
  var groups = cadSetupGroups(facts);
  if (!groups.length) return null;
  var current = { group: groups[0], picks: initialCadPicks(), previews: {} };
  var box = h("div.stack");

  function load() {
    var group = current.group;
    current.previews = {};
    current.picks = initialCadPicks();
    render();
    CAD_SETUP_FILES.forEach(function (f) {
      call("cad_setup_preview", { group: group, kind: f.kind }).then(function (p) {
        if (current.group === group) { current.previews[f.kind] = p; render(); }
      }, function (err) {
        if (current.group === group) { current.previews[f.kind] = { error: errorText(err) }; render(); }
      });
    });
  }

  function create(button) {
    var kinds = cadSetupToCreate(current.previews, current.picks);
    if (!kinds.length) return;
    button.disabled = true;
    var group = current.group, results = [];
    kinds.reduce(function (done, kind) {
      return done.then(function () {
        return call("cad_setup_create", { group: group, kind: kind }).then(function (path) {
          results.push({ kind: kind, path: path });
        }, function (err) { results.push({ kind: kind, error: errorText(err) }); });
      });
    }, Promise.resolve()).then(function () {
      var report = cadSetupReport(results);
      toast(report.text, report.ok ? null : "error", "CAD setup");
      if (current.group === group) load();
    });
  }

  function render() {
    var rows = cadSetupRows(current.previews, current.picks);
    var picker = groups.length > 1
      ? h("label.row", null, ["Project group ", h("select", { on: { change: function (e) { current.group = e.target.value; load(); } } },
        groups.map(function (g) { return h("option", { value: g, selected: g === current.group, text: g }); }))])
      : h("p", null, ["Project group: ", h("strong", { text: current.group })]);
    var createButton = h("button", { type: "button", disabled: !rows.some(function (r) { return r.ticked; }) }, "Create ticked files");
    createButton.addEventListener("click", function () { create(createButton); });
    box.replaceChildren(
      picker,
      h("ul.onboarding-cad", null, rows.map(function (r) {
        var id = "cad-setup-" + r.kind;
        return h("li.stack", null, [
          h("label", { for: id }, [
            h("input", { id: id, type: "checkbox", checked: r.ticked, disabled: r.disabled, on: { change: function (e) { current.picks[r.kind] = e.target.checked; render(); } } }),
            " ", h("strong", { text: r.program }), " – " + r.what
          ]),
          r.path ? h("code.onboarding-path", { text: r.path }) : null,
          r.note ? h("span.small.muted", { text: r.note }) : null,
          r.content ? h("details", { open: r.ticked }, [h("summary", { text: "Preview of the content" }), h("pre.onboarding-preview", { text: r.content })]) : null
        ]);
      })),
      h("div.row", null, [createButton])
    );
  }

  load();
  return box;
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
  function skip() { skippedSteps.add(step); go(); }
  function next() { skippedSteps.delete(step); go(); }
  var actions = [
    { label: "Don't show again", onClick: function () { closeWizard(); saveOnboarding(ONBOARDING_DISMISSED); } },
    back ? { label: "Back", onClick: function () { showStep(back); } } : null,
    last ? null : { label: "Skip", onClick: skip },
    { label: last ? "Finish" : "Next", primary: true, submit: true, onClick: next }
  ].filter(Boolean);
  wizardDialog = dialog("Getting started", body, actions, { wide: true, modal: true });
  wizardDialog.step = step;
}
