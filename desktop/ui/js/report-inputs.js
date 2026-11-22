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
