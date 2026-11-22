// SPDX-License-Identifier: MPL-2.0
// Checks the ES modules of the desktop UI (desktop/ui/js/) and of the files
// both apps share (shared/js/) without running them – plain Node.js, no
// packages (ADR 0004: Node.js only checks, never ships). Run by the CI job
// `ui` in .github/workflows/desktop.yml:
//
//   node scripts/check-desktop-modules.mjs
//
// A name used from another module without an import is a ReferenceError only
// when that code runs, so this catches it before: every import names an
// export of its module, every top-level name of another module that a module
// uses is imported (or declared locally), no top-level name is declared in two
// modules, and every desktop module is reachable from js/main.js.
//
// Drift guard: a desktop module or static/app.js must not declare a name that
// a shared module exports – it imports the shared one instead
// (shared/README.md, "One source per rule").
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dir = path.join(root, "desktop/ui/js");
const files = fs.readdirSync(dir).filter((f) => f.endsWith(".js")).sort();
const errors = [];
const ident = "[A-Za-z_$][\\w$]*";

/** `text` as a literal inside a regular expression (every special character escaped). */
function escapeRegExp(text) {
  return text.replace(/[\\^$.*+?()[\]{}|\/-]/g, "\\$&");
}

/**
 * Source without comments and string/regex contents (rough, line-preserving).
 * A regex literal is a `/…/` where an expression starts (after `(`, `,`, `=`,
 * `:`, `[`, `!`, `&`, `|`, `?`, `{`, `}`, `;`, `return` or at the start of a
 * line) – its `$`, `^` and words are no uses of names (`/^\d+$/` is no `$`).
 * Regexes go before strings, so a quote inside a regex starts no string.
 */
