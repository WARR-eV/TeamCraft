// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – the first-start wizard *Getting started* (#422): one
// job – bring a CAD user who has just signed in to a real assembly of their
// group, with *Check out and open* in front of them. Three steps: where the
// files go, the project group (only with a choice, or none), the first
// assembly. Part of the desktop UI (ES modules, no build step; js/main.js
// starts it). Background: docs/src/research/first-start.md.
//
// The steps, which are skipped, the assemblies offered and the messages are
// pure rules in onboarding-rules.js; the texts come from shared/i18n/. The
// wizard is read-only: it creates no file or folder (*Open folder* only
// opens a working folder that exists), gets and checks out nothing and
// writes nothing to the server. It stores only `done` or `dismissed` on the
// active connection (src/profiles.rs), the working folder when the user
// picks one with *Change…*, and the active group on the final click
// (*Show <number>* or *Finish and show <group> in Home*). The navigator
// switches to *Files* for this run only – the stored mode stays.

import { $, TAURI, api, call, dialog, errorText, h, invoke, state, toast } from "./core.js";
import { t } from "/static/shared/js/i18n.js";
import { ONBOARDING_DISMISSED, ONBOARDING_DONE, ONBOARDING_STEPS, STEP_NAMES, cadLine, chosenGroup, finishLabel, groupCadText, groupList, isFinal, nextStep, noAssemblyMessage, onDisk, openLine, pickAssemblies, roleMessage, shouldShow, showTitle, skipReason, stepPosition, stepSummary } from "./onboarding-rules.js";
import { gotGroups } from "./group-sync.js";
import { emptyHint, fetchStructureRoots, newProjectDialog, selectView, setActiveProject, setNavMode, typeIcon } from "./navigator.js";
import { canEdit, canEditProject } from "./shell.js";
import { emptyGroupHint } from "./empty-rules.js";
import { openRevision } from "./revision.js";
import { cadFormats, groupOpenPlan, installedCad } from "./cad-open.js";
import { thumbIco } from "./list-thumbs.js";
import { renderActive, renderTabs } from "./tabs.js";
import { openWorkspaceFolder } from "./workspace.js";

/** The open wizard: its dialog, the step, the group chosen in step 2 (kept
 *  in the wizard only – set active on the final click, so Back and closing
 *  change nothing), what the computer offers, and the promise's resolver. */
var wizard = null;

/** A message of the rules (`{id, params}`) as text. */
function msg(m) { return m ? t(m.id, m.params) : ""; }

/** What the step rules need to know, read from the session now. */
function onboardingFacts() {
  var me = state.me || {};
  var memberOf = Array.isArray(me.projects) ? me.projects : [];
  var groups = state.projects.filter(function (p) {
    return p.active && memberOf.indexOf(p.id) !== -1;
  }).map(function (p) {
    return { code: p.code, name: p.name, cad: p.default_cad || "", cadVersion: p.default_cad_version || "" };
  });
  var policy = (state.session && state.session.policy) || {};
  return {
    workspaceRootPolicy: !!policy.workspace_root,
    workspaceDir: (state.session && state.session.workspace_dir) || "",
    groups: groups,
    groupsOnDisk: gotGroups(),
    editor: canEdit(),
    login: me.username || ""
  };
}

/** Stores `done` or `dismissed` on the active connection (src/profiles.rs). */
function saveOnboarding(value) {
  return call("set_onboarding", { value: value }).then(function () {
    if (state.session) state.session.onboarding = value;
  }, function (err) { toast(errorText(err), "error", t("onboarding.title")); });
}

/** Whether the user already works on this server – asked directly (not from
 *  a worklist that may not have loaded yet): own check-outs, or a file
 *  version of theirs stored before (`has_checked_in`: a check-in or an
 *  upload, e.g. from another computer). */
function returningUser() {
  return api("GET", "/worklist").then(function (w) {
    return {
      hasCheckouts: !!(w && Array.isArray(w.checkouts) && w.checkouts.length),
      hasCheckedIn: !!(w && w.has_checked_in)
    };
  }, function () { return {}; });
}

/**
 * After the shell has drawn: the wizard on the first start (shouldShow).
 * Returns a promise that resolves when the wizard is closed (at once when it
 * does not open) – the update notice, the Ko-fi tag and the keychain warning
 * wait for it. The stored value is read again from the session: the connect
 * screen may have switched to another connection (each has its own).
 */
