// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – *Local changes* (#416): the pure part (no DOM, no
// server calls, no imports), so it can be checked on its own
// (scripts/desktop-local-changes.test.mjs). navigator.js draws the worklist
// section *Local changes* from it, shell.js counts it into the rail badge.
//
// The scan (`local_changes_scan`, desktop/src/localchanges.rs) lists the
// files of every revision in the working folder that are modified, outdated,
// in conflict, new (a companion of a tracked file, e.g. a STEP export) or
// missing. This module groups them by item and says which actions each file
// and each selection allows – all of them the commands of the Workspace tab:
//
// * Check out (keep my file) – a file changed without a check-out (its
//   read-only flag cleared): `workspace_checkout` keeps locally modified
//   files and makes the revision mine;
// * Get latest – outdated or missing files: `workspace_download` replaces
//   unchanged copies and never overwrites a local change;
// * Check in – modified and new files of my own check-outs: the Check in all
//   flow (`sendLocalChanges` in shell.js) for the selection – always every
//   changed file of a revision, since the check-in releases its check-out;
// * Merge – a conflict (changed here and on the server) is opened in its
//   Workspace tab, a check-in would be refused;
// * Open folder – every row.

/** The states the list shows, in the order of their urgency. */
export var LOCAL_STATES = ["conflict", "modified", "new", "outdated", "missing"];

/** Labels of the states in the list. */
export var LOCAL_LABEL = {
  conflict: "Conflict", modified: "Modified", "new": "New", outdated: "Outdated", missing: "Not in working folder"
};

/**
 * How a row shows its state, outside the file name so a long name never hides
 * it: a modified file is a wrench in the colour of local changes (never the
 * lock colour – the file is not checked out unless the row says so), the other
 * states keep their badge. `notCheckedOut` adds the words "not checked out";
 * `title` is the tooltip, `label` what a screen reader says.
 */
export function localStateMark(file) {
  var label = LOCAL_LABEL[file.state] || file.state;
  var title = label;
  if (file.state === "modified") {
    title = file.without_checkout
      ? "Modified on this computer, but you have not checked it out – Check out keeps your file"
      : "Modified on this computer – check it in to store it on the server";
  } else if (file.without_checkout) {
    title = label + " – you have not checked it out; Check out keeps your file";
  }
  return { kind: file.state === "modified" ? "wrench" : "badge", label: label, title: title, notCheckedOut: !!file.without_checkout };
}

/** One file's key: unique over the whole scan (selection, ticks). */
export function localRowKey(rev, file) {
  return [rev.project || "", rev.item, rev.code, file.path || file.name].join("|").toLowerCase();
}

/** A revision's key: `project|item|code`, ignoring case. */
export function localRevKey(rev) {
  return [rev.project || "", rev.item, rev.code].join("|").toLowerCase();
}

/** The actions a file allows (in the order the row shows them). */
export function localRowActions(rev, file) {
  var out = [];
  if (rev.inaccessible) return ["folder"];
  var changed = file.state === "modified" || file.state === "new";
  if (file.state === "conflict") out.push("merge");
  if (changed && !rev.checked_out) out.push("checkout");
  if (changed && rev.checked_out) out.push("checkin");
  if (file.state === "outdated" || file.state === "missing") out.push("get");
  out.push("folder");
  return out;
}

/** A short line of the file's versions: "v4 on the server, yours v2". */
export function localVersionText(file) {
  if (file.server_version && file.base_version && file.server_version !== file.base_version) {
    return "v" + file.server_version + " on the server, yours v" + file.base_version;
  }
  if (file.server_version) return "v" + file.server_version;
  if (file.base_version) return "yours v" + file.base_version;
  return "";
}

/** Sort order of the rows of a revision: urgency, then path. */
function localRowOrder(a, b) {
  var d = LOCAL_STATES.indexOf(a.state) - LOCAL_STATES.indexOf(b.state);
  if (d) return d;
  var pa = (a.path || a.name).toLowerCase(), pb = (b.path || b.name).toLowerCase();
  return pa < pb ? -1 : pa > pb ? 1 : 0;
}

/**
 * Groups the scan by item and attaches the actions.
 *
 * Returns `{items: [{item, project, revisions: [{…revision, key, rows:
 * [{…file, key, actions}]}]}], count, counts: {state → files}, withoutCheckout,
 * errors: ["PRT-1/A: …"], truncated, compared, tracked}`. `count` is the number
 * of files the rail badge adds.
 */
