// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – rules of the Home tab (#513 T1, T9): pure functions,
// no DOM, no server; tested by scripts/desktop-home-rules.test.mjs.
//
// Home is the pinned first tab (the dashboard). It shows what the Worklist
// navigator beside it cannot – the product counts, what this person opened
// lately and the recent activity – and the worklist only as counts that open
// the Worklist.

/** How many recently opened objects Home lists. */
export var RECENT_MAX = 8;

/** The kinds of tab Home remembers as recently opened. */
var RECENT_KINDS = { revision: true, change: true, request: true };

/**
 * The recent list after `tab` was opened: the tab first, an older entry of
 * the same object dropped, at most `max` entries. Tabs of other kinds (Agent,
 * Check-outs) and broken entries leave the list as it is. Returns a new array.
 */
export function rememberRecent(list, tab, max) {
  var cap = max || RECENT_MAX;
  var clean = (Array.isArray(list) ? list : []).filter(validRecent);
  if (!tab || !RECENT_KINDS[tab.kind] || !tab.key || !tab.number) return clean.slice(0, cap);
  var entry = { key: String(tab.key), kind: tab.kind, number: String(tab.number),
    code: tab.kind === "revision" && tab.code ? String(tab.code) : null,
    title: String(tab.title || tab.number) };
  return [entry].concat(clean.filter(function (e) { return e.key !== entry.key; })).slice(0, cap);
}

/** An entry read back from storage is used only when it has every field. */
export function validRecent(e) {
  return !!e && typeof e === "object" && RECENT_KINDS[e.kind] === true &&
    typeof e.key === "string" && typeof e.number === "string" && typeof e.title === "string" &&
    (e.kind !== "revision" || typeof e.code === "string");
}

/**
 * The worklist as one line of counts for Home: [{text, n}] in the order of
 * the Worklist navigator's sections that need something from this person
 * (own check-outs first, the CAD loop; then reviews, dependents, assigned
 * requests). Empty sections are left out; an empty list means nothing waits.
 */
export function worklistSummary(wl) {
  if (!wl) return [];
  var count = function (a) { return Array.isArray(a) ? a.length : 0; };
  var parts = [
    { n: count(wl.checkouts), one: "check-out", many: "check-outs" },
    { n: count(wl.reviews), one: "review waiting", many: "reviews waiting" },
    { n: count(wl.impact_groups), one: "dependent to check", many: "dependents to check" },
    { n: count(wl.requests_assigned), one: "request assigned to you", many: "requests assigned to you" }
  ];
  return parts.filter(function (p) { return p.n > 0; }).map(function (p) {
    return { n: p.n, text: p.n + " " + (p.n === 1 ? p.one : p.many) };
  });
}