export function maybeStartOnboarding() {
  var signedInThisRun = state.signedInAtStart === false;
  if (!signedInThisRun) return Promise.resolve();
  return invoke("session").then(function (s) {
    if (state.session) state.session.onboarding = s.onboarding;
    if (!shouldShow(s, { signedInThisRun: signedInThisRun })) return null;
    return returningUser().then(function (r) {
      if (!shouldShow(s, { signedInThisRun: signedInThisRun, hasCheckouts: r.hasCheckouts, hasCheckedIn: r.hasCheckedIn })) return null;
      return openOnboarding(false);
    });
  }).catch(function () { /* no wizard rather than a blocked start */ });
}

/** Opens the wizard; `fromHelp` is *Help → Getting started…*, which always
 *  opens it. Resolves when it is closed. */
export function openOnboarding(fromHelp) {
  if (wizard) return wizard.closed;
  var done;
  wizard = { dialog: null, step: null, chosen: null, installed: [], names: {}, folderExists: false, closed: new Promise(function (r) { done = r; }) };
  wizard.resolve = done;
  var w = wizard;
  // what this computer and the server offer: the CAD programs found, the
  // program names of the format families – both cached – and whether the
  // working folder exists (asked now: a Get may have created it since the
  // start); never a reason to keep the wizard closed
  Promise.all([
    installedCad(),
    cadFormats().catch(function () { return { names: {} }; }),
    invoke("session").catch(function () { return null; })
  ]).then(function (r) {
    if (wizard !== w) return;
    w.installed = r[0] || [];
    w.names = (r[1] && r[1].names) || {};
    w.folderExists = !!(r[2] && r[2].workspace_exists);
    showStep(nextStep({ current: null }, onboardingFacts()));
  });
  return w.closed;
}

/** Closes the wizard; `value` (done, dismissed) is stored, null stores nothing. */
function finishWizard(value) {
  var w = wizard;
  if (!w) return Promise.resolve();
  if (w.dialog) w.dialog.close();
  wizard = null;
  var saved = value ? saveOnboarding(value) : Promise.resolve();
  return saved.then(function () { w.resolve(); });
}

/** The list of the steps on the left: done, current, skipped (with why).
 *  The status is also spoken (a visually hidden text), not only the CSS
 *  tick or dash. */
function stepList(current, facts) {
  var passed = true;
  return h("ol.onboarding-steps", { "aria-label": t("onboarding.steps") }, ONBOARDING_STEPS.map(function (s) {
    if (s === current) passed = false;
    var why = skipReason(s, facts);
    var status = s === current ? "current" : why ? "skipped" : passed ? "done" : "";
    return h("li" + (status ? "." + status : ""), { title: why ? t(why) : null, "aria-current": s === current ? "step" : null }, [
      h("span", { text: t(STEP_NAMES[s]) }),
      status === "done" || status === "skipped" ? h("span.visually-hidden", { text: " (" + (status === "done" ? t("onboarding.status.done") : t("onboarding.status.skipped")) + ")" }) : null,
      why ? h("span.small.muted", { text: t(why) }) : null
    ]);
  }));
}

/** The header: the connection, and the facts of steps that are skipped
 *  (the folder and the CAD programs, the one group). */
function header(facts) {
  var me = state.me || {};
  var lines = [h("p.small.muted", { text: t("onboarding.connected", {
    server: (state.session && state.session.server_url) || "",
    user: me.display_name || me.username || "",
    role: t(roleMessage(me.role))
  }) })];
  if (skipReason("folder", facts)) {
    lines.push(h("p.small.muted", { text: t("onboarding.folder.line", { dir: facts.workspaceDir }) + " " + msg(cadLine(wizard.installed)) }));
  }
  var mine = groupList(facts.groups);
  if (mine.length === 1) lines.push(h("p.small.muted", { text: t("onboarding.group.one", { group: mine[0].code }) }));
  return h("div.onboarding-head", null, lines);
}

/** Step 1: the folder, Change… / Open folder, the CAD programs found.
 *  *Open folder* only opens a folder that exists (opening a missing one
 *  would create it); otherwise it is disabled with a hint. */
