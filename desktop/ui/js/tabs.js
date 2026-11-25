// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – Object tabs and the toolbar.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { $, api, append, clear, errorNotice, errorText, h, state, tool } from "./core.js";
import { tabInputs } from "./report-inputs.js";
import { loadLocks, renderLocks } from "./lock-overview.js";
import { fmtDate, fmtSize, label } from "/static/shared/js/format.js";
import { canEdit, refreshWorklistCount } from "./shell.js";
import { emptyHint, selectView, serverUrl } from "./navigator.js";
import { rememberRecent, worklistSummary } from "./home-rules.js";
import { t } from "/static/shared/js/i18n.js";
import { assistantContextChanged } from "./assistant.js";
import { agent, renderAgentTab, schedulePaint } from "./agent.js";
import { loadRevision, openRevision, renderRevision, revisionTitle } from "./revision.js";
import { disposeViewer } from "./workspace.js";
import { loadChange, openChange, renderChange } from "./change.js";
import { loadRequest, openRequest, renderRequest } from "./requests.js";
import { newItemDialog } from "./dialogs.js";
import { dropSlot, keysToClose, moveBy, moveItem, stepIndex } from "./tab-order.js";

// ======================================================================
// Tabs
// ======================================================================

export function findTab(key) { return state.tabs.filter(function (t) { return t.key === key; })[0]; }

export function openTab(tab) {
  noteRecent(tab.kind ? tab : findTab(tab.key));
  var existing = findTab(tab.key);
  if (!existing) state.tabs.push(tab);
  else {
    if (tab.sub) existing.sub = tab.sub;
    if (tab.viewerFile) { existing.viewerFile = tab.viewerFile; existing.viewerVersion = null; }
  }
  activate(tab.key);
}

/**
 * Points an open revision tab at another revision, in place: the tab keeps
 * its position in the strip and its trail (graph.js), everything else is the
 * new revision's. `data` is that revision's payload (revision.js
 * fetchRevision), loaded beforehand so the switch paints once, without a
 * "Loading" state in between. A tab that already shows the target gives way:
 * there is one tab per revision.
 */
export function retargetTab(tab, data, sub) {
  var key = "rev:" + data.rev.item_number + ":" + data.rev.code;
  var wasActive = state.active === tab.key;
  var twin = state.tabs.findIndex(function (t) { return t.key === key && t !== tab; });
  if (twin !== -1) state.tabs.splice(twin, 1);
  if (agent.contextKey === tab.key) agent.contextKey = key;
  tab.key = key;
  tab.number = data.rev.item_number;
  tab.code = data.rev.code;
  tab.title = revisionTitle(data);
  tab.sub = sub || "overview";
  tab.data = data;
  tab.viewerFile = null;
  tab.viewerVersion = null;
  tab.recheckLocal = null;
  if (wasActive) activate(key);
  else renderTabs();
}

export function activate(key) {
  if (key === HOME_KEY) { showHome(); return; }
  state.active = key;
  var t = findTab(key);
  if (t && (t.kind === "revision" || t.kind === "change" || t.kind === "request")) agent.contextKey = key;
  // a Check-outs tab shows the current state each time it comes to the front (#451)
  if (t && t.kind === "locks") t.data = null;
  if (state.view === "agent") schedulePaint(true);
  renderTabs();
  renderActive();
  markSelection();
}

export function markSelection() {
  // re-render cheap navigator selection markers
  document.querySelectorAll("#navigator .node.selected").forEach(function (n) { n.classList.remove("selected"); });
}

export function closeTab(key) {
  var i = state.tabs.findIndex(function (t) { return t.key === key; });
  if (i === -1) return;
  var wasActive = state.active === key;
  state.tabs.splice(i, 1);
  disposeViewer();
  if (wasActive) {
    // the neighbour comes to the front like any other tab (a Check-outs tab
    // loads again, #451; the agent's context follows)
    var next = state.tabs[i] || state.tabs[i - 1];
    if (next) { activate(next.key); return; }
    state.active = null;
  }
  renderTabs();
  renderActive();
}

/** The key of the pinned Home tab (#513 T1): the dashboard. It is no entry
 *  of state.tabs – Home is active whenever no object tab is. */
export var HOME_KEY = "home";

/** Brings the pinned Home tab (the dashboard) to the front. */
export function showHome() {
  state.active = null;
  renderTabs();
  renderActive();
  markSelection();
}

