// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – 3D viewer, workspace, check-out/in, edit, release, revise.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { $, api, badge, call, clear, confirmDialog, dialog, enc, errorNotice, errorText, field, h, invoke, lockBadge, materialField, reportLink, run, setStatus, state, toast, tool } from "./core.js";
import { fmtDate, fmtSize, label } from "/static/shared/js/format.js";
import { canEdit, canEditProject, isCancelled, refreshWorklistCount, rescanLocalChanges, uploadLine } from "./shell.js";
import { reloadActive, renderActive, table } from "./tabs.js";
import { PROGRAM_LABEL, cadPrograms, dd, exportProgram, exportStep, openRevision, previewFormat, revisionAbilities } from "./revision.js";
import { openChange } from "./change.js";
import { reviewerProblem } from "./release-rules.js";
import { requestChangeDialog } from "./requests.js";
import { refreshStructure } from "./navigator.js";
import { fileFamily, openWithCad } from "./cad-open.js";
import { incompleteGet, notOpenedText } from "./get-rules.js";
import { formatKinds } from "./folder-import.js";
import { isNewModelOfItsOwn, workspacePath } from "./new-files.js";
import { newItemDialog } from "./dialogs.js";
import { offerBomUpdate, checkinBomText } from "./checkin-bom.js";
import { revisionInputs } from "./report-inputs.js";
import { t } from "/static/shared/js/i18n.js";

// ---------------------------------------------------------------- 3D viewer

export function disposeViewer() {
  if (state.viewer) {
    try { state.viewer.dispose(); } catch (e) { /* already gone */ }
    state.viewer = null;
  }
}

export function revViewer(tab, el) {
  var files = tab.data.files.filter(function (f) { return previewFormat(f); });
  if (files.length) call("usage_count", { action: "open_3d_preview" }).catch(function () {});
  if (!files.length) {
    el.appendChild(h("p.empty", { text: "No 3D files (STEP, STL, OBJ, 3MF, glTF, PLY, point clouds, simulation results, …) – native CAD files (SolidWorks, NX, Creo, CATIA, …) are shown in 3D through a STEP export next to them." }));
    return;
  }
  var current = files.filter(function (f) { return f.id === tab.viewerFile; })[0] || files[0];
  tab.viewerFile = current.id;
  var canvas = h("canvas.viewer-canvas", { tabindex: "0", "aria-label": "3D view of " + current.filename });
  var status = h("p.small.muted", { role: "status", text: "Loading" });
  // "Loading …" and every error on the canvas itself: the status line below
  // can be out of view, which left a failed preview a silent black canvas (#290)
  var overlay = h("div.viewer-overlay.is-busy", null, h("span", { text: "Loading the 3D model …" }));
  var resetBtn = h("button.small", { type: "button", title: "Back to the initial view (double-click)" }, "Reset view");
  var upBtn = h("button.small", { type: "button", title: "Switch between Z-up and Y-up models" }, "Z up");
  // STL export of STEP/3MF/… models (e.g. for 3D printing); saved through a native dialog in Rust.
  var noExport = ["stl", "pts", "xyz", "las", "e57"].indexOf(previewFormat(current)) !== -1;
  var stlBtn = noExport ? null : h("button.small", {
    type: "button", disabled: true, title: "Save the tessellated model as STL (units as in the file, usually mm)",
    on: {
      click: function () {
        var name = current.filename.replace(/\.[^.]+$/, "") + ".stl";
        var data;
        try { data = new Uint8Array(state.viewer.exportStl()); } catch (e) { toast(errorText(e), "error", "Exporting " + name, previewInputs); return; }
        invoke("save_stl", data, { headers: { "x-file-name": encodeURIComponent(name) } }).then(function (path) {
          if (path) toast("Saved " + path);
        }, function (e) { toast(errorText(e), "error", "Saving " + name, previewInputs); });
      }
    }
  }, "Export STL…");
  el.appendChild(h("div.viewer-layout", null, [
    h("div.viewer-files", null, [h("h3", { text: "Models" })].concat(files.map(function (f) {
      return h("button" + (f.id === current.id ? ".active" : ""), {
        type: "button", title: f.filename,
        on: { click: function () { tab.viewerFile = f.id; tab.viewerVersion = null; renderActive(); } }
      }, f.filename + "  v" + f.version);
    }))),
    h("div", null, [h("div.viewer-stage", null, [canvas, overlay]), h("div.viewer-bar", null, [status, h("div.row", null, [upBtn, resetBtn, stlBtn])]),
      h("p.hint", { text: "Drag to rotate, right-drag or Shift + drag to pan (other buttons: the mouse setting below the view), wheel to zoom, arrow keys to turn, double-click to reset." })])
  ]));
  // the server converts the file into a mesh (pure Rust) – the app only draws it
  var versionId = tab.viewerVersion;
  // *Report…* of a failed preview (#381): the file, its format and version
  var previewInputs = revisionInputs(tab.data.rev, [["file", current.filename], ["format", previewFormat(current)],
    ["version", versionId ? "older version" : "v" + current.version]]);
  try {
    var viewer = state.viewer = window.TeamCraftViewer.mount(canvas, {
      statusEl: status,
      overlayEl: overlay,
      resetBtn: resetBtn,
      upBtn: upBtn,
      onReady: function () { if (stlBtn) stlBtn.disabled = false; },
      // the error on the canvas (parse, graphics memory, driver) offers
      // *Report…* in place (#471)
      onError: function (text) {
        var link = reportLink("Showing the 3D preview of " + current.filename, text, previewInputs);
        var box = overlay.querySelector("span");
        if (link && box) box.append(" ", link);
      },
      load: function () {
        return invoke("file_bytes", versionId ? { kind: "version-mesh", id: versionId } : { kind: "mesh", id: current.id })
          .catch(function (e) {
            // only while this viewer is still on screen: a late failure of a viewer already
            // replaced or disposed is not about the preview the user sees
            if (state.viewer === viewer) toast(errorText(e), "error", "Loading the 3D preview of " + current.filename, previewInputs);
            throw e;
          });
      }
    });
  } catch (err) {
    // the viewer script itself failed: still no silent canvas
    clear(overlay);
    overlay.className = "viewer-overlay is-error";
    overlay.appendChild(h("span", { text: "The 3D view could not start: " + errorText(err) }));
    status.textContent = errorText(err);
  }
}

// ---------------------------------------------------------------- workspace

