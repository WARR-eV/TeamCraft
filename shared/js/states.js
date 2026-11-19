/* SPDX-License-Identifier: MPL-2.0 */
// The glyph of a state: colour is never the only signal (badge, state glyph).
//
// The server's macros (templates/macros.html, the `glyph` filter backed by
// domain::state_glyph) and this table are pinned to the same test vectors
// (shared/test-vectors/states.json). A state this table does not know – from
// a newer server – gets no glyph, never an exception.

var GLYPHS = {};
function set(glyph, states) { states.forEach(function (s) { GLYPHS[s] = glyph; }); }
set("✓", ["released", "approved", "accepted", "implemented", "pass", "sent", "up_to_date", "unchanged"]);
set("○", ["in_review", "pending", "in_triage", "retrying", "out_of_date", "modified", "outdated"]);
set("×", ["rejected", "fail", "failed", "conflict", "clash"]);
set("–", ["obsolete", "cancelled", "withdrawn", "deferred", "missing"]);
set("◐", ["in_work", "draft", "new"]);
set("▲", ["high"]);
set("▲▲", ["critical"]);

/** The glyph of a state, "" for none or an unknown state. */
export function glyph(state) {
  return Object.prototype.hasOwnProperty.call(GLYPHS, state) ? GLYPHS[state] : "";
}
