// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – BOM lines in the Structure tab (#419): Add line… with
// a component picker, quantity editing and removing lines of an own check-out.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { api, badge, confirmDialog, dialog, enc, errorText, field, h, invoke, run, toast } from "./core.js";
import { fmtNum } from "/static/shared/js/format.js";
import { reloadActive } from "./tabs.js";

/** Most suggestions the picker shows (the server caps them the same way). */
export var PICKER_LIMIT = 10;

/** A quantity as typed: decimal comma or point; null when it is not a positive number. */
export function parseQuantity(text) {
  var t = String(text || "").trim().replace(",", ".");
  if (!/^\d*\.?\d+$|^\d+\.$/.test(t)) return null;
  var n = Number(t);
  return n > 0 && isFinite(n) ? n : null;
}

/** Items matching `q` (number, name, file name) the user may read. Older
 *  servers without `GET /items/suggest` answer through the item search. */
export function suggestItems(q) {
  var query = "?limit=" + PICKER_LIMIT + "&q=" + enc(q);
  return api("GET", "/items/suggest" + query).catch(function (err) {
    if (!err || err.status !== 404) throw err;
    return api("GET", "/items" + query);
  }).then(function (r) { return (r && r.items) || []; });
}

var pickerThumbUrls = {};
function pickerThumb(fileId) {
  if (!fileId) return h("span.picker-thumb.picker-thumb-none");
  var img = h("img.picker-thumb.hidden", { alt: "" });
  var url = pickerThumbUrls[fileId] || (pickerThumbUrls[fileId] = invoke("file_bytes", { kind: "thumbnail", id: fileId })
    .then(function (bytes) { return URL.createObjectURL(new Blob([bytes])); }));
  url.then(function (u) { img.src = u; img.classList.remove("hidden"); }, function () { /* none */ });
  return img;
}

/**
 * Add line…: picks a component by typing part of its number, name or file
 * name and adds it with quantity, find number and notes to the BOM of `rev`
 * (my check-out) – `POST /items/{n}/revisions/{c}/bom`, the route of the web
 * UI's *Add component*. A typed exact item number works without choosing.
 */
