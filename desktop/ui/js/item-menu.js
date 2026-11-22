// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – right-click menu on items and files (#417). Every row
// that carries `data-item` and `data-rev` (navigator, Home *Files*,
// *Structure*, worklist, Graph cards) opens it, also with Shift+F10 or the
// context-menu key while the row has the focus. The entries and why one is
// disabled: item-menu-rules.js (the toolbar's rules).
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { call, errorText, h, state, toast } from "./core.js";
import { findTab, openTab } from "./tabs.js";
import { checkOutAndOpen, fetchRevision, openCadFile, revisionAbilities, revisionReasons, revisionTitle } from "./revision.js";
import { checkinDialog, doCheckout, getStructureDialog, openWorkspaceFolder } from "./workspace.js";
import { requestChangeDialog } from "./requests.js";
import { groupDefault, mainCadFile } from "./cad-open.js";
import { itemMenuEntries } from "./item-menu-rules.js";

var menuState = null;
var pending = 0;

export function initItemMenu() {
  document.addEventListener("contextmenu", function (e) {
    var el = e.target && e.target.closest ? e.target.closest("[data-item][data-rev]") : null;
    if (!el || !el.dataset.item || !el.dataset.rev || !state.me) return;
    e.preventDefault();
    var x = e.clientX, y = e.clientY;
    // the context-menu key and Shift+F10 report no position: open under the row
    if (!x && !y) { var r = el.getBoundingClientRect(); x = r.left + 16; y = r.bottom; }
    openItemMenu(el.dataset.item, el.dataset.rev, x, y, el);
  });
}

function openItemMenu(number, code, x, y, anchor) {
  closeMenu(false);
  var mine = ++pending;
  document.body.classList.add("menu-loading");
  fetchRevision(number, code).then(function (d) {
    if (mine !== pending) return;
    var cad = mainCadFile(d.files, groupDefault(d.rev).family);
    var entries = itemMenuEntries({
      number: number, can: revisionAbilities(d), why: revisionReasons(d),
      mine: d.rev.checked_out_by === state.me.id, cadFile: cad ? cad.filename : null
    });
    showMenu(number + "/" + code, entries, x, y, anchor, function (id) { runEntry(id, d, cad); });
  }, function (err) {
    if (mine === pending) toast(errorText(err), "error");
  }).then(function () {
    if (mine === pending) document.body.classList.remove("menu-loading");
  });
}

/** The revision's tab, opened (or brought to the front) with the data the
 *  menu already loaded, so it paints at once. */
function revisionTab(d, sub) {
  var key = "rev:" + d.rev.item_number + ":" + d.rev.code;
  if (findTab(key)) openTab({ key: key, sub: sub || null });
  else openTab({ key: key, kind: "revision", number: d.rev.item_number, code: d.rev.code, title: revisionTitle(d), sub: sub || "overview", viewerFile: null, data: d });
  return findTab(key);
}

function runEntry(id, d, cad) {
  var number = d.rev.item_number;
  if (id === "checkout-open") checkOutAndOpen(revisionTab(d), cad);
  else if (id === "open-cad") openCadFile({ data: d }, cad);
  else if (id === "checkout") doCheckout(revisionTab(d));
  else if (id === "checkin") checkinDialog(revisionTab(d));
  else if (id === "get") getStructureDialog(d.rev);
  else if (id === "folder") openWorkspaceFolder({ data: d });
  else if (id === "copy") {
    var copy = navigator.clipboard && navigator.clipboard.writeText ? navigator.clipboard.writeText(number) : Promise.reject(new Error("The clipboard is not available."));
    copy.then(function () { toast("Copied " + number); }, function (e) { toast(errorText(e), "error"); });
  }
  else if (id === "whereused") revisionTab(d, "whereused");
  else if (id === "request") requestChangeDialog(d.rev);
  else if (id === "browser") call("open_web", { path: "/revisions/" + d.rev.id }).catch(function (e) { toast(errorText(e), "error"); });
}

function closeMenu(refocus) {
  if (!menuState) return;
  var s = menuState;
  menuState = null;
  s.menu.remove();
  document.removeEventListener("pointerdown", s.onDoc, true);
  document.removeEventListener("keydown", s.onKey, true);
  window.removeEventListener("blur", s.close);
  window.removeEventListener("resize", s.close);
  if (refocus && s.anchor && s.anchor.isConnected && s.anchor.focus) s.anchor.focus();
}

function showMenu(label, entries, x, y, anchor, onRun) {
  closeMenu(false);
  var menu = h("div.menu.tab-menu.item-menu", { role: "menu", "aria-label": label });
  var buttons = [];
  entries.forEach(function (it) {
    if (!it) { menu.appendChild(h("div.menu-sep", { role: "separator" })); return; }
    // a disabled entry keeps its place and says why in its tooltip
    var b = h("button.menu-item", {
      type: "button", role: "menuitem", title: it.title || it.text, "aria-disabled": it.off ? "true" : null,
      on: { click: function () { if (it.off) return; closeMenu(false); onRun(it.id); } }
    }, h("span.menu-label", { text: it.text }));
    buttons.push(b);
    menu.appendChild(b);
  });
  document.body.appendChild(menu);
  var w = menu.offsetWidth, hgt = menu.offsetHeight;
  menu.style.left = Math.max(4, Math.min(x, window.innerWidth - w - 4)) + "px";
  menu.style.top = Math.max(4, Math.min(y, window.innerHeight - hgt - 4)) + "px";
  var s = menuState = {
    menu: menu, anchor: anchor,
    close: function () { closeMenu(false); },
    onDoc: function (e) { if (!menu.contains(e.target)) closeMenu(false); },
    onKey: function (e) {
      var at = buttons.indexOf(document.activeElement), n = buttons.length;
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); closeMenu(true); }
      else if (e.key === "ArrowDown") { e.preventDefault(); e.stopPropagation(); buttons[(at + 1) % n].focus(); }
      else if (e.key === "ArrowUp") { e.preventDefault(); e.stopPropagation(); buttons[(at - 1 + n) % n].focus(); }
      else if (e.key === "Home") { e.preventDefault(); e.stopPropagation(); buttons[0].focus(); }
      else if (e.key === "End") { e.preventDefault(); e.stopPropagation(); buttons[n - 1].focus(); }
      else if (e.key === "Tab") { e.preventDefault(); closeMenu(false); }
    }
  };
  document.addEventListener("pointerdown", s.onDoc, true);
  document.addEventListener("keydown", s.onKey, true);
  window.addEventListener("blur", s.close);
  window.addEventListener("resize", s.close);
  // the first enabled entry has the focus; disabled ones stay reachable with
  // the arrow keys, so their reason (tooltip) can be read
  var first = buttons.filter(function (b) { return b.getAttribute("aria-disabled") !== "true"; })[0] || buttons[0];
  if (first) first.focus();
}