export function renderTabs() {
  var bar = $("tabs");
  wireTabStrip(bar);
  // a re-render replaces the elements: keep the focus on the same tab
  var focused = document.activeElement && bar.contains(document.activeElement) && document.activeElement.dataset
    ? document.activeElement.dataset.key : null;
  clear(bar);
  // roving tabindex: one tab is a tab stop, the arrow keys move between them
  var homeActive = !findTab(state.active);
  var stop = homeActive ? HOME_KEY : state.active;
  // the pinned Home tab (#513 T1): always first, cannot be closed or moved
  var home = h("div.tab.home-tab" + (homeActive ? ".active" : ""), {
    role: "tab", "aria-selected": homeActive ? "true" : "false", title: "Home – your overview",
    tabindex: stop === HOME_KEY ? "0" : "-1", "data-key": HOME_KEY,
    on: { click: function () { if (!homeActive) showHome(); } }
  }, [h("span.ico", { "aria-hidden": "true", text: "⌂" }), h("span.tab-label", { text: "Home" })]);
  bar.appendChild(home);
  if (focused === HOME_KEY) home.focus({ preventScroll: true });
  state.tabs.forEach(function (t) {
    var isActive = t.key === state.active;
    var el = h("div.tab" + (isActive ? ".active" : ""), {
      role: "tab", "aria-selected": isActive ? "true" : "false", title: t.title,
      tabindex: t.key === stop ? "0" : "-1", "data-key": t.key,
      on: { click: function () { if (state.active !== t.key) activate(t.key); } }
    }, [
      h("span.ico", { text: t.kind === "change" ? "⇄" : t.kind === "request" ? "✎" : t.kind === "agent" ? "✦" : t.kind === "locks" ? "⚿" : "◆" }),
      h("span.tab-label", { text: t.title }),
      h("button.close", { type: "button", tabindex: "-1", title: "Close (Ctrl+W)", "aria-label": "Close " + t.title, on: { click: function (e) { e.stopPropagation(); closeTab(t.key); } } }, "×")
    ]);
    bar.appendChild(el);
    if (isActive && el.scrollIntoView) el.scrollIntoView({ block: "nearest", inline: "nearest" });
    if (t.key === focused) el.focus({ preventScroll: true });
  });
}

// ---------------------------------------------------------------- order of the tabs

/** The object tabs in the strip – without the pinned Home tab, which is not
 *  moved, dropped on or closed. */
function tabElements() { return Array.prototype.slice.call($("tabs").querySelectorAll(".tab:not(.home-tab)")); }

/** Every tab of the strip, Home first: the order of the arrow keys. */
function allTabElements() { return Array.prototype.slice.call($("tabs").querySelectorAll(".tab")); }

function tabElement(key) {
  return tabElements().filter(function (el) { return el.dataset.key === key; })[0] || null;
}

/** Puts state.tabs in the order of `list` (same entries; the array stays the same object). */
function applyOrder(list) {
  if (list === state.tabs) return;
  Array.prototype.splice.apply(state.tabs, [0, state.tabs.length].concat(list));
  renderTabs();
}

/** Moves the active tab one place; the tab keeps its state and stays active. */
export function moveActiveTab(delta) {
  var i = state.tabs.findIndex(function (t) { return t.key === state.active; });
  applyOrder(moveBy(state.tabs, i, delta));
}

/** Activates the previous or next tab (wrapping). */
export function switchTab(delta) {
  var i = state.tabs.findIndex(function (t) { return t.key === state.active; });
  var j = stepIndex(i, state.tabs.length, delta);
  if (j !== -1 && state.tabs[j].key !== state.active) activate(state.tabs[j].key);
}

/** Ctrl+PageUp/PageDown or Ctrl+Tab / Ctrl+Shift+Tab switch, Ctrl+Shift+PageUp/PageDown move the active tab. */
export function tabShortcut(e) {
  if (!e.ctrlKey || e.altKey || e.metaKey) return false;
  var page = e.key === "PageUp" ? -1 : e.key === "PageDown" ? 1 : 0;
  if (page && e.shiftKey) moveActiveTab(page);
  else if (page) switchTab(page);
  else if (e.key === "Tab") switchTab(e.shiftKey ? -1 : 1);
  else return false;
  e.preventDefault();
  return true;
}