function folderExtras(facts) {
  var exists = wizard.folderExists;
  return h("div.stack", null, [
    h("code.onboarding-path", { text: facts.workspaceDir }),
    h("div.row", null, [
      TAURI.dialog ? h("button", { type: "button", on: { click: function () { changeFolder(facts.workspaceDir); } } }, t("onboarding.folder.change")) : null,
      h("button", { type: "button", disabled: !exists, title: exists ? null : t("onboarding.folder.missing"),
        on: { click: function () { openWorkspaceFolder(null); } } }, t("onboarding.folder.open"))
    ]),
    exists ? null : h("p.small.muted", { text: t("onboarding.folder.missing") }),
    h("p", { text: msg(cadLine(wizard.installed)) })
  ]);
}

/** Step 2: the groups to choose from, or – without a group – how to get one. */
function groupExtras(facts) {
  var mine = groupList(facts.groups);
  if (!mine.length) {
    if (facts.editor) {
      return h("div.row", null, [h("button", { type: "button", on: { click: function () {
        finishWizard(ONBOARDING_DONE).then(function () { selectView("projects"); newProjectDialog(); });
      } } }, t("onboarding.group.create"))]);
    }
    return h("div.row", null, [h("button", { type: "button", on: { click: function () {
      var copy = navigator.clipboard && navigator.clipboard.writeText ? navigator.clipboard.writeText(facts.login) : Promise.reject(new Error("no clipboard"));
      copy.then(function () { toast(t("onboarding.group.copied")); }, function () { toast(facts.login, "info"); });
    } } }, t("onboarding.group.copy_login"))]);
  }
  var current = chosenGroup(facts, wizard.chosen);
  return h("div.onboarding-groups", { role: "radiogroup", "aria-label": t("onboarding.group.question") }, mine.map(function (g) {
    var cad = groupCadText(g, wizard.names);
    return h("label.onboarding-group", null, [
      h("input", { type: "radio", name: "onboarding-group", value: g.code, checked: !!current && current.code === g.code,
        on: { change: function () { if (wizard) wizard.chosen = g.code; } } }),
      h("span.mono", { text: g.code }),
      h("span", { text: g.name }),
      cad ? h("span.dim", { text: cad }) : null,
      onDisk(g.code, facts) ? h("span.small.muted", { text: t("onboarding.group.on_disk") }) : null
    ]);
  }));
}

/** Step 3: what Check out and open does, the group's assemblies with
 *  *Show*, where a group sets up its CAD programs. */
function openExtras(facts) {
  var group = chosenGroup(facts, wizard.chosen);
  var w = wizard;
  var list = h("div.onboarding-assemblies", null, [h("p.loading", { text: t("onboarding.open.loading") })]);
  var project = state.projects.filter(function (p) { return p.code === group.code; })[0];
  fetchStructureRoots(group.code).then(function (top) {
    if (wizard !== w) return;
    list.replaceChildren();
    if (!top.total) {
      // an empty group: the #423 hint (Import CAD folder…, New item, by role)
      list.appendChild(emptyHint(emptyGroupHint(project ? canEditProject(project.id) : false, canEdit()), { project: group.code }));
      return;
    }
    var me = state.me || {};
    var picks = pickAssemblies(top.nodes, { me: { id: me.id, name: me.display_name }, editor: facts.editor });
    if (!picks.length) { list.appendChild(h("p", { text: t(noAssemblyMessage(top.nodes, { editor: facts.editor }), { group: group.code }) })); return; }
    list.appendChild(h("ul.onboarding-rows", null, picks.map(function (n, i) {
      return h("li", null, [
        thumbIco(typeIcon(n.item_type), n),
        h("span.mono", { text: n.item_number + "/" + n.code }),
        h("span.label", { text: n.name }),
        h("button" + (i === 0 ? ".primary" : ""), {
          type: "button", title: msg(showTitle(group.code, n.item_number, facts.editor)),
          on: { click: function () { showAssembly(group.code, n); } }
        }, t("onboarding.open.show", { number: n.item_number }))
      ]);
    })));
  }, function (err) {
    if (wizard !== w) return;
    list.replaceChildren(h("p.notice.error", { text: t("onboarding.open.failed", { group: group.code, error: errorText(err) }) }));
  });
  // the program: the same Rust rule as the chip next to *Check out and
  // open* (cad_open_plan); the line is filled in once it answered
  var line = h("p", { text: "" });
  var lineOpts = { dir: facts.workspaceDir, names: wizard.names, editor: facts.editor, plan: null };
  var planned = facts.editor && group.cad
    ? groupOpenPlan(group.cad, group.cadVersion).catch(function () { return null; })
    : Promise.resolve(null);
  planned.then(function (plan) {
    if (wizard !== w) return;
    lineOpts.plan = plan;
    line.textContent = msg(openLine(group, lineOpts));
  });
  return h("div.stack", null, [
    line,
    list,
    h("p.small.muted", null, [t("onboarding.open.cad_setup") + " ",
      h("button.link.small", { type: "button", on: { click: function () {
        call("open_guide", { page: "cad-setup" }).catch(function (e) { toast(errorText(e), "error"); });
      } } }, t("onboarding.open.cad_setup.link"))])
  ]);
}