export function revWorkspace(tab, el) {
  var rev = tab.data.rev, can = revisionAbilities(tab.data);
  return Promise.all([call("workspace_status", { project: rev.project_code, item: rev.item_number, code: rev.code }), cadPrograms()]).then(function (r) {
    var ws = r[0], programs = r[1];
    var changed = ws.files.filter(function (f) { return f.state === "modified" || f.state === "new"; });
    var outdated = ws.files.filter(function (f) { return f.state === "outdated"; });
    var conflicts = ws.files.filter(function (f) { return f.state === "conflict"; });
    var clashes = ws.files.filter(function (f) { return f.state === "clash"; });
    var lockedByOther = rev.checked_out_by && rev.checked_out_by !== state.me.id;
    // STEP for the 3D preview, written by the CAD program installed here
    function stepButton(f) {
      var program = f.state !== "missing" && f.state !== "clash" && exportProgram(f.name, programs);
      if (!program) return null;
      return h("button.small", {
        type: "button", title: "Create a STEP export with " + PROGRAM_LABEL[program] + " next to the file – check it in with the model for the 3D preview",
        on: { click: function () {
          run("Exporting " + f.name + " to STEP with " + PROGRAM_LABEL[program], function () { return exportStep(rev, f.name); },
            revisionInputs(rev, [["file", f.name], ["program", program]]))
            .then(function (step) { toast("Created " + step + " – check it in together with " + f.name); renderActive(); }, function () {});
        } }
      }, "STEP");
    }
    // SolidCraft designs of my check-out: the item number, revision and name
    // into the design's properties (solidcraft-cli, #369)
    function itemDataButton(f) {
      if (!rev.checked_out_by || lockedByOther || exportProgram(f.name, programs) !== "solidcraft") return null;
      if (f.state === "missing" || f.state === "clash") return null;
      return h("button.small", {
        type: "button", title: "Write " + rev.item_number + ", revision " + rev.code + " and the item name into the design's properties in SolidCraft",
        on: { click: function () {
          run("Writing the item data into " + f.name, function () {
            return call("solidcraft_write_item_data", { project: rev.project_code, item: rev.item_number, code: rev.code, file: f.name,
              data: { item_number: rev.item_number, revision: rev.code, name: rev.name, item_id: String(rev.item_id || ""), revision_id: String(rev.id) } });
          }, revisionInputs(rev, [["file", f.name]])).then(function () { toast(f.name + " now carries " + rev.item_number + "/" + rev.code + " – check it in to keep it"); renderActive(); }, function () {});
        } }
      }, "Item data");
    }
    el.appendChild(h("section.panel", null, [
      h("div.panel-head", null, [
        h("h2", { text: "Working folder" }),
        h("div.row", null, [
          tool("Open folder", "folder", function () { openWorkspaceFolder(tab); }),
          tool("Get files", "download", function () {
            getting(call("workspace_download", { project: rev.project_code, item: rev.item_number, code: rev.code }))
              .then(function (r) { reportDownload(r); renderActive(); }, function (e) { toast(errorText(e), "error", "Getting the files of " + rev.item_number + "/" + rev.code, revisionInputs(rev)); });
          }, { title: "Copy the current files of this revision into the working folder without checking out (read-only)" }),
          tool("Get with structure…", "download", function () { getStructureDialog(rev); },
            { title: "Copy this item and everything it consists of (BOM and referenced CAD files) into the working folder" }),
          can.checkin && changed.length ? tool("Park…", "upload", function () { checkinDialog(tab, true); },
            { title: "Upload your changes as new versions and keep the check-out – safe on the server if this computer is lost" }) : null,
          can.checkin ? tool("Check in…", "checkin", function () { checkinDialog(tab); }, { primary: changed.length > 0 }) : null
        ])
      ]),
      h("p.small.muted", null, ["Folder: ", h("span.mono", { text: ws.dir }),
        " · files you have not checked out are read-only; check out to change them."]),
      ws.released ? releasedNotice(ws.released, changed.length + conflicts.length) : null,
      rev.checked_out_by ? h("p.small", null, [lockBadge(lockedByOther ? rev.checked_out_by_name : "You", !lockedByOther),
        " checked out " + fmtDate(rev.checked_out_at) + " (" + lockAge(rev.checked_out_at) + ")" +
        (lockedByOther ? " – locked by " + rev.checked_out_by_name + ": only they can check in right now; your local changes stay in the working folder." : "")]) : null,
      !ws.exists ? h("p.empty", { text: "Not in your working folder yet – check out (or “Get files”) to copy the files." }) :
        ws.files.length ? table(["File", "Local state", "Server version", "Size", "On disk", ""], ws.files.map(function (f) {
          var version = f.server_version ? "v" + f.server_version : "—";
          if (f.base_version && f.server_version && f.base_version !== f.server_version) version += " (yours: v" + f.base_version + ")";
          if (f.base_version && !f.server_version && f.state !== "new") version = "removed (yours: v" + f.base_version + ")";
          var stateBadge = badge(f.state, SYNC_LABEL[f.state] || label(f.state));
          if (f.state === "clash") stateBadge.title = "Another file is at this place: " + f.clash_with + " – it is never overwritten.";
          if (lockedByOther && f.state !== "missing") stateBadge = h("span", null, [stateBadge, " ", badge("lock", "Locked by " + rev.checked_out_by_name)]);
          if (f.open_in) {
            var openBadge = badge("pending", "Open in " + f.open_in.program);
            openBadge.title = openInText(f) + " – save there before checking in.";
            stateBadge = h("span", null, [stateBadge, " ", openBadge]);
          }
          return {
            cls: f.state === "conflict" ? "row-conflict" : f.state === "clash" ? "row-clash" : null,
            cells: [h("span", null, [
              f.name,
              f.local_name ? h("span.dim.small", { text: " ← " + f.local_name, title: "Newest Creo iteration in the working folder; it is checked in as " + f.name }) : null,
              h("span.ws-path.mono", { text: f.path, title: "Place in the working folder" })
            ]),
              stateBadge, version, f.state === "missing" ? "—" : fmtSize(f.size),
              f.state === "missing" ? "—" : h("span.ro-flag", { text: f.read_only ? "read-only" : "writable" }),
              h("span.row", null, [
                f.state !== "missing" && f.state !== "clash" ? h("button.small", { type: "button", on: { click: function () { openLocalFile(tab, f); } } }, "Open") : null,
                f.state !== "missing" && f.state !== "clash" ? stepButton(f) : null,
                itemDataButton(f),
                h("button.small", { type: "button", title: "Request a change to " + f.name + " (change request)", on: { click: function () { requestChangeDialog(rev, f.name); } } }, "Request…")
              ])]
          };
        }), { numeric: [3] }) : h("p.empty", { text: t(workspaceEmptyText(can)) }),
      clashes.length ? h("p.notice.error", { text: clashes.length + " file(s) cannot be placed because another item's file with the same folder and name is there: " +
        clashes.map(function (f) { return f.path + " (" + f.clash_with + ")"; }).join(", ") + ". Nothing was overwritten – give one of them another folder on the revision page." }) : null,
      conflicts.length ? h("p.notice.error", { text: conflicts.length + " file(s) changed here and on the server since you got them: " +
        conflicts.map(function (f) { return f.name; }).join(", ") + ". A check-in would overwrite the newer version, so the server refuses it – rename your copy, get the current files and merge your changes." }) : null,
      outdated.length ? h("p.notice", { text: outdated.length + " file(s) have a newer version on the server: " +
        outdated.map(function (f) { return f.name; }).join(", ") + " – “Get files” replaces them (your unchanged copies)." }) : null,
      changed.length && !can.checkin ? h("p.notice", { text: changed.length + " local change(s), but you have not checked out this revision – " +
        (lockedByOther ? rev.checked_out_by_name + " holds the lock; check in after they released it." : "check out first, then check in.") }) : null
    ]));
  });
}

/** The line of an empty working folder of a revision, by what the user may
 *  do (#423): the holder of the check-out saves and checks in, who may check
 *  out is told to, everyone else only that it has no files yet. */
function workspaceEmptyText(can) {
  return can.checkin ? "empty.workspace_folder.mine" : can.checkout ? "empty.workspace_folder" : "empty.workspace_folder.none";
}

/** After somebody else released my check-out (issue #173, step 10): who,
 * when, why – and that the local changes stay but need a new check-out. */
export function releasedNotice(released, localChanges) {
  var text = localChanges ? released.message : "Your check-out was released by " + released.by_name + ".";
  return h("p.notice" + (localChanges ? ".error" : ""), { role: "status" }, [
    h("strong", { text: text }),
    " " + fmtDate(released.at) + (released.reason ? " – reason: " + released.reason.replace(/[.!?]?$/, ".") : ".") +
      (localChanges ? " Check the revision out again, then check in or park your changes." : "")
  ]);
}

