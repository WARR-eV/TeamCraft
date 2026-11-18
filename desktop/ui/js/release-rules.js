// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – pure rules of releasing a revision: the reviewers of the
// release dialog and whether a draft change can be submitted.
// No imports and no DOM: scripts/desktop-release-rules.test.mjs tests it
// with plain Node.js. Used by workspace.js (release dialog) and change.js.

/**
 * The reviewers of the release dialog: the ticked group members plus the
 * user names typed into "Other reviewers" (separated by commas, semicolons
 * or spaces, a leading "@" ignored), without duplicates (case-insensitive).
 * Refused before anything is created (#291): the own user name – nobody
 * reviews their own change (four-eyes principle) – and, when the change is
 * to be submitted at once, no reviewer at all.
 *
 * Returns `{ error, reviewers }`; `error` is null when the list is fine.
 */
export function reviewerProblem(checked, othersText, me, submitNow) {
  var typed = String(othersText || "").split(/[\s,;]+/)
    .map(function (s) { return s.replace(/^@/, ""); })
    .filter(Boolean);
  var mine = String(me || "").toLowerCase();
  var seen = {}, reviewers = [];
  var all = (checked || []).concat(typed);
  for (var i = 0; i < all.length; i++) {
    var key = all[i].toLowerCase();
    if (mine && key === mine) {
      return {
        error: "You cannot review your own change (four-eyes principle). Name another CAD editor or administrator" +
          (submitNow ? ", or untick “Submit for review now” to create a draft." : "."),
        reviewers: []
      };
    }
    if (!seen[key]) { seen[key] = true; reviewers.push(all[i]); }
  }
  if (submitNow && !reviewers.length) {
    return {
      error: "Choose at least one reviewer to submit the change – or untick “Submit for review now” to create a draft.",
      reviewers: []
    };
  }
  return { error: null, reviewers: reviewers };
}

/**
 * Why a draft change cannot be submitted yet (the tooltip of the disabled
 * "Submit for review"), or null when it can: it needs a revision and a
 * reviewer, as the server checks (changes::submit).
 */
export function draftBlocker(items, reviewers) {
  if (!items) return "Add the revisions to release first.";
  if (!reviewers) return "Add at least one reviewer first – somebody other than you (four-eyes principle).";
  return null;
}
