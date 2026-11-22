// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – the entries of the right-click menu of an item (#417):
// pure rules, no DOM; tested by scripts/desktop-item-menu.test.mjs.
// What is enabled, and the tooltip saying why not, come from the same
// abilities and reasons as the revision toolbar (revision.js
// revisionAbilities / revisionReasons), so menu and toolbar never disagree.

var NO_CAD_FILE = "This revision has no native CAD file (part, assembly or drawing of one CAD program)";

/**
 * The menu of one revision. `input`: `number` (item number), `can` / `why`
 * (revisionAbilities / revisionReasons), `mine` (checked out by me),
 * `cadFile` (the file name *Check out and open* would open, or null).
 * Returns entries `{ id, text, off, title }`; `null` is a separator.
 */
export function itemMenuEntries(input) {
  var can = input.can || {}, why = input.why || {}, cad = input.cadFile || null;
  var entries = [];
  if (input.mine) {
    entries.push({ id: "open-cad", text: "Open in CAD", off: !cad,
      title: cad ? "Open " + cad + " from your workspace in the CAD program" : NO_CAD_FILE });
    entries.push({ id: "checkin", text: "Check in…", off: !can.checkin,
      title: can.checkin ? "Upload the changed files from your workspace and end your check-out" : why.checkin });
  } else {
    entries.push({ id: "checkout-open", text: "Check out and open", off: !can.checkout || !cad,
      title: !can.checkout ? why.checkout : cad ? "Take the lock, copy the files into your workspace and open " + cad : NO_CAD_FILE });
    entries.push({ id: "checkout", text: "Check out", off: !can.checkout,
      title: can.checkout ? "Take the lock and copy the files into your workspace" : why.checkout });
  }
  entries.push({ id: "get", text: "Get…", off: false,
    title: "Copy the item with everything it consists of into your workspace (read-only)" });
  entries.push(null);
  entries.push({ id: "folder", text: "Show in folder", off: false, title: "Open the item's folder in the working folder" });
  entries.push({ id: "copy", text: "Copy number", off: false, title: "Copy " + input.number + " to the clipboard" });
  entries.push(null);
  entries.push({ id: "whereused", text: "Where used", off: false, title: "The assemblies that use " + input.number });
  entries.push({ id: "request", text: "Request change…", off: false, title: "Report a problem or propose a change (change request)" });
  entries.push({ id: "browser", text: "Open in browser", off: false, title: "Open the revision page in the web UI" });
  return entries;
}
