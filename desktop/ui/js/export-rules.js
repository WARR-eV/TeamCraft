// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – which files the check-in exports to STEP (#342). No
// imports and no DOM: scripts/desktop-export-rules.test.mjs tests it with
// plain Node.js. Used by workspace.js (the check-in dialog).
//
// A STEP export through the CAD program loads the whole file: for a part
// that takes seconds, for a large top-level assembly many minutes and
// gigabytes of memory (an Inventor assembly of 12 MB with about 500 parts
// did not finish in nine minutes). At check-in the app exports parts and
// small assemblies only; a large assembly is skipped with a note – its 3D
// preview is composed from the parts' previews on the server.

/** Assemblies larger than this (bytes) are not exported at check-in. */
export var MAX_ASSEMBLY_EXPORT_BYTES = 5 * 1024 * 1024;

/** Assembly files of the CAD programs the app exports with. */
var ASSEMBLY_EXTENSIONS = { iam: true, asm: true };

/** Whether `name` is an assembly file (Inventor `.iam`, Solid Edge `.asm`). */
export function isAssemblyFile(name) {
  var m = /\.([a-z0-9]+)$/i.exec(name || "");
  return !!m && ASSEMBLY_EXTENSIONS[m[1].toLowerCase()] === true;
}

/** `5 MB` – a size for the note, rounded to whole megabytes. */
function megabytes(bytes) {
  return Math.round(bytes / (1024 * 1024)) + " MB";
}

/**
 * The STEP exports of a check-in. `entries` = `[{name, program, size}]`, the
 * ticked files with the program that would export them (`null` = none) and
 * their size in bytes. Returns `{ export: [name], skipped: [{name, why}] }`:
 * every file with a program, except assemblies above
 * MAX_ASSEMBLY_EXPORT_BYTES, which are skipped with the reason. A missing
 * size counts as small (the export is the user's choice in the dialog).
 */
export function planStepExports(entries) {
  var out = { export: [], skipped: [] };
  (Array.isArray(entries) ? entries : []).forEach(function (e) {
    if (!e || !e.name || !e.program) return;
    if (isAssemblyFile(e.name) && typeof e.size === "number" && e.size > MAX_ASSEMBLY_EXPORT_BYTES) {
      out.skipped.push({ name: e.name, why: "assembly of " + megabytes(e.size) + " – larger than " + megabytes(MAX_ASSEMBLY_EXPORT_BYTES) + ", its preview comes from the parts" });
      return;
    }
    out.export.push(e.name);
  });
  return out;
}
