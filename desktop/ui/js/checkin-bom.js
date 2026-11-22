// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – *Update the BOM from the CAD file* at check-in (#407):
// the pure part (no DOM, no server calls), so it can be checked on its own.
// workspace.js (the *Check in…* dialog) uses it.
//
// The server updates the BOM from the references of the checked-in version
// (new lines added, counted quantities corrected, nothing removed). The box
// is offered only for a revision with an assembly whose references are read:
// the server knows a structure file of it (`bom_structure` of file-status:
// an assembly, or an NX part, STEP file or SolidCraft design whose
// references carry instance counts), the item is an assembly, or a file is
// of an assembly format (Inventor, SOLIDWORKS, Creo, …) or a SolidCraft
// design (the desktop's own CAD program; a part or an assembly by content).
// NX `.prt` and STEP files are left to the server: every Creo part is a
// `.prt` too, and *Check in…* exports a STEP of every part.

/** Extensions whose files may be assemblies by content, whatever their kind
 *  (SolidCraft designs, both extensions of `solidcraft-design`). */
export var BOM_STRUCTURE_EXTENSIONS = ["sc", "solidcraft"];

/** Is *Update the BOM from the CAD file* offered? `status` is the answer of
 *  workspace_status (`bom_structure`, `files`), `rev` the revision (its
 *  `item_type`), `kinds` the extension → format kind map of GET /formats. */
export function offerBomUpdate(status, rev, kinds) {
  if (status && status.bom_structure) return true;
  if (rev && rev.item_type === "assembly") return true;
  var names = ((status && status.files) || []).map(function (f) { return f.name; });
  return bomStructureFile(names, kinds);
}

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