/** "Get": an item with everything it consists of – BOM under a revision
 * rule and the CAD files it references – into the working folder. */
export function getStructureDialog(rev) {
  var rule = h("select", null, [
    h("option", { value: "latest", text: "Latest revision of every part, also in work" }),
    h("option", { value: "released", text: "Latest released revision – parts without a release are listed" })
  ]);
  if (rev.state === "released") rule.value = "released";
  dialog("Get " + rev.item_number + "/" + rev.code + " with its structure", h("div.stack", null, [
    h("p", { text: "Copies the files of this revision, of every item in its BOM and of the parts its CAD files reference into your working folder, " +
      "each in its folder of the project group – so the assembly opens in the CAD program. Only missing and outdated files are downloaded; " +
      "files you changed are kept, and nothing is overwritten that belongs to another item." }),
    field("Revision rule", rule),
    h("p.small.muted", { text: "Files you have not checked out are read-only. Cancel stops after the current part of a file – the next Get continues." })
  ]), [
    { label: "Cancel", onClick: function (d) { d.close(); } },
    { label: "Get files", primary: true, submit: true, onClick: function (d) {
      d.close();
      getting(call("workspace_get", { item: rev.item_number, code: rev.code, rule: rule.value }))
        .then(function (r) { reportDownload(r); renderActive(); }, function (e) { toast(errorText(e), "error"); });
    } }
  ]);
}

/** Shows the progress of a Get in the status bar until it is done. */
export function getting(promise) {
  var box = $("status-get");
  box.classList.remove("hidden");
  $("status-get-text").textContent = "Getting files …";
  $("status-get-bar").value = 0;
  function hide() { box.classList.add("hidden"); }
  // a Get changes the working folder: *Local changes* looks again (#416)
  return promise.then(function (v) { hide(); rescanLocalChanges(); return v; }, function (e) { hide(); rescanLocalChanges(); throw e; });
}

export function onGetProgress(p) {
  var bar = $("status-get-bar");
  bar.max = Math.max(p.bytes_total || 0, 1);
  bar.value = p.bytes_done || 0;
  $("status-get-text").textContent = p.total
    ? "Getting " + Math.min(p.done + (p.current ? 1 : 0), p.total) + " of " + p.total + " files · " +
      fmtSize(p.bytes_done) + " of " + fmtSize(p.bytes_total) + (p.current ? " – " + p.current : "")
    : "Comparing with the working folder …";
}

export function reportDownload(r) {
  var parts = [];
  if (r.cancelled) parts.push("Stopped");
  if (r.downloaded.length) parts.push(r.downloaded.length + " file(s) copied");
  if (r.up_to_date) parts.push(r.up_to_date + " up to date");
  if (r.kept.length) parts.push(r.kept.length + " locally modified file(s) kept");
  if (r.clashes.length) parts.push(r.clashes.length + " name clash(es)");
  if (r.failed.length) parts.push(r.failed.length + " failed");
  if (r.restricted_lines) parts.push(r.restricted_lines + " restricted line(s) skipped – items of project groups you are not a member of");
  var problems = r.clashes.length || r.failed.length || r.unresolved.length || r.missing_references.length;
  if (!problems) {
    toast((parts.length ? parts.join(" · ") : "The working folder is up to date.") + (r.kept.length ? ": " + r.kept.join(", ") : ""), r.kept.length ? "info" : null);
    return;
  }
  function list(title, items) {
    if (!items.length) return null;
    return h("div", null, [h("h3", { text: title }), h("ul.small", null, items.slice(0, 100).map(function (t) { return h("li", { text: t }); })),
      items.length > 100 ? h("p.small.dim", { text: "… and " + (items.length - 100) + " more" }) : null]);
  }
  dialog(r.cancelled ? "Get stopped" : "Get finished with notes", h("div.stack", null, [
    h("p", { text: parts.join(" · ") + "." }),
    h("p.small.muted", null, ["Working folder: ", h("span.mono", { text: r.dir })]),
    list("Name clashes – nothing was overwritten", r.clashes.map(function (c) { return c.path + ": " + c.file + " – the place is taken by " + c.with; })),
    list("Kept (changed locally)", r.kept),
    list("Failed", r.failed),
    list("No revision for the rule", r.unresolved),
    list("Referenced in CAD files, not in TeamCraft", r.missing_references)
  ]), [{ label: "Close", primary: true, submit: true, onClick: function (d) { d.close(); } }], { wide: true });
}

export function openWorkspaceFolder(tab) {
  var rev = tab && tab.data && tab.data.rev;
  call("workspace_open", rev ? { project: rev.project_code, item: rev.item_number, code: rev.code, file: null } : { project: null, item: null, code: null, file: null })
    .catch(function (e) { toast(errorText(e), "error"); });
}

/** Labels of the sync states of working-folder files. */
export var SYNC_LABEL = {
  up_to_date: "Up to date", unchanged: "Up to date", modified: "Modified", outdated: "Outdated", conflict: "Conflict",
  "new": "New", missing: "Not in working folder", clash: "Name clash",
  // the server answers "not found": a restricted project group the user left
  inaccessible: "No longer accessible – local copy only"
};

/** Offers to move a workspace of the old layout <group>/<item>/<revision>/
 * into the working-folder layout (asked after the sign-in). */
export function checkWorkspaceLayout() {
  call("workspace_migration_plan").then(function (plan) {
    if (plan.moves || plan.identical) migrationDialog(plan);
  }, function () { /* offline or no permission: asked again next time */ });
}

export function migrationDialog(plan) {
  function describe(s) {
    if (s.action === "move") return s.from + " → " + s.to + (s.modified ? " (changed locally, moved along)" : "");
    if (s.action === "already_there") return s.from + " – the same file is already at " + s.to + " (old copy removed)";
    return s.from + " – stays: " + s.reason;
  }
  dialog("Move your workspace to the working-folder layout", h("div.stack", null, [
    h("p", { text: "Your workspace still has folders of the old layout <group>/<item>/<revision>/. TeamCraft now keeps one working folder per project group " +
      "that mirrors the folder structure of the files (<group>/<folder>/<file>), so CAD programs find the parts of an assembly." }),
    h("p", { text: plan.moves + " file(s) move, " + plan.identical + " identical cop" + (plan.identical === 1 ? "y is" : "ies are") + " removed, " +
      plan.kept + " stay where they are. Nothing is overwritten – files whose place is taken stay in the old folder and are listed." }),
    h("ul.small", null, plan.steps.slice(0, 200).map(function (s) { return h("li", { text: describe(s) }); })),
    plan.steps.length > 200 ? h("p.small.dim", { text: "… and " + (plan.steps.length - 200) + " more" }) : null,
    h("p.small.muted", null, ["Every step is written to ", h("span.mono", { text: plan.log })])
  ]), [
    { label: "Later", onClick: function (d) { d.close(); } },
    { label: "Move files", primary: true, submit: true, onClick: function (d) {
      d.close();
      run("Moving the workspace", function () { return call("workspace_migrate"); }).then(function (r) {
        var kept = r.steps.filter(function (s) { return s.action === "keep"; });
        dialog("Workspace moved", h("div.stack", null, [
          h("p", { text: r.moves + " file(s) moved, " + r.identical + " identical copies removed, " + r.kept + " kept in the old folders." }),
          kept.length ? h("ul.small", null, kept.slice(0, 200).map(function (s) { return h("li", { text: describe(s) }); })) : null,
          h("p.small.muted", null, ["Log: ", h("span.mono", { text: r.log })])
        ]), [{ label: "Close", primary: true, submit: true, onClick: function (dd) { dd.close(); renderActive(); } }], { wide: true });
      }, function () {});
    } }
  ], { wide: true });
}


