// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – files dropped on the window from the file manager
// (#418). Tauri's native file drop (dragDropEnabled in tauri.conf.json)
// reports the absolute paths; js/drop-rules.js decides where they go:
// *New* with the files, *Add files…* of the open revision, or a refusal.
// The paths then take the same way as files chosen in the file dialog, with
// the same checks; src/commands.rs dropped_files keeps regular files only.
//
// The tabs are reordered with pointer events (tabs.js), not HTML5 drag and
// drop, so the native drop does not get in their way.

import { $, TAURI, call, errorText, h, state, toast } from "./core.js";
import { canEdit, canEditProject } from "./shell.js";
import { activeTab } from "./tabs.js";
import { addFilesDialog, filesHint } from "./workspace.js";
import { newItemDialog } from "./dialogs.js";
import { dropLabel, routeDrop, skippedText } from "./drop-rules.js";

var overlay = null, overlayText = null, dragged = [];

/** May the user create items in at least one active project group? */
function canCreate() {
  return !!canEdit() && state.projects.some(function (p) { return p.active && canEditProject(p.id); });
}

/** Signed in with the main window showing (not the connect screen). */
function shellShown() {
  var shell = $("shell");
  return !!state.me && !!shell && !shell.classList.contains("hidden");
}

/** The route of `paths` for what is on screen now. */
function route(paths) {
  var tab = activeTab() || null;
  var hint = null;
  if (tab && tab.kind === "revision" && tab.data && tab.data.rev) {
    try { hint = filesHint(tab.data); } catch (e) { hint = null; }
  }
  var root = $("dialog-root");
  return routeDrop({ paths: paths, tab: tab, hint: hint, canCreate: canCreate(), dialogOpen: !!(root && root.children.length) });
}

function showOverlay(r) {
  if (!overlay) {
    overlayText = h("p.drop-overlay-text");
    overlay = h("div.drop-overlay", { "aria-hidden": "true" }, overlayText);
    document.body.appendChild(overlay);
  }
  overlayText.textContent = dropLabel(r);
  document.body.classList.add("drop-target");
  document.body.classList.toggle("drop-refused", !!r && r.kind === "refuse");
}

function hideOverlay() {
  document.body.classList.remove("drop-target", "drop-refused");
}

/** Applies a drop: regular files only, then *New*, *Add files…* or the refusal. */
function onDrop(paths) {
  var first = route(paths);
  if (!first) return;
  if (first.kind === "refuse") { toast(first.text, "error"); return; }
  call("dropped_files", { paths: first.paths }).then(function (d) {
    var note = skippedText(d.skipped);
    if (!d.files.length) { toast(note || "Nothing to add – only files can be dropped.", "error"); return; }
    if (note) toast(note, "info");
    // the files may have changed the route (e.g. only folders): decide again
    var r = route(d.files);
    if (!r) return;
    if (r.kind === "refuse") toast(r.text, "error");
    else if (r.kind === "add") addFilesDialog(r.tab, r.paths);
    else newItemDialog({ paths: r.paths });
  }, function (e) { toast(errorText(e), "error"); });
}

/** Listens for the native file drop of the main window. */
export function initDrop() {
  var webview = TAURI.webview && TAURI.webview.getCurrentWebview ? TAURI.webview.getCurrentWebview() : null;
  if (!webview || !webview.onDragDropEvent) return;
  webview.onDragDropEvent(function (e) {
    var p = e.payload || {};
    if (!shellShown()) { hideOverlay(); dragged = []; return; }
    if (p.type === "enter") {
      dragged = p.paths || [];
      showOverlay(route(dragged));
    } else if (p.type === "over") {
      if (dragged.length) showOverlay(route(dragged));
    } else if (p.type === "drop") {
      hideOverlay();
      dragged = [];
      onDrop(p.paths || []);
    } else {
      hideOverlay();
      dragged = [];
    }
  }).catch(function () {});
}