function code(text) {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "))
    .replace(
      /(^|[=(,:[!&|?{};]|\breturn)(\s*)\/(?![*/])(?:\\.|\[(?:\\.|[^\]\\\n])*\]|[^/\\\n[])+\/[dgimsuyv]*/gm,
      (m, pre, space) => pre + space + "/" + " ".repeat(Math.max(0, m.length - pre.length - space.length - 2)) + "/"
    )
    .replace(/"(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*'|`(?:\\.|[^`\\])*`/g, (m) => '""' + " ".repeat(Math.max(0, m.length - 2)))
    .replace(/\/\/[^\n]*/g, "");
}

// Modules by key: desktop modules by file name (main.js), shared modules by
// the URL both apps load them from (/static/shared/js/format.js).
const SHARED_URL = "/static/shared/js/";
const sharedDir = path.join(root, "shared/js");
const sharedFiles = fs.readdirSync(sharedDir).filter((f) => f.endsWith(".js") && !f.endsWith(".test.js")).sort();
const IMPORT = /^import \{ ([^}]*) \} from "((?:\.\/|\/static\/shared\/js\/)[\w-]+\.js)";$/;
const mods = {};
function load(key, file, shared) {
  const text = fs.readFileSync(file, "utf8");
  const exports = new Set();
  for (const m of text.matchAll(new RegExp(`^export (?:function|var|let|const) (${ident})`, "gm"))) exports.add(m[1]);
  for (const m of text.matchAll(new RegExp(`^export var [^\\n]*?,\\s*(${ident})\\s*=`, "gm"))) exports.add(m[1]);
  const topLevel = new Set();
  for (const m of text.matchAll(new RegExp(`^(?:export )?(?:function|var|let|const) (${ident})`, "gm"))) topLevel.add(m[1]);
  const imports = new Map(); // name → module key
  for (const line of text.split("\n")) {
    if (!/^import\s/.test(line)) continue;
    const m = IMPORT.exec(line);
    if (!m) {
      errors.push(`${key}: unsupported import form (use: import { a, b } from "./x.js"; or "${SHARED_URL}x.js"): ${line}`);
      continue;
    }
    let from = m[2];
    if (from.startsWith("./")) from = shared ? SHARED_URL + from.slice(2) : from.slice(2);
    else if (shared) errors.push(`${key}: a shared module imports its neighbours relatively ("./x.js"), never by URL: ${line}`);
    for (const n of m[1].split(",").map((s) => s.trim()).filter(Boolean)) imports.set(n, from);
  }
  mods[key] = { text, body: code(text), exports, imports, topLevel, shared };
}
for (const f of files) load(f, path.join(dir, f), false);
for (const f of sharedFiles) load(SHARED_URL + f, path.join(sharedDir, f), true);

// imports resolve
for (const [f, m] of Object.entries(mods)) {
  for (const [n, from] of m.imports) {
    if (!mods[from]) errors.push(`${f}: imports from a missing module ${from}`);
    else if (!mods[from].exports.has(n)) errors.push(`${f}: ${n} is not exported by ${from}`);
  }
}

// one owner per top-level name
const owner = new Map();
for (const [f, m] of Object.entries(mods)) {
  for (const n of m.exports) {
    if (owner.has(n)) errors.push(`${n} is declared in ${owner.get(n)} and ${f}`);
    owner.set(n, f);
  }
}

// shared/ owns what it exports: an app re-implementing a shared function is
// how the web UI and the desktop app drift apart (shared/README.md, "One source per rule")
const sharedExports = new Map();
for (const [f, m] of Object.entries(mods)) if (m.shared) for (const n of m.exports) sharedExports.set(n, f);
for (const [f, m] of Object.entries(mods)) {
  if (m.shared) continue;
  for (const n of m.topLevel) {
    if (sharedExports.has(n) && !m.imports.has(n)) {
      errors.push(`${f}: declares ${n}, which ${sharedExports.get(n)} exports – import it instead`);
    }
  }
}
const webApp = fs.readFileSync(path.join(root, "static/app.js"), "utf8");
for (const m of code(webApp).matchAll(new RegExp(`\\bfunction\\s+(${ident})\\s*\\(`, "g"))) {
  if (sharedExports.has(m[1])) {
    errors.push(`static/app.js: declares ${m[1]}, which ${sharedExports.get(m[1])} exports – use the shared module`);
  }
}

// names of other modules are imported or declared locally
for (const [f, m] of Object.entries(mods)) {
  const local = new Set();
  for (const d of m.body.matchAll(new RegExp(`\\b(?:var|let|const|function)\\s+(${ident})`, "g"))) local.add(d[1]);
  for (const p of m.body.matchAll(/\bfunction\s*[\w$]*\s*\(([^)]*)\)/g)) {
    for (const n of p[1].split(",").map((s) => s.trim().replace(/\s*=.*$/, ""))) if (n) local.add(n);
  }
  for (const c of m.body.matchAll(new RegExp(`\\bcatch\\s*\\((${ident})\\)`, "g"))) local.add(c[1]);
  for (const v of m.body.matchAll(new RegExp(`\\bvar\\s+[^;]*?,\\s*(${ident})\\s*=`, "g"))) local.add(v[1]);
  const used = new Set();
  for (const u of m.body.matchAll(new RegExp(`(^|[^.\\w$])(${ident})`, "gm"))) used.add(u[2]);
  for (const n of used) {
    const o = owner.get(n);
    if (!o || o === f || m.imports.has(n) || local.has(n)) continue;
    // a shared module can only use shared names (a desktop name is a word in a regex or text)
    if (m.shared && !mods[o].shared) continue;
    // the whole name (`el` is no use inside `else`); an object key
    // ({ name: … }) is not a use
    if (new RegExp(`(^|[^.\\w$])${escapeRegExp(n)}(?![\\w$])(?!\\s*:)`, "m").test(m.body)) {
      errors.push(`${f}: uses ${n} of ${o} without importing it`);
    }
  }
}

// every module is loaded
const seen = new Set();
(function visit(f) {
  if (seen.has(f) || !mods[f]) return;
  seen.add(f);
  for (const from of new Set(mods[f].imports.values())) visit(from);
})("main.js");
for (const f of files) if (!seen.has(f)) errors.push(`${f}: not reachable from main.js`);
const html = fs.readFileSync(path.join(root, "desktop/ui/index.html"), "utf8");
if (!html.includes('<script type="module" src="js/main.js"></script>')) errors.push("desktop/ui/index.html does not load js/main.js as a module");

if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log(`desktop/ui/js: ${files.length} modules, shared/js: ${sharedFiles.length} modules, imports complete, no shared name re-declared`);