export function planLocalChanges(scan) {
  var out = {
    items: [], count: 0, counts: {}, withoutCheckout: 0, errors: [],
    truncated: !!(scan && scan.truncated), compared: (scan && scan.compared) || 0, tracked: (scan && scan.tracked) || 0
  };
  LOCAL_STATES.forEach(function (s) { out.counts[s] = 0; });
  var byItem = {};
  ((scan && scan.revisions) || []).forEach(function (rev) {
    var rows = (rev.files || []).filter(function (f) { return LOCAL_STATES.indexOf(f.state) >= 0; }).map(function (f) {
      out.count++;
      out.counts[f.state]++;
      if (f.without_checkout) out.withoutCheckout++;
      return Object.assign({}, f, { key: localRowKey(rev, f), actions: localRowActions(rev, f) });
    }).sort(localRowOrder);
    if (rev.error) out.errors.push(rev.item + "/" + rev.code + ": " + rev.error);
    if (!rows.length) return;
    var itemKey = ((rev.project || "") + "|" + rev.item).toLowerCase();
    if (!byItem[itemKey]) {
      byItem[itemKey] = { item: rev.item, project: rev.project || null, revisions: [] };
      out.items.push(byItem[itemKey]);
    }
    byItem[itemKey].revisions.push(Object.assign({}, rev, { key: localRevKey(rev), rows: rows }));
  });
  out.items.sort(function (a, b) {
    var x = a.item.toLowerCase(), y = b.item.toLowerCase();
    return x < y ? -1 : x > y ? 1 : 0;
  });
  return out;
}

/**
 * The check-in of one revision when `chosen` (rows of `rev`) holds a file
 * to check in, else null. A check-in releases the check-out, so every
 * changed file of the revision goes up – not only the chosen ones – or the
 * others would be left changed without a check-out; `added` names the ones
 * that were not chosen (the dialog shows them). Conflicts are never checked
 * in: they are named in `conflicts` so the flow keeps the revision checked
 * out.
 */
export function revisionCheckin(rev, chosen) {
  function can(r) { return r.actions.indexOf("checkin") >= 0; }
  if (!chosen.some(can)) return null;
  var picked = {};
  chosen.forEach(function (r) { picked[r.key] = true; });
  var all = rev.rows.filter(can);
  return {
    project: rev.project || null, item: rev.item, code: rev.code,
    upload: all.map(function (r) { return r.name; }),
    added: all.filter(function (r) { return !picked[r.key]; }).map(function (r) { return r.name; }),
    conflicts: rev.rows.filter(function (r) { return r.state === "conflict"; }).map(function (r) { return r.name; })
  };
}

/**
 * What a selection (`ticked`: row key → true) allows, one entry per
 * revision: `checkout` (revisions to check out, keeping the files), `get`
 * (revisions to get), and `checkin` in the shape of the Check in all report
 * (`{changed: [{project, item, code, upload, added, conflicts}], unreadable:
 * []}`, for `sendLocalChanges`; see revisionCheckin – a chosen file takes
 * every changed file of its revision along).
 */
export function selectionActions(plan, ticked) {
  var out = { checkout: [], get: [], checkin: { changed: [], unreadable: [] }, rows: 0 };
  plan.items.forEach(function (it) {
    it.revisions.forEach(function (rev) {
      var chosen = rev.rows.filter(function (r) { return ticked && ticked[r.key]; });
      if (!chosen.length) return;
      out.rows += chosen.length;
      var ref = { project: rev.project || null, item: rev.item, code: rev.code };
      if (chosen.some(function (r) { return r.actions.indexOf("checkout") >= 0; })) out.checkout.push(ref);
      if (chosen.some(function (r) { return r.actions.indexOf("get") >= 0; })) out.get.push(ref);
      var c = revisionCheckin(rev, chosen);
      if (c) out.checkin.changed.push(c);
    });
  });
  return out;
}

/** "3 modified · 1 conflict · 2 outdated" – the summary under the heading. */
export function localSummaryText(plan) {
  var parts = LOCAL_STATES.filter(function (s) { return plan.counts[s]; }).map(function (s) {
    return plan.counts[s] + " " + (s === "missing" ? "missing" : LOCAL_LABEL[s].toLowerCase());
  });
  if (plan.withoutCheckout) parts.push(plan.withoutCheckout + " changed without a check-out");
  return parts.join(" · ");
}
