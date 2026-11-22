// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – pure rules of a file drop (#418): where files dropped
// on the window from the file manager go.
// No imports and no DOM: scripts/desktop-drop-route.test.mjs tests it with
// plain Node.js. js/drop.js listens for the drop and applies the result.

/** The refusal for viewers and editors without a project group to create items in. */
export var NO_CREATE_TEXT = "Only editors of a project group can create items – ask an admin or a group manager for edit rights.";

/** The refusal while a dialog is open (the drop would land behind it). */
export var DIALOG_OPEN_TEXT = "Finish or close the open dialog first, then drop the files again.";

/** The refusal while a revision tab is still loading. */
export var LOADING_TEXT = "The revision is still loading – drop the files again in a moment.";

/**
 * Where a drop of `paths` goes:
 *
 * * `{ kind: "add", tab, paths }` – the active tab is a revision whose
 *   *Add files…* works (`hint.ok`: checked out by the user, or free for the
 *   user to check out – Add files… does that first);
 * * `{ kind: "refuse", text }` – the active tab is a revision that takes no
 *   files (released, in review, checked out by someone else, the user is not
 *   an editor of its group: the hint's text says which), a dialog is open, or
 *   the user may not create items anywhere;
 * * `{ kind: "new", paths }` – everything else: *New* with the files;
 * * `null` – nothing was dropped.
 *
 * `o.tab` is the active tab (null on the start page), `o.hint` is
 * workspace.js filesHint() of a loaded revision tab (`{ ok, text }`),
 * `o.canCreate` = the user may create items in at least one active group,
 * `o.dialogOpen` = a dialog is open.
 */
export function routeDrop(o) {
  var paths = (o && o.paths || []).filter(function (p) { return typeof p === "string" && p !== ""; });
  if (!paths.length) return null;
  if (o.dialogOpen) return { kind: "refuse", text: DIALOG_OPEN_TEXT };
  var tab = o.tab || null;
  if (tab && tab.kind === "revision") {
    if (!tab.data || !tab.data.rev) return { kind: "refuse", text: LOADING_TEXT };
    var hint = o.hint || { ok: false, text: "Files cannot be added to this revision." };
    return hint.ok ? { kind: "add", tab: tab, paths: paths } : { kind: "refuse", text: hint.text };
  }
  if (!o.canCreate) return { kind: "refuse", text: NO_CREATE_TEXT };
  return { kind: "new", paths: paths };
}

/** The line on the drop overlay while files are dragged over the window. */
export function dropLabel(route) {
  if (!route) return "";
  if (route.kind === "refuse") return route.text;
  var t = route.paths.length === 1 ? "the file" : route.paths.length + " files";
  if (route.kind === "add") {
    var rev = route.tab.data.rev;
    return "Drop to add " + t + " to " + rev.item_number + "/" + rev.code;
  }
  return "Drop to create a new item with " + t;
}

/** The note after folders or other entries were left out of a drop. */
export function skippedText(skipped) {
  if (!skipped) return "";
  return skipped === 1 ? "1 folder or other entry was skipped – only files are taken." :
    skipped + " folders or other entries were skipped – only files are taken.";
}
