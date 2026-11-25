// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – pure rules of the first-start wizard *Getting
// started* (#422): which steps there are, which are skipped, when the wizard
// shows, which assemblies step 3 offers and the messages of each step – and
// the *Getting started* card on Home for users who closed it early
// (startCard).
// The wizard has one job: bring a CAD user who has just signed in to a real
// assembly of their group, with *Check out and open* in front of them. It is
// read-only and shaped around no CAD program (docs/src/research/first-start.md).
// The answers are message IDs of shared/i18n/ with their parameters;
// onboarding.js turns them into text (shared/js/i18n.js t()).
// No imports and no DOM: scripts/desktop-onboarding-rules.test.mjs tests it
// with plain Node.js.

/** The steps in order: where the files go, the project group, the first assembly. */
export var ONBOARDING_STEPS = ["folder", "group", "open"];

/** The values the app stores per connection (`onboarding`, src/profiles.rs). */
export var ONBOARDING_DONE = "done", ONBOARDING_DISMISSED = "dismissed";

/** Most assemblies step 3 offers. */
export var MAX_ASSEMBLIES = 5;

/**
 * Whether the wizard opens by itself. `settings.onboarding` is the stored
 * value of the active connection (`"done"`, `"dismissed"` or none);
 * `ctx.fromHelp` is *Help → Getting started…*, which always opens it;
 * `ctx.signedInThisRun` is true when the user came through the connect
 * screen in this run of the app. `ctx.hasCheckouts` (own check-outs) /
 * `ctx.hasCheckedIn` (a file version of the user's stored before, a
 * check-in or an upload – `has_checked_in` of GET /worklist), asked before
 * the wizard opens: a user who already works on this server – a new
 * computer, an expired sign-in – does not get it. An app that starts
 * already signed in (an update, a reload) does not show it either.
 */
export function shouldShow(settings, ctx) {
  ctx = ctx || {};
  if (ctx.fromHelp) return true;
  var stored = settings && settings.onboarding;
  if (stored === ONBOARDING_DONE || stored === ONBOARDING_DISMISSED) return false;
  if (ctx.hasCheckouts || ctx.hasCheckedIn) return false;
  return !!ctx.signedInThisRun;
}

/** The user's groups as `{code, name, cad, cadVersion}` – a plain code
 *  string counts as a group without name; empty codes are dropped. */
export function groupList(list) {
  return (Array.isArray(list) ? list : []).map(function (g) {
    if (typeof g === "string") g = { code: g };
    if (!g || typeof g.code !== "string" || !g.code.trim()) return null;
    return { code: g.code.trim(), name: g.name || "", cad: g.cad || "", cadVersion: g.cadVersion || "" };
  }).filter(Boolean);
}

/** Lower-case codes of a list of group codes (empty and non-text ones dropped). */
function codes(list) {
  return (Array.isArray(list) ? list : []).filter(function (c) {
    return typeof c === "string" && c.trim() !== "";
  }).map(function (c) { return c.trim().toLowerCase(); });
}

/** Whether a group (code) is already in the working folder. */
export function onDisk(code, facts) {
  return codes((facts || {}).groupsOnDisk).indexOf(String(code || "").toLowerCase()) !== -1;
}

/**
 * Why a step is skipped (a message ID), or null when it is shown. `facts`:
 *
 * * `workspaceRootPolicy` – an organization policy sets where working
 *   folders go (`WorkspaceRoot`, #378);
 * * `groupsOnDisk` – codes of the groups already fetched into this working
 *   folder: the folder was chosen before;
 * * `groups` – the project groups the user is a member of (groupList).
 *
 * A skipped folder step shows the folder as a line in the header; with one
 * group the group step is skipped and that group used; without a group the
 * group step is the last one (it says how to get one).
 */
