// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – the rules of refreshing lock data (#451): when the
// *Check-outs* tab polls, which response may still be applied, which tabs
// load again. Pure functions (no DOM, no Tauri), tested by
// scripts/desktop-lock-rules.test.mjs; lock-overview.js and shell.js use them.

/**
 * Numbers a request of `o` (a tab, or the worklist's counter): the returned
 * sequence number is handed to acceptSeq when its response arrives.
 */
export function takeSeq(o) {
  o.seq = (o.seq || 0) + 1;
  return o.seq;
}

/**
 * Whether the response of request `seq` may still be applied: not when a
 * newer request's response was applied already – requests run in parallel
 * and may finish in any order, and an older answer must not paint over a
 * newer one. Records `seq` as applied when it is accepted.
 */
export function acceptSeq(o, seq) {
  if (seq < (o.applied || 0)) return false;
  o.applied = seq;
  return true;
}

/** Whether the tab in front polls its overview: a Check-outs tab with data
 *  (it is not loading) while someone is signed in. */
export function shouldPoll(tab, me) {
  return !!tab && tab.kind === "locks" && !!tab.data && !!me;
}

/** Whether a fetched overview differs from the one shown (repaint only then). */
export function locksDiffer(a, b) {
  return JSON.stringify(a) !== JSON.stringify(b);
}

/**
 * The own check-outs changed: every Check-outs tab but the one in front
 * forgets its data (it loads again when it comes to the front). The one in
 * front keeps it until a new fetch succeeds, so a failed fetch leaves it
 * pollable instead of stuck on an overview nobody refreshes.
 */
export function forgetHiddenLocks(tabs, active) {
  (tabs || []).forEach(function (t) { if (t.kind === "locks" && t !== active) t.data = null; });
}
