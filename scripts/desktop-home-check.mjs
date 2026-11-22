// SPDX-License-Identifier: MPL-2.0
// Browser check of the Files mode of the desktop Home view (the folders and
// files of the active project group): the whole desktop UI (desktop/ui) is
// loaded with a stubbed window.__TAURI__ whose `api` command answers like a
// small server. Needs Playwright with its browsers; Node.js only checks, it
// never ships (ADR 0004).
//
//   node scripts/desktop-home-check.mjs [screenshot-folder]
//
// HOME_CHECK_BROWSERS=chromium (comma-separated) limits the run to the
// engines that are installed; the default is webkit and chromium.
//
// The fake server has three project groups: ROVER (member; folders, files
// whose names sort differently from their numbers, one very long name, an
// item without files, a folder with more files than one page), ROCKET
// (member, restricted, empty) and PUMP (not a member; its server "does not
// know" the file browser yet, as an older server). Checks, per browser:
//   * the rail has Projects below Agent; a group chosen there or on the first
//     Home is the active one, also after a reload;
//   * Home starts in the Files mode and lists folders first, then the files
//     by name (case-blind, digits by value), then the items without files –
//     the name is the first thing in every row and starts at its left, the
//     number follows it; the count is the rows of the whole group;
//   * the order chosen on Home (here: by number) applies to the files;
//   * a folder opens its own level, "show … more" loads the next page, and
//     the folder stays open when Home is shown again;
//   * a click on a file opens its revision in a tab;
//   * the folder of the Inventor project file is suggested as the favourite
//     folder; Home then starts there, also after a reload, under a banner in
//     the accent colour; *Whole group* shows the top and leads back, *Remove
//     favourite* ends it, *No, thanks* stops the suggestion; a folder's star
//     makes it the favourite (issue #352);
//   * a server without the file browser gets the items by name (All items);
//   * an empty group says so and offers its next steps (#423);
//   * New item starts with the active group and the folder last opened; the
//     + of a folder row (New item here) fills in that folder;
//   * the All items and Structure modes are still there;
//   * no page errors.
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);
let playwright;
try {
  playwright = require("playwright");
} catch {
  console.error("Playwright is not installed: npm i -D playwright && npx playwright install webkit chromium");
  process.exit(2);
}
const shots = process.argv[2] || path.join(root, "target");

// The desktop UI: desktop/ui with the shared files at /static/shared and the
// logos at /static/media.
function routes(url) {
  const types = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8",
                  ".svg": "image/svg+xml", ".woff2": "font/woff2", ".png": "image/png" };
  const p = url === "/" ? "desktop/ui/index.html"
    : url.startsWith("/static/shared/") ? path.join("shared", url.slice("/static/shared/".length))
    : url.startsWith("/static/media/") ? path.join("media", url.slice("/static/media/".length))
    : path.join("desktop/ui", url);
  const abs = path.join(root, p);
  if (!abs.startsWith(root) || !fs.existsSync(abs) || fs.statSync(abs).isDirectory()) return null;
  return [types[path.extname(abs)] || "application/octet-stream", fs.readFileSync(abs)];
}

function serve() {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      const found = routes(req.url.split("?")[0]);
      if (!found) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { "content-type": found[0] });
      res.end(found[1]);
    });
    server.listen(0, "127.0.0.1", () => resolve(server));
  });
}

