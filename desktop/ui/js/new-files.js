// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – new items from files on this computer (#359): the pure
// part (no DOM, no server calls), so it can be checked on its own.
// dialogs.js (*New*) and navigator.js (the worklist section *New on this
// computer*) use it.
//
// A file created in the CAD program inside the working folder already has
// its place: `<working folder>/<group>/<folder>/<file>`. *New* takes group and
// folder from there instead of asking, so the item lands on the server where
// the assemblies of everybody else expect it. Before anything is created the
// duplicates are refused: a file that belongs to an item already, a file
// another item has at the same place, the same file name elsewhere in the
// group (CAD programs find parts by file name) – the last one can be
// overridden, a file with the same content is only a warning.

/** Format kinds (from GET /formats) that make a file the model of an item. */
export var MODEL_KINDS = ["part", "assembly", "model"];
/** Kinds that join the model of the same name as one item (drawing, STEP …). */
export var COMPANION_KINDS = ["drawing", "exchange", "mesh", "visualization", "electronics"];

/** `Bracket.ipt` → {stem: "Bracket", ext: "ipt"}. */
export function nameParts(name) {
  var dot = (name || "").lastIndexOf(".");
  return dot > 0 ? { stem: name.slice(0, dot), ext: name.slice(dot + 1).toLowerCase() } : { stem: name || "", ext: "" };
}

/** The format kind of a file name, from the extension → kind map. */
export function kindOf(name, kinds) {
  return (kinds && kinds[nameParts(name).ext]) || "";
}

/** `Design/Struktur` → shown as is; the top of a group gets a name. */
export function folderText(folder) {
  return folder ? folder : "the top of the group";
}

/**
 * Where a new item goes, from the places of its picked files (the answer of
 * the command `new_item_places`). Files in the working folder decide group
 * and folder themselves (`inside`): the item's folder is the folder of its
 * first model file, every file keeps its own folder, files picked from
 * elsewhere are copied into the item's folder. Without a file in the working
 * folder the dialog's group and folder decide (`inside` false).
 *
 * Returns {inside, project, folder, targets: [folder per place], main, problems};
 * `main` is the index of the file that names the item (its first model file).
 */
export function newItemPlacement(places, kinds) {
  var out = { inside: false, project: null, folder: "", targets: [], main: -1, problems: [] };
  places = places || [];
  if (!places.length) return out;
  function isModel(p) { return MODEL_KINDS.indexOf(kindOf(p.name, kinds)) >= 0; }
  var seen = {};
  places.forEach(function (p) {
    var key = (p.name || "").toLowerCase();
    if (seen[key]) out.problems.push("Two of the chosen files are called " + p.name + " – an item has one file of a name.");
    seen[key] = true;
  });
  var inside = places.filter(function (p) { return p.inside && p.project; });
  if (!inside.length) {
    out.targets = places.map(function () { return null; });
    out.main = Math.max(0, places.findIndex(isModel));
    return out;
  }
  var groups = [];
  inside.forEach(function (p) {
    if (!groups.some(function (g) { return g.toLowerCase() === p.project.toLowerCase(); })) groups.push(p.project);
  });
  if (groups.length > 1) {
    out.problems.push("The files are in the folders of different project groups (" + groups.join(", ") +
      ") – an item belongs to one group.");
  }
  var main = inside.filter(isModel)[0] || inside[0];
  out.main = places.indexOf(main);
  out.inside = true;
  out.project = main.project;
  out.folder = main.folder || "";
  out.targets = places.map(function (p) { return p.inside && p.project ? (p.folder || "") : out.folder; });
  return out;
}

function itemText(m) {
  return m.item_number + "/" + m.code;
}

/**
 * What the duplicate checks found. `places` and `targets` as above, `check`
 * the server's answer of POST /projects/{code}/file-check (its `files`, in
 * the same order; null when the server is older and has no such check).
 *
 * Returns {blocking: [text], sameName: [text], warnings: [text]}: blocking
 * findings stop the creation, `sameName` stops it unless the user confirms
 * *Create anyway*, warnings are only shown.
 */
export function newItemFindings(places, targets, check) {
  var out = { blocking: [], sameName: [], warnings: [] };
  (places || []).forEach(function (p, i) {
    var where = folderText(targets && targets[i] != null ? targets[i] : p.folder);
    if (p.tracked) {
      out.blocking.push(p.name + " belongs to " + p.tracked + " already – open that item and check it out instead.");
      return;
    }
    if (p.pending) {
      out.blocking.push(p.name + " was added to " + p.pending + " with Add files… – check it in there.");
      return;
    }
    var c = check && check[i];
    if (!c) return;
    if (c.same_place) {
      out.blocking.push(p.name + " in " + where + " belongs to " + itemText(c.same_place) +
        " already – open that item instead of creating a duplicate.");
      return;
    }
    (c.same_name || []).forEach(function (m) {
      out.sameName.push(p.name + " exists in " + folderText(m.folder) + " (" + itemText(m) +
        ") – CAD programs find parts by file name, a second file of that name can break assemblies.");
    });
    (c.same_content || []).forEach(function (m) {
      out.warnings.push(p.name + " has the same content as " + m.filename + " in " + folderText(m.folder) +
        " (" + itemText(m) + ") – a copy?");
    });
  });
  return out;
}

/** The text an idempotency key of a new item is made of: one key per file
 *  content at its place, so repeating *Create* after a lost answer or a
 *  failed upload finds the item made the first time instead of a second. */
export function newItemKeyText(project, folder, name, sha256) {
  return ["new-item", project || "", folder || "", (name || "").toLowerCase(), sha256 || ""].join("\u0000");
}