/** Closes several tabs at once; `anchor` becomes active when the active one is among them. */
function closeMany(keys, anchor) {
  if (!keys.length) return;
  var activeGone = keys.indexOf(state.active) !== -1;
  var keep = state.tabs.filter(function (t) { return keys.indexOf(t.key) === -1; });
  Array.prototype.splice.apply(state.tabs, [0, state.tabs.length].concat(keep));
  if (activeGone) activate(anchor);
  else renderTabs();
}

var DRAG_THRESHOLD = 5;   // px before a press becomes a drag; below it the click activates
var EDGE = 36;            // px from the strip's edge where it scrolls while dragging
var drag = null;

/** The drop slot under the pointer; the strip is never re-rendered while dragging. */
function currentSlot() {
  var mids = tabElements().map(function (el) {
    var r = el.getBoundingClientRect();
    return r.left + r.width / 2;
  });
  return dropSlot(mids, drag.x);
}

function showMarker() {
  var els = tabElements();
  els.forEach(function (el) { el.classList.remove("drop-before", "drop-after"); });
  var from = state.tabs.findIndex(function (t) { return t.key === drag.key; });
  var slot = currentSlot();
  drag.slot = slot;
  // the own slot and the one behind it change nothing: no marker
  if (slot === from || slot === from + 1) return;
  if (slot < els.length) els[slot].classList.add("drop-before");
  else if (els.length) els[els.length - 1].classList.add("drop-after");
}

function autoScroll() {
  if (!drag || !drag.moving) return;
  var bar = $("tabs");
  var r = bar.getBoundingClientRect();
  var speed = 0;
  if (drag.x < r.left + EDGE) speed = -Math.ceil((r.left + EDGE - drag.x) / 4);
  else if (drag.x > r.right - EDGE) speed = Math.ceil((drag.x - (r.right - EDGE)) / 4);
  if (speed) { bar.scrollLeft += Math.max(-24, Math.min(24, speed)); showMarker(); }
  drag.raf = requestAnimationFrame(autoScroll);
}

function endDrag(apply) {
  if (!drag) return;
  var d = drag;
  drag = null;
  if (d.raf) cancelAnimationFrame(d.raf);
  var bar = $("tabs");
  try { if (d.moving) bar.releasePointerCapture(d.id); } catch (_) { /* already released */ }
  bar.classList.remove("reordering");
  tabElements().forEach(function (el) { el.classList.remove("dragging", "drop-before", "drop-after"); });
  if (apply && d.moving) {
    var keys = state.tabs.map(function (t) { return t.key; });
    var order = moveItem(keys, keys.indexOf(d.key), d.slot);
    if (order !== keys) applyOrder(order.map(findTab));
  }
}

function onTabPointerDown(e) {
  if (e.button !== 0 || e.isPrimary === false) return;
  var el = e.target.closest(".tab");
  if (!el || el.classList.contains("home-tab") || e.target.closest(".close")) return;
  closeTabMenu();
  drag = { id: e.pointerId, key: el.dataset.key, x0: e.clientX, x: e.clientX, moving: false, slot: -1, raf: 0 };
}

function onTabPointerMove(e) {
  if (!drag || e.pointerId !== drag.id) return;
  drag.x = e.clientX;
  if (!drag.moving) {
    if (Math.abs(e.clientX - drag.x0) < DRAG_THRESHOLD) return;
    drag.moving = true;
    // all later pointer events go to the strip, also outside the window
    $("tabs").setPointerCapture(e.pointerId);
    $("tabs").classList.add("reordering");
    var el = tabElement(drag.key);
    if (el) el.classList.add("dragging");
    drag.raf = requestAnimationFrame(autoScroll);
  }
  showMarker();
}

function onTabKeyDown(e) {
  if (e.key === "Escape" && drag) { e.preventDefault(); endDrag(false); return; }
  var el = e.target.closest ? e.target.closest(".tab") : null;
  if (!el || e.target !== el) return;
  var els = allTabElements();
  var i = els.indexOf(el);
  var to = -1;
  if (!e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
    if (e.key === "ArrowRight") to = (i + 1) % els.length;
    else if (e.key === "ArrowLeft") to = (i - 1 + els.length) % els.length;
    else if (e.key === "Home") to = 0;
    else if (e.key === "End") to = els.length - 1;
  }
  if (to !== -1) {
    e.preventDefault();
    els.forEach(function (t) { t.setAttribute("tabindex", "-1"); });
    els[to].setAttribute("tabindex", "0");
    els[to].focus();
  } else if ((e.key === "Enter" || e.key === " ") && !e.ctrlKey && !e.altKey && !e.metaKey) {
    e.preventDefault();
    if (state.active !== el.dataset.key) activate(el.dataset.key);
  }
}