export function skipReason(step, facts) {
  facts = facts || {};
  var mine = groupList(facts.groups);
  switch (step) {
    case "folder":
      if (facts.workspaceRootPolicy) return "onboarding.skip.folder_policy";
      if (codes(facts.groupsOnDisk).length) return "onboarding.skip.folder_used";
      return null;
    case "group":
      return mine.length === 1 ? "onboarding.skip.one_group" : null;
    case "open":
      return mine.length ? null : "onboarding.skip.no_group";
    default:
      return "onboarding.skip.unknown";
  }
}

/** The steps that are shown, in order. */
export function visibleSteps(facts) {
  return ONBOARDING_STEPS.filter(function (s) { return skipReason(s, facts) === null; });
}

/**
 * The step after `current` that is shown, or null when the wizard is at its
 * end. `current` null (or unknown) starts at the first shown step.
 * `wizard.back` goes the other way: the shown step before `current`, null
 * at the start.
 */
export function nextStep(wizard, facts) {
  wizard = wizard || {};
  var shown = visibleSteps(facts);
  var at = shown.indexOf(wizard.current);
  if (wizard.back) return at > 0 ? shown[at - 1] : null;
  if (at === -1) {
    var full = ONBOARDING_STEPS.indexOf(wizard.current);
    if (full === -1) return shown[0] || null;
    for (var i = full + 1; i < ONBOARDING_STEPS.length; i++) {
      if (shown.indexOf(ONBOARDING_STEPS[i]) !== -1) return ONBOARDING_STEPS[i];
    }
    return null;
  }
  return at + 1 < shown.length ? shown[at + 1] : null;
}

/** Whether `step` is the last shown one (it has the commit buttons). */
export function isFinal(step, facts) {
  return visibleSteps(facts).indexOf(step) !== -1 && nextStep({ current: step }, facts) === null;
}

/** `Step 2 of 3` of a shown step as a message; null for one that is skipped. */
export function stepPosition(step, facts) {
  var shown = visibleSteps(facts);
  var at = shown.indexOf(step);
  return at === -1 ? null : { id: "onboarding.position", params: { at: at + 1, total: shown.length } };
}

/**
 * The group the wizard starts with: the first of the user's groups that a
 * Get already fetched into this working folder, else the first group; null
 * without groups. Returns the code as given.
 */
export function startGroup(facts) {
  facts = facts || {};
  var mine = groupList(facts.groups);
  if (!mine.length) return null;
  var fetched = mine.filter(function (g) { return onDisk(g.code, facts); });
  return (fetched[0] || mine[0]).code;
}

/** The group the user chose in step 2 (`wanted`, a code) when it is one of
 *  theirs, else startGroup's; null without groups. The group object. */
export function chosenGroup(facts, wanted) {
  var mine = groupList((facts || {}).groups);
  var code = String(wanted || "").toLowerCase();
  var hit = mine.filter(function (g) { return g.code.toLowerCase() === code; })[0];
  if (hit) return hit;
  var start = startGroup(facts);
  return mine.filter(function (g) { return g.code === start; })[0] || null;
}

/** The group's folder in the working folder (`<dir>/<GROUP>`), with the
 *  separator the folder already uses. */
export function groupFolder(dir, code) {
  dir = String(dir || "");
  if (!dir) return code || "";
  var sep = dir.indexOf("\\") !== -1 && dir.indexOf("/") === -1 ? "\\" : "/";
  return dir.replace(/[\\/]+$/, "") + sep + (code || "");
}

/** Whether a node's check-out is the user's: by user id when the node has
 *  one (`checked_out_by`), else by display name – names are not unique, so
 *  the name is only a fallback for an older server. `me` = `{id, name}`. */
export function lockedByMe(x, me) {
  me = me || {};
  if (x.checked_out_by && me.id) return x.checked_out_by === me.id;
  return !!x.checked_out_by_name && x.checked_out_by_name === me.name;
}

