// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – *Check-outs* tab of a project group (#428): who holds
// which lock, as a graph (layout: lock-layout.js, the web page's rules) and
// a list per holder. For administrators and the group's managers: the
// server decides, GET /me `managed_projects` says where to offer it.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { $, api, append, call, clear, enc, errorText, h, state, toast, tool, toolGroup } from "./core.js";
import { fmtDate, fmtAge, label } from "/static/shared/js/format.js";
import { activeTab, openTab, reloadActive } from "./tabs.js";
import { openRevision } from "./revision.js";
import { HOLDER_W, HOLDER_X, LOCK_W, LOCK_X, NODE_H, lockGraph } from "./lock-layout.js";
import { acceptSeq, forgetHiddenLocks, locksDiffer, shouldPoll, takeSeq } from "./lock-rules.js";

var SVG_NS = "http://www.w3.org/2000/svg";

/** Whether the user may see a group's overview: administrators, managers. */
export function canSeeLocks(project) {
  if (!state.me || !project) return false;
  return state.me.role === "admin" || (state.me.managed_projects || []).indexOf(project.id) !== -1;
}

export function openLockOverview(code) {
  watchLocks();
  openTab({ key: "locks:" + code, kind: "locks", code: code, title: "Check-outs " + code });
}

function fetchLocks(code) {
  return api("GET", "/projects/" + enc(code) + "/checkouts");
}

/** The overview of a tab. Each time the tab is shown it is loaded again
 *  (tabs.js activate drops `tab.data`, #451): opening it shows the current
 *  state, not the one of the first visit. Every fetch of a tab is numbered
 *  (lock-rules.js takeSeq): an answer older than one already applied – a
 *  slow poll after *Refresh* – is dropped. */
export function loadLocks(tab) {
  if (tab.data) return Promise.resolve(tab.data);
  var my = takeSeq(tab);
  return fetchLocks(tab.code).then(function (o) {
    if (acceptSeq(tab, my)) tab.data = o;
    return tab.data || o;
  });
}

/** Every POLL_MS while a Check-outs tab is in front and the window has the
 *  focus, and whenever the window gets it back, the overview is fetched again
 *  and painted when something changed – a check-out by someone else shows up
 *  without *Refresh* (#451). */
var POLL_MS = 30000;
var watching = false;

function watchLocks() {
  if (watching) return;
  watching = true;
  setInterval(function () { if (document.hasFocus()) pollLocks(); }, POLL_MS);
  window.addEventListener("focus", pollLocks);
}

function pollLocks() {
  var tab = activeTab();
  if (!shouldPoll(tab, state.me)) return;
  var my = takeSeq(tab);
  fetchLocks(tab.code).then(function (o) {
    if (activeTab() !== tab || !acceptSeq(tab, my) || !locksDiffer(o, tab.data)) return;
    tab.data = o;
    repaint();
  }, function () { /* the next poll tries again; Refresh shows the error */ });
}

/** Paints the active tab again from its data, keeping the scroll position
 *  and the keyboard focus: a revision link or button that is still there
 *  gets it back (`data-focus-key`), a released one hands it to the heading
 *  instead of letting it fall back to the page. */
function repaint() {
  var tab = activeTab(), body = $("tab-body"), toolbar = $("toolbar");
  var top = body.scrollTop;
  var focus = focusedPlace(body, toolbar);
  clear(body);
  clear(toolbar);
  renderLocks(tab, body, toolbar);
  body.scrollTop = top;
  restoreFocus(focus, body, toolbar);
}

/** Where the focus is inside the overview: the key of a body element or the
 *  index of a toolbar button; null when it is elsewhere. */
function focusedPlace(body, toolbar) {
  var a = document.activeElement;
  if (!a || a === document.body) return null;
  if (toolbar.contains(a)) return { tool: Array.prototype.indexOf.call(toolbar.querySelectorAll("button"), a) };
  if (body.contains(a)) return { key: a.getAttribute ? a.getAttribute("data-focus-key") : null };
  return null;
}

function restoreFocus(place, body, toolbar) {
  if (!place) return;
  var el = null;
  if (place.tool != null && place.tool >= 0) el = toolbar.querySelectorAll("button")[place.tool] || null;
  else if (place.key) el = body.querySelector('[data-focus-key="' + CSS.escape(place.key) + '"]');
  if (!el) {
    el = body.querySelector("h1");
    if (el) el.setAttribute("tabindex", "-1");
  }
  if (el) el.focus({ preventScroll: true });
}

/** The user's own check-outs changed (shell.js fetchWorklist, or a lock of
 *  someone else released in workspace.js): every Check-outs tab loads again;
 *  the one in front at once. That one keeps its data until the new data is
 *  there, so a failed fetch leaves it polling (lock-rules.js forgetHiddenLocks). */