/** Listeners of the strip itself (it is never replaced, only its children are): once. */
function wireTabStrip(bar) {
  if (bar.dataset.wired) return;
  bar.dataset.wired = "1";
  bar.setAttribute("aria-label", "Open objects");
  bar.addEventListener("pointerdown", onTabPointerDown);
  bar.addEventListener("pointermove", onTabPointerMove);
  bar.addEventListener("pointerup", function (e) { if (drag && e.pointerId === drag.id) endDrag(true); });
  bar.addEventListener("pointercancel", function () { endDrag(false); });
  bar.addEventListener("lostpointercapture", function () { endDrag(false); });
  bar.addEventListener("keydown", onTabKeyDown);
  bar.addEventListener("focusin", function (e) {
    // the tab that has the focus is the tab stop
    var el = e.target.closest ? e.target.closest(".tab") : null;
    if (!el) return;
    allTabElements().forEach(function (t) { t.setAttribute("tabindex", t === el ? "0" : "-1"); });
  });
  bar.addEventListener("contextmenu", function (e) {
    var el = e.target.closest(".tab");
    if (!el || el.classList.contains("home-tab")) return;
    e.preventDefault();
    var x = e.clientX, y = e.clientY;
    // the context-menu key reports no position: open under the tab
    if (!x && !y) { var r = el.getBoundingClientRect(); x = r.left; y = r.bottom; }
    openTabMenu(el.dataset.key, x, y);
  });
}

// ---------------------------------------------------------------- context menu

var tabMenuState = null;

function closeTabMenu() {
  if (!tabMenuState) return;
  var s = tabMenuState;
  tabMenuState = null;
  s.menu.remove();
  document.removeEventListener("pointerdown", s.onDoc, true);
  document.removeEventListener("keydown", s.onKey, true);
  window.removeEventListener("blur", s.close);
  window.removeEventListener("resize", s.close);
  if (s.refocus) { var el = tabElement(s.key); if (el) el.focus(); }
}

function openTabMenu(key, x, y) {
  closeTabMenu();
  var keys = state.tabs.map(function (t) { return t.key; });
  var i = keys.indexOf(key);
  if (i === -1) return;
  var items = [
    { text: "Move to start", off: i === 0, run: function () { applyOrder(moveItem(state.tabs, i, 0)); } },
    { text: "Move to end", off: i === keys.length - 1, run: function () { applyOrder(moveItem(state.tabs, i, keys.length)); } },
    null,
    { text: "Close", run: function () { closeTab(key); } },
    { text: "Close others", off: keys.length < 2, run: function () { closeMany(keysToClose(keys, key, "others"), key); } },
    { text: "Close tabs to the right", off: i === keys.length - 1, run: function () { closeMany(keysToClose(keys, key, "right"), key); } }
  ];
  var menu = h("div.menu.tab-menu", { role: "menu", "aria-label": "Tab" });
  var buttons = [];
  items.forEach(function (it) {
    if (!it) { menu.appendChild(h("div.menu-sep", { role: "separator" })); return; }
    var b = h("button.menu-item", {
      type: "button", role: "menuitem", "aria-disabled": it.off ? "true" : null,
      on: { click: function () { if (it.off) return; closeTabMenu(); it.run(); } }
    }, h("span.menu-label", { text: it.text }));
    buttons.push(b);
    menu.appendChild(b);
  });
  document.body.appendChild(menu);
  var w = menu.offsetWidth, hgt = menu.offsetHeight;
  menu.style.left = Math.max(4, Math.min(x, window.innerWidth - w - 4)) + "px";
  menu.style.top = Math.max(4, Math.min(y, window.innerHeight - hgt - 4)) + "px";
  var s = tabMenuState = {
    menu: menu, key: key, refocus: true,
    close: function () { if (tabMenuState) tabMenuState.refocus = false; closeTabMenu(); },
    onDoc: function (e) { if (!menu.contains(e.target)) { s.refocus = false; closeTabMenu(); } },
    onKey: function (e) {
      var enabled = buttons.filter(function (b) { return b.getAttribute("aria-disabled") !== "true"; });
      var at = enabled.indexOf(document.activeElement);
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); closeTabMenu(); }
      else if (e.key === "ArrowDown") { e.preventDefault(); e.stopPropagation(); enabled[(at + 1) % enabled.length].focus(); }
      else if (e.key === "ArrowUp") { e.preventDefault(); e.stopPropagation(); enabled[(at - 1 + enabled.length) % enabled.length].focus(); }
      else if (e.key === "Tab") { e.preventDefault(); s.refocus = false; closeTabMenu(); }
    }
  };
  document.addEventListener("pointerdown", s.onDoc, true);
  document.addEventListener("keydown", s.onKey, true);
  window.addEventListener("blur", s.close);
  window.addEventListener("resize", s.close);
  var first = buttons.filter(function (b) { return b.getAttribute("aria-disabled") !== "true"; })[0];
  if (first) first.focus();
}

