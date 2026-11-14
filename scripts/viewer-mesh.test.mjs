// SPDX-License-Identifier: MPL-2.0
// Unit tests of TeamCraftViewer.checkMesh (shared/js/viewer.js): an empty,
// cut-off or invalid TCMESH01 model is an error message, never a silently
// empty canvas (#290). Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/viewer-mesh.test.mjs
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
const { checkMesh } = sandbox.window.TeamCraftViewer;

/** A TCMESH01 buffer as Mesh::encode writes it (positions, indices, colours). */
function mesh({ vertices = 3, indices = 3, flags = 0, bounds = [0, 0, 0, 1, 1, 0], magic = "TCMESH01", cut = 0 } = {}) {
  const size = 48 + vertices * 12 + indices * 4 + (flags & 1 ? vertices * 3 : 0);
  const buffer = new ArrayBuffer(Math.max(0, size - cut));
  const bytes = new Uint8Array(buffer);
  for (let i = 0; i < 8 && i < bytes.length; i++) bytes[i] = magic.charCodeAt(i);
  if (buffer.byteLength >= 48) {
    const dv = new DataView(buffer);
    dv.setUint32(8, vertices, true);
    dv.setUint32(12, indices, true);
    dv.setUint32(16, 1, true);
    dv.setUint32(20, flags, true);
    bounds.forEach((v, i) => dv.setFloat32(24 + i * 4, v, true));
  }
  return buffer;
}

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

test("a triangle and a point cloud are fine", () => {
  assert.equal(checkMesh(mesh()), null);
  assert.equal(checkMesh(mesh({ vertices: 5, indices: 0 })), null, "points have no indices");
  assert.equal(checkMesh(mesh({ flags: 1 })), null, "with vertex colours");
});

test("anything that is no TCMESH01 model says so", () => {
  assert.match(checkMesh(mesh({ magic: "NOTAMESH" })), /no 3D model/);
  assert.match(checkMesh(new ArrayBuffer(10)), /no 3D model/);
  assert.match(checkMesh(null), /no 3D model/);
});

test("an empty model is named, not drawn as nothing", () => {
  assert.match(checkMesh(mesh({ vertices: 0, indices: 0 })), /empty/);
});

test("a cut-off download and an incomplete triangle are errors", () => {
  assert.match(checkMesh(mesh({ cut: 4 })), /cut off/);
  assert.match(checkMesh(mesh({ flags: 1, cut: 1 })), /cut off/, "the colours count");
  assert.match(checkMesh(mesh({ indices: 4 })), /incomplete triangle/);
});

test("NaN or infinite bounds (a black canvas otherwise) are errors", () => {
  assert.match(checkMesh(mesh({ bounds: [0, 0, 0, NaN, 1, 1] })), /invalid coordinates/);
  assert.match(checkMesh(mesh({ bounds: [-Infinity, 0, 0, 1, 1, 1] })), /invalid coordinates/);
});

console.log(n + " tests passed");
