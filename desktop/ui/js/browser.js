// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – Files view of Home: the folders and files of the active
// project group, one folder at a time (GET /projects/{code}/files), so a big
// group is never loaded as a whole. The rules are in files-view.js.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { ICONS, api, badge, clear, enc, errorText, h, lockIcon, state, svgIcon, toast } from "./core.js";
import { label } from "/static/shared/js/format.js";
import { emptyHint, typeIcon } from "./navigator.js";
import { emptyGroupHint } from "./empty-rules.js";
import { openRevision } from "./revision.js";
import { canEdit, canEditProject } from "./shell.js";
import { newItemDialog } from "./dialogs.js";
import { checkedOutClass, countText, entryIconType, entryName, entryRevision, entryTitle, favouritePath, favouriteSuggestions, folderKey, groupTotal, mergeEntries, parentFolder, remaining, sortEntries, sortFolders } from "./files-view.js";

/** Entries asked for per request (the server's maximum). */
var PAGE = 500;

/** The folder one works in, per group: *New* (Ctrl/Cmd+N) starts there. */
function remember(p, path) {
  if (!state.filesFolder) state.filesFolder = {};
  state.filesFolder[p.code] = path || "";
}

/** A row that opens with a click, Enter or Space. */
function row(spec, attrs, children, onOpen) {
  attrs.tabindex = "0";
  attrs.on = {
    click: onOpen,
    keydown: function (e) {
      if (e.target !== el || (e.key !== "Enter" && e.key !== " ")) return;
      e.preventDefault();
      onOpen();
    }
  };
  var el = h(spec, attrs, children);
  return el;
}

function fetchLevel(p, path, offset) {
  return api("GET", "/projects/" + enc(p.code) + "/files?folder=" + enc(path) + "&limit=" + PAGE + "&offset=" + offset);
}

/**
 * Draws the files of project group `p` into the list `ul`.
 * `opts.sort` is the order of the files (files-view.js sortEntries),
 * `opts.count` the element that gets the number of rows of the group, and
 * `opts.olderServer()` draws the list for a server that does not know the
 * file browser yet.
 *
 * The favourite folder (issue #352): `opts.favourite` is the group's
 * favourite folder (`""` = none), `opts.atTop` shows the top of the group
 * although it has one, `opts.bar` is the element above the list for the
 * banner or the suggestion, and `opts.actions` = `{ favour(path),
 * showTop(on), decline() }` changes them (each draws Home again);
 * without `opts.actions` there are no stars.
 */
export function renderFiles(p, ul, opts) {
  var root = opts.favourite && !opts.atTop ? opts.favourite : "";
  clear(ul);
  ul.appendChild(h("li.loading", { text: "Loading" }));
  // the banner is up before the list: a folder that cannot be listed can
  // still be left
  drawBar(p, opts, null);
  return fetchLevel(p, root, 0).then(function (level) {
    if (opts.count) opts.count.textContent = String(groupTotal(level));
    drawBar(p, opts, level);
    // an empty group names the next step by role (#423)
    drawLevel(p, root, ul, level, opts, root ? "The favourite folder has no files." : function () {
      return emptyHint(emptyGroupHint(canEditProject(p.id)), { project: p.code, tag: "li" });
    });
  }, function (err) {
    if (err && err.status === 404 && opts.olderServer) {
      if (opts.bar) clear(opts.bar);
      return opts.olderServer();
    }
    clear(ul);
    ul.appendChild(h("li.notice.error", { text: errorText(err) }));
  });
}

/**
 * Above the list: in the favourite folder a banner in the accent colour
 * (where it is, the way to the whole group, removing it); at the top of a
 * group with a favourite the way back to it; else, once the top is loaded,
 * the folders with an Inventor project file as suggestions.
 */
function drawBar(p, opts, level) {
  var bar = opts.bar, act = opts.actions;
  if (!bar) return;
  clear(bar);
  bar.className = "home-root";
  if (!act) return;
  if (opts.favourite && !opts.atTop) {
    bar.className = "home-root on";
    bar.setAttribute("role", "note");
    bar.appendChild(h("p.home-root-title", null, [
      h("span.home-root-star", { "aria-hidden": "true", text: "★" }),
      h("strong", { text: "Favourite folder" }), " – not the top of the group"
    ]));
    bar.appendChild(h("p.home-root-path", { title: favouritePath(p.code, opts.favourite), text: favouritePath(p.code, opts.favourite) }));
    bar.appendChild(h("div.row", null, [
      h("button.small", { type: "button", title: "Show the top of " + p.code + " – the favourite stays", on: { click: function () { act.showTop(true); } } }, "Whole group"),
      h("button.small.ghost", { type: "button", title: "Home starts at the top of the group again", on: { click: function () { act.favour(""); } } }, "Remove favourite")
    ]));
    return;
  }
  bar.removeAttribute("role");
  if (opts.favourite) {
    bar.appendChild(h("button.link.small.home-root-back", {
      type: "button", title: "Back to the favourite folder " + favouritePath(p.code, opts.favourite),
      on: { click: function () { act.showTop(false); } }
    }, [h("span.home-root-star", { "aria-hidden": "true", text: "★" }), "Back to the favourite folder"]));
    return;
  }
  var suggested = favouriteSuggestions(level, opts.favourite, opts.declined);
  if (!suggested.length) return;
  bar.className = "home-root hint";
  bar.appendChild(h("p", { text: suggested.length === 1
    ? "The Inventor project file of this group is in " + suggested[0] + ". Start Home in that folder?"
    : "This group has Inventor project files in several folders. Start Home in one of them?" }));
  bar.appendChild(h("div.row", null, suggested.map(function (path) {
    return h("button.small", { type: "button", title: "Make " + path + " the favourite folder: Home starts there (on this computer)",
      on: { click: function () { act.favour(path); } } }, suggested.length === 1 ? "Start there" : path);
  }).concat([h("button.small.ghost", { type: "button", title: "Do not suggest it again; a folder's ☆ still makes it the favourite",
    on: { click: function () { act.decline(); } } }, "No, thanks")])));
}