export function activeTab() { return findTab(state.active); }

export function renderActive() {
  disposeViewer();
  assistantContextChanged();
  var body = $("tab-body");
  var toolbar = $("toolbar");
  clear(body);
  clear(toolbar);
  var tab = activeTab();
  if (!tab) {
    renderWelcome(body, toolbar);
    return;
  }
  if (tab.kind === "agent") {
    renderAgentTab(body);
    return;
  }
  body.appendChild(h("p.loading", { text: "Loading " + tab.title }));
  var load = tab.kind === "change" ? loadChange(tab) : tab.kind === "request" ? loadRequest(tab) :
    tab.kind === "locks" ? loadLocks(tab) : loadRevision(tab);
  load.then(function () {
    if (state.active !== tab.key) return;
    clear(body);
    clear(toolbar);
    if (tab.kind === "change") renderChange(tab, body, toolbar);
    else if (tab.kind === "request") renderRequest(tab, body, toolbar);
    else if (tab.kind === "locks") renderLocks(tab, body, toolbar);
    else renderRevision(tab, body, toolbar);
    if (state.view === "agent" && agent.contextKey === tab.key) schedulePaint(true);
  }).catch(function (err) {
    // a failed load or a bug while rendering: never leave an empty tab
    if (state.active !== tab.key) return;
    clear(body);
    // a render error may have left half a toolbar whose buttons act on
    // data the tab no longer shows
    clear(toolbar);
    body.appendChild(errorNotice("p.notice.error", errorText(err), "Opening " + tab.title, tabInputs(tab)));
  });
}

export function reloadActive() {
  var tab = activeTab();
  if (tab) tab.data = null;
  renderActive();
}

// ---------------------------------------------------------------- recently opened

function recentKey() { return "tc.recent:" + serverUrl(); }

/** The objects opened lately on this computer, for this server (Home). */
export function recentList() {
  var list = [];
  try { list = JSON.parse(window.localStorage.getItem(recentKey()) || "[]"); } catch (e) { list = []; }
  return rememberRecent(list, null);
}

function noteRecent(tab) {
  if (!tab) return;
  var next = rememberRecent(recentList(), tab);
  try { window.localStorage.setItem(recentKey(), JSON.stringify(next)); } catch (e) { /* storage off: no list */ }
}

function openRecent(e) {
  if (e.kind === "change") openChange(e.number);
  else if (e.kind === "request") openRequest(e.number);
  else openRevision(e.number, e.code);
}

// ---------------------------------------------------------------- Home

/**
 * The pinned Home tab (#513 T1, T9): what the navigator beside it cannot
 * show – the worklist only as counts that open the Worklist, the product
 * counts, the objects opened lately on this computer and the recent activity
 * on the server (GET /activity). The worklist itself is the Worklist view.
 */