/**
 * The assemblies step 3 offers, from the top-level nodes of a group's
 * structure (`GET /projects/{code}/structure`: no BOM line uses them).
 * An assembly is an item of type `assembly` or one with BOM lines; a
 * restricted placeholder and a node without a revision are left out. For a
 * CAD editor (`opts.editor`, the default) only free revisions in work:
 * those are the ones *Check out and open* is the next step for (one the
 * user holds already would offer *Open in CAD* or *Check in…* instead). A viewer gets them in
 * any state. Order: the most BOM lines (the main assembly of a group, not a
 * small sub-assembly), then by number. At most `n` (MAX_ASSEMBLIES).
 */
export function pickAssemblies(nodes, opts) {
  opts = opts || {};
  var n = typeof opts.n === "number" && opts.n > 0 ? opts.n : MAX_ASSEMBLIES;
  var editor = opts.editor !== false;
  return (Array.isArray(nodes) ? nodes : []).filter(function (x) {
    if (!x || x.restricted || !x.item_number || !x.code) return false;
    if (x.item_type !== "assembly" && !(x.bom_lines > 0)) return false;
    if (!editor) return true;
    if (x.state !== "in_work") return false;
    return !(x.checked_out_by || x.checked_out_by_name);
  }).slice().sort(function (a, b) {
    var la = a.bom_lines || 0, lb = b.bom_lines || 0;
    if (la !== lb) return lb - la;
    return a.item_number < b.item_number ? -1 : a.item_number > b.item_number ? 1 : 0;
  }).slice(0, n);
}

/**
 * Why step 3 offers no assembly: the group has none (`onboarding.open.none`),
 * or – for a CAD editor – none of them is free and in work, e.g. all are
 * released or checked out (`onboarding.open.none_free`, which says so).
 */
export function noAssemblyMessage(nodes, opts) {
  opts = opts || {};
  var any = (Array.isArray(nodes) ? nodes : []).some(function (x) {
    return x && !x.restricted && x.item_number && x.code && (x.item_type === "assembly" || x.bom_lines > 0);
  });
  return any && opts.editor !== false ? "onboarding.open.none_free" : "onboarding.open.none";
}

/**
 * The one CAD line of step 1: the CAD programs found on this computer
 * (`cad_installed`: `{family, version, label}`), in neutral words. Labels are
 * listed once each, in the order found.
 */
export function cadLine(programs) {
  var seen = [];
  (Array.isArray(programs) ? programs : []).forEach(function (p) {
    var label = p && (p.label || p.family);
    if (label && seen.indexOf(label) === -1) seen.push(label);
  });
  return seen.length
    ? { id: "onboarding.cad.found", params: { programs: seen.join(", ") } }
    : { id: "onboarding.cad.none", params: {} };
}

/** The group's default CAD program as text (`names`: family → program
 *  name of the server's format list); "" without one. */
export function groupCadText(group, names) {
  if (!group || !group.cad) return "";
  var name = (names && names[group.cad]) || group.cad;
  return group.cadVersion ? name + " " + group.cadVersion : name;
}

/**
 * What *Check out and open* will do, as the lead of step 3. `opts`:
 * `dir` (working folder), `names` (family → program name), `editor` (may
 * check out) and `plan` – the answer of `cad_open_plan` for a file of the
 * group's default program (`{program, label, fallback}`), the same Rust rule
 * (cadlaunch::plan) the chip next to the button and the launch use: the
 * group's version when installed, else the newest installed version. So the
 * line names an installed program by the plan's label (with the group's
 * version named as missing when the plan fell back to another version), and
 * the default named with "not installed on this computer" when the plan has
 * no program. Without a default or without a plan: the program chosen for
 * each file. A viewer gets *Get…* instead – checking out needs the CAD
 * editor role.
 */
