/* SPDX-License-Identifier: MPL-2.0 */
// DOM helpers of the shared modules: text only, never innerHTML.

/** el("div", { class, text, on: { click }, …attributes }, children) –
 * string children become text nodes; null, undefined and false are skipped. */
export function el(tag, attrs, children) {
  var n = document.createElement(tag);
  Object.keys(attrs || {}).forEach(function (k) {
    var v = attrs[k];
    if (v === null || v === undefined || v === false) return;
    if (k === "class") n.className = v;
    else if (k === "text") n.textContent = v;
    else if (k === "on") Object.keys(v).forEach(function (e) { n.addEventListener(e, v[e]); });
    else n.setAttribute(k, v === true ? "" : v);
  });
  [].concat(children === undefined ? [] : children).forEach(function (c) {
    if (c === null || c === undefined || c === false) return;
    n.appendChild(typeof c === "string" ? document.createTextNode(c) : c);
  });
  return n;
}
