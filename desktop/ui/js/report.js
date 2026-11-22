// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – Report a problem (#381): one reviewed report for any
// failure, from Help → Report a Problem… or the *Report…* link of an error
// message. The text is built and redacted in Rust (src/report.rs); nothing
// leaves the app without a click on *Open issue form*, *Copy report* or
// *Save report…*.

import { call, dialog, errorText, field, h, reportHook, state, toast } from "./core.js";

/** Opens the report dialog for `failure` = { action, error, inputs }. */
export function reportDialog(failure) {
  failure = failure || {};
  var recent = (state.recentErrors || []).slice(-50);
  var titleInput = h("input", { type: "text", maxlength: "200", spellcheck: "false" });
  var answer = h("textarea", { rows: "3", placeholder: "What were you trying to do? (optional – left out when empty)" });
  var withLog = h("input", { type: "checkbox", checked: recent.length > 0, disabled: recent.length === 0 });
  var preview = h("textarea.report-text", { rows: "12", spellcheck: "false", "aria-label": "The report as it will be sent" });
  var problem = {
    action: failure.action || "",
    error: failure.error || "",
    inputs: (failure.inputs || []).map(function (kv) { return [String(kv[0]), String(kv[1])]; }),
    server_version: state.serverVersion || null
  };
  // the server's version, asked once per app run (null when it does not answer)
  var versionAsked = state.serverVersion !== undefined ? Promise.resolve() : call("server_version").then(function (v) {
    state.serverVersion = v || null;
    problem.server_version = state.serverVersion;
  }, function () { state.serverVersion = null; });
  function rebuild() {
    problem.answer = answer.value;
    problem.log = withLog.checked ? recent : null;
    return call("problem_report", { problem: problem }).then(function (r) {
      if (!titleInput.dataset.edited) titleInput.value = r.title;
      preview.value = r.body;
      return r;
    });
  }
  titleInput.addEventListener("input", function () { titleInput.dataset.edited = "1"; });
  answer.addEventListener("change", rebuild);
  withLog.addEventListener("change", rebuild);
  function text() { return titleInput.value + "\n\n" + preview.value; }
  var target = state.session && state.session.report_url ? state.session.report_url : "the TeamCraft issue form on GitHub";
  var body = h("div.stack", null, [
    h("p.hint", { text: "Review the report – nothing is sent until you open the form, copy or save it. Home folder, server addresses and tokens are removed." }),
    field("Title", titleInput),
    field("What were you trying to do?", answer),
    h("label.check", null, [withLog, "Attach the recent messages of this session (" + recent.length + ")"]),
    recent.length ? h("details", null, [h("summary", { text: "Show the recent messages" }), h("pre.report-log", { text: recent.join("\n") })]) : null,
    field("Report", preview, "you can edit it"),
    h("p.small.muted", { text: "Open issue form: " + target + "." })
  ]);
  dialog("Report a problem", body, [
    { label: "Close", onClick: function (d) { d.close(); } },
    { label: "Save report…", onClick: function (d) {
      call("problem_report_save", { text: text() }).then(function (path) { if (path) toast("Saved " + path); }, function (e) { d.fail(e); });
    } },
    { label: "Copy report", onClick: function (d) {
      navigator.clipboard.writeText(text()).then(function () { toast("Report copied"); }, function (e) { d.fail(e); });
    } },
    { label: "Open issue form", primary: true, onClick: function (d) {
      call("problem_report_open", { title: titleInput.value, body: preview.value }).then(function (cut) {
        if (!cut) return;
        return navigator.clipboard.writeText(text()).then(function () {
          toast("The report was too long for the link – the full text is on the clipboard, paste it into the form.", "info");
        });
      }).catch(function (e) { d.fail(e); });
    } }
  ], { wide: true });
  versionAsked.then(rebuild).catch(function (e) { preview.value = errorText(e); });
}

reportHook.open = reportDialog;
