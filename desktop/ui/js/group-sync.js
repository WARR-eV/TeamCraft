// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – Whole project groups locally.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { api, call, dialog, enc, errorText, field, h, state, toast } from "./core.js";
import { label } from "/static/shared/js/format.js";
import { groupsGetResult } from "./get-rules.js";
import { freshProjects } from "./shell.js";

// ------------------------------------------- whole project groups locally

/** Every item of a project group (all pages of the item list). */
export function projectItems(code) {
  var all = [];
  function page(offset) {
    return api("GET", "/items?project=" + enc(code) + "&limit=200&offset=" + offset).then(function (r) {
      all = all.concat(r.items);
      return r.items.length && all.length < r.total ? page(all.length) : all;
    });
  }
  return page(0);
}

/** Key of the groups fetched with *Get project groups* into this working
 *  folder (per folder: each connection has its own). */
function gotGroupsKey() {
  return "teamcraft.got-groups:" + String(state.session && state.session.workspace_dir || "");
}

/** Codes of the project groups a finished *Get project groups* copied into
 *  the current working folder on this computer – the first-start wizard
 *  (onboarding.js) skips its group step for them. Only a convenience: an
 *  empty answer (private window, cleared storage) just shows the step. */
export function gotGroups() {
  try {
    var list = JSON.parse(window.localStorage.getItem(gotGroupsKey()) || "[]");
    return Array.isArray(list) ? list.filter(function (c) { return typeof c === "string"; }) : [];
  } catch (e) { return []; }
}

/** Remembers the groups of a Get that ran to its end without a problem
 *  (not stopped, no failed item or name clash, rule *Latest revision*). */
export function rememberGotGroups(list) {
  var all = gotGroups();
  (list || []).forEach(function (c) { if (all.indexOf(c) === -1) all.push(c); });
  try { window.localStorage.setItem(gotGroupsKey(), JSON.stringify(all.slice(-200))); } catch (e) { /* storage blocked: the step shows again */ }
}

/** For new team members or a fresh computer: copies the files of every item
 * of the chosen project groups into the workspace, one folder per group.
 * `opts.project` (a group code) ticks only that group – the *Get…* of an
 * empty state starts with the active group (#423); without it, the groups
 * the user is a member of are ticked. */
export function getProjectsDialog(opts) { freshProjects().then(function () { openGetProjectsDialog(opts); }); }