export function locksChanged() {
  var tab = activeTab();
  forgetHiddenLocks(state.tabs, tab);
  if (!tab || tab.kind !== "locks") return;
  var my = takeSeq(tab);
  fetchLocks(tab.code).then(function (o) {
    if (activeTab() !== tab || !acceptSeq(tab, my)) return;
    tab.data = o;
    repaint();
  }, function () { /* data kept: the next poll tries again */ });
}

function svg(name, attrs, children) {
  var el = document.createElementNS(SVG_NS, name);
  Object.keys(attrs || {}).forEach(function (k) { if (attrs[k] != null) el.setAttribute(k, String(attrs[k])); });
  (children || []).forEach(function (c) { if (c) el.appendChild(typeof c === "string" ? document.createTextNode(c) : c); });
  return el;
}

/** The graph as SVG – the same classes as the web page (shared/css/components.css). */
function graphSvg(o) {
  var g = lockGraph(o, Date.now());
  var root = svg("svg", { "class": "lock-graph", viewBox: "0 0 " + g.width + " " + g.height, role: "group",
    "aria-label": "Check-outs of " + o.project_code + ": people and the revisions they hold" });
  g.locks.forEach(function (l) { root.appendChild(svg("path", { "class": "lg-edge lg-" + l.age, d: l.edge })); });
  g.holders.forEach(function (n) {
    root.appendChild(svg("g", { "class": "lg-node lg-holder" + (n.active ? "" : " lg-inactive") }, [
      svg("rect", { x: HOLDER_X, y: n.y, width: HOLDER_W, height: NODE_H, rx: 6 }),
      svg("text", { x: HOLDER_X + 10, y: n.y + NODE_H / 2 }, [n.label]),
      svg("text", { "class": "lg-count", x: HOLDER_X + HOLDER_W - 10, y: n.y + NODE_H / 2, "text-anchor": "end" }, [String(n.count)])
    ]));
  });
  g.locks.forEach(function (l) {
    // a revision is a link: click or Enter opens it
    var link = svg("a", { href: "#", tabindex: "0", role: "link", "aria-label": l.label + " – " + label(l.sub),
      "data-focus-key": "g:" + l.number + "/" + l.code }, [
      svg("g", { "class": "lg-node lg-lock lg-" + l.age }, [
        svg("rect", { x: LOCK_X, y: l.y, width: LOCK_W, height: NODE_H, rx: 6 }),
        svg("text", { x: LOCK_X + 10, y: l.y + NODE_H / 2 }, [l.label])
      ])
    ]);
    function open(e) { e.preventDefault(); openRevision(l.number, l.code); }
    link.addEventListener("click", open);
    link.addEventListener("keydown", function (e) { if (e.key === "Enter") open(e); });
    root.appendChild(link);
  });
  return root;
}

function legend() {
  return h("ul.lock-legend", null, [
    h("li.lg-today", { text: "checked out today" }),
    h("li.lg-week", { text: "for 1–7 days" }),
    h("li.lg-old", { text: "longer than a week" }),
    h("li.lg-inactive", { text: "holder deactivated" })
  ]);
}

export function renderLocks(tab, body, toolbar) {
  var o = tab.data;
  append(toolbar, [
    toolGroup("View", [
      tool("Refresh", "refresh", function () { reloadActive(); }),
      tool("Open in browser", "web", function () {
        call("open_web", { path: "/projects/" + o.project_id + "/checkouts" }).catch(function (e) { toast(errorText(e), "error"); });
      }, { title: "The same overview in the web UI" })
    ])
  ]);
  var people = o.holders.length;
  body.appendChild(h("div.page-head", null, [
    h("h1", { text: "Check-outs: " + o.project_code }),
    h("p.muted", { text: "Who has what checked out in " + o.project_name + " – " + o.total + " revision" + (o.total === 1 ? "" : "s") +
      " held by " + people + " " + (people === 1 ? "person" : "people") + ". Click a revision to open it; a lock is released there." })
  ]));
  if (!o.total) {
    body.appendChild(h("p.empty", { text: "Nothing is checked out in " + o.project_code + "." }));
    return;
  }
  body.appendChild(h("section.panel", null, [h("h2", { text: "Graph" }), graphSvg(o), legend()]));
  o.holders.forEach(function (holder) {
    body.appendChild(h("section.panel", null, [
      h("h3", null, [holder.name, holder.active ? null : h("span.badge.badge-obsolete", { text: " deactivated" }),
        h("span.muted.small", { text: " " + holder.locks.length + " checked out" })]),
      h("ul.lock-list", null, holder.locks.map(function (l) {
        return h("li", null, [
          h("button.link.mono", { type: "button", "data-focus-key": "l:" + l.item_number + "/" + l.code, on: { click: function () { openRevision(l.item_number, l.code); } } }, l.item_number + "/" + l.code),
          " " + l.name + " ",
          h("span.muted.small", { text: label(l.item_type) + " · since " + fmtDate(l.checked_out_at) + " (" + fmtAge(Math.max(0, (Date.now() - Date.parse(l.checked_out_at)) / 1000)) + ")" })
        ]);
      }))
    ]));
  });
}
