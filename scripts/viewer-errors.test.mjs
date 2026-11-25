// SPDX-License-Identifier: MPL-2.0
// Unit tests of the error hook of TeamCraftViewer.mount (shared/js/viewer.js):
// an error shown on the canvas also reaches options.onError – the desktop
// adds *Report…* there and keeps the error for the report (#471) – and a
// hook that throws never escapes mount(). Plain Node.js, no packages; run by
// the CI job `ui`:
//
//   node scripts/viewer-errors.test.mjs
//
// viewer.js is a classic script that sets window.TeamCraftViewer; it runs
// here in a sandbox without a page, with a fake canvas that has no WebGL 2.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../shared/js/viewer.js");

/** A minimal element: text, attributes, children. */
function element() {
  return {
    textContent: "", hidden: false, className: "", attrs: {}, children: [],
    setAttribute(k, v) { this.attrs[k] = String(v); },
    appendChild(c) { this.children.push(c); return c; }
  };
}

const sandbox = { window: {}, document: { getElementById: () => null, createElement: () => element() } };
vm.runInNewContext(fs.readFileSync(file, "utf8"), sandbox, { filename: file });
const { mount } = sandbox.window.TeamCraftViewer;

/** A canvas without WebGL 2. */
function canvas() {
  const c = element();
  c.getContext = () => null;
  c.addEventListener = () => {};
  return c;
}

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

test("no WebGL 2: onError gets the text once, after the overlay shows it", () => {
  const overlayEl = element();
  const statusEl = element();
  const calls = [];
  const c = canvas();
  const v = mount(c, {
    overlayEl, statusEl,
    onError(text) { calls.push({ text, overlay: overlayEl.children.map((s) => s.textContent).join("") }); },
    load: () => { throw new Error("not loaded without WebGL"); }
  });
  assert.equal(calls.length, 1);
  assert.match(calls[0].text, /WebGL 2/);
  assert.equal(calls[0].overlay, calls[0].text, "the overlay already holds the text");
  assert.equal(statusEl.textContent, calls[0].text);
  assert.equal(c.attrs["data-ready"], "error");
  assert.equal(overlayEl.attrs.role, "alert");
  assert.equal(typeof v.dispose, "function");
  assert.throws(() => v.exportStl(), /not available/);
});

test("a hook that throws does not escape mount()", () => {
  const v = mount(canvas(), { overlayEl: element(), onError() { throw new Error("hook bug"); } });
  assert.equal(typeof v.exportStl, "function", "still the inert viewer");
});

test("without onError (the web UI) mount() does not throw", () => {
  const c = canvas();
  const v = mount(c, { overlayEl: element() });
  assert.equal(c.attrs["data-ready"], "error");
  assert.equal(typeof v.dispose, "function");
  // no options at all
  assert.doesNotThrow(() => mount(canvas()));
});

console.log(`${n} tests passed`);