export function openGetProjectsDialog(opts) {
  var preselect = opts && opts.project;
  function member(p) { return state.me.projects.indexOf(p.id) !== -1; }
  function ticked(p) { return preselect ? p.code === preselect : member(p); }
  var groups = state.projects.filter(function (p) { return p.active; }).sort(function (a, b) {
    return (member(b) - member(a)) || a.code.localeCompare(b.code);
  });
  var boxes = groups.map(function (p) {
    return { code: p.code, input: h("input", { type: "checkbox", checked: ticked(p) }), label: p.code + " – " + p.name + (member(p) ? " (member)" : "") };
  });
  // Latest first (#355): in a team that releases only before manufacturing,
  // most items are still in work – "released" would leave the working folder
  // almost empty and assemblies without their parts.
  var rule = h("select", null, [
    h("option", { value: "latest", text: "Latest revision, also in work" }),
    h("option", { value: "released", text: "Latest released revision – items without a release are skipped" })
  ]);
  var bar = h("progress.hidden", { max: "1", value: "0" });
  var report = h("p.small", { role: "status" });
  var stop = false, running = false, finished = false;
  var content = h("div.stack", null, [
    h("p", { text: "Copies the files of every item of the chosen project groups into your working folder – one folder per group with the folder structure of the files: " +
      state.session.workspace_dir + "/<group>/<folder>/<file>. Unchanged files are skipped, files you changed locally are kept, " +
      "two items with the same folder and file name are reported (never overwritten). Files you have not checked out are read-only." }),
    groups.length ? h("div.stack", null, boxes.map(function (b) { return h("label.check", null, [b.input, b.label]); }))
      : h("p.empty", { text: "There are no project groups yet." }),
    field("Revision", rule),
    bar, report
  ]);
  dialog("Get project groups", content, [
    { label: "Close", onClick: function (d) { stop = true; if (running) call("workspace_cancel").catch(function () {}); d.close(); } },
    {
      label: "Get files", primary: true, submit: true, onClick: function (d) {
        if (running) return;
        // the result is shown: Enter closes the dialog
        if (finished) { d.close(); return; }
        var codes = boxes.filter(function (b) { return b.input.checked; }).map(function (b) { return b.code; });
        if (!codes.length) { d.fail("Choose at least one project group."); return; }
        running = true; stop = false;
        busy(d, true);
        var done = { items: 0, files: 0, kept: 0, skipped: 0, clashes: 0, errors: [] };
        report.textContent = "Listing the items …";
        Promise.all(codes.map(function (code) {
          return projectItems(code).then(function (items) { return items.map(function (it) { return { project: code, item: it }; }); });
        })).then(function (lists) {
          var jobs = [].concat.apply([], lists);
          bar.max = Math.max(jobs.length, 1); bar.value = 0; bar.classList.remove("hidden");
          return jobs.reduce(function (chain, job, i) {
            return chain.then(function () {
              if (stop) return;
              var code = rule.value === "released" ? job.item.released_code : job.item.latest_code;
              bar.value = i + 1;
              report.textContent = "Item " + (i + 1) + " of " + jobs.length + ": " + job.project + " / " + job.item.number;
              if (!code) { done.skipped++; return; }
              return call("workspace_download", { project: job.project, item: job.item.number, code: code }).then(function (r) {
                done.items++; done.files += r.downloaded.length; done.kept += r.kept.length; done.clashes += r.clashes.length;
                if (r.cancelled) stop = true;
                r.failed.forEach(function (t) { done.errors.push(job.item.number + ": " + t); });
                r.clashes.forEach(function (c) { done.errors.push(c.path + ": " + c.file + " – taken by " + c.with); });
              }, function (err) { done.errors.push(job.item.number + ": " + errorText(err)); });
            });
          }, Promise.resolve());
        }).then(function () {
          running = false; finished = true;
          // only a complete copy: no problems, no clashes, the latest revision of every item
          if (!stop && !done.errors.length && rule.value === "latest") rememberGotGroups(codes);
          var result = groupsGetResult(done, stop, codes);
          showResult(d, content, result);
          // the toast stays for whoever closed the dialog meanwhile
          toast(result.title, result.outcome === "done" ? null : "error");
        }, function (err) { running = false; busy(d, false); d.fail(err); });
      }
    }
  ], { wide: true });
}

/** The dialog's buttons: [Close, Get files]. */
function buttons(d) { return d.form.querySelectorAll(".dialog-actions button"); }

/** While the files are copied, *Get files* waits and *Close* stops. */
function busy(d, on) {
  var b = buttons(d);
  b[0].textContent = on ? "Stop" : "Close";
  b[1].disabled = on;
  b[1].textContent = on ? "Getting files…" : "Get files";
}

/**
 * The end of the Get as its own step of the dialog (#449): a heading that
 * says whether it is done, the counts, every problem, and *Open working
 * folder* / *Close* in place of *Get files* – a plain status line under the
 * progress bar was easy to miss.
 */
function showResult(d, content, r) {
  var ok = r.outcome === "done";
  content.replaceChildren(
    h("div.notice." + (ok ? "ok" : r.outcome === "stopped" ? "warn" : "error") + ".get-result", { role: "status" }, [
      h("h3", null, [h("span.get-result-mark", { "aria-hidden": "true", text: ok ? "✓ " : "! " }), r.title]),
      h("ul.small", null, r.lines.map(function (l) { return h("li", { text: l }); }))
    ]),
    r.problems.length ? h("div.stack", null, [
      h("p.small", { text: r.problems.length === 1 ? "1 problem:" : r.problems.length + " problems:" }),
      h("ul.small.get-problems", null, r.problems.map(function (p) { return h("li.mono", { text: p }); }))
    ]) : null,
    r.outcome === "stopped" ? h("p.small.muted", { text: "Get the groups again to copy the rest – files already copied are skipped." }) : null
  );
  var b = buttons(d);
  var openFolder = h("button", { type: "button", on: { click: function () {
    call("workspace_open", { project: null, item: null, code: null, file: null }).catch(function (e) { toast(errorText(e), "error"); });
  } } }, "Open working folder");
  b[1].replaceWith(openFolder);
  b[0].textContent = "Close";
  b[0].classList.add("primary");
  b[0].focus();
}
