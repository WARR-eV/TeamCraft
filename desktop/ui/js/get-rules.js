// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – pure rules of a Get: whether the files it copied into
// the working folder are complete enough to open the main CAD file.
// No imports and no DOM: scripts/desktop-get-rules.test.mjs tests it with
// plain Node.js. Used by revision.js (Check out and open) and workspace.js
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
