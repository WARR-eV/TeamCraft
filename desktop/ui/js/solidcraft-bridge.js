// SPDX-License-Identifier: MPL-2.0
// Requests of the TeamCraft plugin for SolidCraft (src/scbridge.rs picks them
// up from the bridge folder, event `solidcraft-request`). A request is an
// intent from another program, so nothing happens without the user: *open*
// shows the revision, *check out* asks first, *check in* opens the usual
// check-in dialog.

import { confirmDialog, state, toast } from "./core.js";
import { openRevision } from "./revision.js";
import { findTab } from "./tabs.js";
import { checkinDialog, doCheckout } from "./workspace.js";

/** Calls `then(tab)` once the revision tab has its data (at most ~15 s). */
function whenLoaded(key, then) {
  var tries = 0;
  (function wait() {
    var tab = findTab(key);
    if (tab && tab.data && tab.data.rev) { then(tab); return; }
    if (++tries > 60) { toast("The revision did not open in time – try again from SolidCraft.", "info"); return; }
    setTimeout(wait, 250);
  })();
}

/** One request: `{action, item, revision, project}`. */
export function onSolidcraftRequest(r) {
  if (!r || !r.item || !r.revision) return;
  if (!state.me) {
    toast("SolidCraft asked TeamCraft about " + r.item + "/" + r.revision + " – sign in first, then try again.", "info");
    return;
  }
  var name = r.item + "/" + r.revision;
  openRevision(r.item, r.revision);
  var key = "rev:" + r.item + ":" + r.revision;
  if (r.action === "open") return;
  whenLoaded(key, function (tab) {
    if (r.action === "check_out") {
      confirmDialog("Check out from SolidCraft", "SolidCraft asks TeamCraft to check out " + name + ". Check it out now?", "Check out")
        .then(function (yes) { if (yes) doCheckout(tab); });
    } else if (r.action === "check_in") {
      toast("SolidCraft asks to check in " + name + ". Save the design in SolidCraft before checking in.");
      checkinDialog(tab, false);
    }
  });
}
