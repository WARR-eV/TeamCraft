// SPDX-License-Identifier: MPL-2.0
// Unit tests of desktop/ui/js/update-rules.js (the update window: what
// *Update now* does per installation, the line about the server's version,
// where *Release page* leads and whether a transfer still runs – #377, #466,
// #468). Plain Node.js, no packages; run by the CI job `ui`:
//
//   node scripts/desktop-update-rules.test.mjs
//
// The module sits in a folder without package.json, so it is imported as a
// data: URL module instead of by path.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../desktop/ui/js/update-rules.js");
const src = fs.readFileSync(file, "utf8");
const { INSTALL_TEXT, updateAction, installText, serverNote, offeredMisfits, releasePageArgs, releasePageLabel, misfitQuestion, transferBusy } = await import(
  "data:text/javascript;base64," + Buffer.from(src).toString("base64"));

let n = 0;
function test(name, fn) { fn(); n++; console.log("ok - " + name); }

const fits = { server: "0.3.1", needs: "0.3", offered_fits: true, note: "Your server runs 0.3.1 and needs TeamCraft Desktop 0.3.x – this update fits it." };
const misfits = { server: "0.2.3", needs: "0.2", offered_fits: false, note: "Your server runs 0.2.3 and needs TeamCraft Desktop 0.2.x – version 0.3.0 may not work with it." };

test("each install kind gets its button and path (#466)", () => {
  assert.deepEqual(updateAction("windows-portable"), { label: "Update now", mode: "swap" });
  assert.deepEqual(updateAction("app-image"), { label: "Update now", mode: "swap" });
  assert.deepEqual(updateAction("windows-msi"), { label: "Update now", mode: "installer" });
  assert.deepEqual(updateAction("windows-setup"), { label: "Update now", mode: "installer" });
  assert.deepEqual(updateAction("mac-dmg"), { label: "Update now", mode: "open" });
  assert.deepEqual(updateAction("other"), { label: "Download", mode: "reveal" });
  assert.deepEqual(updateAction("package"), { label: "Download", mode: "reveal" });
  assert.deepEqual(updateAction(undefined), { label: "Download", mode: "reveal" });
});

test("the install text: a package manager's hint first", () => {
  assert.equal(installText({ install: "windows-portable" }), INSTALL_TEXT["windows-portable"]);
  assert.equal(installText({ install: "package", package_hint: "flatpak update x" }), "flatpak update x");
  assert.equal(installText({ install: "windows-msi", package_hint: "hint" }), "hint");
  assert.equal(installText({ install: "package" }), "");
  assert.equal(installText({ install: "unknown" }), "");
  assert.equal(installText(null), "");
});

test("the server line: hidden without a note, a warning when the version misfits (#468)", () => {
  assert.equal(serverNote(null), null);
  assert.equal(serverNote({ offered_fits: true, note: null }), null);
  assert.deepEqual(serverNote(fits), { text: fits.note, cls: "notice info" });
  assert.deepEqual(serverNote(misfits), { text: misfits.note, cls: "notice warn" });
});

test("Release page: the offered release, or all releases when it misfits (#468)", () => {
  assert.deepEqual(releasePageArgs(fits, "0.3.0"), { version: "0.3.0" });
  // never the "needs" value: "0.2" is no tag (releases/tag/v0.2 does not exist)
  assert.deepEqual(releasePageArgs(misfits, "0.3.0"), {});
  // the answer is not there yet, or there is no server: as before
  assert.deepEqual(releasePageArgs(null, "0.3.0"), { version: "0.3.0" });
  assert.equal(releasePageLabel(misfits), "All releases");
  assert.equal(releasePageLabel(fits), "Release page");
  assert.equal(releasePageLabel(null), "Release page");
});

test("Update now asks first when the version misfits the server (#468)", () => {
  assert.equal(misfitQuestion(fits, "0.3.0"), null);
  assert.equal(misfitQuestion(null, "0.3.0"), null);
  const q = misfitQuestion(misfits, "0.3.0");
  assert.match(q, /0\.3\.0 may not work with your server/);
  assert.match(q, /needs TeamCraft Desktop 0\.2\.x/);
  assert.match(q, /Update anyway\?$/);
  assert.equal(offeredMisfits({ offered_fits: true }), false);
  assert.equal(offeredMisfits({}), false, "no answer about the fit: nothing to ask");
});

test("a running transfer blocks the swap; idle status lines do not (#466)", () => {
  assert.equal(transferBusy({ transfers: 0, getVisible: false, packing: false }), false);
  assert.equal(transferBusy({ transfers: 1, getVisible: false, packing: false }), true, "a check-in or park");
  assert.equal(transferBusy({ transfers: 0, getVisible: true, packing: false }), true, "the Get bar");
  assert.equal(transferBusy({ transfers: 0, getVisible: false, packing: true }), true, "Pack and Go");
  assert.equal(transferBusy({}), false);
  assert.equal(transferBusy(null), false);
  // the rule reads no dialog status lines at all: a hint ("Type at least 2
  // characters."), an import report or the update window's own "Verified: …"
  // line never count
  assert.equal(transferBusy.length, 1);
  assert.ok(!/role=status|querySelector|document/.test(src), "no DOM in the rule");
});

console.log(`1..${n}`);