// Runs in the page before the app: window.__TAURI__ with a small fake server.
function fakeBackend() {
  const me = { id: "u-me", username: "erika", display_name: "Erika", role: "editor", projects: ["p-rover", "p-rocket"], capabilities: {} };
  const projects = [
    { id: "p-pump", code: "PUMP", name: "Centrifugal pump P-200", description: "", active: true, visibility: "internal" },
    { id: "p-rocket", code: "ROCKET", name: "Sounding rocket", description: "", active: true, visibility: "restricted" },
    { id: "p-rover", code: "ROVER", name: "Mars rover 2027", description: "Chassis and drive", active: true, visibility: "internal" }
  ];
  const file = (filename, number, kind, more) => Object.assign({
    file_id: "f-" + filename, filename, size_bytes: 1024, format: "x", format_label: "Inventor " + (kind || "part"),
    format_kind: kind || "part", item_number: number, item_type: kind === "assembly" ? "assembly" : "part",
    name: filename.replace(/\.[^.]+$/, ""), code: "A", state: "in_work", checked_out_by_name: null }, more || {});
  const many = [];
  for (let i = 1; i <= 520; i++) many.push(file("Bolt " + i + ".ipt", "PRT-" + String(9000 - i).padStart(7, "0")));
  // lower(name) order, as the server pages them
  many.sort((a, b) => (a.filename.toLowerCase() < b.filename.toLowerCase() ? -1 : 1));
  const levels = {
    "ROVER:": {
      folders: [{ name: "wheels", path: "wheels", files: 3 }, { name: "Chassis", path: "Chassis", files: 521 }],
      // the folder of the Inventor project file: suggested as the favourite
      project_folders: ["Chassis/brackets"],
      entries: [
        file("Rover.iam", "ASM-0000001", "assembly", { state: "released" }),
        file("part2.ipt", "PRT-0000310"),
        file("Part10.ipt", "PRT-0000007"),
        file("Bracket for the left rocker of the suspension – revised mounting pattern.ipt", "PRT-0000042"),
        file("axle.ipt", "PRT-0000999", "part", { checked_out_by_name: "Erika" }),
        file("Rover.idw", "ASM-0000001", "drawing", { state: "released", name: "Rover" }),
        { file_id: null, filename: null, size_bytes: null, format: "", format_label: null, format_kind: null,
          item_number: "PRT-0000003", item_type: "part", name: "Bought motor", code: "A", state: "in_work", checked_out_by_name: null }
      ]
    },
    "ROVER:Chassis": { folders: [{ name: "brackets", path: "Chassis/brackets", files: 1 }], entries: many },
    "ROVER:Chassis/brackets": { folders: [], entries: [file("Gusset.ipt", "PRT-0000100")] },
    "ROVER:wheels": { folders: [], entries: [file("Wheel.ipt", "PRT-0000200"), file("Rim.ipt", "PRT-0000201"), file("Tyre.ipt", "PRT-0000202")] },
    "ROCKET:": { folders: [], entries: [] }
  };
  window.__calls = [];
  function api(method, pathAndQuery, body) {
    window.__calls.push(method + " " + pathAndQuery);
    const [p, query] = pathAndQuery.split("?");
    const q = new URLSearchParams(query || "");
    const notFound = (what) => Promise.reject({ status: 404, message: what + " not found." });
    if (method === "GET" && p === "/me") return Promise.resolve(me);
    if (method === "GET" && p === "/projects") return Promise.resolve(projects.slice());
    if (method === "POST" && p === "/projects") {
      const created = { id: "p-" + body.code.toLowerCase(), code: body.code.toUpperCase(), name: body.name, description: body.description,
                        active: true, visibility: body.visibility };
      window.__posted = body;
      projects.push(created);
      me.projects.push(created.id);
      levels[created.code + ":"] = { folders: [], entries: [] };
      return Promise.resolve(created);
    }
    if (method === "GET" && p === "/worklist") {
      return Promise.resolve({ stats: { items: 0, revisions_in_work: 0, revisions_in_review: 0, revisions_released: 0, open_changes: 0, stored_bytes: 0, files: 0 },
        checkouts: [], reviews: [], impacts: [], my_changes: [], requests_assigned: [], my_requests: [] });
    }
    if (method === "GET" && p === "/notifications") return Promise.resolve({ unread: 0, notifications: [] });
    let m = p.match(/^\/projects\/([^/]+)\/files$/);
    if (method === "GET" && m) {
      const code = decodeURIComponent(m[1]);
      if (code === "PUMP") return notFound("Route"); // an older server
      const level = levels[code + ":" + (q.get("folder") || "")];
      if (!level) return Promise.resolve({ folder: q.get("folder") || "", folders: [], entries: [], total: 0, project_folders: [] });
      const offset = Number(q.get("offset") || 0), limit = Number(q.get("limit") || 50);
      return Promise.resolve({ folder: q.get("folder") || "", folders: level.folders,
        entries: level.entries.slice(offset, offset + limit), total: offset < level.entries.length ? level.entries.length : 0,
        project_folders: level.project_folders || [] });
    }
    m = p.match(/^\/projects\/([^/]+)\/structure$/);
    if (method === "GET" && m) {
      return Promise.resolve({ nodes: [{ item_number: "ASM-0000001", item_type: "assembly", name: "Rover", code: "A", state: "released",
        checked_out_by_name: null, used_in: 0, bom_lines: 0, find_number: null, quantity: null, restricted: false }], total: 1 });
    }
    if (method === "GET" && p === "/items" && q.get("project") === "PUMP") {
      return Promise.resolve({ total: 3, limit: 500, offset: 0, items: [
        { number: "PRT-0000500", item_type: "part", name: "volute casing", latest_code: "A", latest_state: "in_work" },
        { number: "ASM-0000050", item_type: "assembly", name: "Pump P-200", latest_code: "B", latest_state: "released" },
        { number: "PRT-0000501", item_type: "part", name: "Impeller", latest_code: "A", latest_state: "in_work" }] });
    }
    if (method === "GET" && p === "/items") return Promise.resolve({ total: 0, limit: 500, offset: 0, items: [] });
    return notFound(p);
  }
  function invoke(cmd, args) {
    if (cmd === "session") {
      return Promise.resolve({ server_url: "https://plm.example.org", signed_in: true, workspace_dir: "/home/erika/TeamCraft",
        skin: "teamcraft", zoom: 1, update_check: false, token_storage: "keychain" });
    }
    if (cmd === "api") return api(args.method, args.path, args.body);
    if (cmd === "crash_reports") return Promise.resolve(null);
    return Promise.reject({ message: cmd + " is not in the harness" });
  }
  window.__TAURI__ = {
    core: { invoke, convertFileSrc: (p) => p },
    event: { listen: async () => () => {} },
    window: { getCurrentWindow: () => ({ onFocusChanged: async () => () => {}, setTitle: async () => {} }) },
    webview: { getCurrentWebview: () => ({ onDragDropEvent: async () => () => {} }) },
    dialog: {}, opener: {}, shell: {}
  };
}

