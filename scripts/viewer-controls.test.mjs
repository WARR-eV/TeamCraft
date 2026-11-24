// SPDX-License-Identifier: MPL-2.0
// Unit tests of the view controls of the shared 3D viewer (shared/js/viewer.js,
// #526): when *Export STL* applies (never for a simulation result with result
// fields – STL would drop them), the state of the *Z up* toggle for screen
// readers, and what happens to the export button once the model is shown.
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/viewer-controls.test.mjs
//
// viewer.js is a classic script that sets window.TeamCraftViewer; it runs
// here in a sandbox without a page (no #viewer-canvas, so it mounts nothing).
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../shared/js/viewer.js");
const sandbox = { window: {}, document: { getElementById: () => null } };
vm.runInNewContext(fs.readFileSync(file, "utf8"), sandbox, { filename: file });
const { stlExportBlocker, upToggle, showStlButton } = sandbox.window.TeamCraftViewer;

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

const triangle = { positions: new Float32Array(9), indices: new Uint32Array([0, 1, 2]), fields: [] };
const field = (name) => ({ name, unit: "MPa", min: 0, max: 1, values: new Float32Array(3) });

test("a triangle model can be exported as STL", () => {
  assert.equal(stlExportBlocker(triangle), null);
  assert.equal(stlExportBlocker({ positions: triangle.positions, indices: triangle.indices }), null, "no fields array");
});

test("a result mesh with a field is not exported: STL would drop the field", () => {
  assert.match(stlExportBlocker({ ...triangle, fields: [field("von Mises")] }), /only the shape, not the result field –/);
  assert.match(stlExportBlocker({ ...triangle, fields: [field("a"), field("b")] }), /result fields –/);
});

test("nothing to export without triangles (point clouds, no model yet)", () => {
  assert.match(stlExportBlocker({ positions: new Float32Array(9), indices: null, fields: [] }), /no triangles/);
  assert.match(stlExportBlocker({ positions: new Float32Array(0), indices: new Uint32Array(0), fields: [] }), /no triangles/);
  assert.match(stlExportBlocker(null), /no triangles/);
});

test("the up toggle keeps its visible name and is pressed while Z is up", () => {
  // no aria-label: the visible text is the name, the state only aria-pressed (WCAG 2.5.3)
  assert.deepEqual({ ...upToggle(true) }, { text: "Z up", pressed: "true" });
  assert.deepEqual({ ...upToggle(false) }, { text: "Z up", pressed: "false" });
});

test("the export button is enabled for a shape and removed for a result", () => {
  const button = () => ({ disabled: true, removed: false, remove() { this.removed = true; } });
  const shape = button();
  showStlButton(shape, { stlBlocker: null });
  assert.equal(shape.disabled, false);
  assert.equal(shape.removed, false);
  const result = button();
  showStlButton(result, { stlBlocker: stlExportBlocker({ ...triangle, fields: [field("T")] }) });
  assert.equal(result.removed, true);
  assert.equal(result.disabled, true, "never enabled");
  showStlButton(null, { stlBlocker: null }); // pages without the button
});

console.log(n + " tests passed");
