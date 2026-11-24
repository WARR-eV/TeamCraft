// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – pure rules of a Get: whether the files it copied into
// the working folder are complete enough to open the main CAD file.
// No imports and no DOM: scripts/desktop-get-rules.test.mjs tests it with
// plain Node.js. Used by revision.js (Check out and open), group-sync.js
// (the tally and result of Get project groups) and workspace.js
// (opening a CAD file of the Files or Workspace tab).

/**
 * Why a Get or check-out (`workspace::Downloaded`) left the working folder
 * too incomplete to open a file in the CAD program (#348), or null when the
 * file can be opened:
 *
 * * the Get was stopped (Cancel next to the progress bar) – the assembly
 *   would open without the parts that were still to come;
 * * files could not be copied (`failed`).
 *
 * Kept local changes, name clashes and references missing in the vault do
 * not stop the opening: those files are local (or never were in the vault),
 * and the Get's report lists them.
 */
export function incompleteGet(r) {
  if (!r) return null;
  if (r.cancelled) return "the Get was stopped before every file was in the working folder";
  var failed = (r.failed || []).length;
  if (failed) return failed === 1 ? "1 file could not be copied" : failed + " files could not be copied";
  return null;
}

/** The note when a file is not opened because of `why` (incompleteGet). */
export function notOpenedText(filename, why) {
  return filename + " was not opened: " + why + ". Get the files again, then open it.";
}

/**
 * The end of *Get project groups* (#449), from its tally `done` = `{ items,
 * files, kept, skipped, clashes, errors: [text] }` (see addDownload), whether
 * it was `stopped` and the group codes: `{ outcome, title, lines, problems }`.
 * `outcome` is "done" (everything copied), "problems" (finished, but some
 * items failed or clashed) or "stopped"; `lines` are the counts worth
 * naming, `problems` the problem texts (all of them – the dialog lists them;
 * each name clash is one of them, so it is not counted again in `lines`).
 */
export function groupsGetResult(done, stopped, codes) {
  var n = function (k, one, many) { return k + " " + (k === 1 ? one : many); };
  var errors = done.errors || [];
  var where = (codes || []).join(", ");
  var outcome = stopped ? "stopped" : errors.length ? "problems" : "done";
  var title = outcome === "stopped" ? "Stopped – the working folder has only part of " + where
    : outcome === "problems" ? "Finished with problems: " + where
    : !done.items && done.skipped ? "Nothing to get: no item in " + where + " has a released revision"
    : "Done: " + where + (codes && codes.length === 1 ? " is" : " are") + " in your working folder";
  // "up to date" only when the items it got really were unchanged – not when
  // nothing was got, something failed or the Get was stopped
  var upToDate = !done.files && done.items > 0 && !stopped && !errors.length;
  var lines = [n(done.items, "item", "items") + " got, " + n(done.files, "file", "files") + " copied" +
    (upToDate ? " (everything was up to date)" : "")];
  if (done.kept) lines.push(n(done.kept, "locally modified file", "locally modified files") + " kept");
  if (done.skipped) lines.push(n(done.skipped, "item", "items") + " without a released revision skipped");
  if (done.clashes) lines.push("Name clashes are listed below – nothing was overwritten");
  return { outcome: outcome, title: title, lines: lines, problems: errors.slice() };
}

/**
 * Adds the result `r` of one `workspace_download` of item `itemNumber` to the
 * tally `done` of *Get project groups*: the counts, and every failed file and
 * name clash as a problem ("<item>: <reason>", "<path>: <file> – taken by
 * <item>"). Returns true when the download was cancelled (the Get stops).
 */
export function addDownload(done, itemNumber, r) {
  var clashes = r.clashes || [];
  done.items++;
  done.files += (r.downloaded || []).length;
  done.kept += (r.kept || []).length;
  done.clashes += clashes.length;
  (r.failed || []).forEach(function (t) { done.errors.push(itemNumber + ": " + t); });
  clashes.forEach(function (c) { done.errors.push(c.path + ": " + c.file + " – taken by " + c.with); });
  return !!r.cancelled;
}

/**
 * Whether a finished *Get project groups* copied its groups completely, so
 * that the first-start wizard skips them: not stopped, no failed item or
 * name clash, and the rule *Latest revision* (`rule` as taken at the start).
 */
export function rememberGroups(done, stopped, rule) {
  return !stopped && !(done.errors || []).length && !done.clashes && rule === "latest";
}

/**
 * The stored list of got groups `previous` (as read from storage: anything
 * that is not an array counts as empty) with `codes` added – strings only,
 * no duplicates, order kept, the last 200.
 */
export function mergeGotGroups(previous, codes) {
  var out = [];
  (Array.isArray(previous) ? previous : []).concat(Array.isArray(codes) ? codes : []).forEach(function (c) {
    if (typeof c === "string" && out.indexOf(c) === -1) out.push(c);
  });
  return out.slice(-200);
}
