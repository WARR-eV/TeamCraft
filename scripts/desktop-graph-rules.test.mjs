// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/graph-rules.js (which neighbours the Graph
// sub-tab draws – #518 – and its notes – #519).
// Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-graph-rules.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/graph-rules.js");
const src = fs.readFileSync(file, "utf8");
const { graphAloneText, graphSkippedText, neighbours } = await import(
  "data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

const noRefs = { outgoing: [], incoming: [], files: [], file_referrers: [], impacts: [] };
function part(extra) {
  return { rev: { item_number: "PRT-000009", code: "A", item_type: "part" }, bom: [], refs: noRefs, ...extra };
}
function usage(number, code, extra) {
  return { parent_number: number, parent_code: code, parent_name: number + " assembly", parent_state: "in_work",
           find_number: 10, quantity: 2, ...extra };
}
const brief = (list) => list.map((x) => `${x.dir}:${x.rel}:${x.number}/${x.code}`);

test("a part used in an assembly's BOM draws the assembly above it (#518)", () => {
  const nodes = neighbours(part({ whereUsed: [usage("ASM-000003", "B")] }));
  assert.deepEqual(brief(nodes), ["up:used-by:ASM-000003/B"]);
  assert.equal(nodes[0].type, null, "the type comes with the details (enrich)");
  assert.equal(nodes[0].inBom, true);
  assert.equal(nodes[0].state, "in_work");
  assert.equal(nodes[0].name, "ASM-000003 assembly");
  assert.equal(nodes[0].qty, undefined, "a parent shows no quantity chip");
});

test("several revisions of one parent: the newest (the first row) is drawn once", () => {
  const nodes = neighbours(part({ whereUsed: [usage("ASM-000003", "C"), usage("ASM-000003", "B"), usage("ASM-000005", "A")] }));
  assert.deepEqual(brief(nodes), ["up:used-by:ASM-000003/C", "up:used-by:ASM-000005/A"]);
});

test("without the where-used list (not loaded, an older server) the graph has the references only", () => {
  assert.deepEqual(neighbours(part({})), []);
  const refs = { ...noRefs, incoming: [{ from_number: "DRW-000001", from_code: "A", from_state: "released", from_name: "Drawing" }] };
  assert.deepEqual(brief(neighbours(part({ refs }))), ["up:used-by:DRW-000001/A"]);
});

test("a BOM parent that also references the item is one node, marked as BOM parent", () => {
  const refs = { ...noRefs,
    incoming: [{ from_number: "ASM-000003", from_code: "B", from_state: "in_work", from_name: "Frame" }],
    file_referrers: [{ item_number: "ASM-000003", revision_code: "B", revision_state: "in_work", item_name: "Frame", filename: "frame.iam" }] };
  const nodes = neighbours(part({ refs, whereUsed: [usage("ASM-000003", "B")] }));
  assert.deepEqual(brief(nodes), ["up:used-by:ASM-000003/B"]);
  assert.equal(nodes[0].inBom, true);
  assert.equal(nodes[0].via, undefined);
});

test("a row without a revision code is not drawn (nothing to open)", () => {
  assert.deepEqual(neighbours(part({ whereUsed: [usage("ASM-000003", "")] })), []);
});

test("an assembly: BOM children and references below, parents above; instances add up", () => {
  const d = {
    rev: { item_number: "ASM-000003", code: "B", item_type: "assembly" },
    bom: [
      { child_number: "PRT-000009", child_latest_code: "A", child_latest_state: "released", child_type: "part", child_name: "Bracket", quantity: 2 },
      { child_number: "PRT-000009", child_latest_code: "A", child_latest_state: "released", child_type: "part", child_name: "Bracket", quantity: 3 }
    ],
    refs: { ...noRefs, outgoing: [{ to_number: "DOC-000001", to_latest_code: "A", to_latest_state: "released", to_name: "Spec" }] },
    whereUsed: [usage("ASM-000001", "A")]
  };
  const nodes = neighbours(d);
  assert.deepEqual(brief(nodes), ["down:bom:PRT-000009/A", "down:reference:DOC-000001/A", "up:used-by:ASM-000001/A"]);
  assert.equal(nodes[0].qty, 5);
});

test("the notes", () => {
  assert.equal(graphAloneText("PRT-000009"), "PRT-000009 has no BOM children, references or assemblies that use it.");
  assert.equal(graphSkippedText(0, 30), null);
  assert.equal(graphSkippedText(12, 60), "12 of 60 neighbours shown without preview and complexity – open one to see its details.");
});

console.log(`${n} tests passed`);
