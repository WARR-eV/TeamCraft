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
 * Whether the newest upload of a revision is the user's own – they checked
 * their work in last, so the revision is ready for the next step.
 */
export function lastCheckinMine(files, me) {
  var newest = null;
  (files || []).forEach(function (f) {
    if (!newest || time(f.uploaded_at) > time(newest.uploaded_at)) newest = f;
  });
  return !!newest && newest.uploaded_by === me;
}

/**
 * The most likely next step on a revision – drawn as the toolbar's one
 * primary button. `facts`:
 *
 * * `rev` – `state`, `checked_out_by`, `checked_out_at`;
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
 *   order) and my own check-in was the last one: *Release…*;
 * * `"checkout"` – a free revision in work otherwise: *Check out and open*;
 * * `"revise"` – a released revision: *Revise*;
 * * `null` – nothing: in review, obsolete, checked out by somebody else, or
 *   the user may not act.
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
    if (can.release && lastCheckinMine(facts.files, facts.me)) return "release";
    if (can.checkout) return "checkout";
    return null;
  }
  if (rev.state === "released" && can.revise) return "revise";
  return null;
}
