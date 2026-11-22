// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – *Update the BOM from the CAD file* at check-in (#407):
// the pure part (no DOM, no server calls), so it can be checked on its own.
// workspace.js (the *Check in…* dialog) uses it.
//
// The server updates the BOM from the references of the checked-in version
// (new lines added, counted quantities corrected, nothing removed). The box
// is offered only when a file of the revision can tell the structure:
// assemblies (Inventor, SOLIDWORKS, …), NX parts and SolidCraft designs (an
// assembly by their content) and STEP files (assemblies with external
// parts) – the server checks the content and leaves the BOM alone otherwise.

/** Extensions whose files may be assemblies by content, whatever their kind. */
export var BOM_STRUCTURE_EXTENSIONS = ["prt", "stp", "step", "sc"];

/** Does one of the file names lead to a BOM from the CAD file? `kinds` is
 *  the extension → format kind map of GET /formats. */
export function bomStructureFile(names, kinds) {
  return (names || []).some(function (name) {
    var dot = String(name || "").lastIndexOf(".");
    if (dot <= 0) return false;
    var ext = String(name).slice(dot + 1).toLowerCase();
    // Creo iterations: bracket.asm.7 → asm
    if (/^\d+$/.test(ext)) {
      var base = String(name).slice(0, dot);
      var inner = base.lastIndexOf(".");
      if (inner <= 0) return false;
      ext = base.slice(inner + 1).toLowerCase();
    }
    return (kinds && kinds[ext] === "assembly") || BOM_STRUCTURE_EXTENSIONS.indexOf(ext) >= 0;
  });
}

function plural(n, one, many) { return n + " " + (n === 1 ? one : many); }

/** The `bom` part of the check-in answer as one line: "BOM: 4 lines added,
 *  1 quantity changed · not in TeamCraft: Lager.ipt · no longer in the CAD
 *  file: PRT-000012 (kept)". Empty when no line was added, changed or
 *  refused (nothing changed: no line), and for no `bom` at all. */
export function checkinBomText(bom) {
  if (!bom) return "";
  var added = bom.added || [], updated = bom.updated || [], refused = bom.refused || [];
  if (!added.length && !updated.length && !refused.length) return "";
  var changes = [];
  if (added.length) changes.push(plural(added.length, "line", "lines") + " added");
  if (updated.length) changes.push(plural(updated.length, "quantity", "quantities") + " changed");
  var parts = [changes.length ? "BOM: " + changes.join(", ") : "BOM unchanged"];
  if (refused.length) parts.push("refused (circular): " + refused.map(function (r) { return r.child; }).join(", "));
  if ((bom.not_in_teamcraft || []).length) parts.push("not in TeamCraft: " + bom.not_in_teamcraft.join(", "));
  if ((bom.kept_not_in_cad || []).length) parts.push("no longer in the CAD file: " + bom.kept_not_in_cad.join(", ") + " (kept)");
  return parts.join(" · ");
}
