/* SPDX-License-Identifier: MPL-2.0 */
// The assistant's markdown-lite as DOM nodes: headings, lists, pipe tables,
// code blocks, paragraphs, `code` and **bold**. Nothing is ever parsed as
// HTML. A blank line between two list items keeps the list open, and a
// numbered list keeps its first number (<ol start>). As in CommonMark, two
// numbered lists separated only by a blank line are therefore one list
// (numbered straight through); text or a heading between them starts a new
// one. The server's assistant::render_markdown renders the same dialect as
// HTML; both are pinned to shared/test-vectors/markdown.json
// (src/ui_vectors.rs, scripts/shared-vectors.test.mjs).

import { el } from "./dom.js";

var ITEM_NUMBER = /\b(?:PRT|ASM|DOC|SW|SIM)-\d{3,}\b/g;

function itemNodes(text, link) {
  if (!link) return [text];
  var out = [], last = 0, m;
  ITEM_NUMBER.lastIndex = 0;
  while ((m = ITEM_NUMBER.exec(text)) !== null) {
    if (m.index > last) out.push(text.slice(last, m.index));
    out.push(link(m[0]) || m[0]);
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

function inlineNodes(text, link) {
  var out = [];
  var parts = text.split("`");
  var ticks = parts.length - 1;
  var paired = ticks - (ticks % 2);
  parts.forEach(function (part, k) {
    if (k % 2 === 1 && k < paired) { out.push(el("code", { text: part })); return; }
    if (k % 2 === 1) out.push("`");
    var pieces = part.split("**"), markers = pieces.length - 1, strong = null;
    pieces.forEach(function (piece, n) {
      if (n > 0) {
        if (markers % 2 === 1 && n === markers) out.push("**");
        else if (strong) { out.push(strong); strong = null; }
        else strong = el("strong");
      }
      var nodes = itemNodes(piece, link);
      nodes.forEach(function (c) {
        var node = typeof c === "string" ? document.createTextNode(c) : c;
        if (strong) strong.appendChild(node); else out.push(node);
      });
    });
    if (strong) out.push(strong);
  });
  return out.map(function (n) { return typeof n === "string" ? document.createTextNode(n) : n; });
}

function cells(line) {
  return line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map(function (c) { return c.trim(); });
}

function isSeparator(line) {
  var c = cells(line);
  return c.length > 0 && c.every(function (x) { return /^[\s:]*-[-\s:]*$/.test(x); });
}

/** The assistant's markdown-lite as a <div class="tc-md"> of DOM nodes. */
export function markdown(text, link) {
  var root = el("div", { class: "tc-md assistant-text" });
  var lines = String(text || "").split(/\r?\n/);
  var para = null, list = null;
  function end() { para = null; list = null; }
  function add(parent, nodes) { nodes.forEach(function (n) { parent.appendChild(n); }); }
  for (var i = 0; i < lines.length; i++) {
    var t = lines[i].trim(), m;
    if (t.indexOf("```") === 0) {
      end();
      var code = [];
      for (i++; i < lines.length && lines[i].trim().indexOf("```") !== 0; i++) code.push(lines[i]);
      root.appendChild(el("pre", null, el("code", { text: code.join("\n") })));
      continue;
    }
    // a blank line keeps the list open: an item after it continues the list
    // (a "loose" list), anything else closes it
    if (!t) { para = null; continue; }
    if ((m = /^(#{1,6})\s+(.*)$/.exec(t))) {
      end();
      var head = el(m[1].length <= 2 ? "h3" : "h4");
      add(head, inlineNodes(m[2], link));
      root.appendChild(head);
      continue;
    }
    if (t.charAt(0) === "|" && i + 1 < lines.length && isSeparator(lines[i + 1])) {
      end();
      var tr = el("tr");
      cells(t).forEach(function (c) { var th = el("th"); add(th, inlineNodes(c, link)); tr.appendChild(th); });
      var body = el("tbody");
      for (i += 2; i < lines.length && lines[i].trim().charAt(0) === "|"; i++) {
        var row = el("tr");
        cells(lines[i]).forEach(function (c) { var td = el("td"); add(td, inlineNodes(c, link)); row.appendChild(td); });
        body.appendChild(row);
      }
      i--;
      root.appendChild(el("div", { class: "table-wrap" }, el("table", { class: "grid" }, [el("thead", null, tr), body])));
      continue;
    }
    if (/^[-*_]{3,}$/.test(t)) { end(); root.appendChild(el("hr")); continue; }
    var bullet = /^(?:[-*•])\s+(.*)$/.exec(t), numbered = /^(\d{1,3})[.)]\s+(.*)$/.exec(t);
    if (bullet || numbered) {
      para = null;
      var tag = bullet ? "ul" : "ol";
      if (!list || list.tagName.toLowerCase() !== tag) {
        // a numbered list keeps its first number
        var start = numbered ? parseInt(numbered[1], 10) : 1;
        list = el(tag, start !== 1 ? { start: String(start) } : null);
        root.appendChild(list);
      }
      var li = el("li");
      add(li, inlineNodes(bullet ? bullet[1] : numbered[2], link));
      list.appendChild(li);
      continue;
    }
    list = null;
    if (!para) { para = el("p"); root.appendChild(para); } else para.appendChild(el("br"));
    add(para, inlineNodes(t.replace(/^>\s/, ""), link));
  }
  return root;
}