/** How long ago a lock was taken: "3 days", "5 h", "12 min", "just now". */
export function lockAge(iso) {
  var s = Math.max(0, Math.floor((Date.now() - Date.parse(iso)) / 1000));
  if (!isFinite(s)) return "—";
  if (s < 60) return "just now";
  if (s < 3600) return Math.floor(s / 60) + " min";
  if (s < 86400) return Math.floor(s / 3600) + " h";
  if (s < 172800) return "1 day";
  return Math.floor(s / 86400) + " days";
}

/** Warning before opening a file someone else has checked out, or null. */
export function foreignLockText(rev) {
  if (!rev.checked_out_by || rev.checked_out_by === state.me.id) return null;
  return rev.checked_out_by_name + " has checked out " + rev.item_number + "/" + rev.code + " (for " + lockAge(rev.checked_out_at) +
    ") and may be changing it – you can look at the files, but changes you make cannot be checked in until they release it.";
}

/** Opens a file of the workspace tab – with a warning if it is outdated,
 * in conflict or checked out by somebody else. */
export function openLocalFile(tab, f) {
  var rev = tab.data.rev;
  var lockText = foreignLockText(rev);
  function open(got) { openFile(tab, f.name, true, true, got); }
  if (f.state === "outdated") {
    dialog("Newer version on the server", h("div.stack", null, [
      h("p", { text: f.name + " in your workspace is v" + (f.base_version || "?") + "; the server has " +
        (f.server_version ? "v" + f.server_version : "removed it") + ". Get the current files first, so you do not work on an old version." }),
      lockText ? h("p.notice", { text: lockText }) : null
    ]), [
      { label: "Cancel", onClick: function (d) { d.close(); } },
      { label: "Open my copy", onClick: function (d) { d.close(); open(); } },
      { label: "Get latest and open", primary: true, submit: true, onClick: function (d) {
        d.close();
        getting(call("workspace_download", { project: rev.project_code, item: rev.item_number, code: rev.code }))
          .then(function (r) { reportDownload(r); open(r); renderActive(); }, function (e) { toast(errorText(e), "error", "Getting the latest files to open " + f.name, revisionInputs(rev, [["file", f.name]])); });
      } }
    ]);
    return;
  }
  var warnings = [];
  if (f.state === "conflict") warnings.push(f.name + " was changed here and on the server since you got it (yours: v" + (f.base_version || "?") +
    ", server: " + (f.server_version ? "v" + f.server_version : "removed") + "). Save your work under another name before you get the current version.");
  if (lockText) warnings.push(lockText);
  if (!warnings.length) { open(); return; }
  dialog(f.state === "conflict" ? "Conflicting file" : "Checked out by " + rev.checked_out_by_name,
    h("div.stack", null, warnings.map(function (w) { return h("p", { text: w }); })), [
      { label: "Cancel", onClick: function (d) { d.close(); } },
      { label: "Open anyway", primary: true, submit: true, onClick: function (d) { d.close(); open(); } }
    ]);
}

/** Opens a file of a revision: `local` = it is in the working folder already
 * (`got`: the Get that just brought it, if any), else it is fetched first. */
export function openFile(tab, filename, local, warned, got) {
  var rev = tab.data.rev;
  var lockText = warned ? null : foreignLockText(rev);
  var asked = lockText ? confirmDialog("Checked out by " + rev.checked_out_by_name, lockText, "Open anyway") : Promise.resolve(true);
  asked.then(function (ok) {
    if (!ok) return;
    var ready = local ? Promise.resolve(got) :
      getting(call("workspace_download", { project: rev.project_code, item: rev.item_number, code: rev.code }));
    return ready.then(function (r) {
      // a native CAD file: in the CAD program chosen for it (cad-open.js) –
      // not after a stopped or failed Get, an assembly would open without
      // its parts (#348)
      var file = (tab.data.files || []).filter(function (f) { return f.filename === filename; })[0];
      if (fileFamily(file)) {
        var why = incompleteGet(r);
        if (why) { toast(notOpenedText(filename, why), "info"); return; }
        return openWithCad(rev, file);
      }
      return call("workspace_open", { project: rev.project_code, item: rev.item_number, code: rev.code, file: filename });
    });
  }).catch(function (e) { if (e) toast(errorText(e), "error", "Opening " + filename, revisionInputs(rev, [["file", filename]])); });
}

export function doCheckout(tab, then) {
  var rev = tab.data.rev;
  run("Checking out " + rev.item_number + "/" + rev.code, function () {
    return getting(call("workspace_checkout", { project: rev.project_code, item: rev.item_number, code: rev.code }));
  }, revisionInputs(rev)).then(function (r) {
    // with structure: the parts came read-only, local files were never overwritten
    toast(r.summary || ("Checked out " + rev.item_number + "/" + rev.code + " – the files in " + r.dir + " are writable now"));
    if (r.kept.length || r.clashes.length || r.failed.length || r.unresolved.length) reportDownload(r);
    refreshStructure();
    tab.sub = "workspace";
    reloadActive();
    refreshWorklistCount();
    // Check out and open: the files are in the working folder now
    if (then) then(r);
  }, function () {});
}

/** Why a part of the structure cannot be ticked in *Check out parts…*, or
 * null; `info` is its revision in the working set (state, lock, group). */
export function checkoutBlocker(info) {
  if (!info) return "Not in the working set";
  if (info.locked_by_me) return "Already checked out by you";
  if (!canEdit()) return "Viewers cannot check out";
  // the group by its code; one the app does not know is left to the server
  var group = info.project_code ? state.projects.filter(function (p) { return p.code === info.project_code; })[0] : null;
  if (group && !canEditProject(group.id)) return "Not a member of project group " + info.project_code;
  if (info.state === "released") return "Released – revise it first";
  if (info.state === "in_review") return "In review";
  if (info.state !== "in_work") return label(info.state) + " – cannot be checked out";
  if (info.locked_by) return "Checked out by " + (info.locked_by_name || "another user");
  return null;
}

/** "Check out parts…" (#415): the parts of an assembly as a tree with
 * check boxes – state and lock holder per part, the ones that cannot be
 * checked out disabled with the reason. One step checks out the ticked
 * revisions (`POST /checkouts`, one transaction), makes their files in the
 * working folder writable and lists what the server refused. */
export function checkoutPartsDialog(tab) {
  var rev = tab.data.rev;
  var path = "/items/" + enc(rev.item_number) + "/revisions/" + enc(rev.code);
  run("Loading the structure of " + rev.item_number + "/" + rev.code, function () {
    return Promise.all([api("GET", path + "/bom?expand=true&rule=latest"), api("GET", path + "/workset?rule=latest")]);
  }).then(function (r) { showCheckoutParts(tab, r[0], r[1]); }, function () {});
}

