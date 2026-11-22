// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/checkin-bom.js (*Update the BOM from the CAD
// file* at check-in, #407: when the box is offered, the result line).
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-checkin-bom.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const source = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/checkin-bom.js");
const { bomStructureFile, checkinBomText } =
  await import("data:text/javascript;base64," + fs.readFileSync(source).toString("base64"));

let passed = 0;
function test(name, fn) {
  fn();
  passed++;
  console.log("ok - " + name);
}

const kinds = { iam: "assembly", sldasm: "assembly", asm: "assembly", ipt: "part", prt: "model", stp: "exchange", pdf: "document" };

test("assemblies, NX parts, STEP files and SolidCraft designs offer the box", () => {
  assert.equal(bomStructureFile(["Wagon.iam"], kinds), true);
  assert.equal(bomStructureFile(["Pump.SLDASM"], kinds), true);
  assert.equal(bomStructureFile(["frame.prt"], {}), true, "NX: an assembly by content");
  assert.equal(bomStructureFile(["pump.STEP"], {}), true);
  assert.equal(bomStructureFile(["cart.sc"], {}), true);
  assert.equal(bomStructureFile(["bracket.asm.7"], kinds), true, "a Creo iteration");
});

test("parts, drawings and documents do not", () => {
  assert.equal(bomStructureFile(["Bolt.ipt", "Bolt.pdf"], kinds), false);
  assert.equal(bomStructureFile([], kinds), false);
  assert.equal(bomStructureFile(["README"], kinds), false);
  assert.equal(bomStructureFile(["bolt.ipt.3"], kinds), false);
  assert.equal(bomStructureFile(null, kinds), false);
});

test("the result line says what changed", () => {
  assert.equal(checkinBomText(null), "");
  assert.equal(checkinBomText({ added: [], updated: [], kept_not_in_cad: [], not_in_teamcraft: [], refused: [] }), "");
  assert.equal(checkinBomText({
    added: ["PRT-1", "PRT-2", "PRT-3", "PRT-4"], updated: ["PRT-5"], kept_not_in_cad: ["PRT-000012"],
    not_in_teamcraft: ["Lager.ipt"], refused: []
  }), "BOM: 4 lines added, 1 quantity changed · not in TeamCraft: Lager.ipt · no longer in the CAD file: PRT-000012 (kept)");
});

test("nothing changed: no line, even with something to report", () => {
  assert.equal(checkinBomText({ added: [], updated: [], kept_not_in_cad: ["PRT-9"], not_in_teamcraft: ["x.ipt"], refused: [] }), "");
  assert.equal(checkinBomText({ added: [], updated: [], refused: [{ child: "ASM-1", reason: "circular" }] }),
    "BOM unchanged · refused (circular): ASM-1");
  assert.equal(checkinBomText({ added: ["PRT-1"], updated: ["PRT-2", "PRT-3"] }), "BOM: 1 line added, 2 quantities changed");
});

console.log(passed + " tests passed");
