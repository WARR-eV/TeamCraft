// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/export-rules.js (which files the check-in
// exports to STEP, #342). Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-export-rules.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/export-rules.js");
const { MAX_ASSEMBLY_EXPORT_BYTES, isAssemblyFile, planStepExports } = await import(
  "data:text/javascript;base64," + Buffer.from(fs.readFileSync(file, "utf8")).toString("base64")
);

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

const MB = 1024 * 1024;

test("assembly files are the .iam and Solid Edge .asm ones", () => {
  assert.equal(isAssemblyFile("Rover.iam"), true);
  assert.equal(isAssemblyFile("rover.IAM"), true);
  assert.equal(isAssemblyFile("frame.asm"), true);
  assert.equal(isAssemblyFile("bracket.ipt"), false);
  assert.equal(isAssemblyFile("plate.par"), false);
  assert.equal(isAssemblyFile("design.sc"), false);
  assert.equal(isAssemblyFile(""), false);
  assert.equal(isAssemblyFile(null), false);
});

test("parts are always exported, whatever their size (#342)", () => {
  const plan = planStepExports([{ name: "huge.ipt", program: "inventor", size: 80 * MB }]);
  assert.deepEqual(plan, { export: ["huge.ipt"], skipped: [] });
});

test("a large assembly is skipped with the reason; a small one is exported", () => {
  const plan = planStepExports([
    { name: "Rover.iam", program: "inventor", size: 12 * MB },
    { name: "Bracket.iam", program: "inventor", size: 2 * MB },
    { name: "bracket.ipt", program: "inventor", size: 1 * MB }
  ]);
  assert.deepEqual(plan.export, ["Bracket.iam", "bracket.ipt"]);
  assert.equal(plan.skipped.length, 1);
  assert.equal(plan.skipped[0].name, "Rover.iam");
  assert.match(plan.skipped[0].why, /12 MB/);
  assert.match(plan.skipped[0].why, /larger than 5 MB/);
});

test("the limit itself still exports; one byte more skips", () => {
  assert.deepEqual(planStepExports([{ name: "a.iam", program: "inventor", size: MAX_ASSEMBLY_EXPORT_BYTES }]).export, ["a.iam"]);
  assert.deepEqual(planStepExports([{ name: "a.iam", program: "inventor", size: MAX_ASSEMBLY_EXPORT_BYTES + 1 }]).export, []);
});

test("files without a program are left out; a missing size counts as small", () => {
  const plan = planStepExports([
    { name: "notes.txt", program: null, size: 10 },
    { name: "frame.asm", program: "solid-edge" },
    null,
    { name: "", program: "inventor", size: 1 }
  ]);
  assert.deepEqual(plan, { export: ["frame.asm"], skipped: [] });
  assert.deepEqual(planStepExports(undefined), { export: [], skipped: [] });
});

console.log(`${n} tests passed`);