function showCheckoutParts(tab, tree, set) {
  var rev = tab.data.rev;
  var byId = {};
  (set.revisions || []).forEach(function (w) { byId[w.revision_id] = w; });
  var seen = {}, rows = [], boxes = [];
  function row(depth, itemNumber, revision, name, note) {
    var info = revision ? byId[revision.id] : null;
    var blocker = revision ? checkoutBlocker(info) : note;
    var box = h("input", { type: "checkbox", disabled: !!blocker, checked: !!(info && info.locked_by_me),
      "aria-label": "Check out " + itemNumber + (revision ? "/" + revision.code : "") });
    if (!blocker) boxes.push({ input: box, pick: { project: info.project_code || null, item: itemNumber, code: revision.code } });
    var indent = h("span.indent");
    indent.style.width = (depth * 1.2) + "rem";
    rows.push({
      cells: [
        h("span", null, [indent, box, " ", h("span.mono", { text: itemNumber + (revision ? "/" + revision.code : "") })]),
        name,
        revision ? badge(revision.state) : "",
        h("span" + (blocker ? ".muted" : ""), { text: blocker || (info && info.via === "reference" ? "Referenced by a CAD file" : "Can be checked out") })
      ]
    });
  }
  row(0, tree.root_item_number, tree.root, tree.root.name, null);
  seen[tree.root.id] = true;
  function walk(nodes, depth) {
    (nodes || []).forEach(function (n) {
      var r = n.revision;
      if (n.restricted) { row(depth, "—", null, h("span.muted", { text: "Restricted item" }), "In a project group you are not a member of"); return; }
      if (!r) { row(depth, n.item_number, null, "", "No revision"); return; }
      // a part used more than once is offered at its first place only
      if (seen[r.id]) return;
      seen[r.id] = true;
      row(depth, n.item_number, r, r.name, null);
      walk(n.children, depth + 1);
    });
  }
  walk(tree.children, 1);
  // parts the CAD files reference without a BOM line
  (set.revisions || []).forEach(function (w) {
    if (seen[w.revision_id]) return;
    seen[w.revision_id] = true;
    row(1, w.item_number, { id: w.revision_id, code: w.code, state: w.state }, w.name, null);
  });
  function tickAll(on) { boxes.forEach(function (b) { b.input.checked = on; }); }
  var body = h("div.stack", null, [
    h("p", { text: "Tick the parts you want to change in the context of the assembly. They are checked out in one step and their files in your working folder become writable; " +
      "the others stay read-only. Parts that are released, in review or checked out by someone else cannot be ticked." }),
    boxes.length ? h("div.row", null, [
      h("button.small", { type: "button", on: { click: function () { tickAll(true); } } }, "Tick all"),
      h("button.small", { type: "button", on: { click: function () { tickAll(false); } } }, "Tick none")
    ]) : h("p.notice", { text: "Nothing in this structure can be checked out by you right now." }),
    table(["Item", "Name", "State", "Check-out"], rows)
  ]);
  dialog("Check out parts of " + rev.item_number + "/" + rev.code, body, [
    { label: "Cancel", onClick: function (d) { d.close(); } },
    { label: "Check out", primary: true, submit: true, onClick: function (d) {
      var picks = boxes.filter(function (b) { return b.input.checked; }).map(function (b) { return b.pick; });
      if (!picks.length) { d.fail("Tick at least one part."); return; }
      d.close();
      doCheckoutMany(picks);
    } }
  ], { wide: true });
}

/** Checks out `picks` ({project, item, code}) in one step and reports the
 * result: a toast, and the refused revisions with the server's reason. */
export function doCheckoutMany(picks) {
  run("Checking out " + picks.length + (picks.length === 1 ? " revision" : " revisions"), function () {
    return getting(call("workspace_checkout_many", { revisions: picks }));
  }).then(function (r) {
    toast(r.summary, r.refused.length ? "info" : null);
    var f = r.files;
    if (r.refused.length) {
      dialog("Some parts were not checked out", h("div.stack", null, [
        h("p", { text: r.summary }),
        h("ul.small", null, r.refused.map(function (o) {
          return h("li", null, [h("span.mono", { text: o.item ? o.item + "/" + o.code : (o.revision_id || "?") }), " – " + (o.message || o.reason || "refused")]);
        })),
        f.kept.length || f.clashes.length || f.failed.length
          ? h("p.small.muted", { text: "Files: " + f.kept.length + " kept (your local changes), " + f.clashes.length + " name clash(es), " + f.failed.length + " failed." }) : null
      ]), [{ label: "Close", primary: true, submit: true, onClick: function (d) { d.close(); } }], { wide: true });
    } else if (f.kept.length || f.clashes.length || f.failed.length) {
      reportDownload(f);
    }
    refreshStructure();
    reloadActive();
    refreshWorklistCount();
  }, function () {});
}

// Cancelling one's own check-out takes effect at once (the local files stay
// in the workspace); releasing someone else's lock is confirmed first.
export function doCancelCheckout(tab) {
  var rev = tab.data.rev;
  var other = rev.checked_out_by !== state.me.id;
  function release() {
    run("Releasing the check-out", function () {
      return api("POST", "/items/" + enc(rev.item_number) + "/revisions/" + enc(rev.code) + "/cancel-checkout");
    }, revisionInputs(rev)).then(function () {
      toast("Released the check-out of " + rev.item_number + "/" + rev.code);
      call("usage_count", { action: "cancel_checkout" }).catch(function () {});
      // the unchanged local files become read-only again
      call("workspace_status", { project: rev.project_code, item: rev.item_number, code: rev.code }).catch(function () {}).then(function () {
        reloadActive(); refreshWorklistCount(); rescanLocalChanges();
      });
    }, function () {});
  }
  if (!other) { release(); return; }
  confirmDialog("Force unlock",
    "Release the check-out of " + rev.item_number + "/" + rev.code + " held by " + rev.checked_out_by_name + "?",
    "Force unlock", true).then(function (ok) { if (ok) release(); });
}

/** Follows the check-in upload of the open check-in dialog (`upload-progress`). */
export var uploadWatcher = null;

/** "bracket.sc is open in SolidCraft on WS-12 (kim)" – from the lock file the program keeps (cadlock.rs). */
function openInText(f) {
  var o = f.open_in;
  return f.name + " is open in " + o.program + " on " + o.computer + (o.user ? " (" + o.user + ")" : "");
}

/** "Check in…" (`park` false) or "Park…" (`park` true, issue #173 step 10):
 * the same dialog with both buttons; `park` only decides the default one. */
