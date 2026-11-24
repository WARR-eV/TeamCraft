// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – Files view of Home (the folders and files of the active
// project group): the pure part (no DOM, no server calls), so it can be
// checked on its own. browser.js draws it.
//
// The view shows the group the way its working folder looks: folders first,
// then the files – by name unless another order is chosen. The file name is
// what an engineer knows a part by; the item number follows it, where a
// narrow navigator can cut it off.

var collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

/** Names as a file manager orders them: without regard to case, digits by
 *  their value (Part2 before Part10); a plain comparison breaks ties. */
export function compareNames(a, b) {
  a = a || ""; b = b || "";
  return collator.compare(a, b) || (a < b ? -1 : a > b ? 1 : 0);
}

/** The sub-folders of a level by name. */
export function sortFolders(folders) {
  return (folders || []).slice().sort(function (a, b) { return compareNames(a.name, b.name); });
}

/** What a row is known by: its file name, or the item's name without a file. */
export function entryName(e) {
  return e.filename || e.name || e.item_number || "";
}

/** `PRT-000042/B` – the revision a row opens. */
export function entryRevision(e) {
  return (e.item_number || "") + (e.code ? "/" + e.code : "");
}

/** The item type whose icon a row gets: what the file contains when the
 *  format tells (the drawing of a part is a drawing), else the item's type. */
export function entryIconType(e) {
  var byKind = { part: "part", assembly: "assembly", drawing: "document" };
  return byKind[e.format_kind] || e.item_type || "";
}

/** The orders Home offers (navigator.js SORTS): what comes first for each. */
var SORT_KEYS = {
  name: function () { return ""; },
  number: function (e) { return e.item_number || ""; },
  type: function (e) { return entryIconType(e); },
  state: function (e) { return e.state || ""; }
};

/**
 * The entries of a level in the chosen order (`name`, `number`, `type` or
 * `state`; anything else is `name`): the files first, then the items without
 * files; rows that are equal in the order follow by name, then by number.
 */
export function sortEntries(entries, by) {
  var key = SORT_KEYS[by] || SORT_KEYS.name;
  return (entries || []).slice().sort(function (a, b) {
    var ka = key(a), kb = key(b);
    return (a.filename ? 0 : 1) - (b.filename ? 0 : 1) ||
      (ka < kb ? -1 : ka > kb ? 1 : 0) ||
      compareNames(entryName(a), entryName(b)) ||
      compareNames(a.item_number, b.item_number);
  });
}

/** The class a checked-out row gets (issue #362): one marker in one colour,
 *  whoever holds the lock – the text colour stays as it is. `""` for a row
 *  nobody has checked out. */
export function checkedOutClass(e) {
  return e && e.checked_out_by_name ? ".checked-out" : "";
}

/** The preview image of a row (#420), `{ id, sha }` for GET /files/{id}/thumbnail
 *  and the cache, or null: a Files row (folders.rs Entry: `thumbnail`, `file_id`,
 *  `sha256`) or a row of All items, the search and the structure (the main
 *  thumbnail of the revision: `thumbnail_file_id`, `thumbnail_sha256`). */
export function rowThumb(e) {
  if (!e) return null;
  if (e.thumbnail_file_id && e.thumbnail_sha256) return { id: e.thumbnail_file_id, sha: e.thumbnail_sha256 };
  if (e.thumbnail === true && e.file_id && e.sha256) return { id: e.file_id, sha: e.sha256 };
  return null;
}

/** The tooltip of a row: everything the dense row leaves out. */
export function entryTitle(e, labelOf) {
  var parts = [entryRevision(e) + " – " + (e.name || "")];
  if (e.filename) parts.push(e.format_label || "file");
  else parts.push("no files yet");
  if (e.state) parts.push(labelOf ? labelOf(e.state) : e.state);
  if (e.checked_out_by_name) parts.push("checked out by " + e.checked_out_by_name);
  return parts.join(" · ");
}

/** Adds a further page of entries to the ones already shown; a row that both
 *  pages hold (the list changed between two requests) stays once. */
export function mergeEntries(have, more) {
  var seen = {};
  function id(e) { return e.file_id || "item:" + e.item_number; }
  var out = [];
  (have || []).concat(more || []).forEach(function (e) {
    if (seen[id(e)]) return;
    seen[id(e)] = true;
    out.push(e);
  });
  return out;
}

/** Entries not loaded yet. */
export function remaining(total, shown) {
  return Math.max((total || 0) - shown, 0);
}

/** Rows a group has in all: the entries at its top and the files in its folders. */
export function groupTotal(level) {
  return (level.folders || []).reduce(function (n, f) { return n + (f.files || 0); }, level.total || 0);
}

/** The key of a folder in the expanded map. */
export function folderKey(project, path) {
  return "dir:" + project + ":" + (path || "");
}

