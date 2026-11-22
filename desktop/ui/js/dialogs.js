// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – Dialogs: new item, settings, about.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { $, ITEM_TYPES, TAURI, api, call, dialog, enc, errorText, field, h, materialField, setStatus, state, toast } from "./core.js";
import { label } from "/static/shared/js/format.js";
import { canEdit, canEditProject, freshProjects, refreshWorklistCount, rescanNewFiles } from "./shell.js";
import { activeProject, selectView } from "./navigator.js";
import { newItemFolder } from "./files-view.js";
import { describeFromDesign, formatKinds } from "./folder-import.js";
import { newItemFindings, newItemKeyText, newItemPlacement, newItemSummary } from "./new-files.js";
import { openRevision } from "./revision.js";
import { revisionInputs } from "./report-inputs.js";

// ======================================================================
// Dialogs: new item, settings, about
// ======================================================================

/** `C:\cad\Bracket.ipt` → `Bracket.ipt`. */
export function baseName(path) {
  return String(path).split(/[\\/]/).pop();
}

export var ASSEMBLY_EXT = ["iam", "asm", "sldasm", "catproduct", "f3z"];
export var DRAWING_EXT = ["idw", "dwg", "dxf", "dft", "slddrw", "catdrawing", "drw", "ipn", "pdf"];

/** Name and item type suggested by a CAD file (Creo iterations ignored). */
export function guessFromFile(path) {
  var base = baseName(path).replace(/\.\d+$/, "");
  var dot = base.lastIndexOf(".");
  var ext = dot > 0 ? base.slice(dot + 1).toLowerCase() : "";
  return {
    name: dot > 0 ? base.slice(0, dot) : base,
    type: ASSEMBLY_EXT.indexOf(ext) >= 0 ? "assembly" : DRAWING_EXT.indexOf(ext) >= 0 ? "document" : "part"
  };
}

/** *New item*; `opts.project` (code) and `opts.folder` preselect the group and
 *  the folder (*New item here* in the Files view) – without them the active
 *  group and the folder last opened in it. */
export function newItemDialog(opts) { freshProjects().then(function () { openNewItemDialog(opts); }); }

/** SHA-256 of a text as hex (idempotency keys); null without WebCrypto. */
function sha256Hex(text) {
  if (!window.crypto || !crypto.subtle) return Promise.resolve(null);
  return crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)).then(function (buf) {
    return Array.prototype.map.call(new Uint8Array(buf), function (b) { return ("0" + b.toString(16)).slice(-2); }).join("");
  });
}

/** The project group (of the ones the user may create items in) whose
 *  folder in the working folder is `folderName` – the folder carries the
 *  group code (layout.rs `local_rel`). */
export function editableGroupOfFolder(folderName) {
  var wanted = (folderName || "").toLowerCase();
  return state.projects.filter(function (p) {
    return p.active && canEditProject(p.id) && p.code.toLowerCase() === wanted;
  })[0] || null;
}

/** The server's duplicate check of the files of a new item (#359): one
 *  answer per file, in order; null from an older server without the check. */
export function checkNewItemFiles(project, places, targets) {
  if (!project || !places.length) return Promise.resolve(null);
  var files = places.map(function (p, i) {
    return { folder: targets[i] || "", filename: p.name, sha256: p.sha256 || null };
  });
  return api("POST", "/projects/" + enc(project) + "/file-check", { files: files }).then(function (r) {
    return r && Array.isArray(r.files) ? r.files : null;
  }, function (err) {
    if (err && (err.status === 404 || err.status === 405)) return null;
    throw err;
  });
}

/**
 * Creates an item from files on this computer – the one way *New* and the
 * worklist's *Create items…* take (#359). Files picked from outside the
 * working folder are copied into the item's folder first (never moved); the
 * item is created with an idempotency key from its main file, so repeating
 * after a lost answer or a failed upload finds the item made the first time;
 * every file is uploaded into its folder with the server's duplicate guard
 * (`refuse_clash`, and `refuse_same_name` unless `allowSameName`), and the
 * files at their place are recorded in the working folder, so no other
 * check-out and no worklist suggestion offers them again.
 *
 * `spec`: {body (POST /items fields), project, folder, places, targets, main,
 * allowSameName}. Resolves with the new revision; an upload failure rejects
 * with `err.createdRevision` set. An empty description is filled from a
 * SolidCraft design among the files (#367).
 */
