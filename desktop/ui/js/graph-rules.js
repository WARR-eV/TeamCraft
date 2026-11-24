// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – pure rules of the Graph sub-tab (js/graph.js): which
// neighbours a revision's graph draws and the note when it has none.
// No imports and no DOM: scripts/desktop-graph-rules.test.mjs tests it with
// plain Node.js.

/**
 * The neighbour nodes of a centre, deduplicated by item number. "down" = BOM
 * children and references; "up" = the assemblies whose BOM uses this item
 * (`d.whereUsed`, the rows of `GET /items/{number}/where-used`, #518) and the
 * items that reference it or whose files reference it (`d.refs`).
 *
 * The where-used rows are those of every revision of a parent, newest first
 * (the server orders them so): the first row of a parent is the one drawn.
 * Rows the user may not read are not in the list (the server leaves them out).
 */
export function neighbours(d) {
  var by = {}, order = [];
  function add(n) {
    if (!n.number || !n.code) return; // unresolved: nothing to open or draw
    var k = n.number;
    var ex = by[k];
    if (!ex) { by[k] = n; order.push(k); return; }
    // the same item on several BOM lines: its instances add up
    if (ex.rel === "bom" && n.rel === "bom") { ex.qty = (ex.qty || 0) + (n.qty || 0); return; }
    // keep a "down" (structural) relation over an "up" one, and fill blanks
    if (ex.dir === "up" && n.dir === "down") { n.alsoUsedBy = true; by[k] = n; }
    else { ex.type = ex.type || n.type; ex.name = ex.name || n.name; }
  }
  var refs = d.refs || {};
  (d.bom || []).forEach(function (b) {
    add({ number: b.child_number, code: b.child_latest_code, state: b.child_latest_state,
      type: b.child_type, name: b.child_name, dir: "down", rel: "bom", qty: b.quantity });
  });
  (refs.outgoing || []).forEach(function (r) {
    add({ number: r.to_number, code: r.to_latest_code, state: r.to_latest_state,
      type: null, name: r.to_name, dir: "down", rel: "reference" });
  });
  (d.whereUsed || []).forEach(function (w) {
    add({ number: w.parent_number, code: w.parent_code, state: w.parent_state,
      type: null, name: w.parent_name, dir: "up", rel: "used-by", inBom: true });
  });
  (refs.incoming || []).forEach(function (r) {
    add({ number: r.from_number, code: r.from_code, state: r.from_state,
      type: null, name: r.from_name, dir: "up", rel: "used-by" });
  });
  (refs.file_referrers || []).forEach(function (f) {
    add({ number: f.item_number, code: f.revision_code, state: f.revision_state,
      type: null, name: f.item_name, dir: "up", rel: "used-by", via: f.filename });
  });
  return order.map(function (k) { return by[k]; });
}

/** The note of a graph with no neighbour: only the centre is drawn. */
export function graphAloneText(number) {
  return number + " has no BOM children, references or assemblies that use it.";
}

/** The note when `skipped` of `total` neighbours get no preview and
 *  complexity (above the enrichment cap). Null when none is skipped. */
export function graphSkippedText(skipped, total) {
  if (!skipped) return null;
  return skipped + " of " + total + " neighbours shown without preview and complexity – open one to see its details.";
}