export function checkinDialog(tab, park) {
  var rev = tab.data.rev;
  Promise.all([call("workspace_status", { project: rev.project_code, item: rev.item_number, code: rev.code }), cadPrograms(),
    formatKinds().catch(function () { return {}; })]).then(function (r) {
    var ws = r[0], programs = r[1], kinds = r[2] || {};
    var candidates = ws.files.filter(function (f) { return f.state === "modified" || f.state === "new" || f.state === "conflict"; });
    // conflicting files are offered, but not ticked: the server refuses them
    // anyway; a second model file is usually a new item of its own (#359)
    var boxes = candidates.map(function (f) {
      var ownItem = isNewModelOfItsOwn(f, ws.files, kinds);
      return { name: f.name, path: f.path, ownItem: ownItem, input: h("input", { type: "checkbox", checked: f.state !== "conflict" && !ownItem }), state: f.state };
    });
    var comment = h("input", { type: "text", maxlength: "2000", placeholder: "What changed, e.g. wall thickness 2 → 2.5 mm" });
    var progressLine = h("p.small.muted.hidden", { role: "status" });
    // Like the CAD add-ins of PDM systems: viewables at check-in.
    var exportable = boxes.filter(function (b) { return exportProgram(b.name, programs); });
    // off by default (#342): an export through the CAD program can take many
    // minutes for a large assembly; the files are uploaded before it runs
    var withStep = h("input", { type: "checkbox", checked: false });
    var stepNote = exportable.length ? h("label.check", null, [withStep,
      "Create STEP exports with " + exportable.map(function (b) { return PROGRAM_LABEL[exportProgram(b.name, programs)]; })
        .filter(function (v, i, a) { return a.indexOf(v) === i; }).join(" / ") +
      " for the 3D preview (" + exportable.length + (exportable.length === 1 ? " file" : " files") +
      ") – your files are uploaded first, a slow export can be skipped"]) : null;
    // #407: on by default every time (the choice is not remembered); only on
    // a check-in – Park keeps the revision checked out, nothing is final yet
    var withBom = h("input", { type: "checkbox", checked: true });
    var bomNote = offerBomUpdate(ws, rev, kinds) ? h("label.check", null, [withBom,
      "Update the BOM from the CAD file – adds the parts the assembly uses and corrects counted quantities, never removes a line (not when parking)"]) : null;
    var body = h("div.stack", null, [
      h("p.small.muted", null, ["From ", h("span.mono", { text: ws.dir })]),
      candidates.length ? h("div.stack", null, boxes.map(function (b) {
        var row = h("label.check", null, [b.input, b.name, " ", badge(b.state, SYNC_LABEL[b.state])]);
        if (!b.ownItem) return row;
        return h("div.stack", null, [row, h("p.small.muted", null, [
          "A new part or assembly – usually an item of its own, not a file of " + rev.item_number + ". ",
          h("button.link.small", { type: "button", on: { click: function () {
            newItemDialog({ paths: [workspacePath(state.session.workspace_dir, b.path)] });
          } } }, "New item from this file")
        ])]);
      })) : h("p.empty", { text: "No modified or new files – checking in only ends your check-out; the revision stays in work." }),
      candidates.some(function (f) { return f.open_in; }) ? h("p.notice", { text: "Still open: " +
        candidates.filter(function (f) { return f.open_in; }).map(openInText).join("; ") +
        ". Unsaved changes there are not checked in – save first, or check in anyway." }) : null,
      boxes.some(function (b) { return b.state === "conflict"; }) ? h("p.notice.error", { text: "Conflict: these files changed on the server since you got them. " +
        "The server refuses to overwrite the newer version (nothing is stored) – save your copy under another name, get the current files and merge." }) : null,
      stepNote,
      bomNote,
      field("Comment", comment, "stored with every new version"),
      progressLine
    ]);
    // While the check-in runs the dialog stays open (no Escape, no backdrop).
    // Without STEP exports Cancel stops it between two chunks of a large file
    // or half-way through the request with the small files – nothing is
    // stored, the revision stays checked out. Once every byte is sent the
    // server finishes the check-in; the dialog then says so.
    // With STEP exports (#342) the ticked files go to the server first,
    // parked (the lock stays), so a slow or failed export never costs the
    // check-in; during an export Cancel skips it (the CAD program it started
    // is ended), and the check-in follows with the STEP files written so far.
    var running = false, cancelled = false, phase = "", skipping = false;
    // *Report…* of a failure (#381): the revision, the ticked file names, STEP
    function ticked() { return boxes.filter(function (b) { return b.input.checked; }).map(function (b) { return b.name; }); }
    function checkinInputs() {
      return revisionInputs(rev, [["files", ticked().join(", ")], ["park", park ? "yes" : ""],
        ["step exports", stepNote && withStep.checked ? "yes" : ""]]);
    }
    var opts = { wide: true, inputs: checkinInputs };
    function stopped() { return { status: 499, message: "Cancelled." }; }
    dialog((park ? "Park " : "Check in ") + rev.item_number + "/" + rev.code, body, [
      { label: "Cancel", onClick: function (d) {
        if (!running) { d.close(); return; }
        if (phase === "export") {
          if (skipping) return;
          skipping = true;
          progressLine.textContent = "Skipping the STEP exports …";
          call("cad_export_cancel").catch(function () {});
          return;
        }
        if (cancelled) return;
        cancelled = true;
        progressLine.textContent = "Stopping …";
        progressLine.classList.remove("hidden");
        call("cancel_upload").catch(function () {});
      } },
      // Park: the same upload, but the lock stays (issue #173, step 10) –
      // the work is safe on the server, the revision stays yours.
      { label: "Park (keep checked out)", primary: !!park, submit: !!park, onClick: function (d) { send(d, true); } },
      { label: "Check in", primary: !park, submit: !park, onClick: function (d) { send(d, false); } }
    ], opts);
    function send(d, parking) {
      if (running) return;
      var files = boxes.filter(function (b) { return b.input.checked; }).map(function (b) { return b.name; });
      if (parking && !files.length) { d.fail("Tick the files to park – parking keeps the check-out and uploads only those."); return; }
      running = true; cancelled = false; skipping = false; opts.modal = true;
      var toExport = stepNote && withStep.checked ? files.filter(function (n) { return exportProgram(n, programs); }) : [];
      var ref = rev.item_number + "/" + rev.code;
      var args = { project: rev.project_code, item: rev.item_number, code: rev.code, comment: comment.value };
      function show(text) { if (!cancelled) { progressLine.textContent = text; progressLine.classList.remove("hidden"); } }
      var bomResult = null; // the `bom` part of the check-in answer (#407)
      function upload(command, names, text) {
        phase = "upload";
        show(text);
        uploadWatcher = function (p) { show(uploadLine(p)); };
        var extra = { files: names };
        if (command === "workspace_checkin") extra.bomFromReferences = !!(bomNote && withBom.checked);
        return call(command, Object.assign({}, args, extra)).then(function (r) {
          uploadWatcher = null;
          if (command === "workspace_checkin" && r) bomResult = r.bom || null;
          return r;
        });
      }
      var steps = [], exportFailures = [], stored = false;
      // one export after the other: the CAD program handles one document at a
      // time. A failed or skipped export never stops the check-in (#342,
      // #368): the file goes without its STEP and the result says why.
      function exportAll() {
        phase = "export";
        return toExport.reduce(function (chain, name) {
          return chain.then(function () {
            if (skipping) { exportFailures.push(name + ": skipped"); return; }
            var started = Date.now();
            var what = "Exporting " + name + " to STEP with " + PROGRAM_LABEL[exportProgram(name, programs)];
            function tick() { show(what + " … " + elapsedText(Date.now() - started) + " – your files are on the server; Cancel skips the export"); }
            tick();
            var timer = setInterval(tick, 1000);
            return exportStep(rev, name).then(function (step) {
              clearInterval(timer);
              if (steps.indexOf(step) < 0 && files.indexOf(step) < 0) steps.push(step);
            }, function (e) {
              clearInterval(timer);
              exportFailures.push(name + ": " + (skipping ? "skipped" : errorText(e)));
            });
          });
        }, Promise.resolve());
      }
      var done;
      if (!toExport.length) {
        done = upload(parking ? "workspace_park" : "workspace_checkin", files, parking ? "Parking …" : "Checking in …");
      } else {
        done = upload("workspace_park", files, "Uploading " + files.join(", ") + " …").then(function () {
          stored = true;
          if (cancelled) throw stopped();
          return exportAll();
        }).then(function () {
          if (cancelled) throw stopped();
          if (parking && !steps.length) return null;
          return upload(parking ? "workspace_park" : "workspace_checkin", steps, parking ? "Parking the STEP exports …" : "Checking in …");
        });
      }
      done.then(function () {
        running = false; uploadWatcher = null; phase = "";
        setStatus("");
        d.close();
        var all = files.concat(steps);
        var bomLine = parking ? "" : checkinBomText(bomResult);
        toast((parking ? "Parked " + all.join(", ") + " on the server – " + ref + " stays checked out to you"
          : "Checked in " + ref + (all.length ? " with " + all.join(", ") : "")) +
          (bomLine ? ". " + bomLine : "") +
          (cancelled ? " – the server already had every file when you cancelled" : "") +
          (exportFailures.length ? ". No STEP export for " + exportFailures.join("; ") : ""),
          cancelled || exportFailures.length ? "info" : null);
        reloadActive();
        refreshWorklistCount(); rescanLocalChanges();
      }, function (err) {
        running = false; uploadWatcher = null; opts.modal = false; phase = "";
        setStatus("");
        if (stored) {
          // the chosen files are on the server already: never offer to send them again
          d.close();
          toast(files.join(", ") + " are on the server (parked) and " + ref + " is still checked out to you – " +
            (cancelled || isCancelled(err) ? "the check-in was cancelled." : "the check-in failed: " + errorText(err)), cancelled || isCancelled(err) ? "info" : "error",
            "Checking in " + ref, checkinInputs);
          reloadActive();
          refreshWorklistCount(); rescanLocalChanges();
          return;
        }
        if (cancelled || isCancelled(err)) {
          d.close();
          toast("Check-in of " + ref + " cancelled – nothing was stored; the revision is still checked out to you.", "info");
          reloadActive();
          return;
        }
        progressLine.classList.add("hidden");
        d.fail(err);
      });
    }
  }, function (err) { toast(errorText(err), "error", "Comparing the working folder of " + rev.item_number + "/" + rev.code, revisionInputs(rev)); });
}

