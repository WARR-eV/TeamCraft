// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – pure rules of the first-start wizard (*Getting
// started*, #422): which steps there are, which are skipped, when the wizard
// shows and the texts of each step.
// No imports and no DOM: scripts/desktop-onboarding-rules.test.mjs tests it
// with plain Node.js. Used by onboarding.js.

/** The steps in order. */
export var ONBOARDING_STEPS = ["server", "signin", "folder", "group", "cad", "loop"];

/** The values the app stores in settings.json (`onboarding`, src/settings.rs). */
export var ONBOARDING_DONE = "done", ONBOARDING_DISMISSED = "dismissed";

/**
 * Whether the wizard opens by itself. `settings.onboarding` is the stored
 * value (`"done"`, `"dismissed"` or none); `ctx.fromHelp` is *Help → Getting
 * started…*, which always opens it; `ctx.signedInThisRun` is true when the
 * user came through the connect screen in this run of the app – a first start.
 * An app that starts already signed in (an update of an older version, a
 * reload of the window) does not show it: those users know the app.
 */
export function shouldShow(settings, ctx) {
  ctx = ctx || {};
  if (ctx.fromHelp) return true;
  var stored = settings && settings.onboarding;
  if (stored === ONBOARDING_DONE || stored === ONBOARDING_DISMISSED) return false;
  return !!ctx.signedInThisRun;
}

/** Lower-case codes of a list of group codes (empty and non-text ones dropped). */
function codes(list) {
  return (Array.isArray(list) ? list : []).filter(function (c) {
    return typeof c === "string" && c.trim() !== "";
  }).map(function (c) { return c.trim().toLowerCase(); });
}

/**
 * Why a step is skipped, or null when it is shown. `facts`:
 *
 * * `serverFixed` – an organisation policy sets the server (#378);
 * * `signedIn` – the app has a sign-in;
 * * `groups` – codes of the project groups the user is a member of;
 * * `groupsOnDisk` – codes of the groups already fetched into this working folder.
 */
export function skipReason(step, facts) {
  facts = facts || {};
  switch (step) {
    case "server":
      return facts.serverFixed ? "Set by your organization" : null;
    case "signin":
      return facts.signedIn ? "Signed in" : null;
    case "group": {
      var mine = codes(facts.groups);
      if (!mine.length) return "You are not a member of a project group yet";
      var onDisk = codes(facts.groupsOnDisk);
      if (mine.every(function (c) { return onDisk.indexOf(c) !== -1; })) return "Your project groups are already in the working folder";
      return null;
    }
    default:
      return ONBOARDING_STEPS.indexOf(step) === -1 ? "Unknown step" : null;
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
    // a step that is skipped now (the group was fetched meanwhile): go on
    // with the first shown step after it in the full order
    var full = ONBOARDING_STEPS.indexOf(wizard.current);
    if (full === -1) return shown[0] || null;
    for (var i = full + 1; i < ONBOARDING_STEPS.length; i++) {
      if (shown.indexOf(ONBOARDING_STEPS[i]) !== -1) return ONBOARDING_STEPS[i];
    }
    return null;
  }
  return at + 1 < shown.length ? shown[at + 1] : null;
}

/**
 * The project group the last step opens in Home (*Finish and open <group>*):
 * the first of the user's groups that a Get already fetched into this working
 * folder, else the first of the user's groups; null without groups (the step
 * then offers the Projects page). Returns the code as given in `facts.groups`.
 */
export function startGroup(facts) {
  facts = facts || {};
  var mine = (Array.isArray(facts.groups) ? facts.groups : []).filter(function (c) {
    return typeof c === "string" && c.trim() !== "";
  });
  if (!mine.length) return null;
  var onDisk = codes(facts.groupsOnDisk);
  var fetched = mine.filter(function (c) { return onDisk.indexOf(c.trim().toLowerCase()) !== -1; });
  return (fetched[0] || mine[0]).trim();
}

/** `Step 2 of 4` of a shown step; "" for one that is skipped. */
export function stepPosition(step, facts) {
  var shown = visibleSteps(facts);
  var at = shown.indexOf(step);
  return at === -1 ? "" : "Step " + (at + 1) + " of " + shown.length;
}

/**
 * The setup files of the CAD step (#478), in the order shown. `kind` is the
 * value the app's commands `cad_setup_preview` / `cad_setup_create` take
 * (src/cadsetup.rs, `Kind::as_str`); the app writes the content, the web
 * view never names a path. Formats and their sources:
 * docs/src/research/cad-setup-files.md.
 */
export var CAD_SETUP_FILES = [
  { kind: "inventor", program: "Inventor", what: "a project file (<group>.ipj) whose workspace is the group's folder, so references resolve inside it" },
  { kind: "nx", program: "NX", what: "a text file with the group's folder (and its subfolders) as search folder and where to enter it in the Assembly Load Options" },
  { kind: "solidedge", program: "Solid Edge", what: "a link management file (LinkMgmt.txt) with the group's folder as search path" }
];

/**
 * The project groups the CAD step offers: the user's groups that a Get
 * already fetched into this working folder (the files go into the group's
 * folder, so it must be there). Codes as given in `facts.groups`, in order.
 */
export function cadSetupGroups(facts) {
  facts = facts || {};
  var onDisk = codes(facts.groupsOnDisk);
  return (Array.isArray(facts.groups) ? facts.groups : []).filter(function (c) {
    return typeof c === "string" && c.trim() !== "" && onDisk.indexOf(c.trim().toLowerCase()) !== -1;
  }).map(function (c) { return c.trim(); });
}

/** The picks at the start of the step: nothing ticked. */
export function initialCadPicks() {
  return {};
}

