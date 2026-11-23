// SPDX-License-Identifier: MPL-2.0
// Unit tests of TeamCraftViewer.dragAction (shared/js/viewer.js): which mouse
// drag rotates and which pans in each scheme of the viewer's *Mouse* setting
// (#459). Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/viewer-input.test.mjs
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
const { dragAction, MOUSE_SCHEMES } = sandbox.window.TeamCraftViewer;

const LEFT = 1, RIGHT = 2, MIDDLE = 4;
// Array.from: an array of this realm – the sandbox's arrays have another
// Array.prototype, which assert.deepEqual (strict) tells apart
const schemes = Array.from(MOUSE_SCHEMES, (s) => s.id);

// four schemes with their own ids and labels; the default is the first
assert.deepEqual(schemes, ["left-rotate", "right-rotate", "middle-rotate", "middle-pan"]);
assert.ok(MOUSE_SCHEMES.every((s) => s.label.length > 0));

// in every scheme: no button = nothing, a plain left drag rotates, Shift or
// Ctrl + left drag and the left and right button together pan – the
// combinations of the issue
for (const s of schemes) {
  assert.equal(dragAction(s, 0, false, false), null, s);
  assert.equal(dragAction(s, LEFT, false, false), "rotate", s);
  assert.equal(dragAction(s, LEFT, true, false), "pan", `${s}: Shift + left`);
  assert.equal(dragAction(s, LEFT, false, true), "pan", `${s}: Ctrl + left`);
  assert.equal(dragAction(s, LEFT | RIGHT, false, false), "pan", `${s}: left + right`);
}

// left drag rotates (default): right and middle pan
assert.equal(dragAction("left-rotate", RIGHT, false, false), "pan");
assert.equal(dragAction("left-rotate", MIDDLE, false, false), "pan");

// right drag rotates: middle pans
assert.equal(dragAction("right-rotate", RIGHT, false, false), "rotate");
assert.equal(dragAction("right-rotate", MIDDLE, false, false), "pan");

// middle drag rotates, Shift + middle pans; right pans
assert.equal(dragAction("middle-rotate", MIDDLE, false, false), "rotate");
assert.equal(dragAction("middle-rotate", MIDDLE, true, false), "pan");
assert.equal(dragAction("middle-rotate", RIGHT, false, false), "pan");

// middle drag pans, Shift + middle rotates; right pans
assert.equal(dragAction("middle-pan", MIDDLE, false, false), "pan");
assert.equal(dragAction("middle-pan", MIDDLE, true, false), "rotate");
assert.equal(dragAction("middle-pan", RIGHT, false, false), "pan");

// an unknown or missing scheme behaves like the default
assert.equal(dragAction("nonsense", RIGHT, false, false), "pan");
assert.equal(dragAction(undefined, LEFT, false, false), "rotate");

console.log("viewer input: all checks passed");
