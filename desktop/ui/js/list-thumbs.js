// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – small preview images in the rows of Home (Files, All
// items, Structure) and the search (#420). The server names the picture of
// each row in the list answer (no request per row); the image itself is
// fetched only once its row scrolls into view (IntersectionObserver), and
// kept as a blob URL per content hash, so the same picture is fetched once.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { h, invoke } from "./core.js";
import { rowThumb } from "./files-view.js";

// Blob URL promises by SHA-256; beyond LIST_THUMB_MAX the least recently used
// one is revoked, so a long session does not keep every picture alive.
var listThumbUrls = {};
var listThumbOrder = [];
var LIST_THUMB_MAX = 300;
var listThumbObserver = null;
// The cells being watched. A row can leave the page before it ever comes
// into view (a long list drawn again on Refresh, sort or navigation); the
// observer would keep such a cell, and its row, alive. So the first cell of
// each new drawing lets go of the watched cells that are no longer on the page.
var listThumbCells = new Set();
var listThumbBatch = false;

function listThumbForget(ico) {
  listThumbObserver.unobserve(ico);
  listThumbCells.delete(ico);
}

function listThumbPrune() {
  listThumbCells.forEach(function (ico) { if (!ico.isConnected) listThumbForget(ico); });
}

/** The blob URL of a picture, fetched once per content hash. */
function listThumbUrl(sha, id) {
  var p = listThumbUrls[sha];
  if (p) listThumbOrder.splice(listThumbOrder.indexOf(sha), 1);
  else {
    p = listThumbUrls[sha] = invoke("file_bytes", { kind: "thumbnail", id: id })
      .then(function (bytes) { return URL.createObjectURL(new Blob([bytes])); });
    while (listThumbOrder.length >= LIST_THUMB_MAX) {
      var old = listThumbOrder.shift();
      listThumbUrls[old].then(function (u) { URL.revokeObjectURL(u); }, function () {});
      delete listThumbUrls[old];
    }
  }
  listThumbOrder.push(sha);
  return p;
}

/** Loads the picture of an icon cell (`span.ico` with an `img.row-thumb`). */
function listThumbLoad(ico) {
  var img = ico.querySelector("img.row-thumb");
  if (!img) return;
  listThumbUrl(img.getAttribute("data-sha"), img.getAttribute("data-file")).then(function (u) {
    img.src = u;
    img.classList.remove("hidden");
    ico.classList.add("has-thumb");
  }, function () { /* no picture: the type icon stays */ });
}

/** Loads the picture when the cell comes into view (the cell, not the hidden image, is observed). */
function listThumbWatch(ico) {
  if (typeof IntersectionObserver === "undefined") { listThumbLoad(ico); return; }
  if (!listThumbObserver) {
    listThumbObserver = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        // a row that left the page (the list was drawn again) is let go
        if (!en.target.isConnected) { listThumbForget(en.target); return; }
        if (!en.isIntersecting) return;
        listThumbForget(en.target);
        listThumbLoad(en.target);
      });
    }, { rootMargin: "200px 0px" });
  }
  if (!listThumbBatch) {
    listThumbBatch = true;
    listThumbPrune();
    setTimeout(function () { listThumbBatch = false; }, 0);
  }
  listThumbCells.add(ico);
  listThumbObserver.observe(ico);
}

/**
 * The icon cell of a row: the type icon `glyph` (text or a node), with the
 * row's preview image over it once it is loaded – when the row has one
 * (files-view.js rowThumb).
 */
export function thumbIco(glyph, row) {
  var thumb = rowThumb(row);
  var ico = h("span.ico", null, [h("span.ico-glyph", null, glyph)]);
  if (!thumb) return ico;
  ico.appendChild(h("img.row-thumb.hidden", { alt: "", draggable: "false", "data-file": thumb.id, "data-sha": thumb.sha }));
  listThumbWatch(ico);
  return ico;
}
