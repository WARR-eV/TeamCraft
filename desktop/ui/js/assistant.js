// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – the company AI: the Agent view's server backend (the
// shared assistant of the web UI's "Ask TeamCraft" sidebar, inline), and the
// links in answers (item numbers, "Open 3D preview").
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

// (`$` only for scripts/check-desktop-modules.mjs, which reads the `$` of the
// regular expressions below as a use)
import { $, api, call, errorText, h, toast } from "./core.js";
import { canEdit, refreshWorklistCount } from "./shell.js";
import { openRevision } from "./revision.js";

// ======================================================================
// Item numbers and preview links in answers (the answers themselves are
// markdown-lite built by shared/js/markdown.js)
// ======================================================================

/** Text with generated item numbers as links that open the item. */
export function itemLink(number) {
  return h("button.link", { type: "button", title: "Open " + number, on: { click: function () { openRevision(number, "latest"); } } }, number);
}

// item numbers follow the group's numbering scheme: any short token without
// a slash or blank (they become API path segments, encoded)
var ITEM_NUMBER = /^[^\/\s]{1,40}$/;
var REVISION_CODE = /^[^\/\s]{1,10}$/;

/** An "Open 3D preview" link of the company AI (open_3d_preview, the
 * path already checked by the shared module): opens the revision tab on its
 * 3D viewer with that file instead of leaving the app for the web page.
 * Anything that does not look like an item and a revision stays a plain
 * label (null: the shared module would make a web link, which the desktop
 * does not follow). */
export function previewLink(link) {
  var m = /^\/files\/([0-9a-f-]{36})\/preview$/.exec(link.url);
  if (!m || !ITEM_NUMBER.test(link.item) || !REVISION_CODE.test(link.revision)) {
    return h("span.tc-a-open", { text: link.label });
  }
  return h("button.tc-a-open", { type: "button", title: "Open " + link.item + "/" + link.revision + " in the 3D viewer", on: { click: function () { openRevision(link.item, link.revision, "viewer", m[1]); } } }, link.label);
}

// ----------------------------------------------------------------------
// The company AI: the shared assistant (shared/js/assistant.js, the same
// module as the web UI's sidebar) inline in the Agent tab, when the Agent
// view's backend is the server (src/acp.rs, choose_backend). The provider
// key stays on the server; the conversation lives there (one per user).
// ----------------------------------------------------------------------

export var companyAi = {
  host: null,    // the panel's element, kept across renders of the Agent tab
  panel: null,   // the mounted shared module
  ready: null,   // Promise of the panel
  info: null     // GET /assistant: provider, model, host
};

/** What the user looks at: a revision, change or request tab (the last one
 * opened before the Agent tab), else the dashboard. Identifiers only – the
 * server checks every one with the user's rights. */
export function assistantContext(tab) {
  if (tab && tab.kind === "revision") {
    var sub = tab.sub || "overview";
    var ctx = {
      kind: sub === "structure" ? "bom" : sub === "whereused" ? "where_used" : "revision",
      item: tab.number, revision: tab.code, view: sub === "whereused" ? "where-used" : sub
    };
    if (sub === "structure") ctx.rule = tab.bomRule || "latest";
    var project = tab.data && tab.data.item && tab.data.item.project_code;
    if (project) ctx.project = project;
    return ctx;
  }
  if (tab && tab.kind === "change") return { kind: "change", change: tab.number };
  if (tab && tab.kind === "request") return { kind: "request", request: tab.number };
  return { kind: "dashboard" };
}

/** Errors reach the shared module as Error objects with a message. */
export function assistantError(err) {
  if (err && err.name === "AbortError") return err;
  return new Error(errorText(err));
}

/** A question in the conversation; Cancel drops the request on the Rust side. */
export function assistantSend(body, signal) {
  var cancelled = false;
  if (signal) {
    signal.addEventListener("abort", function () {
      cancelled = true;
      call("assistant_cancel").catch(function () {});
    });
  }
  return call("assistant_send", { body: Object.assign({ thread: true }, body) }).catch(function (err) {
    if (cancelled) {
      var abort = new Error("Cancelled.");
      abort.name = "AbortError";
      throw abort;
    }
    throw assistantError(err);
  });
}

/** Where the questions go, for the panel's privacy line and the navigator. */
export function companyAiWhere() {
  var info = companyAi.info || {};
  return info.provider ? info.provider + (info.host ? ", " + info.host : "") : "";
}

/** The company AI panel of the Agent tab, mounted once per sign-in.
 * `getContext` gives the page context, `onBusy(busy)` follows a question. */
export function companyPanel(getContext, onBusy) {
  if (companyAi.host) return companyAi.host;
  var host = companyAi.host = h("section");
  companyAi.ready = Promise.all([
    import("/static/shared/js/assistant.js"),
    api("GET", "/assistant").catch(function () { return null; })
  ]).then(function (r) {
    if (companyAi.host !== host) return null; // signed out meanwhile
    companyAi.info = r[1] || {};
    var where = companyAiWhere();
    companyAi.panel = r[0].mount(host, {
      inline: true,
      title: "Company AI",
      canEdit: canEdit(),
      privacy: "the AI provider configured by your administrator" + (where ? " (" + where + ")" : ""),
      getContext: getContext,
      link: itemLink,
      openLink: previewLink,
      api: {
        thread: function () { return api("GET", "/assistant/thread").catch(function (e) { throw assistantError(e); }); },
        send: assistantSend,
        reset: function () { return api("DELETE", "/assistant/thread").catch(function (e) { throw assistantError(e); }); }
      },
      onBusy: onBusy,
      onDraftsMayHaveChanged: refreshWorklistCount
    });
    return companyAi.panel;
  }, function (err) {
    // the next render of the Agent tab tries again
    if (companyAi.host === host) { companyAi.host = null; companyAi.ready = null; }
    toast(errorText(err), "error");
    return null;
  });
  return host;
}

/** A question to the company AI is running. */
export function companyAiBusy() {
  return !!(companyAi.panel && companyAi.panel.isBusy());
}

/** Signing out: the next user starts with a fresh panel (no conversation of the last one). */
export function resetCompanyAi() {
  companyAi.host = null;
  companyAi.panel = null;
  companyAi.ready = null;
  companyAi.info = null;
}

/** The tab or view changed: the panel re-reads where the user is. */
export function assistantContextChanged() {
  if (companyAi.panel) companyAi.panel.contextChanged();
}