export function openLine(group, opts) {
  opts = opts || {};
  var folder = groupFolder(opts.dir, group && group.code);
  if (!opts.editor) return { id: "onboarding.open.viewer", params: { folder: folder } };
  var plan = opts.plan;
  if (!group || !group.cad || !plan) return { id: "onboarding.open.any", params: { folder: folder } };
  if (!plan.program) {
    return { id: "onboarding.open.program_missing", params: { folder: folder, program: groupCadText(group, opts.names) } };
  }
  var label = plan.label || groupCadText(group, opts.names);
  if (plan.fallback && group.cadVersion) {
    return { id: "onboarding.open.program_other", params: { folder: folder, program: label, wanted: groupCadText(group, opts.names) } };
  }
  return { id: "onboarding.open.program", params: { folder: folder, program: label } };
}

/** What the *Show* button of step 3 says it does: an editor's next step is
 *  *Check out and open*; a viewer only looks at the item. */
export function showTitle(code, number, editor) {
  return { id: editor ? "onboarding.open.show.title" : "onboarding.open.show.viewer_title", params: { group: code, number: number } };
}

/** The message ID of a role in the header line (`admin`, `editor`, `viewer`). */
export function roleMessage(role) {
  return { admin: "onboarding.role.admin", editor: "onboarding.role.editor" }[role] || "onboarding.role.viewer";
}

/** The secondary commit: *Finish and show <group> in Home*. */
export function finishLabel(code) {
  return code ? { id: "onboarding.finish_home", params: { group: code } } : { id: "onboarding.finish", params: {} };
}

/** Per computer: where the user is with the daily-loop tip of the revision
 *  tab (none, `learned` after the first check-in or cancelled check-out,
 *  `done` once the closing line was shown). */
export var LOOP_TIP_KEY = "tc.loopTip";

/**
 * The daily loop is taught where it happens, not on a wizard page: a tip
 * on the revision tab while its next step is *Check out and open*, *Open in
 * CAD* or *Check in…* (next-step.js; `checkout-open` in revision.js), until
 * the user's first check-in or cancelled check-out; then one closing line,
 * once. Returns the message ID to show (or null) and the value to store
 * (or null to keep it).
 */
export function loopTip(stored, next) {
  if (stored === "done") return { show: null, store: null };
  if (stored === "learned") return { show: "onboarding.tip.loop_done", store: "done" };
  return ["checkout", "checkout-open", "open", "checkin"].indexOf(next) !== -1
    ? { show: "onboarding.tip.loop", store: null }
    : { show: null, store: null };
}

/** The stored value after `action` – a check-in (`checkin`) or a cancelled
 *  check-out (`cancel`) ends the tip; parking and anything else keep it. */
export function loopTipAfter(stored, action) {
  if (stored === "done" || stored === "learned") return stored;
  return action === "checkin" || action === "cancel" ? "learned" : (stored || null);
}

/** Short names of the steps (the list on the left of the wizard). */
export var STEP_NAMES = {
  folder: "onboarding.step.folder",
  group: "onboarding.step.group",
  open: "onboarding.step.open"
};

/**
 * Title and paragraphs of a step as messages. `facts` as for skipReason,
 * plus `workspaceDir`, `editor` (CAD editor or administrator), `login`.
 */
export function stepSummary(step, facts) {
  facts = facts || {};
  var dir = facts.workspaceDir || "";
  var mine = groupList(facts.groups);
  switch (step) {
    case "folder":
      return { title: STEP_NAMES.folder, text: [
        { id: "onboarding.folder.text", params: { dir: dir } },
        { id: "onboarding.folder.later", params: {} }
      ] };
    case "group":
      if (!mine.length) {
        return { title: STEP_NAMES.group, text: [facts.editor
          ? { id: "onboarding.group.none.editor", params: {} }
          : { id: "onboarding.group.none.viewer", params: { login: facts.login || "" } }] };
      }
      return { title: STEP_NAMES.group, text: [{ id: "onboarding.group.question", params: {} }]
        .concat(facts.editor ? [] : [{ id: "onboarding.group.viewer", params: {} }]) };
    case "open":
      return { title: STEP_NAMES.open, text: [{ id: "onboarding.open.lead", params: {} }] };
    default:
      return { title: "", text: [] };
  }
}