/** One line before *Create*: what will happen. */
export function newItemSummary(project, folder, files, copies) {
  var n = files || 0;
  var text = "Creates a new item in " + (project || "?") + (folder ? " / " + folder : "") +
    (n ? ", uploads " + n + (n === 1 ? " file" : " files") : ", without files") +
    (copies ? " (" + copies + (copies === 1 ? " copy" : " copies") + " into the working folder first)" : "") +
    " – it stays checked out to you.";
  return text;
}

// ------------------------------------------------ the worklist suggestions

/**
 * The worklist suggestions from the scan of the working folder (command
 * `new_files_scan`): one suggestion per model file, with the drawings and
 * exports of the same name in the same folder (`Bracket.ipt` +
 * `Bracket.idw` + `Bracket.step` → one item), the files that belong to no
 * model under `other`. Only groups the user may create items in
 * (`editable(code)`). Assemblies referenced by the user's check-outs are not
 * known here; the order is: newest first.
 *
 * Returns {suggestions: [{key, project, folder, name, type, files, modified_ms}], other: [file]}.
 */
export function planNewFiles(files, kinds, editable) {
  var groups = {}, order = [], loose = [];
  (files || []).forEach(function (f) {
    if (editable && !editable(f.project)) return;
    var kind = kindOf(f.name, kinds);
    var stem = nameParts(f.name).stem;
    var key = [f.project, f.folder || "", stem].join("\u0000").toLowerCase();
    if (MODEL_KINDS.indexOf(kind) >= 0 || COMPANION_KINDS.indexOf(kind) >= 0) {
      if (!groups[key]) { groups[key] = { key: key, project: f.project, folder: f.folder || "", name: stem, files: [], kinds: [], modified_ms: 0 }; order.push(key); }
      groups[key].files.push(f);
      groups[key].kinds.push(kind);
      groups[key].modified_ms = Math.max(groups[key].modified_ms, f.modified_ms || 0);
    } else {
      loose.push(f);
    }
  });
  var suggestions = [], other = loose.slice();
  order.forEach(function (key) {
    var g = groups[key];
    var models = g.kinds.filter(function (k) { return MODEL_KINDS.indexOf(k) >= 0; });
    if (!models.length) { other = other.concat(g.files); return; }
    // the model file first: it names the item and decides its type
    g.files.sort(function (a, b) {
      var am = MODEL_KINDS.indexOf(kindOf(a.name, kinds)) >= 0 ? 0 : 1;
      var bm = MODEL_KINDS.indexOf(kindOf(b.name, kinds)) >= 0 ? 0 : 1;
      return am - bm || a.name.localeCompare(b.name);
    });
    suggestions.push({
      key: g.key, project: g.project, folder: g.folder, name: g.name,
      type: models.indexOf("assembly") >= 0 ? "assembly" : "part",
      files: g.files, modified_ms: g.modified_ms
    });
  });
  suggestions.sort(function (a, b) { return (b.modified_ms - a.modified_ms) || a.key.localeCompare(b.key); });
  other.sort(function (a, b) { return (a.rel || "").localeCompare(b.rel || ""); });
  return { suggestions: suggestions, other: other };
}

/** The summary of a batch *Create items…* before anything is created:
 *  how many items go into which folders, and which are refused (with why). */
export function batchSummary(planned, refused) {
  var byFolder = {};
  (planned || []).forEach(function (s) {
    var f = s.project + (s.folder ? " / " + s.folder : "");
    byFolder[f] = (byFolder[f] || 0) + 1;
  });
  var parts = Object.keys(byFolder).sort().map(function (f) { return f + " (" + byFolder[f] + ")"; });
  var n = (planned || []).length;
  var text = "Creates " + n + (n === 1 ? " item" : " items") + (parts.length ? " in " + parts.join(", ") : "") + ".";
  if (refused && refused.length) text += " " + refused.length + " refused: " + refused.join("; ");
  return text;
}

/** How old a file is, for the worklist row (`3 min`, `2 h`, `5 d`). */
export function ageText(modifiedMs, nowMs) {
  if (!modifiedMs) return "";
  var s = Math.max(0, Math.round(((nowMs || Date.now()) - modifiedMs) / 1000));
  if (s < 60) return "just now";
  if (s < 3600) return Math.floor(s / 60) + " min";
  if (s < 86400) return Math.floor(s / 3600) + " h";
  return Math.floor(s / 86400) + " d";
}

// ------------------------------------------------ check-in of another item

/** In *Check in…* of a revision: is the untracked file `f` a new part or
 *  assembly of its own rather than a file of this revision? A revision that
 *  has a model already gets a second model file only by mistake – e.g. a new
 *  `Bracket.iam` next to the checked-out `Bracket.ipt` (the same name stem
 *  makes it a candidate). Such a file is offered unticked, with *New item
 *  from this file*. Drawings and exports of the same name stay ticked. */
export function isNewModelOfItsOwn(f, files, kinds) {
  if (!f || f.state !== "new" || MODEL_KINDS.indexOf(kindOf(f.name, kinds)) < 0) return false;
  return (files || []).some(function (o) {
    return o !== f && o.state !== "new" && MODEL_KINDS.indexOf(kindOf(o.name, kinds)) >= 0;
  });
}

/** The absolute path of a working-folder file from the folder and its
 *  `/`-separated relative path, with the separator the folder uses. */
export function workspacePath(root, rel) {
  var base = String(root || "");
  var sep = base.indexOf("\\") >= 0 ? "\\" : "/";
  while (base.length > 1 && (base.charAt(base.length - 1) === "/" || base.charAt(base.length - 1) === "\\")) base = base.slice(0, -1);
  return base + sep + String(rel || "").split("/").filter(Boolean).join(sep);
}