export function createItemFromFiles(spec) {
  var places = spec.places || [];
  var paths = places.map(function (p) { return p.path; });
  var outside = [];
  places.forEach(function (p, i) { if (!p.inside) outside.push(i); });
  var copied = outside.length
    ? call("new_item_copy", { project: spec.project, folder: spec.folder, paths: outside.map(function (i) { return places[i].path; }) })
      .then(function (copies) { copies.forEach(function (c, j) { paths[outside[j]] = c; }); })
    : Promise.resolve();
  var main = places[spec.main >= 0 ? spec.main : 0];
  return copied.then(function () {
    return main ? sha256Hex(newItemKeyText(spec.project, spec.folder, main.name, main.sha256)) : null;
  }).then(function (hex) {
    var body = Object.assign({}, spec.body, { project: spec.project, folder: spec.folder });
    if (hex) body.idempotency_key = "new-item:" + hex;
    return api("POST", "/items", body);
  }).then(function (rev) {
    call("usage_count", { action: "new_item" }).catch(function () {});
    if (!places.length) return rev;
    var folders = {};
    places.forEach(function (p, i) { folders[p.name] = spec.targets[i] != null ? spec.targets[i] : spec.folder; });
    return call("upload_files", {
      item: rev.item_number, code: rev.code, paths: paths, comment: null, folders: folders,
      project: spec.project, track: true, refuseClash: true, refuseSameName: !spec.allowSameName
    }).then(function (records) {
      // an empty description from a SolidCraft design's properties (#367) – best effort
      return describeFromDesign(rev, records).then(function () { return rev; });
    }, function (err) {
      var e = new Error(errorText(err));
      e.createdRevision = rev;
      throw e;
    });
  });
}

