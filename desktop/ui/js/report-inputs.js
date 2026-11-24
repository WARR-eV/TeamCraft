// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – Key inputs of a failed action for *Report…* (#381).
// No DOM, no state: unit-tested by scripts/desktop-report-inputs.test.mjs
// (plain Node.js, run by the CI job `ui`). core.js passes the result of
// run(statusText, fn, inputs) on to the report dialog; desktop/src/report.rs
// redacts the values (home folder, server address, tokens) and caps them
// again.

/** At most this many inputs per report (report.rs: MAX_INPUTS). */
export var MAX_REPORT_INPUTS = 12;
/** A value longer than this is cut and ended with "…" (report.rs: MAX_INPUT). */
export var MAX_REPORT_INPUT = 200;

/**
 * `inputs` = [[name, value], …] as [name, value] pairs of strings: entries
 * without a name or value (null, undefined, "") are left out, a path is
 * reduced to its file name (the folders stay on this computer), long values
 * are cut. Anything that is not an array gives [].
 */
export function reportInputs(inputs) {
  if (!Array.isArray(inputs)) return [];
  var out = [];
  for (var i = 0; i < inputs.length && out.length < MAX_REPORT_INPUTS; i++) {
    var kv = inputs[i];
    if (!Array.isArray(kv) || kv.length < 2) continue;
    if (kv[0] === null || kv[0] === undefined || kv[1] === null || kv[1] === undefined) continue;
    var name = String(kv[0]).trim();
    var value = String(kv[1]).trim();
    if (!name || !value) continue;
    if (/^(file|path|folder)$/i.test(name) || /[\\/]/.test(value) && /^[A-Za-z]:[\\/]|^[\\/~]/.test(value)) {
      value = lastPathPart(value);
    }
    if (value.length > MAX_REPORT_INPUT) value = value.slice(0, MAX_REPORT_INPUT - 1) + "…";
    out.push([name, value]);
  }
  return out;
}

/** Last part of a Windows or Unix path ("C:\\work\\a.ipt" → "a.ipt"). */
function lastPathPart(value) {
  var parts = value.split(/[\\/]+/).filter(function (p) { return p !== ""; });
  return parts.length ? parts[parts.length - 1] : value;
}

/**
 * The key inputs of an action on revision `rev`: its project group, item
 * number and revision code, then `extra` ([[name, value], …], e.g. the file).
 */
export function revisionInputs(rev, extra) {
  rev = rev || {};
  return [["group", rev.project_code], ["item", rev.item_number], ["revision", rev.code]].concat(extra || []);
}

/** The failure handed to the report dialog by an error message. */
export function failureOf(action, error, inputs) {
  return { action: action || "", error: error || "", inputs: reportInputs(inputs) };
}

/**
 * The key inputs of an action on change order `c` (#471): its number and
 * status, then `extra` (the reviewer, the decision …).
 */
export function changeInputs(c, extra) {
  c = c || {};
  return [["change", c.number], ["status", c.status]].concat(extra || []);
}

/** The key inputs of an action on change request `r` (#471). */
export function requestInputs(r, extra) {
  r = r || {};
  return [["request", r.number], ["status", r.status]].concat(extra || []);
}

/**
 * The key inputs of a setting that could not be saved (#471): its name and
 * the value it was to get – a boolean as "on" / "off".
 */
export function settingInputs(name, value) {
  return [["setting", name], ["value", value === true ? "on" : value === false ? "off" : value]];
}

/**
 * The key inputs of a tab whose content could not be loaded (#471): a change
 * order or request by its number, the check-outs of a group, otherwise a
 * revision with its sub-tab.
 */
export function tabInputs(tab) {
  if (tab.kind === "change") return changeInputs({ number: tab.number });
  if (tab.kind === "request") return requestInputs({ number: tab.number });
  if (tab.kind === "locks") return [["group", tab.code], ["view", "check-outs"]];
  return [["item", tab.number], ["revision", tab.code], ["sub-tab", tab.sub]];
}

/** At most this many messages are kept for the report's recent messages. */
export var MAX_RECENT_ERRORS = 50;
/** A message equal to one of this many latest ones is not kept again. */
var RECENT_REPEAT = 5;

/**
 * The report's recent messages `list` ("HH:MM:SS text", oldest first) after
 * the error `text` of `action` came up at `now` (a Date): "action: text",
 * the last MAX_RECENT_ERRORS. The same message drawn again – a list redrawn
 * with the same error in place – is not kept again while it is among the
 * latest few, so it cannot push older errors out.
 */
export function recentErrorsAfter(list, action, text, now) {
  list = Array.isArray(list) ? list : [];
  var message = (action ? action + ": " : "") + text;
  var latest = list.slice(-RECENT_REPEAT);
  for (var i = 0; i < latest.length; i++) {
    if (String(latest[i]).slice(9) === message) return list;
  }
  return list.concat([now.toISOString().slice(11, 19) + " " + message]).slice(-MAX_RECENT_ERRORS);
}

/**
 * The failure of *Check in all* for *Report…* (#471): `list` = the revisions
 * it could not check in (texts); null when there were none.
 */
export function checkInAllFailure(list) {
  if (!Array.isArray(list) || !list.length) return null;
  return failureOf("Checking in all my check-outs", list.join("; "), [["failed", String(list.length)]]);
}
