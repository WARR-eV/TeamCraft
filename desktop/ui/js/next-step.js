// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – the pure rule of a revision's "next step": the one
// highlighted (primary) button of the revision toolbar (#536, #513 T2).
// No imports and no DOM: scripts/desktop-next-step.test.mjs tests it with
// plain Node.js. Used by revision.js (renderRevision and its re-check of the
// working folder).

/** Milliseconds of an ISO time stamp, NaN when there is none. */
function time(stamp) {
  return stamp ? Date.parse(stamp) : NaN;
}

/**
 * Whether files of the revision were uploaded by `me` after the current
 * check-out started – *Park…* uploads new versions and keeps the check-out,
 * so the working copy then matches the server although work was done.
 */
export function parkedSinceCheckout(rev, files, me) {
  var since = time(rev && rev.checked_out_at);
  if (!(since === since)) return false;
  return (files || []).some(function (f) {
    return f.uploaded_by === me && time(f.uploaded_at) > since;
  });
}

/**
 * Whether the revision was checked in by `me` since it was created: the
 * newest upload *made in this revision* is the user's own. `true`/`false`,
 * or `null` when that cannot be told (no creation time in the payload).
 *
 * Only uploads at or after the revision's `created_at` count: *Revise*
 * copies every file of the previous revision with its old `uploaded_by` /
 * `uploaded_at` (`services::revisions::revise`), so those carried-over files
 * are older than the revision and say nothing about work done in it. A
 * cancelled check-out uploads nothing, so it changes nothing here either.
 */
export function lastCheckinMine(rev, files, me) {
  var created = time(rev && rev.created_at);
  if (!(created === created)) return null;
  var newest = null;
  (files || []).forEach(function (f) {
    var at = time(f.uploaded_at);
    if (!(at >= created)) return;
    if (!newest || at > time(newest.uploaded_at)) newest = f;
  });
  return !!newest && newest.uploaded_by === me;
}

/**
 * The most likely next step on a revision – drawn as the toolbar's one
 * primary button. `facts`:
 *
 * * `rev` – `state`, `checked_out_by`, `checked_out_at`, `created_at`;
 * * `files` – the revision's files (`uploaded_by`, `uploaded_at`);
 * * `me` – the user's id;
 * * `localChanges` – my check-out: `true` (modified, new or conflicting files
 *   in the working folder), `false` (none) or `null` (not known: the working
 *   folder could not be read in time);
 * * `cadFile` – whether the revision has a native CAD file to open;
 * * `can` – `checkout`, `release`, `revise` of revisionAbilities.
 *
 * Returns one of:
 *
 * * `"open"` – my check-out, nothing changed or parked yet: *Open in CAD*
 *   (the CAD step comes first – an unchanged revision is not sent to review);
 * * `"checkin"` – my check-out with local changes, with versions parked since
 *   the check-out, with an unreadable working folder, or without a CAD file
 *   to open: *Check in…*;
 * * `"release"` – not checked out, in work, releasable (no open change
 *   order) and the newest upload made in this revision is mine
 *   (lastCheckinMine): *Release…*;
 * * `"checkout"` – a free revision in work otherwise – also one that holds
 *   only the files *Revise* carried over, or whose check-out was cancelled
 *   without an upload: *Check out and open*;
 * * `"revise"` – a released revision: *Revise*;
 * * `null` – nothing: in review, obsolete, checked out by somebody else, the
 *   user may not act, or a releasable free revision whose payload has no
 *   creation time to tell a check-in from carried-over files (no guess).
 */
export function revisionNextStep(facts) {
  var rev = facts.rev, can = facts.can || {};
  if (rev.checked_out_by) {
    if (rev.checked_out_by !== facts.me) return null;
    if (facts.localChanges !== false) return "checkin";
    if (parkedSinceCheckout(rev, facts.files, facts.me)) return "checkin";
    return facts.cadFile ? "open" : "checkin";
  }
  if (rev.state === "in_work") {
    if (can.release) {
      var checkedIn = lastCheckinMine(rev, facts.files, facts.me);
      if (checkedIn === null) return null;
      if (checkedIn) return "release";
    }
    if (can.checkout) return "checkout";
    return null;
  }
  if (rev.state === "released" && can.revise) return "revise";
  return null;
}