/**
 * The rows of the CAD step. `previews` maps a kind to what the app answered
 * for it: `{ exists, path, content }`, `{ error }`, or nothing while it
 * loads; `picks` maps a kind to true when the user ticked it. A row can be
 * ticked only with its preview shown and when its file does not exist yet –
 * an existing file is never overwritten.
 */
export function cadSetupRows(previews, picks) {
  previews = previews || {};
  picks = picks || {};
  return CAD_SETUP_FILES.map(function (f) {
    var p = previews[f.kind];
    var note = !p ? "Loading the preview …"
      : p.error ? p.error
      : p.exists ? "Already there – TeamCraft never overwrites it."
      : "";
    var disabled = !p || !!p.error || !!p.exists;
    return {
      kind: f.kind,
      program: f.program,
      what: f.what,
      path: p && !p.error ? p.path || "" : "",
      content: p && !p.error && !p.exists ? p.content || "" : "",
      disabled: disabled,
      ticked: !disabled && picks[f.kind] === true,
      note: note
    };
  });
}

/** The kinds to create: ticked, previewed and not there yet. */
export function cadSetupToCreate(previews, picks) {
  return cadSetupRows(previews, picks).filter(function (r) { return r.ticked; }).map(function (r) { return r.kind; });
}

/**
 * The message after creating: `results` lists `{ kind, path }` for a file
 * written and `{ kind, error }` for one refused. `{ ok, lines, text }`.
 */
export function cadSetupReport(results) {
  results = Array.isArray(results) ? results : [];
  var name = function (kind) {
    var f = CAD_SETUP_FILES.filter(function (x) { return x.kind === kind; })[0];
    return f ? f.program : kind;
  };
  var made = results.filter(function (r) { return r && r.path && !r.error; });
  var failed = results.filter(function (r) { return r && r.error; });
  var lines = made.map(function (r) { return name(r.kind) + ": created " + r.path + "."; })
    .concat(failed.map(function (r) { return name(r.kind) + ": " + r.error; }));
  if (made.length) lines.push("A starting point: check the files in your CAD program.");
  return { ok: failed.length === 0 && made.length > 0, lines: lines, text: lines.join(" ") };
}

/** Short names of the steps (the list on the left of the wizard). */
export var STEP_NAMES = {
  server: "Server",
  signin: "Sign-in",
  folder: "Working folder",
  group: "Project group",
  cad: "CAD setup",
  loop: "Daily loop"
};

/**
 * Title and paragraphs of a step. `facts` as for skipReason, plus
 * `serverUrl`, `workspaceDir`, `username`, `groups`.
 */
export function stepSummary(step, facts) {
  facts = facts || {};
  var ws = facts.workspaceDir || "the default folder";
  var mine = codes(facts.groups);
  switch (step) {
    case "server":
      return { title: "Your TeamCraft server", text: [
        facts.serverUrl ? "TeamCraft Desktop works with the server at " + facts.serverUrl + "." : "TeamCraft Desktop needs the address of your TeamCraft server.",
        facts.serverFixed ? "Your organization set this server; it cannot be changed here." : "You can switch to another server later in Settings → Account."
      ] };
    case "signin":
      return { title: "Sign in", text: [
        facts.signedIn ? "You are signed in" + (facts.username ? " as " + facts.username : "") + "." : "Sign in with your TeamCraft account or your organization's single sign-on.",
        "The sign-in is kept in your system's keychain, so you do not have to sign in at every start."
      ] };
    case "folder":
      return { title: "Your working folder", text: [
        "Your CAD files live in the working folder on this computer: " + ws + ".",
        "Every project group gets its own folder in it, with the folders of its files: <working folder>/<group>/<folder>/<file>. Open and save your CAD files there.",
        "Files you have not checked out are read-only; checking out makes them writable, checking in sends your changes to the server."
      ] };
    case "group":
      return { title: "Get your project group", text: [
        mine.length ? "You are a member of " + (mine.length === 1 ? "the project group " : mine.length + " project groups: ") + facts.groups.filter(function (c) { return typeof c === "string" && c.trim(); }).join(", ") + "." : "You are not a member of a project group yet.",
        "Get project groups… copies the files of every item of a group into your working folder – the latest revision, also the ones still in work – so assemblies open with all their parts.",
        "Nothing is overwritten: files you changed locally are kept and name clashes are reported."
      ] };
    case "cad": {
      var onDisk = cadSetupGroups(facts);
      return { title: "CAD setup (optional)", text: [
        "Some CAD programs find the parts of an assembly through a project file or search paths. TeamCraft can create small setup files that point them at a project group's folder.",
        onDisk.length
          ? "Tick the files you want and check the preview: nothing is ticked at the start, nothing is written before you choose Create, and a file that is already there is never overwritten."
          : "Get a project group first (the step before): the files go into its folder. You can come back with Help → Getting started….",
        "The files are a starting point: check them in your CAD program. They were not tested with every version of Inventor, NX and Solid Edge."
      ] };
    }
    case "loop": {
      var start = startGroup(facts);
      return { title: "Your daily loop in 3 steps", text: [
        "1. Check out and open – on an item or in its Workspace tab: locks the item for you, copies its files (and the parts of an assembly) and opens the main file in your CAD program.",
        "2. Save in your CAD program – into the working folder, as always.",
        "3. Check in… – sends the changed files to the server with a comment and releases the lock. Your worklist shows what you have checked out.",
        start ? "Start now: Finish and open " + start + " shows the files of the group in Home – right-click the assembly → Check out and open."
          : "Start now: Finish and show Projects lists the project groups – choose one, then right-click an assembly in its files → Check out and open."
      ] };
    }
    default:
      return { title: "", text: [] };
  }
}