/** The status word of a step on the Home card (startCard): `open` = not yet,
 *  `active` = begun (an own check-out, not checked in yet). */
export var CARD_STATUS = {
  done: "onboarding.status.done",
  skipped: "onboarding.status.skipped",
  open: "onboarding.card.status.open",
  active: "onboarding.card.status.active"
};

/**
 * The *Getting started* card on Home for a user who closed the wizard early
 * (*Don't show again*: stored `dismissed`): the wizard's three steps, each
 * ticked by itself from what the app already knows. `settings` as for
 * shouldShow; `facts` as for skipReason plus `workspaceDir`, `editor`,
 * `login`, `workspaceExists` (the working folder is there – a Get, a
 * check-out or *Change…* made or picked it; `workspace_exists` of the
 * session) and `worklist`, the answer of GET /worklist (own `checkouts` with
 * their `project_code`, `has_checked_in`). `ctx.seenOpen`: the card was
 * shown with a step still open earlier in this run of the app, for this
 * connection.
 *
 * * No card unless the stored value is `dismissed`: not while the wizard may
 *   still open by itself (none), not once it was finished or the card hidden
 *   or completed (`done`) – and not before the worklist answered: nothing is
 *   decided from facts that are missing.
 * * **Where your files go** ticks when the working folder is settled: set by
 *   the organization, there, or used by a Get or an own check-out.
 * * **Your project group** ticks when one of the user's groups is in the
 *   working folder: a Get fetched it or an own check-out copied files into
 *   it. Without a group it says how to get one, by role.
 * * **Your first assembly** is begun with an own check-out and done with the
 *   first check-in (`has_checked_in`) – the end of the loop, which ticks
 *   every step. A viewer cannot check out: the step is skipped for them, and
 *   their card is complete with the first two steps.
 * * Complete: `store` is `done`, so the card does not come back on this
 *   connection. It says so once (`show`) when the user saw it with a step
 *   open earlier in this run; a loop finished elsewhere (another computer,
 *   the web UI, before the card existed) ends it without a word.
 *
 * Returns null (no card, nothing to store) or `{show, complete, store,
 * progress, lead: [messages], steps: [{step, name, status, text: [messages],
 * note}]}`; `status` is a key of CARD_STATUS, a complete card has no texts.
 */
export function startCard(settings, facts, ctx) {
  ctx = ctx || {};
  facts = facts || {};
  if (!settings || settings.onboarding !== ONBOARDING_DISMISSED) return null;
  var wl = facts.worklist;
  if (!wl || typeof wl !== "object") return null;
  var mine = groupList(facts.groups);
  var dir = facts.workspaceDir || "";
  var editor = !!facts.editor;
  var policy = !!facts.workspaceRootPolicy;
  var rows = (Array.isArray(wl.checkouts) ? wl.checkouts : []).filter(Boolean);
  var checkedOut = rows.length > 0;
  var loopEnded = !!wl.has_checked_in;
  // the group the card names: one of the user's in the working folder (a
  // Get, else an own check-out), else the one the wizard starts with
  var held = rows.map(function (r) { return String(r.project_code || "").toLowerCase(); });
  var here = mine.filter(function (g) { return onDisk(g.code, facts); })[0] ||
    mine.filter(function (g) { return held.indexOf(g.code.toLowerCase()) !== -1; })[0] || null;
  var group = here || chosenGroup(facts, null);
  var none = {};
  function step(name, status, text, note) {
    return { step: name, name: STEP_NAMES[name], status: status, text: text || [], note: note || null };
  }
  function m(id, params) { return { id: id, params: params || none }; }

  var folderDone = loopEnded || checkedOut || policy || !!facts.workspaceExists || codes(facts.groupsOnDisk).length > 0;
  var steps = [
    folderDone
      ? step("folder", "done", [m("onboarding.folder.line", { dir: dir })], policy ? m("onboarding.skip.folder_policy") : null)
      : step("folder", "open", [m("onboarding.folder.text", { dir: dir }), m("onboarding.folder.missing")]),
    !mine.length
      ? step("group", "open", [editor ? m("onboarding.group.none.editor") : m("onboarding.group.none.viewer", { login: facts.login || "" })])
      : here || loopEnded
        ? step("group", "done", [m("onboarding.group.one", { group: group.code })], m("onboarding.group.on_disk"))
        : step("group", "open", [m(editor ? "onboarding.card.group.editor" : "onboarding.card.group.viewer",
          { group: group.code, folder: groupFolder(dir, group.code) })]),
    !editor ? step("open", "skipped", [], m("onboarding.group.viewer"))
      : loopEnded ? step("open", "done")
        : checkedOut ? step("open", "active", [m("onboarding.tip.loop")])
          : !mine.length ? step("open", "open", [], m("onboarding.skip.no_group"))
            : step("open", "open", [m("onboarding.card.open", { group: group.code })])
  ];
  var shown = steps.filter(function (s) { return s.status !== "skipped"; });
  var complete = loopEnded || shown.every(function (s) { return s.status === "done"; });
  if (complete) {
    shown.forEach(function (s) { s.status = "done"; s.text = []; s.note = null; });
  }
  var done = shown.filter(function (s) { return s.status === "done"; }).length;
  return {
    show: complete ? !!ctx.seenOpen : true,
    complete: complete,
    store: complete ? ONBOARDING_DONE : null,
    progress: m("onboarding.card.progress", { done: done, total: shown.length }),
    lead: complete
      ? [m("onboarding.card.complete")].concat(loopEnded ? [m("onboarding.tip.loop_done")] : [])
      : [m("onboarding.card.lead")],
    steps: steps
  };
}