export function openNewItemDialog(opts) {
  opts = opts || {};
  if (!canEdit()) { toast("Viewers cannot create items.", "error"); return; }
  var projects = state.projects.filter(function (p) { return p.active && canEditProject(p.id); });
  if (!projects.length) { toast("You are not a member of any project group – ask an admin to add you.", "error"); return; }
  var type = h("select", null, ITEM_TYPES.map(function (t) { return h("option", { value: t.value, text: t.label }); }));
  var project = h("select", null, projects.map(function (p) { return h("option", { value: p.code, text: p.code + " – " + p.name }); }));
  // the group one works in (Home), when one may create items in it
  var active = activeProject();
  if (active && projects.indexOf(active) !== -1) project.value = active.code;
  if (opts.project && projects.some(function (p) { return p.code === opts.project; })) project.value = opts.project;
  // the folder inside the group: the item's place in the Files view, its files go there
  var folder = h("input.mono", { type: "text", maxlength: "400", spellcheck: "false", placeholder: "(top of the group)",
    value: opts.folder != null && project.value === opts.project ? opts.folder : newItemFolder(state.filesFolder, project.value) });
  var folderTyped = false;
  folder.addEventListener("input", function () { folderTyped = true; scheduleCheck(); });
  project.addEventListener("change", function () {
    if (!folderTyped) folder.value = newItemFolder(state.filesFolder, project.value);
    scheduleCheck();
  });
  var name = h("input", { type: "text", maxlength: "200", required: true, placeholder: "from the first file" });
  var number = h("input", { type: "text", maxlength: "64", placeholder: "automatic (PRT-000042 …)" });
  var materialF = materialField(""), material = materialF.input;
  var mass = h("input", { type: "text", inputmode: "decimal" });
  var desc = h("textarea", { maxlength: "10000" });
  // The item's files: chosen first, name and type follow from the first one;
  // a file in the working folder brings its group and folder (#359).
  var paths = [];
  var places = [], placement = newItemPlacement([], {}), findings = null, inspection = 0, checkTimer = null, busy = false;
  var createdIn = null;
  var kinds = {};
  var typeChosen = false;
  type.addEventListener("change", function () { typeChosen = true; });
  var fileList = h("ul.file-picks");
  var info = h("div.stack.new-item-info", { role: "status" });
  var anyway = h("input", { type: "checkbox" });
  var anywayRow = h("label.check.hidden", null, [anyway, "Create anyway – the file name is meant to exist twice in the group"]);
  function showFiles() {
    fileList.textContent = "";
    paths.forEach(function (p, i) {
      fileList.appendChild(h("li", null, [h("span.mono", { text: baseName(p) }),
        h("button.small", { type: "button", title: "Remove", on: { click: function () { paths.splice(i, 1); showFiles(); inspect(); } } }, "×")]));
    });
  }
  function addPaths(chosen) {
    (Array.isArray(chosen) ? chosen : [chosen]).forEach(function (p) { if (paths.indexOf(p) < 0) paths.push(p); });
    showFiles();
    var guess = guessFromFile(paths[0]);
    if (!name.value.trim()) name.value = guess.name;
    if (!typeChosen) type.value = guess.type;
    inspect();
  }
  var pick = TAURI.dialog ? h("button", { type: "button", on: { click: function () {
    TAURI.dialog.open({ multiple: true, title: "Files of the new item" }).then(function (chosen) {
      if (chosen) addPaths(chosen);
    });
  } } }, "Choose files…") : null;
  // where the item goes: from the files when they are in the working folder
  function group() { return placement.inside ? (editableGroupOfFolder(placement.project) || {}).code : project.value; }
  function itemFolder() { return placement.inside ? placement.folder : folder.value.trim(); }
  function targets() {
    return placement.inside ? placement.targets : places.map(function () { return folder.value.trim(); });
  }
  function lockPlace(locked) {
    project.disabled = locked;
    folder.readOnly = locked;
  }
  function line(cls, text) { return h("p" + cls, { text: text }); }
  function showInfo(note) {
    info.textContent = "";
    if (note) { info.appendChild(line(".small.muted", note)); return; }
    if (!paths.length) { anywayRow.classList.add("hidden"); return; }
    if (placement.inside) {
      info.appendChild(line(".small", "Group and folder come from where the files are in the working folder: " +
        placement.project + (placement.folder ? " / " + placement.folder : "") +
        ". To put the item elsewhere, save its files there in the CAD program."));
    }
    var stops = placement.problems.concat(findings ? findings.blocking : []);
    stops.forEach(function (t) { info.appendChild(line(".notice.error", t)); });
    var same = findings ? findings.sameName : [];
    same.forEach(function (t) { info.appendChild(line(".notice.warn", t)); });
    anywayRow.classList.toggle("hidden", !same.length || stops.length > 0);
    (findings ? findings.warnings : []).forEach(function (t) { info.appendChild(line(".small.muted", t)); });
    if (!stops.length) {
      var copies = places.filter(function (p) { return !p.inside; }).length;
      info.appendChild(line(".small", newItemSummary(group(), itemFolder(), paths.length, copies)));
    }
  }
  /** Where the picked files are and what the server knows about them; the
   *  newest call wins (files are picked and removed while it runs). */
  function inspect() {
    var token = ++inspection;
    if (!paths.length) {
      places = []; placement = newItemPlacement([], kinds); findings = null;
      lockPlace(false); showInfo();
      return Promise.resolve(true);
    }
    showInfo("Checking the files …");
    return call("new_item_places", { paths: paths }).then(function (ps) {
      if (token !== inspection) return false;
      places = ps;
      placement = newItemPlacement(ps, kinds);
      if (placement.inside) {
        var g = editableGroupOfFolder(placement.project);
        if (!g) {
          placement.problems.push("The files are in the folder " + placement.project + " of the working folder, " +
            "but there is no project group " + placement.project + " you may create items in.");
        } else {
          project.value = g.code;
          folder.value = placement.folder;
        }
      }
      lockPlace(placement.inside);
      return runCheck(token);
    }).catch(function (err) {
      if (token !== inspection) return false;
      findings = null;
      showInfo();
      info.appendChild(line(".notice.error", errorText(err)));
      return false;
    });
  }
  function runCheck(token) {
    if (placement.problems.length || !group()) { findings = newItemFindings(places, targets(), null); showInfo(); return Promise.resolve(true); }
    return checkNewItemFiles(group(), places, targets()).then(function (check) {
      if (token !== inspection) return false;
      findings = newItemFindings(places, targets(), check);
      showInfo();
      return true;
    });
  }
  /** Group or folder typed while the files are outside the working folder:
   *  check again where they would go, after a short pause. */
  function scheduleCheck() {
    if (!places.length || placement.inside) return;
    clearTimeout(checkTimer);
    checkTimer = setTimeout(function () {
      var token = ++inspection;
      runCheck(token).catch(function (err) { if (token === inspection) { findings = null; showInfo(); info.appendChild(line(".notice.error", errorText(err))); } });
    }, 400);
  }
  // *Report…* (#381): what the new item was to be, never its description
  function newItemInputs() {
    return [["group", group()], ["type", type.value], ["item folder", itemFolder()], ["number", number.value.trim()],
      ["files", paths.map(baseName).join(", ")]];
  }
  dialog("New item", h("div.stack", null, [
    field("Files", h("div.stack", null, [pick, fileList]), "the CAD model with its drawing and a STEP export for the 3D preview – uploaded right after the item is created; files in the working folder keep their folder"),
    h("div.grid-2", null, [field("Type", type), field("Project group", project)]),
    field("Folder", folder, "inside the group, e.g. chassis/brackets – the item and its files go there; empty: the top"),
    info, anywayRow,
    field("Name", name), field("Number", number, "optional"),
    h("div.grid-2", null, [materialF.el, field("Mass per unit (kg)", mass)]),
    field("Description", desc)
  ]), [
    { label: "Cancel", onClick: function (d) { inspection++; clearTimeout(checkTimer); d.close(); } },
    {
      label: "Create", primary: true, submit: true, onClick: function (d) {
        if (busy) return;
        var m = mass.value.trim().replace(",", ".");
        var massKg = m ? Number(m) : null;
        if (m && !isFinite(massKg)) { d.fail("Mass: '" + mass.value + "' is not a number."); return; }
        busy = true;
        clearTimeout(checkTimer);
        // checked again right before creating: files may have changed meanwhile
        inspect().then(function (current) {
          if (!current) throw new Error("The files changed while they were checked – try again.");
          var stops = placement.problems.concat(findings ? findings.blocking : []);
          if (stops.length) throw new Error(stops[0]);
          if (findings && findings.sameName.length && !anyway.checked) {
            throw new Error("A file of that name exists elsewhere in the group – tick “Create anyway” if that is intended.");
          }
          var code = group();
          if (!code) throw new Error("Choose a project group.");
          createdIn = code;
          var body = {
            item_type: type.value, name: name.value, number: number.value.trim() || null,
            material: material.value, description: desc.value, mass_kg: massKg
          };
          if (paths.length) setStatus("Uploading " + paths.length + (paths.length === 1 ? " file" : " files") + " …");
          return createItemFromFiles({
            body: body, project: code, folder: itemFolder(), places: places, targets: targets(),
            main: placement.main, allowSameName: anyway.checked
          });
        }).then(function (rev) {
          setStatus("");
          d.close();
          toast("Created " + rev.item_number + "/" + rev.code + (paths.length ? " with " + paths.length + (paths.length === 1 ? " file" : " files") : "") + (rev.item_folder ? " in " + rev.item_folder : "") + " – it is checked out to you");
          afterCreate(rev);
        }, function (err) {
          setStatus("");
          busy = false;
          if (err && err.createdRevision) {
            var rev = err.createdRevision;
            d.close();
            toast("Created " + rev.item_number + "/" + rev.code + ", but the files could not be uploaded: " + errorText(err), "error",
              "Uploading the files of the new item", revisionInputs(rev, [["files", paths.map(baseName).join(", ")]]));
            afterCreate(rev);
            return;
          }
          d.fail(err);
        });
      }
    }
  ], { wide: true, inputs: newItemInputs });
  function afterCreate(rev) {
    openRevision(rev.item_number, rev.code);
    if (createdIn) delete state.expanded[createdIn];
    if (state.view === "home") selectView("home");
    refreshWorklistCount();
    rescanNewFiles();
  }
  formatKinds().catch(function () { return {}; }).then(function (k) {
    kinds = k || {};
    if (opts.name) name.value = opts.name;
    if (opts.type) { type.value = opts.type; typeChosen = true; }
    if (opts.paths && opts.paths.length) addPaths(opts.paths);
  });
}
