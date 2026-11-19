/* SPDX-License-Identifier: MPL-2.0 */
// Labels, numbers, sizes, ages and dates as both apps show them.
//
// The server renders the same values in Rust (filters in src/templates.rs,
// src/util.rs, domain::format_age); both are pinned to the test vectors in
// shared/test-vectors/labels.json and format.json (src/ui_vectors.rs,
// scripts/shared-vectors.test.mjs). Change a rule in both places, or the
// vectors fail.

var MISSING = "—";

/** Codes with a fixed label; every other code is title-cased word by word. */
export var LABELS = { in_work: "WIP", editor: "CAD editor" };

/** `in_review` → `In Review`, `in_work` → `WIP`; "" for none. */
export function label(code) {
  if (code === null || code === undefined || code === "") return "";
  code = String(code);
  if (Object.prototype.hasOwnProperty.call(LABELS, code)) return LABELS[code];
  return code.split("_").filter(function (w) { return w; })
    .map(function (w) { return w.charAt(0).toUpperCase() + w.slice(1); }).join(" ");
}

/** Up to `digits` decimals (default 3), trailing zeros dropped, no grouping. */
export function fmtNum(v, digits) {
  if (v === null || v === undefined || v === "") return MISSING;
  var n = Number(v);
  if (!isFinite(n)) return MISSING;
  var s = n.toFixed(digits === undefined ? 3 : digits);
  return s.indexOf(".") < 0 ? s : s.replace(/0+$/, "").replace(/\.$/, "");
}

/** Exactly `digits` decimals (default 2), so a column lines up. */
export function fmtFixed(v, digits) {
  if (v === null || v === undefined || v === "") return MISSING;
  var n = Number(v);
  if (!isFinite(n)) return MISSING;
  return n.toFixed(digits === undefined ? 2 : digits);
}

/** A byte count in binary units: `1023 B`, `1.5 KiB`, `1 TiB`. */
export function fmtSize(bytes) {
  if (bytes === null || bytes === undefined) return MISSING;
  var units = ["B", "KiB", "MiB", "GiB", "TiB"];
  var v = Math.max(0, Number(bytes)), i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return (i === 0 ? String(bytes) : fmtNum(v, 1)) + " " + units[i];
}

/** An age in seconds: `just now`, `5 min`, `3 h`, `1 day`, `4 days`. */
export function fmtAge(seconds) {
  var s = Math.max(0, Math.floor(Number(seconds) || 0));
  if (s < 60) return "just now";
  if (s < 3600) return Math.floor(s / 60) + " min";
  if (s < 86400) return Math.floor(s / 3600) + " h";
  if (s < 172800) return "1 day";
  return Math.floor(s / 86400) + " days";
}

/** An RFC 3339 timestamp in the viewer's local time zone; the text itself
 * when it is no date. (The server writes UTC and the web UI rewrites
 * <time> elements into local time the same way.) */
export function fmtDate(iso) {
  if (!iso) return MISSING;
  var d = new Date(iso);
  if (isNaN(d.getTime())) return String(iso);
  return d.toLocaleString(undefined, { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}