/** Loads one folder into `ul`. */
function loadFolder(p, path, ul, opts) {
  clear(ul);
  ul.appendChild(h("li.loading", { text: "Loading" }));
  return fetchLevel(p, path, 0).then(function (level) {
    drawLevel(p, path, ul, level, opts, "This folder is empty.");
  }, function (err) {
    clear(ul);
    ul.appendChild(h("li.notice.error", { text: errorText(err) }));
  });
}

function drawLevel(p, path, ul, level, opts, emptyText) {
  var loaded = level.entries || [];
  var total = level.total || loaded.length;
  function more() {
    fetchLevel(p, path, loaded.length).then(function (next) {
      var before = loaded.length;
      loaded = mergeEntries(loaded, next.entries);
      // nothing new came: the list shrank meanwhile, there is no more
      if (loaded.length === before) total = loaded.length;
      paint();
    }, function (err) { toast(errorText(err), "error"); });
  }
  function paint() {
    clear(ul);
    var folders = sortFolders(level.folders);
    var entries = sortEntries(loaded, opts.sort);
    if (!folders.length && !entries.length) {
      ul.appendChild(typeof emptyText === "function" ? emptyText() : h("li.empty", { text: emptyText }));
      return;
    }
    folders.forEach(function (f) { ul.appendChild(folderNode(p, f, opts)); });
    var files = entries.filter(function (e) { return e.filename; });
    var loose = entries.filter(function (e) { return !e.filename; });
    files.forEach(function (e) { ul.appendChild(entryNode(p, path, e)); });
    if (loose.length) {
      ul.appendChild(h("li.tree-sep", { text: "Items without files" }));
      loose.forEach(function (e) { ul.appendChild(entryNode(p, path, e)); });
    }
    var rest = remaining(total, loaded.length);
    if (rest) {
      ul.appendChild(h("li", null, h("button.link.small", { type: "button", on: { click: more } },
        "show " + Math.min(rest, PAGE) + " more of " + total)));
    }
  }
  paint();
}

/** The star of a folder row: makes it the favourite folder, or (filled, on
 *  the favourite) removes it. */
function favouriteButton(f, opts) {
  if (!opts.actions) return null;
  var on = f.path === opts.favourite;
  var text = on ? "Remove the favourite folder " + f.path : "Make " + f.path + " the favourite folder: Home starts there";
  return h("button.fav" + (on ? ".on" : ""), {
    type: "button", title: text, "aria-label": text, "aria-pressed": String(on),
    on: { click: function (ev) { ev.stopPropagation(); opts.actions.favour(on ? "" : f.path); } }
  }, on ? "★" : "☆");
}

/** A folder: expands into its own level when opened. */
function folderNode(p, f, opts) {
  var key = folderKey(p.code, f.path);
  var open = !!state.expanded[key];
  var children = h("ul.tree" + (open ? "" : ".hidden"));
  var twisty = h("span.twisty", { text: open ? "▾" : "▸" });
  // New item here: the dialog starts with this group and folder
  var here = canEdit() && p.active && canEditProject(p.id) ? h("button.icon.folder-new", {
    type: "button", title: "New item in " + f.path, "aria-label": "New item in " + f.path,
    on: { click: function (ev) {
      ev.stopPropagation();
      remember(p, f.path);
      newItemDialog({ project: p.code, folder: f.path });
    } }
  }, svgIcon(ICONS.plus)) : null;
  var node = row("div.node.folder", {
    title: f.path + " – " + countText(f.files, "file", "files"), "aria-expanded": String(open)
  }, [
    twisty, h("span.ico", null, svgIcon(ICONS.folder)),
    h("span.label", null, h("strong", { text: f.name })),
    h("span.meta", null, [here, favouriteButton(f, opts), h("span.count", { text: String(f.files) })])
  ], function () {
    open = state.expanded[key] = !state.expanded[key];
    remember(p, open ? f.path : parentFolder(f.path));
    twisty.textContent = open ? "▾" : "▸";
    node.setAttribute("aria-expanded", String(open));
    children.classList.toggle("hidden", !open);
    if (open) loadFolder(p, f.path, children, opts);
  });
  if (open) loadFolder(p, f.path, children, opts);
  return h("li", null, [node, children]);
}

/** A file (or an item without files) of the folder `path`: the name first,
 *  the revision dimmed after it – like the rows of All items (navigator.js
 *  itemNode). */
function entryNode(p, path, e) {
  var key = "rev:" + e.item_number + ":" + e.code;
  var meta = [];
  if (e.state) meta.push(badge(e.state));
  if (e.checked_out_by_name) {
    var mine = !!state.me && e.checked_out_by_name === state.me.display_name;
    meta.push(h("span.lock-owner", { title: "Checked out by " + (mine ? "you" : e.checked_out_by_name) }, lockIcon(mine)));
  }
  var node = row("div.node.file" + checkedOutClass(e) + (state.active === key ? ".selected" : ""), {
    title: entryTitle(e, label), "data-item": e.item_number, "data-rev": e.code
  }, [
    h("span.twisty"), h("span.ico", { text: typeIcon(entryIconType(e)) }),
    h("span.label", null, [h("span.fname", { text: entryName(e) }), h("span.dim.mono", { text: " " + entryRevision(e) })]),
    h("span.meta", null, meta)
  ], function () { remember(p, path); openRevision(e.item_number, e.code); });
  return h("li", null, node);
}