/** `0:07`, `12:34` – how long an export has been running. */
export function elapsedText(ms) {
  var s = Math.max(0, Math.floor(ms / 1000));
  return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0");
}

export function editDialog(tab) {
  var rev = tab.data.rev;
  var name = h("input", { type: "text", value: rev.name, maxlength: "200", required: true });
  var materialF = materialField(rev.material), material = materialF.input;
  var mass = h("input", { type: "text", inputmode: "decimal", value: rev.mass_kg === null ? "" : String(rev.mass_kg) });
  var cost = h("input", { type: "text", inputmode: "decimal", value: rev.unit_cost === null ? "" : String(rev.unit_cost) });
  var desc = h("textarea", { maxlength: "10000" });
  desc.value = rev.description || "";
  function num(input) {
    var v = input.value.trim().replace(",", ".");
    if (!v) return null;
    var n = Number(v);
    if (!isFinite(n)) throw new Error("'" + input.value + "' is not a number.");
    return n;
  }
  dialog("Edit " + rev.item_number + "/" + rev.code, h("div.stack", null, [
    field("Name", name), materialF.el,
    h("div.grid-2", null, [field("Mass per unit (kg)", mass), field("Unit cost (€)", cost)]),
    field("Description", desc)
  ]), [
    { label: "Cancel", onClick: function (d) { d.close(); } },
    {
      label: "Save", primary: true, submit: true, onClick: function (d) {
        var body;
        try {
          body = { name: name.value, material: material.value, description: desc.value, mass_kg: num(mass), unit_cost: num(cost) };
        } catch (e) { d.fail(e.message); return; }
        api("PATCH", "/items/" + enc(rev.item_number) + "/revisions/" + enc(rev.code), body).then(function () {
          d.close(); toast("Saved " + rev.item_number + "/" + rev.code); reloadActive();
        }, function (err) { d.fail(err); });
      }
    }
  ], { wide: true });
}

/**
 * Release workflow: creates a change order for this revision, assigns the
 * reviewers (CAD editors of the project group are offered) and submits it.
 */
export function releaseDialog(tab) {
  var rev = tab.data.rev;
  var mine = rev.checked_out_by === state.me.id;
  var title = h("input", { type: "text", maxlength: "200", required: true, value: "Release " + rev.item_number + "/" + rev.code + " – " + rev.name });
  var priority = h("select", null, ["low", "normal", "high", "critical"].map(function (p) {
    return h("option", { value: p, text: label(p), selected: p === "normal" });
  }));
  var desc = h("textarea", { maxlength: "20000", placeholder: "What is released and why" });
  var others = h("input", { type: "text", placeholder: "user names, comma-separated" });
  var submitNow = h("input", { type: "checkbox", checked: true });
  // A change releases a whole product: unreleased BOM children, drawings and
  // specifications have to go with it (the server blocks the submit otherwise).
  var withDeps = h("input", { type: "checkbox", checked: true });
  var reviewerBox = h("div.stack", null, h("p.loading", { text: "Loading the members of " + (rev.project_code || "the project group") }));
  var boxes = [];
  if (rev.project_code) {
    api("GET", "/projects/" + enc(rev.project_code) + "/members").then(function (members) {
      clear(reviewerBox);
      var candidates = members.filter(function (m) {
        return m.active && m.user_id !== state.me.id && (m.role === "editor" || m.role === "admin");
      });
      if (!candidates.length) {
        // A release needs a second person (four-eyes principle): with nobody
        // else in the group, offer the draft first instead of a submit that
        // cannot work (a single-user test server never gets past it).
        submitNow.checked = false;
        reviewerBox.appendChild(h("p.notice", { text: "Nobody else in " + rev.project_code + " can review this change. " +
          "Name a CAD editor or administrator of another group below, or create the change as a draft – " +
          "a release always needs a second person (four-eyes principle)." }));
      }
      candidates.forEach(function (m) {
        var input = h("input", { type: "checkbox" });
        boxes.push({ username: m.username, input: input });
        reviewerBox.appendChild(h("label.check", null, [input, m.display_name, h("span.dim", { text: " " + m.username + " · " + label(m.role) })]));
      });
    }, function (err) { clear(reviewerBox); reviewerBox.appendChild(errorNotice("p.notice.error", errorText(err), "Loading the reviewers of " + rev.project_code, revisionInputs(rev))); });
  } else {
    clear(reviewerBox);
  }
  var body = h("div.stack", null, [
    mine ? h("p.notice", { text: "You still hold the check-out. Check in your workspace changes first – submitting releases the lock without uploading anything." }) : null,
    field("Title", title),
    h("div.grid-2", null, [field("Priority", priority), h("label.check", null, [submitNow, "Submit for review now"])]),
    h("label.check", null, [withDeps, "Include unreleased dependencies", h("span.dim", { text: " – BOM children, drawings and specifications in work, level by level" })]),
    field("Description", desc),
    h("h3", { text: "Reviewers" }),
    h("p.hint", { text: "All reviewers must approve; you cannot review your own change (four-eyes principle)." }),
    reviewerBox,
    field("Other reviewers", others, "optional")
  ]);
  dialog("Release " + rev.item_number + "/" + rev.code, body, [
    { label: "Cancel", onClick: function (d) { d.close(); } },
    {
      label: "Create change", primary: true, submit: true, onClick: function (d) {
        var problem = reviewerProblem(boxes.filter(function (b) { return b.input.checked; }).map(function (b) { return b.username; }),
          others.value, state.me.username, submitNow.checked);
        if (problem.error) { d.fail(problem.error); return; }
        var reviewers = problem.reviewers;
        var number = null, submitting = false;
        setStatus("Creating the change …");
        api("POST", "/changes", { title: title.value, description: desc.value, priority: priority.value }).then(function (c) {
          number = c.number;
          return api("POST", "/changes/" + enc(number) + "/items", { item: rev.item_number, revision: rev.code });
        }).then(function () {
          return withDeps.checked ? api("POST", "/changes/" + enc(number) + "/dependencies") : null;
        }).then(function () {
          return reviewers.reduce(function (p, username) {
            return p.then(function () { return api("POST", "/changes/" + enc(number) + "/reviewers", { username: username }); });
          }, Promise.resolve());
        }).then(function () {
          submitting = true;
          return submitNow.checked ? api("POST", "/changes/" + enc(number) + "/submit") : null;
        }).then(function () {
          setStatus("");
          d.close();
          call("usage_count", { action: "release" }).catch(function () {});
          toast(number + (submitNow.checked ? " submitted for review – " + rev.item_number + "/" + rev.code + " is now In Review" : " created as draft"));
          tab.data = null;
          refreshWorklistCount();
          openChange(number);
        }, function (err) {
          if (!number) { setStatus(""); d.fail(err); return; }
          if (submitting) {
            // complete (revisions and reviewers) – the change tab finishes it
            setStatus("");
            d.fail(errorText(err) + " – " + number + " stays a draft; fix this and submit it in its tab.");
            tab.data = null;
            openChange(number);
            return;
          }
          // a revision, dependency or reviewer was refused: take the half-made
          // draft back, so no orphan blocks the revisions (#291)
          api("POST", "/changes/" + enc(number) + "/cancel").then(function () {
            setStatus("");
            d.fail(errorText(err) + " – nothing was released; the draft " + number + " was cancelled.");
          }, function () {
            setStatus("");
            d.fail(errorText(err) + " – " + number + " exists as a draft; finish or cancel it in its tab.");
            tab.data = null;
            openChange(number);
          });
        });
      }
    }
  ], { wide: true });
}