/** `12 files` / `1 file`. */
export function countText(n, one, many) {
  return n + " " + (n === 1 ? one : many);
}

/** The folder above `path` (`chassis/brackets` → `chassis`, `chassis` → ``). */
export function parentFolder(path) {
  var cut = (path || "").lastIndexOf("/");
  return cut < 0 ? "" : path.slice(0, cut);
}

/** The folder *New item* starts with in group `code`: the one last opened or
 *  clicked in the Files view (`remembered` = state.filesFolder), else the top. */
export function newItemFolder(remembered, code) {
  return (remembered && code && remembered[code]) || "";
}

// ---------------------------------------------------------------- the favourite folder (issue #352)
//
// A group's favourite folder is where Home starts: an engineer whose work is
// several folders down (the folder of the Inventor project file) does not
// open them one by one each time. It is kept per computer, per server and
// group; Home marks it in the accent colour, so it is never taken for the
// top of the group.

/** The storage key of a group's favourite folder on a server. */
export function favouriteKey(server, project) {
  return "teamcraft.home-root:" + JSON.stringify([server || "", project || ""]);
}

/** The storage key that remembers a declined suggestion of a group. */
export function suggestionKey(server, project) {
  return "teamcraft.home-root-hint:" + JSON.stringify([server || "", project || ""]);
}

/** A stored folder as the server writes folders (`Design/Rover`): forward
 *  slashes, none at the ends, no empty or `.` segments; `""` (the top of the
 *  group) for anything else or a `..` segment. */
export function normalizeFavourite(value) {
  if (typeof value !== "string") return "";
  var parts = value.split(/[\\/]/).map(function (s) { return s.trim(); }).filter(function (s) { return s && s !== "."; });
  if (parts.some(function (s) { return s === ".."; })) return "";
  return parts.join("/");
}

/** `ROVER / Design / Rover`: where a favourite folder is in its group. */
export function favouritePath(project, folder) {
  return [project].concat(normalizeFavourite(folder).split("/").filter(Boolean)).join(" / ");
}

/** Suggestions to offer, at most 3: the folders with an Inventor project
 *  file (`project_folders` of the group's top) – none while the group has a
 *  favourite or its suggestion was declined. */
export function favouriteSuggestions(level, favourite, declined) {
  if (favourite || declined || !level) return [];
  var out = [];
  (Array.isArray(level.project_folders) ? level.project_folders : []).forEach(function (f) {
    var path = normalizeFavourite(f);
    if (path && out.indexOf(path) === -1 && out.length < 3) out.push(path);
  });
  return out;
}

/**
 * The revisions whose lock the user took or gave up between two worklists
 * (GET /worklist, `checkouts`): `{ taken: [{item, code}], released: [...] }`.
 * Home marks those rows again in place (#451): a check-out from Home shows
 * its colour at once, not only after leaving Home. Without an earlier
 * worklist nothing is known to have changed.
 */
export function lockChanges(before, after) {
  var out = { taken: [], released: [] };
  if (!before || !after) return out;
  function keyed(w) {
    var m = {};
    (w.checkouts || []).forEach(function (r) { m[r.item_number + "/" + r.code] = { item: r.item_number, code: r.code }; });
    return m;
  }
  var was = keyed(before), now = keyed(after);
  Object.keys(now).forEach(function (k) { if (!was[k]) out.taken.push(now[k]); });
  Object.keys(was).forEach(function (k) { if (!now[k]) out.released.push(was[k]); });
  return out;
}

/**
 * How Home marks its rows after the own check-outs changed (#451): one
 * `{ item, code, holder, holderId }` per revision – a taken lock is held by
 * the user (`me`: display name and id), a released one by nobody (null).
 */
export function lockMarks(before, after, me) {
  var c = lockChanges(before, after);
  return c.taken.map(function (r) {
    return { item: r.item, code: r.code, holder: me ? me.display_name : null, holderId: me ? me.id : null };
  }).concat(c.released.map(function (r) {
    return { item: r.item, code: r.code, holder: null, holderId: null };
  }));
}

/**
 * Whether the lock of a row (a Files entry or a structure node) is the
 * user's own: by the holder's id (`checked_out_by`) – display names are not
 * unique and change when an administrator renames someone. An older server
 * that sends only the name is compared by name.
 */
export function isMineLock(e, me) {
  if (!e || !me) return false;
  if (e.checked_out_by) return e.checked_out_by === me.id;
  return !!e.checked_out_by_name && e.checked_out_by_name === me.display_name;
}

/** The entry of a row after its lock changed in place (browser.js markLock):
 *  held by `holder` (a display name, id `holderId`) or by nobody (null). */
export function relockedEntry(e, holder, holderId) {
  return Object.assign({}, e, { checked_out_by_name: holder || null, checked_out_by: holder ? holderId || null : null });
}