/** The commit: the group becomes the active one, the wizard closes and the
 *  real revision tab of the assembly opens – for a CAD editor *Check out
 *  and open* is its highlighted next step (next-step.js: step 3 offers only
 *  revisions in work, free or checked out by the user). */
function showAssembly(code, node) {
  setActiveProject(code);
  finishWizard(ONBOARDING_DONE);
  selectView("home");
  openRevision(node.item_number, node.code);
}

/** The secondary commit: the group becomes active, the navigator and the
 *  work area both show Home – the group's files. The navigator mode is
 *  switched for this run only; the user's stored choice stays. */
function finishInHome(code) {
  if (code) {
    setActiveProject(code);
    setNavMode("files", { remember: false });
  }
  finishWizard(ONBOARDING_DONE);
  selectView("home");
  state.active = null;
  renderTabs();
  renderActive();
}

/** *Change…*: the folder picker, then the same command as Settings → Workspace. */
function changeFolder(current) {
  TAURI.dialog.open({ directory: true, defaultPath: current || undefined }).then(function (dir) {
    if (!dir) return null;
    return call("set_workspace_dir", { dir: dir }).then(function () {
      state.session.workspace_dir = dir;
      var status = $("status-ws");
      if (status) status.textContent = "Workspace: " + dir;
      // the picker chooses an existing folder
      if (wizard) wizard.folderExists = true;
      if (wizard && wizard.step === "folder") showStep("folder");
    });
  }).catch(function (err) { toast(errorText(err), "error", t("onboarding.folder.change")); });
}

/** Shows `step`; null finishes the wizard (`done`). */
function showStep(step) {
  if (!wizard) return;
  if (wizard.dialog) wizard.dialog.close();
  wizard.dialog = null;
  if (!step) { finishWizard(ONBOARDING_DONE); return; }
  wizard.step = step;
  var facts = onboardingFacts();
  var sum = stepSummary(step, facts);
  var back = nextStep({ current: step, back: true }, facts);
  var last = isFinal(step, facts);
  var extras = step === "folder" ? folderExtras(facts) : step === "group" ? groupExtras(facts) : openExtras(facts);
  var body = h("div.stack", null, [
    header(facts),
    h("div.onboarding", null, [
      stepList(step, facts),
      h("div.onboarding-body.stack", null, [
        h("p.hint", { text: msg(stepPosition(step, facts)) }),
        h("h3", { text: t(sum.title) }),
        sum.text.map(function (m) { return h("p", { text: msg(m) }); }),
        extras
      ])
    ])
  ]);
  var group = chosenGroup(facts, wizard.chosen);
  var actions = [
    { label: t("onboarding.dismiss"), onClick: function () { finishWizard(ONBOARDING_DISMISSED); } },
    back ? { label: t("onboarding.back"), onClick: function () { showStep(back); } } : null,
    last
      ? { label: msg(finishLabel(group && group.code)), primary: step !== "open", submit: true, onClick: function () { finishInHome(group && group.code); } }
      : { label: t("onboarding.next"), primary: true, submit: true, onClick: function () { showStep(nextStep({ current: step }, onboardingFacts())); } }
  ].filter(Boolean);
  wizard.dialog = dialog(t("onboarding.title"), body, actions, { wide: true, modal: true });
}