export function doRevise(tab) {
  reviseRevision(tab.data.rev.item_number, tab.data.rev.code);
}

export function reviseRevision(number, code) {
  confirmDialog("Revise", "Create a new revision of " + number + " based on " + code + "? It is checked out to you.", "Revise").then(function (ok) {
    if (!ok) return;
    run("Creating the next revision", function () {
      return api("POST", "/items/" + enc(number) + "/revisions/" + enc(code) + "/revise");
    }, [["item", number], ["revision", code]]).then(function (nr) {
      toast("Created " + nr.item_number + "/" + nr.code + " (in work, checked out to you)");
      openRevision(nr.item_number, nr.code, "workspace");
      refreshWorklistCount();
    }, function () {});
  });
}

export function doObsolete(tab) {
  var rev = tab.data.rev;
  confirmDialog("Mark obsolete", "Mark " + rev.item_number + "/" + rev.code + " as obsolete? This cannot be undone.", "Mark obsolete", true).then(function (ok) {
    if (!ok) return;
    run("Marking obsolete", function () {
      return api("POST", "/items/" + enc(rev.item_number) + "/revisions/" + enc(rev.code) + "/obsolete");
    }, revisionInputs(rev)).then(function () { toast(rev.item_number + "/" + rev.code + " is now obsolete"); reloadActive(); }, function () {});
  });
}

// ---------------------------------------------------------------- add files

/** What the Files section says about adding files, by state: { text, ok }.
 * `ok` = Add files… works (it checks the revision out first if needed). */
export function filesHint(d) {
  var rev = d.rev, can = revisionAbilities(d), item = rev.item_number + "/" + rev.code;
  if (can.edit) return { ok: true, text: "Add files… copies files into your workspace; or edit the files there, then check in." };
  if (rev.state === "in_work" && rev.checked_out_by) {
    return { ok: false, text: "Checked out by " + rev.checked_out_by_name + " since " + fmtDate(rev.checked_out_at) + " – files can be added after check-in." };
  }
  if (rev.state === "in_work" && can.checkout) return { ok: true, text: "To add or replace files, check out this revision – Add files… does that for you." };
  if (rev.state === "released" || rev.state === "obsolete") {
    return { ok: false, text: label(rev.state) + " revisions are frozen – Revise to add files to a new revision" + (can.revise ? "." : " (editors of the project group can do that).") };
  }
  if (rev.state === "in_review") return { ok: false, text: item + " is in review and frozen – files can be added once the change is decided, or to a new revision." };
  return { ok: false, text: "Only editors of the project group can add files." };
}

/** "Add files…": pick files, copy them into the working folder of the
 * revision (checking it out first if needed), optionally check in now.
 * The originals stay where they are; a name that exists asks first.
 * `presetPaths`: files dropped on the window (#418, js/drop.js) – used
 * instead of the file dialog, with the same checks. */
export function addFilesDialog(tab, presetPaths) {
  var rev = tab.data.rev, hint = filesHint(tab.data);
  if (!hint.ok) { toast(hint.text, "error"); return; }
  var mine = rev.checked_out_by === state.me.id;
  var where = { project: rev.project_code, item: rev.item_number, code: rev.code };
  var chosen = presetPaths && presetPaths.length ? Promise.resolve(presetPaths.slice()) : call("pick_files");
  chosen.then(function (paths) {
    if (!paths.length) return;
    var subfolder = h("input", { type: "text", placeholder: "revision's main folder", spellcheck: "false" });
    var checkNow = h("input", { type: "checkbox" });
    var listing = h("div.stack");
    var ticks = [];
    function refresh() {
      call("workspace_add_plan", Object.assign({ paths: paths, subfolder: subfolder.value.trim() }, where)).then(function (items) {
        clear(listing);
        ticks = [];
        items.forEach(function (p) {
          var tick = null;
          if (p.new_version || p.exists_locally) {
            tick = h("input", { type: "checkbox" });
            ticks.push({ name: p.name, input: tick });
          }
          listing.appendChild(h("label.check", null, [tick, h("span", null, [p.name, " ",
            h("span.dim.small", { text: "→ " + p.rel }),
            p.new_version ? h("span.small", null, [" ", badge("modified", "new version"), " – replaces the copy in the working folder"]) :
              p.exists_locally ? h("span.small", null, [" ", badge("clash", "exists"), " – replaces the file that is there"]) : null])]));
        });
        if (ticks.length) listing.appendChild(h("p.small.muted", { text: "Tick the files that may replace what is there; unticked ones are not added." }));
      }, function (e) { clear(listing); listing.appendChild(h("p.error-text", { text: errorText(e) })); });
    }
    subfolder.addEventListener("change", refresh);
    refresh();
    dialog("Add files to " + rev.item_number + "/" + rev.code, h("div.stack", null, [
      h("p.small.muted", { text: "The files are copied into your working folder – your originals are not moved or changed. Same name as an existing file means a new version." }),
      mine ? null : h("p.notice", { text: "This revision is not checked out by you: it is checked out first." }),
      listing,
      field("Sub-folder", subfolder, "optional, below the revision's main folder; ignored for files that are new versions"),
      h("label.check", null, [checkNow, "Check in now (otherwise they are listed as new files in Check in…)"])
    ]), [
      { label: "Cancel", onClick: function (d) { d.close(); } },
      { label: "Add files", primary: true, submit: true, onClick: function (d) {
        var confirmed = ticks.filter(function (t) { return t.input.checked; }).map(function (t) { return t.name; });
        var start = mine ? Promise.resolve() : run("Checking out " + rev.item_number + "/" + rev.code, function () {
          return getting(call("workspace_checkout", where));
        }, revisionInputs(rev));
        start.then(function () {
          return call("workspace_add_files", Object.assign({ paths: paths, subfolder: subfolder.value.trim(), confirmed: confirmed }, where));
        }).then(function (r) {
          d.close();
          toast((r.copied.length ? "Added " + r.copied.join(", ") + " to the working folder" : "Nothing added") +
            (r.skipped.length ? " – not replaced: " + r.skipped.join(", ") : ""), r.skipped.length ? "info" : null);
          refreshWorklistCount(); rescanLocalChanges();
          if (checkNow.checked && r.copied.length) { reloadActive(); checkinDialog(tab); } else { tab.sub = "workspace"; reloadActive(); }
        }, function (e) { d.fail(e); reloadActive(); });
      } }
    ], { wide: true, inputs: function () { return revisionInputs(rev, [["files", paths.map(function (p) { return String(p).split(/[\\/]/).pop(); }).join(", ")]]); } });
  }, function (e) { toast(errorText(e), "error"); });
}
