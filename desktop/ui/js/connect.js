// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – Connect and sign in.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { $, call, errorText, invoke, state, toast } from "./core.js";
import { enterShell } from "./shell.js";

// ======================================================================
// Connect / sign in
// ======================================================================

export function showConnect(step, error) {
  $("shell").classList.add("hidden");
  $("connect").classList.remove("hidden");
  $("server-form").classList.toggle("hidden", step !== "server");
  $("signin-step").classList.toggle("hidden", step !== "signin");
  // "Try TeamCraft on this computer" (downloads with local server only);
  // "starting": the app starts its local server before signing in
  // a server fixed by a policy (#378): prefilled and read-only, no local server
  var policy = state.session.policy || {};
  var locked = (policy.fixed || []).indexOf("server") !== -1;
  $("local-server").classList.toggle("hidden", step !== "server" || !state.session.local_server_available || locked);
  $("local-server-wait").classList.add("hidden");
  $("local-server-start").disabled = false;
  $("local-server-boot").classList.toggle("hidden", step !== "starting");
  $("signin-wait").classList.add("hidden");
  $("signin-browser").disabled = false;
  if (locked) $("server-url").value = policy.server_url;
  else if (!$("server-url").value) $("server-url").value = state.session.server_url || policy.server_url || "";
  $("server-url").readOnly = locked;
  $("server-url").title = locked ? "Set by your organization" : "";
  $("signin-server").textContent = state.session.server_url || "";
  var err = $("connect-error");
  err.textContent = error || "";
  err.classList.toggle("hidden", !error);
  if (step === "server") {
    $("connect-notice").classList.add("hidden");
    $("server-url").focus();
  }
  checkServerUrl($("server-url").value, $("server-http-warning"));
  checkServerUrl(state.session.server_url || "", $("signin-http-warning"));
}

/** Shows the warning while an address would reach another computer over
 * plain http:// (src/settings.rs, insecure_http). Per warning, the answer
 * to the newest address wins. */
export function checkServerUrl(url, warning) {
  var mine = warning.checkSeq = (warning.checkSeq || 0) + 1;
  if (!url.trim()) { warning.classList.add("hidden"); return; }
  invoke("server_url_insecure", { url: url }).then(function (insecure) {
    if (mine === warning.checkSeq) warning.classList.toggle("hidden", !insecure);
  }, function () { if (mine === warning.checkSeq) warning.classList.add("hidden"); });
}

/** A server of another version (`server_notice`): a warning, never an error,
 * and signing in stays possible. Shown once per server – in the connect box
 * before the sign-in, or as a toast in the main window when the app starts
 * signed in. */
export var versionNoticeShown = null; // the server whose notice the user has seen
export function checkServerVersion() {
  var server = state.session.server_url;
  return call("server_notice").then(function (n) {
    if (server !== state.session.server_url) return; // the user switched meanwhile
    var box = $("connect-notice");
    box.textContent = n || "";
    box.classList.toggle("hidden", !n);
    if (!n || versionNoticeShown === server) return;
    versionNoticeShown = server;
    if ($("connect").classList.contains("hidden")) toast(n, "warn");
  }, function () {});
}

export function initConnect() {
  initLocalServer();
  $("server-url").addEventListener("input", function () {
    checkServerUrl($("server-url").value, $("server-http-warning"));
  });
  $("server-form").addEventListener("submit", function (e) {
    e.preventDefault();
    var url = $("server-url").value;
    $("connect-error").classList.add("hidden");
    // a saved connection to that server comes back signed in, with its own
    // working folder (src/profiles.rs)
    call("set_server", { url: url }).then(function () {
      state.serverVersion = undefined; // another server: ask its version again (report.js)
      return call("session");
    }).then(function (s) {
      state.session = s;
      if (s.signed_in) { enterShell(); return; }
      showConnect("signin");
      checkServerVersion();
    }, function (err) { showConnect("server", errorText(err)); });
  });
  $("change-server").addEventListener("click", function () { showConnect("server"); });
  // Each attempt has a number; "cancel" resets the screen at once and makes
  // the pending attempt's late result irrelevant.
  var attempt = 0;
  $("signin-browser").addEventListener("click", function () {
    var mine = ++attempt;
    $("signin-browser").disabled = true;
    $("signin-wait").classList.remove("hidden");
    $("connect-error").classList.add("hidden");
    call("sign_in_browser").then(function () {
      if (mine !== attempt) return;
      state.session.signed_in = true;
      enterShell();
    }, function (err) {
      if (mine === attempt) showConnect("signin", errorText(err));
    });
  });
  $("signin-cancel").addEventListener("click", function () {
    attempt++;
    showConnect("signin");
    call("cancel_sign_in").catch(function () {});
  });
  $("token-form").addEventListener("submit", function (e) {
    e.preventDefault();
    call("sign_in_token", { token: $("token-input").value }).then(function () {
      $("token-input").value = "";
      state.session.signed_in = true;
      enterShell();
    }, function (err) { showConnect("signin", errorText(err)); });
  });
}

/** The app's own local test server is the configured one: start it before
 * signing in or opening the main window (src/localserver.rs). Resolves
 * with its address, which follows a changed port. */
export function startLocalServer() {
  showConnect("starting");
  return call("local_server_start").then(function (base) {
    state.session.server_url = base;
    return base;
  });
}

/** "Try TeamCraft on this computer": start the bundled server, use it as the
 * server and open the browser sign-in – on the first start the browser
 * shows the page that creates the administrator, then the confirmation. */
/** Starts the bundled server, makes it the app's server and opens the
 * browser sign-in (the button, and the first start of TeamCraft Local). */
export function tryLocalServer() {
  showConnect("server");
  $("local-server-start").disabled = true;
  $("local-server-wait").classList.remove("hidden");
  $("connect-error").classList.add("hidden");
  call("local_server_start").then(function (base) {
    return call("set_server", { url: base });
  }).then(function (base) {
    state.session.server_url = base;
    state.session.signed_in = false;
    state.session.local_server_in_use = true;
    $("server-url").value = base;
    showConnect("signin");
    checkServerVersion();
    $("signin-browser").click();
  }, function (err) { showConnect("server", errorText(err)); });
}

export function initLocalServer() {
  $("local-server-start").addEventListener("click", tryLocalServer);
}