/** Every message ID these rules and onboarding.js use (the test checks
 *  both catalogs and their placeholders). */
export var ONBOARDING_MESSAGE_IDS = [
  "onboarding.title", "onboarding.steps", "onboarding.status.done", "onboarding.status.skipped",
  "onboarding.connected", "onboarding.position",
  "onboarding.role.admin", "onboarding.role.editor", "onboarding.role.viewer",
  "onboarding.step.folder", "onboarding.step.group", "onboarding.step.open",
  "onboarding.skip.folder_policy", "onboarding.skip.folder_used", "onboarding.skip.one_group",
  "onboarding.skip.no_group", "onboarding.skip.unknown",
  "onboarding.folder.text", "onboarding.folder.later", "onboarding.folder.line",
  "onboarding.folder.change", "onboarding.folder.open", "onboarding.folder.missing",
  "onboarding.cad.found", "onboarding.cad.none",
  "onboarding.group.question", "onboarding.group.viewer", "onboarding.group.on_disk", "onboarding.group.one",
  "onboarding.group.none.editor", "onboarding.group.none.viewer", "onboarding.group.create",
  "onboarding.group.copy_login", "onboarding.group.copied",
  "onboarding.open.lead", "onboarding.open.program", "onboarding.open.program_other", "onboarding.open.program_missing",
  "onboarding.open.any", "onboarding.open.viewer", "onboarding.open.show", "onboarding.open.show.title",
  "onboarding.open.show.viewer_title", "onboarding.open.loading",
  "onboarding.open.failed", "onboarding.open.none", "onboarding.open.none_free", "onboarding.open.cad_setup", "onboarding.open.cad_setup.link",
  "onboarding.finish", "onboarding.finish_home", "onboarding.next", "onboarding.back", "onboarding.dismiss",
  "onboarding.tip.loop", "onboarding.tip.loop_done",
  "onboarding.card.lead", "onboarding.card.progress", "onboarding.card.status.open", "onboarding.card.status.active",
  "onboarding.card.group.editor", "onboarding.card.group.viewer", "onboarding.card.open", "onboarding.card.complete",
  "onboarding.card.open_wizard", "onboarding.card.hide", "onboarding.card.hide.label", "onboarding.card.hide.title",
  "onboarding.card.hidden"
];
