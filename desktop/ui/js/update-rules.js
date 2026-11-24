// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – pure rules of the update window (#377, #466, #468):
// what *Update now* does per kind of installation, the line about the
// server's version, where *Release page* leads and whether a check-in or
// download still runs.
// No imports and no DOM: scripts/desktop-update-rules.test.mjs tests it
// with plain Node.js. Used by shell.js (openUpdateWindow).

/** What *Update now* does for each kind of installation (src/update.rs InstallKind). */
export var INSTALL_TEXT = {
  "windows-msi": "Downloads the installer, checks it against the release's checksums, then closes TeamCraft and starts it.",
  "windows-setup": "Downloads the installer, checks it against the release's checksums, then closes TeamCraft and starts it.",
  "windows-portable": "Downloads the portable zip, checks it against the release's checksums, replaces the files of your copy and restarts TeamCraft.",
  "mac-dmg": "Downloads the disk image, checks it against the release's checksums and opens it – drag TeamCraft to Applications.",
  "app-image": "Downloads the AppImage, checks it against the release's checksums, replaces your copy with it and restarts TeamCraft.",
  package: "",
  other: "Download the new version from the release page."
};

/**
 * The button of the update window for an install kind: `mode` is
 * "swap" (portable zip / AppImage replaced in place), "installer" (MSI or
 * setup.exe started, the app closes), "open" (the .dmg opens) or "reveal"
 * (the download is shown in its folder).
 */
export function updateAction(install) {
  if (install === "windows-portable" || install === "app-image") return { label: "Update now", mode: "swap" };
  if (install === "windows-msi" || install === "windows-setup") return { label: "Update now", mode: "installer" };
  if (install === "mac-dmg") return { label: "Update now", mode: "open" };
  return { label: "Download", mode: "reveal" };
}

/** The text under the release notes: a package manager's hint first. */
export function installText(offer) {
  return (offer && (offer.package_hint || INSTALL_TEXT[offer.install])) || "";
}

/** The line about the connected server (`server_compat`, #468): `null` to
 *  hide it, else its text and class – a warning when the offered version
 *  does not fit the server. */
export function serverNote(compat) {
  if (!compat || !compat.note) return null;
  return { text: compat.note, cls: "notice " + (compat.offered_fits ? "info" : "warn") };
}

/** Does the offered version not fit the server? `false` while the answer
 *  is not there yet (`null`) or without a server. */
export function offeredMisfits(compat) {
  return !!(compat && compat.offered_fits === false);
}

/** The arguments of `open_release_page`: the offered release, or – when it
 *  does not fit the server – no version, i.e. the list of all releases,
 *  where the `needs`.x release is (`needs` is "0.2", no tag). */
export function releasePageArgs(compat, version) {
  return offeredMisfits(compat) ? {} : { version: version };
}

/** The label of the release page button. */
export function releasePageLabel(compat) {
  return offeredMisfits(compat) ? "All releases" : "Release page";
}

/** The question before *Update now* installs a version that does not fit
 *  the server; `null` when there is nothing to ask. */
export function misfitQuestion(compat, version) {
  if (!offeredMisfits(compat)) return null;
  return "Version " + version + " may not work with your server (it runs " + compat.server +
    " and needs TeamCraft Desktop " + compat.needs + ".x). Update anyway?";
}

/**
 * Does a check-in, park, upload or download run, so the app must not be
 * replaced or closed now? `s.transfers`: the count of running transfers
 * (core.js beginTransfer); `s.getVisible`: the Get progress in the status
 * bar; `s.packing`: a Pack and Go runs. Status lines of dialogs do not
 * count – a hint or a finished report is no transfer.
 */
export function transferBusy(s) {
  return !!(s && ((s.transfers || 0) > 0 || s.getVisible || s.packing));
}
