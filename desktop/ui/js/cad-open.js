// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – "Check out and open": the CAD program that opens a file.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).
//
// The choice is made in Rust (desktop/src/cadlaunch.rs): the project group's
// default CAD program and version, else the newest installed version of the
// program that fits the file, else the operating system's default application.

import { api, call, errorText, h, toast } from "./core.js";
import { projectById } from "./shell.js";

export var cadFormatsPromise = null;
/** From the server's format list: `byExt` = extension → format family (only
 * extensions of one program; `.prt`, `.asm` … are ambiguous and left out),
 * `names` = family → program name ("Autodesk Inventor"). */
export function cadFormats() {
  if (!cadFormatsPromise) {
    cadFormatsPromise = api("GET", "/formats").then(function (r) {
      var byExt = {}, names = {};
      r.systems.forEach(function (s) {
        s.formats.forEach(function (f) {
          if (!f.family || f.ambiguous) return;
          names[f.family] = names[f.family] || s.name;
          f.extensions.forEach(function (e) { if (!byExt[e]) byExt[e] = f.family; });
        });
      });
      return { byExt: byExt, names: names };
    }, function (err) { cadFormatsPromise = null; throw err; });
  }
  return cadFormatsPromise;
}

/** The format family of a file of a revision ("" = neutral: STEP, PDF …). */
export function fileFamily(f) {
  return (f && f.format_info && f.format_info.family) || "";
}

/** The project group's default CAD program of a revision. */
export function groupDefault(rev) {
  var p = rev && projectById(rev.project_id);
  return { family: (p && p.default_cad) || "", version: (p && p.default_cad_version) || "" };
}

/** What the Rust side needs to choose the program (`cadlaunch::Wish`). */
export function cadWish(rev, file) {
  var g = groupDefault(rev);
  return { file_family: fileFamily(file), group_family: g.family, group_version: g.version };
}

var KIND_RANK = { assembly: 0, model: 1, part: 1, drawing: 2 };

/**
 * The file "Check out and open" opens: a native CAD file (one with a format
 * family), of the group's default program when there is one – the assembly
 * before the part before the drawing, then by name. null without one.
 */
export function mainCadFile(files, groupFamily) {
  var natives = (files || []).filter(function (f) {
    return fileFamily(f) && KIND_RANK[f.format_info.kind] !== undefined;
  });
  var own = natives.filter(function (f) { return fileFamily(f) === groupFamily; });
  var pool = own.length ? own : natives;
  pool = pool.slice().sort(function (a, b) {
    return (KIND_RANK[a.format_info.kind] - KIND_RANK[b.format_info.kind]) || a.filename.localeCompare(b.filename);
  });
  return pool[0] || null;
}

/** How a file would be opened (`cadlaunch::OpenPlan`). */
export function openPlan(rev, file) {
  return call("cad_open_plan", { wish: cadWish(rev, file) });
}

/** Opens a file of the working folder with the chosen program (an Inventor
 * that already runs with the working folder's project, #350) and says with which. */
export function openWithCad(rev, file) {
  return call("workspace_open_cad", {
    project: rev.project_code, item: rev.item_number, code: rev.code, file: file.filename, wish: cadWish(rev, file)
  }).then(function (plan) {
    // reused: the running Inventor with the working folder's project opened
    // it (#350); note: why a running one was not used
    toast("Opened " + file.filename + " in " + (plan.program ? plan.label : "the default application") +
      (plan.reused ? " (the instance that was already running)" : "") +
      (plan.fallback ? " – " + plan.reason : "") + (plan.note ? " – " + plan.note : ""), plan.note ? "info" : null);
    return plan;
  });
}

/** The chip next to "Check out and open": the program that will open the
 * file; a click opens the group's setting in the web UI. */
export function cadTargetChip(rev, file) {
  var chip = h("button.cad-target", {
    type: "button", text: "…", title: "Looking for the CAD program on this computer",
    disabled: !file,
    on: {
      click: function () {
        if (!rev.project_id) return;
        call("open_web", { path: "/projects/" + rev.project_id + "#default-cad" }).catch(function (e) { toast(errorText(e), "error"); });
      }
    }
  });
  if (!file) {
    chip.textContent = "No CAD file";
    chip.title = "This revision has no native CAD file (part, assembly or drawing of one CAD program).";
    return chip;
  }
  openPlan(rev, file).then(function (p) {
    chip.textContent = p.label;
    chip.title = file.filename + ": " + p.reason + (rev.project_id ? " Click to see the project group's default CAD program (web UI)." : "");
    chip.classList.toggle("is-fallback", !!p.fallback);
  }, function () {
    chip.textContent = "Default app";
    chip.title = "The program could not be chosen – the operating system opens the file.";
  });
  return chip;
}

/**
 * The CAD program of a scanned folder (Import CAD folder): the format family
 * with the most files, with the newest version the scan read from the files
 * (`scan.versions`, Inventor only). null without native CAD files.
 */
export function detectFolderProgram(scan, formats) {
  var counts = {}, total = 0;
  scan.files.forEach(function (f) {
    var dot = f.name.lastIndexOf(".");
    var fam = dot > 0 ? formats.byExt[f.name.slice(dot + 1).toLowerCase()] : null;
    if (!fam) return;
    counts[fam] = (counts[fam] || 0) + 1;
    total++;
  });
  var best = Object.keys(counts).sort(function (a, b) { return counts[b] - counts[a] || a.localeCompare(b); })[0];
  if (!best) return null;
  var version = (scan.versions || []).filter(function (v) { return v.family === best; })[0];
  return {
    family: best, version: version ? version.version : "", files: counts[best], total: total,
    name: (formats.names[best] || best) + (version ? " " + version.version : "")
  };
}