export function addLineDialog(rev) {
  var chosen = null, rows = [], active = -1, timer = null, asked = "";
  var search = h("input", { type: "text", autocomplete: "off", placeholder: "Number, name or file name", role: "combobox",
    "aria-autocomplete": "list", "aria-expanded": "false", "aria-controls": "add-line-suggestions" });
  var list = h("ul.picker-list.static", { id: "add-line-suggestions", role: "listbox", "aria-label": "Matching items" });
  var info = h("p.hint", { text: "Type at least 2 characters." });
  var qty = h("input.qty", { type: "text", value: "1", inputmode: "decimal" });
  var find = h("input.find", { type: "text", placeholder: "auto", inputmode: "numeric" });
  var notes = h("input", { type: "text", placeholder: "optional", maxlength: "500" });

  function highlight(i) {
    active = i;
    rows.forEach(function (r, k) { r.el.setAttribute("aria-selected", k === i || (i < 0 && chosen === r.item) ? "true" : "false"); });
    if (i >= 0 && rows[i]) rows[i].el.scrollIntoView({ block: "nearest" });
  }
  function choose(i) {
    var r = rows[i];
    if (!r) return;
    chosen = r.item;
    search.value = r.item.number;
    asked = r.item.number;
    info.textContent = "Chosen: " + r.item.number + " – " + r.item.name;
    highlight(-1);
    qty.focus();
    qty.select();
  }
  function render(items) {
    list.replaceChildren();
    rows = [];
    active = -1;
    items.forEach(function (item, i) {
      var li = h("li.picker-option", { id: "add-line-option-" + i, role: "option", "aria-selected": "false",
        on: { mousedown: function (e) { e.preventDefault(); choose(i); } } }, [
        pickerThumb(item.thumbnail_file_id),
        h("span.picker-main", null, [
          h("span", null, [h("span.mono", { text: item.number + "/" + item.latest_code }), " ", badge(item.latest_state),
            item.project_code ? h("span.hint", { text: " " + item.project_code }) : null]),
          h("span.picker-name", { text: item.name }),
          item.matched_file ? h("span.hint.mono", { text: item.matched_file }) : null
        ])
      ]);
      list.appendChild(li);
      rows.push({ el: li, item: item });
    });
    search.setAttribute("aria-expanded", items.length ? "true" : "false");
    info.textContent = items.length ? items.length + " match" + (items.length === 1 ? "" : "es") + " – click one or use ↑/↓ and Enter"
      : "No readable item matches – an exact item number still works.";
  }
  function lookup() {
    var q = search.value.trim();
    if (q === asked) return;
    asked = q;
    chosen = null;
    if (q.length < 2) { render([]); info.textContent = "Type at least 2 characters."; return; }
    suggestItems(q).then(function (items) { if (search.value.trim() === q) render(items); },
      function (err) { info.textContent = errorText(err); });
  }
  search.addEventListener("input", function () { clearTimeout(timer); timer = setTimeout(lookup, 200); });
  search.addEventListener("keydown", function (e) {
    if (!rows.length) return;
    if (e.key === "ArrowDown") { e.preventDefault(); highlight(active < rows.length - 1 ? active + 1 : 0); }
    else if (e.key === "ArrowUp") { e.preventDefault(); highlight(active > 0 ? active - 1 : rows.length - 1); }
    else if (e.key === "Enter" && active >= 0) { e.preventDefault(); e.stopPropagation(); choose(active); }
  });

  var body = h("div.stack", null, [
    field("Item", search), list, info,
    h("div.row", null, [field("Qty", qty), field("Find no.", find), field("Notes", notes)])
  ]);
  dialog("Add line to " + rev.item_number + "/" + rev.code, body, [
    { label: "Cancel", onClick: function (d) { d.close(); } },
    { label: "Add", primary: true, submit: true, onClick: function (d) {
      var child = chosen ? chosen.number : search.value.trim();
      if (!child) { d.fail("Choose an item."); return; }
      var quantity = parseQuantity(qty.value);
      if (quantity === null) { d.fail("The quantity must be a number above 0."); return; }
      var payload = { child: child, quantity: quantity };
      var f = find.value.trim();
      if (f) {
        if (!/^\d+$/.test(f)) { d.fail("The find number must be a whole number."); return; }
        payload.find_number = parseInt(f, 10);
      }
      if (notes.value.trim()) payload.notes = notes.value.trim();
      api("POST", "/items/" + enc(rev.item_number) + "/revisions/" + enc(rev.code) + "/bom", payload).then(function (line) {
        d.close();
        toast("Added " + child + " × " + fmtNum(quantity, 6) + (line && line.find_number ? " as find no. " + line.find_number : ""));
        reloadActive();
      }, d.fail);
    } }
  ], { wide: true });
}

/** The quantity of a BOM line of my check-out as an input; a change is saved
 *  at once (`PATCH /bom-lines/{id}`). */
export function quantityInput(line) {
  var old = String(line.quantity);
  var input = h("input.qty", { type: "text", value: old, inputmode: "decimal", "aria-label": "Quantity of " + line.item_number,
    on: {
      click: function (e) { e.stopPropagation(); },
      dblclick: function (e) { e.stopPropagation(); },
      keydown: function (e) { if (e.key === "Enter") input.blur(); else if (e.key === "Escape") { input.value = old; input.blur(); } },
      change: function () {
        var quantity = parseQuantity(input.value);
        if (quantity === null) { toast("The quantity must be a number above 0.", "error", "Change quantity"); input.value = old; return; }
        run("Saving the quantity of " + line.item_number, function () {
          return api("PATCH", "/bom-lines/" + line.line_id, { quantity: quantity });
        }).then(reloadActive, function () { input.value = old; });
      }
    } });
  return input;
}

/** Remove button of a BOM line of my check-out (`DELETE /bom-lines/{id}`). */
export function removeLineButton(line) {
  return h("button.small.danger", { type: "button", title: "Remove " + line.item_number + " from the BOM", on: {
    dblclick: function (e) { e.stopPropagation(); },
    click: function (e) {
      e.stopPropagation();
      confirmDialog("Remove line", "Remove " + line.item_number + " from the BOM?", "Remove", true).then(function (ok) {
        if (!ok) return;
        run("Removing " + line.item_number, function () { return api("DELETE", "/bom-lines/" + line.line_id); })
          .then(reloadActive, function () {});
      });
    }
  } }, "Remove");
}