export function renderWelcome(body, toolbar) {
  append(toolbar, [
    canEdit() ? tool("New item", "plus", function () { newItemDialog(); }, { primary: true }) : null,
    tool("Refresh", "refresh", function () { selectView(state.view); refreshWorklistCount(); renderActive(); })
  ]);
  var w = state.worklist;
  var search = /Mac/.test(navigator.platform) ? "⌘F" : "Ctrl+F";
  var files = /Mac/.test(navigator.platform) ? "⌘1" : "Ctrl+1";
  var summary = h("p.home-summary", { role: "status" });
  var welcome = h("div.welcome", null, [
    h("h1", { text: "Home" }),
    h("p.muted", { text: "Find your files in Files (" + files + ") or press " + search + " to search." }),
    // why a viewer account has no editing buttons, like the web dashboard (#423)
    canEdit() ? null : h("p.small.dim.viewer-note", { text: t("empty.viewer_note") }),
    summary
  ]);
  body.appendChild(welcome);
  var columns = h("div.home-columns");
  var recent = recentList();
  append(columns, h("section.panel", null, [h("div.panel-head", null, [panelTitle("Recently opened", recent.length)]),
    recent.length ? table(["Opened", "Kind"], recent.map(function (e) {
      var open = function () { openRecent(e); };
      return { cells: [recordLink(e.title, open), e.kind === "change" ? "Change order" : e.kind === "request" ? "Change request" : "Revision"],
        open: open, single: true };
    })) : emptyHint("What you open here – revisions, changes, requests – is listed for a quick way back.")]));
  var activity = h("section.panel", null, [h("div.panel-head", null, [h("h2", { text: "Recent activity" })]),
    h("p.small.dim", { text: "Loading…" })]);
  append(columns, activity);
  api("GET", "/activity?limit=12").then(function (entries) {
    clear(activity);
    append(activity, h("div.panel-head", null, [h("h2", { text: "Recent activity" })]));
    append(activity, entries && entries.length ? table(["When", "Who", "What"], entries.map(function (a) {
      return { cells: [h("span.nowrap", { text: fmtDate(a.at) }),
        h("span", { title: a.username, text: a.user_display_name || a.username }), a.summary] };
    })) : emptyHint("No activity yet."));
  }, function () {
    // an older server has no /activity: Home goes without
    activity.remove();
  });
  var fill = function (wl) {
    if (!wl) return;
    clear(summary);
    var parts = worklistSummary(wl);
    if (!parts.length) summary.appendChild(document.createTextNode("Nothing waits for you right now."));
    parts.forEach(function (p, i) {
      if (i) summary.appendChild(document.createTextNode(" · "));
      summary.appendChild(h("button.link", { type: "button", title: "Open the Worklist", on: { click: function () { selectView("worklist"); } } }, p.text));
    });
    var s = wl.stats;
    welcome.insertBefore(h("div.stats", null, [
      stat(s.items, "Items"), stat(s.revisions_in_work, "WIP"), stat(s.revisions_in_review, "In Review"),
      stat(s.revisions_released, "Released"), stat(s.open_changes, "Open changes"), stat(fmtSize(s.stored_bytes), s.files + " files in the vault")
    ]), columns);
  };
  append(welcome, columns);
  if (w) fill(w);
  else refreshWorklistCount().then(function (wl) { if (wl) fill(wl); }, function () {});
}

/** A Home card title with its count as a muted trailing number, like the
 *  web dashboard's card titles and the worklist navigator. */
export function panelTitle(text, n) {
  return h("h2", null, [text + " ", h("span.count", { text: String(n) })]);
}

/** A record number in a table: a link like in the web UI and the dependents
 *  rows, so every number that opens something looks the same. */
export function recordLink(text, open) {
  return h("button.link.mono", { type: "button", on: { click: function (e) { e.stopPropagation(); open(); } } }, text);
}

export function stat(value, text) {
  return h("div.stat", null, [h("div.value", { text: String(value) }), h("div.label", { text: text })]);
}

/** rows: [{cells: [...], open: fn, cls: "selected"}] */
export function table(headers, rows, opts) {
  opts = opts || {};
  return h("div.table-wrap", null, h("table.grid", null, [
    h("thead", null, h("tr", null, headers.map(function (t, i) {
      // a header is text or an element (e.g. a label with a help tip)
      var text = typeof t === "string";
      return h("th" + (opts.numeric && opts.numeric.indexOf(i) !== -1 ? ".num" : ""), text ? { text: t } : null, text ? null : t);
    }))),
    h("tbody", null, rows.map(function (r) {
      return h("tr" + (r.open ? ".clickable" : "") + (r.cls ? "." + r.cls : ""), {
        on: r.open ? { dblclick: r.open, click: r.single ? r.open : null } : null,
        title: r.open ? "Double-click to open" : null
      }, r.cells.map(function (c, i) {
        var td = h("td" + (opts.numeric && opts.numeric.indexOf(i) !== -1 ? ".num" : "") +
          (opts.nowrap && opts.nowrap.indexOf(i) !== -1 ? ".nowrap" : ""));
        append(td, c);
        return td;
      }));
    }))
  ]));
}
