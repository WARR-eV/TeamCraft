// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – pure rules of a Get: whether the files it copied into
// the working folder are complete enough to open the main CAD file.
// No imports and no DOM: scripts/desktop-get-rules.test.mjs tests it with
// plain Node.js. Used by revision.js (Check out and open), group-sync.js
// (the result of Get project groups) and workspace.js
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
 * files, kept, skipped, clashes, errors: [text] }`, whether it was
 * `stopped` and the group codes: `{ outcome, title, lines, problems }`.
 * `outcome` is "done" (everything copied), "problems" (finished, but some
 * items failed or clashed) or "stopped"; `lines` are the counts worth
 * naming, `problems` the problem texts (all of them – the dialog lists them).
 */
export function groupsGetResult(done, stopped, codes) {
  var n = function (k, one, many) { return k + " " + (k === 1 ? one : many); };
  var where = (codes || []).join(", ");
  var outcome = stopped ? "stopped" : (done.errors || []).length ? "problems" : "done";
  var title = outcome === "stopped" ? "Stopped – the working folder has only part of " + where
    : outcome === "problems" ? "Finished with problems: " + where
    : "Done: " + where + (codes && codes.length === 1 ? " is" : " are") + " in your working folder";
  var lines = [n(done.items, "item", "items") + " got, " + n(done.files, "file", "files") + " copied" +
    (done.files ? "" : " (everything was up to date)")];
  if (done.kept) lines.push(n(done.kept, "locally modified file", "locally modified files") + " kept");
  if (done.skipped) lines.push(n(done.skipped, "item", "items") + " without a released revision skipped");
  if (done.clashes) lines.push(n(done.clashes, "name clash", "name clashes") + " – nothing was overwritten");
  return { outcome: outcome, title: title, lines: lines, problems: (done.errors || []).slice() };
}