/** The rows of a tree level as text, in order: "[folder] name count",
 *  "name | revision" for a file or an item, "-- text" for a separator. */
function rows(page, selector) {
  return page.evaluate((sel) => Array.prototype.map.call(document.querySelectorAll(sel), (li) => {
    if (li.classList.contains("tree-sep")) return "-- " + li.textContent;
    const node = li.querySelector(":scope > .node");
    if (!node) return "?? " + li.textContent;
    if (node.classList.contains("folder")) return "[folder] " + node.querySelector("strong").textContent + " " + node.querySelector(".count").textContent;
    const ref = node.querySelector(".label .dim.mono").textContent.trim();
    const label = node.querySelector(".label").textContent;
    return label.slice(0, label.length - node.querySelector(".label .dim.mono").textContent.length) + " | " + ref;
  }), selector);
}

async function check(browserType, name) {
  const failures = [];
  const fail = (m) => failures.push(`${name}: ${m}`);
  const same = (label, got, want) => {
    if (JSON.stringify(got) !== JSON.stringify(want)) fail(`${label}: ${JSON.stringify(got)}, expected ${JSON.stringify(want)}`);
  };
  const server = await serve();
  const base = `http://127.0.0.1:${server.address().port}`;
  const browser = await browserType.launch();
  const context = await browser.newContext({ viewport: { width: 1280, height: 860 }, deviceScaleFactor: 2 });
  await context.addInitScript(fakeBackend);
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  const heading = () => page.locator("#navigator .nav-head h2").textContent();
  const home = (code) => page.waitForFunction((c) => {
    const h2 = document.querySelector("#navigator .nav-head h2");
    return h2 && h2.textContent === c && !document.querySelector("#navigator ul.tree .loading");
  }, code, { timeout: 15000 });
  const top = "#navigator > ul.tree > li";
  await page.goto(base + "/");
  await page.waitForSelector("#navigator .node.group", { timeout: 15000 });

  // the rail: Projects below Agent
  same("rail", await page.evaluate(() => Array.prototype.map.call(document.querySelectorAll(".rail button[data-view]"), (b) => b.dataset.view)),
    ["home", "worklist", "changes", "search", "agent", "projects"]);

  // no group chosen yet: Home offers one's own groups; one click chooses
  same("groups offered on the first start", await page.evaluate(() => Array.prototype.map.call(document.querySelectorAll("#navigator .node.group strong"), (s) => s.textContent)),
    ["ROCKET", "ROVER"]);
  await page.locator("#navigator .node.group", { hasText: "ROVER" }).click();
  await home("ROVER");

  // Files is the mode Home starts with: folders first, then the files by name
  // (case-blind, digits by value), then the items without files
  same("modes", await page.evaluate(() => Array.prototype.map.call(document.querySelectorAll("#navigator .chip"), (c) => c.textContent + (c.classList.contains("active") ? "*" : ""))),
    ["Files*", "All items", "Structure"]);
  const long = "Bracket for the left rocker of the suspension – revised mounting pattern.ipt";
  same("the top of ROVER", await rows(page, top), [
    "[folder] Chassis 521", "[folder] wheels 3",
    "axle.ipt | PRT-0000999/A", long + " | PRT-0000042/A", "part2.ipt | PRT-0000310/A", "Part10.ipt | PRT-0000007/A",
    "Rover.iam | ASM-0000001/A", "Rover.idw | ASM-0000001/A",
    "-- Items without files", "Bought motor | PRT-0000003/A"]);
  same("rows of the group", await page.locator("#navigator .nav-sort .count").textContent(), "531");

  // the name is the first thing in a row and starts at its left; the long
  // name is cut at its end, never at its start
  const geometry = await page.evaluate(() => {
    const nav = document.getElementById("navigator").getBoundingClientRect();
    return Array.prototype.map.call(document.querySelectorAll("#navigator ul.tree .node.file"), (node) => {
      const label = node.querySelector(".label"), fname = node.querySelector(".fname"), box = fname.getBoundingClientRect();
      return { text: fname.textContent, first: label.firstChild === fname,
               inside: box.left >= nav.left && box.left < nav.left + 80, cut: label.scrollWidth > label.clientWidth,
               gap: Math.abs(label.getBoundingClientRect().left - box.left) };
    });
  });
  geometry.forEach((g) => {
    if (!g.first) fail(`${g.text}: the name is not the first thing in the row`);
    if (!g.inside) fail(`${g.text}: the name does not start at the left of the row`);
    if (g.gap > 1) fail(`${g.text}: something stands before the name`);
  });
  if (!geometry.find((g) => g.text === long).cut) fail("the long name was expected to be cut at its end in the narrow navigator");
  await page.screenshot({ path: path.join(shots, `home-check-files-${name}.png`) });

  // the order chosen on Home applies to the files as well
  await page.locator("#navigator .nav-sort select").selectOption("number");
  await home("ROVER");
  same("the top of ROVER by number", (await rows(page, top)).slice(2, 8), [
    "Rover.iam | ASM-0000001/A", "Rover.idw | ASM-0000001/A", "Part10.ipt | PRT-0000007/A",
    long + " | PRT-0000042/A", "part2.ipt | PRT-0000310/A", "axle.ipt | PRT-0000999/A"]);
  await page.locator("#navigator .nav-sort select").selectOption("name");
  await home("ROVER");

  // a folder opens its own level; the next page is loaded on request
  await page.locator("#navigator .node.folder", { hasText: "Chassis" }).click();
  const chassis = "#navigator .node.folder[aria-expanded='true'] + ul > li";
  await page.waitForSelector(chassis + " > .node.file");
  let inside = await rows(page, chassis + ":not(:last-child)");
  same("Chassis starts with", inside.slice(0, 4), ["[folder] brackets 1", "Bolt 1.ipt | PRT-0008999/A", "Bolt 2.ipt | PRT-0008998/A", "Bolt 3.ipt | PRT-0008997/A"]);
  same("Chassis shows one page", inside.length, 501);
  same("the rest is offered", await page.locator(chassis + ":last-child button").textContent(), "show 20 more of 520");
  await page.locator(chassis + ":last-child button").click();
  await page.waitForFunction((sel) => document.querySelectorAll(sel).length === 521, chassis);
  inside = await rows(page, chassis);
  same("Chassis after the second page", [inside.length, inside[10], inside[520]], [521, "Bolt 10.ipt | PRT-0008990/A", "Bolt 520.ipt | PRT-0008480/A"]);
  // leaving Home and coming back keeps the folder open
  await page.locator(".rail button[data-view='worklist']").click();
  await page.locator(".rail button[data-view='home']").click();
  await page.waitForSelector(chassis + " > .node.file");
  await page.locator("#navigator .node.folder[aria-expanded='true']").click();

  // a file opens its revision in a tab
  await page.locator("#navigator .node.file", { hasText: "Rover.iam" }).click();
  await page.waitForSelector("#tabs .tab");
  same("the opened tab", await page.locator("#tabs .tab .tab-label").textContent(), "ASM-0000001/A");

  // the chosen group survives a reload
  await page.reload();
  await home("ROVER");
  await page.waitForSelector("#navigator .node.file");

  // the favourite folder (issue #352): the folder of the Inventor project
  // file is suggested; Home then starts there under an accent banner
  const bar = "#navigator .home-root";
  const barText = () => page.locator(bar).textContent();
  await page.waitForSelector(bar + ".hint");
  same("the suggestion", await page.locator(bar + " p").textContent(),
    "The Inventor project file of this group is in Chassis/brackets. Start Home in that folder?");
  await page.locator(bar + " button", { hasText: "Start there" }).click();
  await page.waitForSelector(bar + ".on");
  await home("ROVER");
  await page.waitForSelector(top + " > .node.file");
  same("the favourite folder", await rows(page, top), ["Gusset.ipt | PRT-0000100/A"]);
  same("its banner", await page.locator(bar + " .home-root-path").textContent(), "ROVER / Chassis / brackets");
  if (!(await barText()).includes("not the top of the group")) fail("the banner does not say that this is not the top of the group");
  same("rows below the favourite", await page.locator("#navigator .nav-sort .count").textContent(), "1");
  const accent = await page.evaluate((sel) => {
    const el = document.querySelector(sel), probe = document.createElement("span");
    probe.style.color = "var(--accent)";
    document.body.appendChild(probe);
    const want = getComputedStyle(probe).color;
    probe.remove();
    return { border: getComputedStyle(el).borderTopColor, want };
  }, bar);
  same("the banner is in the accent colour", accent.border, accent.want);
  await page.screenshot({ path: path.join(shots, `home-check-favourite-${name}.png`) });
  // kept per computer: a reload starts there again
  await page.reload();
  await page.waitForSelector(bar + ".on");
  await home("ROVER");
  same("the favourite after a reload", await rows(page, top), ["Gusset.ipt | PRT-0000100/A"]);
  // the whole group, and back
  await page.locator(bar + " button", { hasText: "Whole group" }).click();
  await page.waitForSelector(bar + " .home-root-back");
  await home("ROVER");
  same("the whole group", (await rows(page, top)).slice(0, 2), ["[folder] Chassis 521", "[folder] wheels 3"]);
  same("no suggestion beside a favourite", await page.locator(bar + ".hint").count(), 0);
  await page.locator(bar + " .home-root-back").click();
  await page.waitForSelector(bar + ".on");
  await home("ROVER");
  same("back in the favourite", await rows(page, top), ["Gusset.ipt | PRT-0000100/A"]);
  // removed: the top again, with the suggestion; declined: no suggestion, also after a reload
  await page.locator(bar + " button", { hasText: "Remove favourite" }).click();
  await page.waitForSelector(bar + ".hint");
  await home("ROVER");
  same("the top after removing the favourite", (await rows(page, top)).slice(0, 2), ["[folder] Chassis 521", "[folder] wheels 3"]);
  await page.locator(bar + " button", { hasText: "No, thanks" }).click();
  await page.waitForFunction((sel) => !document.querySelector(sel).childNodes.length, bar);
  await page.reload();
  await home("ROVER");
  await page.waitForSelector("#navigator .node.file");
  same("a declined suggestion stays away", await page.locator(bar).count() ? await barText() : "", "");
  // a folder's star makes it the favourite; the filled star removes it
  const wheels = page.locator("#navigator .node.folder", { hasText: "wheels" });
  same("an empty star", [await wheels.locator(".fav").textContent(), await wheels.locator(".fav").getAttribute("aria-pressed")], ["☆", "false"]);
  await wheels.locator(".fav").click();
  await page.waitForSelector(bar + ".on");
  await home("ROVER");
  same("the starred folder", await rows(page, top), ["Rim.ipt | PRT-0000201/A", "Tyre.ipt | PRT-0000202/A", "Wheel.ipt | PRT-0000200/A"]);
  same("the starred folder's banner", await page.locator(bar + " .home-root-path").textContent(), "ROVER / wheels");
  await page.locator(bar + " button", { hasText: "Whole group" }).click();
  await page.waitForSelector(bar + " .home-root-back");
  await home("ROVER");
  same("a filled star on the favourite", await page.locator("#navigator .node.folder", { hasText: "wheels" }).locator(".fav").textContent(), "★");
  if (await page.locator("#navigator .node.folder", { hasText: "wheels" }).getAttribute("aria-expanded") !== "false") fail("the star opened the folder");
  await page.locator("#navigator .node.folder", { hasText: "wheels" }).locator(".fav").click();
  await page.waitForFunction((sel) => !document.querySelector(sel).childNodes.length, bar);
  await home("ROVER");
  same("the top without a favourite", (await rows(page, top)).slice(0, 2), ["[folder] Chassis 521", "[folder] wheels 3"]);

  // the Projects page marks the active group; a server without the file
  // browser gets the items of the group instead (All items, by name)
  await page.locator(".rail button[data-view='projects']").click();
  await page.waitForSelector("#navigator .node.group");
  same("project list", await page.evaluate(() => Array.prototype.map.call(document.querySelectorAll("#navigator .node.group"),
    (el) => el.querySelector(".twisty").textContent + el.querySelector("strong").textContent)), ["ROCKET", "✓ROVER", "PUMP"]);
  await page.screenshot({ path: path.join(shots, `home-check-projects-${name}.png`) });
  await page.locator("#navigator .node.group", { hasText: "PUMP" }).click();
  await home("PUMP");
  await page.waitForSelector(top + " > .node");
  same("PUMP on an older server", await rows(page, top), ["Impeller | PRT-0000501/A", "Pump P-200 | ASM-0000050/B", "volute casing | PRT-0000500/A"]);
  same("no favourite banner on an older server", await page.locator("#navigator .home-root").textContent(), "");

  // an empty group
  await page.locator(".rail button[data-view='projects']").click();
  await page.locator("#navigator .node.group", { hasText: "ROCKET" }).click();
  await home("ROCKET");
  // an empty group names the next step (#423): a CAD editor and member gets
  // Import CAD folder… and New item
  same("an empty group", await page.locator(top + " .empty").allTextContents(),
    ["This project group has no items yet. Import an existing CAD folder or create a new item."]);
  same("the next steps of an empty group", await page.locator(top + " .empty-hint button").allTextContents(), ["Import CAD folder…", "New item"]);

  // New item starts with the active group, not with the first one of the list
  await page.locator(".rail button[data-view='projects']").click();
  await page.locator("#navigator .node.group", { hasText: "ROVER" }).click();
  await home("ROVER");
  await page.locator("#navigator .nav-head button", { hasText: "New" }).click();
  await page.waitForSelector("form.dialog select");
  same("project group of a new item", await page.evaluate(() => Array.prototype.map.call(document.querySelectorAll("form.dialog select"), (s) => s.value)
    .filter((v) => /^[A-Z]+$/.test(v))), ["ROVER"]);
  const folderField = "form.dialog input[placeholder='(top of the group)']";
  same("folder of a new item from the top", await page.locator(folderField).inputValue(), "");
  await page.locator("form.dialog button", { hasText: "Cancel" }).click();

  // New item here: the + of a folder row opens New with the group and that
  // folder, without opening the folder
  const chassisRow = page.locator("#navigator .node.folder", { hasText: "Chassis" });
  await chassisRow.hover();
  await chassisRow.locator("button.folder-new").click();
  await page.waitForSelector(folderField);
  same("folder of New item here", await page.locator(folderField).inputValue(), "Chassis");
  same("project group of New item here", await page.evaluate(() => Array.prototype.map.call(document.querySelectorAll("form.dialog select"), (s) => s.value)
    .filter((v) => /^[A-Z]+$/.test(v))), ["ROVER"]);
  same("the folder stays closed", await chassisRow.getAttribute("aria-expanded"), "false");
  await page.locator("form.dialog button", { hasText: "Cancel" }).click();
  // New (Ctrl/Cmd+N) starts in the folder last opened
  await chassisRow.click();
  await page.locator("#navigator .nav-head button", { hasText: "New" }).click();
  await page.waitForSelector(folderField);
  same("folder of New after opening a folder", await page.locator(folderField).inputValue(), "Chassis");
  await page.locator("form.dialog button", { hasText: "Cancel" }).click();
  await chassisRow.click();

  // the other modes are still there for the active group
  await page.locator("#navigator .chip", { hasText: "Structure" }).click();
  await page.waitForSelector("#navigator ul.tree .node[data-item='ASM-0000001']");
  if (!(await page.evaluate(() => window.__calls.includes("GET /projects/ROVER/structure?limit=500")))) fail("the structure of the active group was not asked for");
  await page.locator("#navigator .chip", { hasText: "All items" }).click();
  await page.waitForFunction(() => window.__calls.includes("GET /items?project=ROVER&limit=500"));
  await page.locator("#navigator .chip", { hasText: "Files" }).click();
  await home("ROVER");
  await page.waitForSelector("#navigator .node.file");

  if (errors.length) fail("page errors: " + errors.join(" | "));
  await browser.close();
  server.close();
  return failures;
}

const engines = (process.env.HOME_CHECK_BROWSERS || "webkit,chromium").split(",").map((s) => s.trim()).filter(Boolean);
const unknown = engines.filter((n) => !["webkit", "chromium"].includes(n));
if (unknown.length || !engines.length) {
  console.error(`HOME_CHECK_BROWSERS: use webkit and/or chromium, not ${JSON.stringify(process.env.HOME_CHECK_BROWSERS)}`);
  process.exit(2);
}
const failures = [];
for (const name of engines) {
  try {
    failures.push(...await check(playwright[name], name));
  } catch (e) {
    failures.push(`${name}: ${String(e.message || e).split("\n")[0]}`);
  }
}
if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log(`home check passed in ${engines.join(" and ")}; screenshots in ${shots}`);
