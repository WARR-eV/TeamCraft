// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – layout of the check-out graph of a project group
// (#428): pure, no DOM. The same rules and sizes as the server's
// `lock_overview::graph` (src/services/lock_overview.rs), so web and desktop
// draw the same picture: holders on the left, centred on their revisions,
// one row per checked-out revision on the right, a curved line between.
// Tested by scripts/desktop-lock-layout.test.mjs.

export var ROW = 34;
export var PAD = 12;
export var NODE_H = 26;
export var HOLDER_X = 8;
export var HOLDER_W = 190;
export var LOCK_X = 300;
export var LOCK_W = 330;
export var LABEL_MAX = 40;

/** How long a lock has been held: "today" (< 24 h), "week" (< 7 days), "old". */
export function ageClass(checkedOutAt, now) {
  var hours = Math.floor((now - checkedOutAt) / 3600000);
  return hours < 24 ? "today" : hours < 24 * 7 ? "week" : "old";
}

/** A label cut to LABEL_MAX characters, ending in "…". */
export function cutLabel(text) {
  var chars = Array.from(text || "");
  if (chars.length <= LABEL_MAX) return chars.join("");
  return chars.slice(0, LABEL_MAX - 1).join("").trimEnd() + "…";
}

/**
 * The graph of an overview (GET /projects/{code}/checkouts); `now` in ms.
 * Returns { width, height, holders: [{ y, label, count, active }],
 * locks: [{ y, number, code, label, sub, age, edge }] }.
 */
export function lockGraph(overview, now) {
  var holders = [], locks = [], row = 0;
  (overview.holders || []).forEach(function (h) {
    var n = h.locks.length;
    var first = PAD + row * ROW, last = PAD + (row + Math.max(n - 1, 0)) * ROW;
    var hy = Math.floor((first + last) / 2);
    h.locks.forEach(function (l) {
      var y = PAD + row * ROW;
      var y1 = hy + NODE_H / 2, y2 = y + NODE_H / 2;
      var x1 = HOLDER_X + HOLDER_W, x2 = LOCK_X, mid = Math.floor((x1 + x2) / 2);
      locks.push({
        y: y, number: l.item_number, code: l.code,
        label: cutLabel(l.item_number + "/" + l.code + " " + l.name),
        sub: l.item_type,
        age: ageClass(Date.parse(l.checked_out_at), now),
        edge: "M" + x1 + "," + y1 + " C" + mid + "," + y1 + " " + mid + "," + y2 + " " + x2 + "," + y2
      });
      row++;
    });
    holders.push({ y: hy, label: cutLabel(h.name), count: n, active: h.active });
  });
  return {
    width: LOCK_X + LOCK_W + HOLDER_X,
    height: row === 0 ? 0 : 2 * PAD + (row - 1) * ROW + NODE_H,
    holders: holders, locks: locks
  };
}
