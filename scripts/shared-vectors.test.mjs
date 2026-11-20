// SPDX-License-Identifier: MPL-2.0
// The shared rules of shared/js/ against shared/test-vectors/*.json – the
// same vectors src/ui_vectors.rs runs the server's Rust implementations
// against, so the web UI and the desktop app cannot drift apart. Plain
// Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/shared-vectors.test.mjs
//
// The modules sit in a folder without package.json, so each is imported as a
// data: URL module; its relative imports ("./dom.js") are rewritten to data:
// URLs of their own.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const vectors = (name) => JSON.parse(fs.readFileSync(path.join(root, "shared/test-vectors", name), "utf8"));

function dataUrl(file) {
  const text = fs.readFileSync(file, "utf8").replace(/from "\.\/([\w-]+\.js)";/g, (_, dep) =>
    `from "${dataUrl(path.join(path.dirname(file), dep))}";`);
  return "data:text/javascript;base64," + Buffer.from(text).toString("base64");
}
const load = (name) => import(dataUrl(path.join(root, "shared/js", name)));

// A minimal DOM: what dom.js and markdown.js call, and an HTML serializer
// that leaves out class attributes (the vectors pin the structure).
class Text {
  constructor(text) { this.text = String(text); }
  html() { return escape(this.text); }
}
class Element {
  constructor(tag) { this.tagName = tag.toUpperCase(); this.children = []; this.attrs = {}; this.className = ""; }
  set textContent(v) { this.children = [new Text(v)]; }
  setAttribute(k, v) { this.attrs[k] = String(v); }
  addEventListener() {}
  appendChild(c) { this.children.push(c); return c; }
  html() {
    const tag = this.tagName.toLowerCase();
    const attrs = Object.entries(this.attrs).map(([k, v]) => ` ${k}="${escape(v)}"`).join("");
    if (tag === "br" || tag === "hr") return `<${tag}${attrs}>`;
    return `<${tag}${attrs}>${this.children.map((c) => c.html()).join("")}</${tag}>`;
  }
}
function escape(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");
}
globalThis.document = { createElement: (tag) => new Element(tag), createTextNode: (t) => new Text(t) };

const { label, fmtNum, fmtFixed, fmtSize, fmtAge } = await load("format.js");
const { glyph } = await load("states.js");
const { markdown } = await load("markdown.js");

let passed = 0;
function check(what, actual, expected) {
  assert.equal(actual, expected, `${what}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  passed++;
}

for (const [code, text] of vectors("labels.json").cases) check(`label(${JSON.stringify(code)})`, label(code), text);

const format = vectors("format.json");
for (const [[v, d], text] of format.num) check(`fmtNum(${v}, ${d})`, fmtNum(v, d), text);
for (const [[v, d], text] of format.fixed) check(`fmtFixed(${v}, ${d})`, fmtFixed(v, d), text);
for (const [bytes, text] of format.bytes) check(`fmtSize(${bytes})`, fmtSize(bytes), text);
for (const [seconds, text] of format.age) check(`fmtAge(${seconds})`, fmtAge(seconds), text);
check("fmtNum(null)", fmtNum(null), "—");
check("fmtSize(undefined)", fmtSize(undefined), "—");

const states = vectors("states.json");
for (const [state, g] of Object.entries(states.glyphs)) check(`glyph(${state})`, glyph(state), g);
for (const state of states.unknown) check(`glyph(${state})`, glyph(state), "");
check("glyph(\"toString\")", glyph("toString"), "");

for (const [input, html] of vectors("markdown.json").cases) {
  const out = markdown(input).children.map((c) => c.html()).join("");
  check(`markdown(${JSON.stringify(input)})`, out, html);
}
// item numbers go through the app's link callback
const linked = markdown("See PRT-000012.", (n) => { const a = new Element("a"); a.setAttribute("href", "/items/" + n); a.textContent = n; return a; });
check("markdown with link", linked.children.map((c) => c.html()).join(""), '<p>See <a href="/items/PRT-000012">PRT-000012</a>.</p>');

console.log(`shared vectors: ${passed} checks passed`);
