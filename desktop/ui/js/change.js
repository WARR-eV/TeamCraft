// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – Change tab.
// Part of the desktop UI (ES modules, no build step; js/main.js starts it).

import { api, append, badge, call, clear, confirmDialog, enc, errorNotice, errorText, h, lockBadge, moreMenu, priorityBadge, run, state, toast, tool, toolGroup } from "./core.js";
import { fmtDate, label } from "/static/shared/js/format.js";
import { canEdit, refreshWorklistCount, watchTool } from "./shell.js";
import { openTab, recordLink, reloadActive, renderTabs, table } from "./tabs.js";
import { agentButton } from "./agent.js";
import { dd, dt, openRevision } from "./revision.js";
import { relationText } from "./references.js";
import { draftBlocker, reviewerProblem, withdrawAction } from "./release-rules.js";
import { changeInputs } from "./report-inputs.js";

// ======================================================================
// Change tab
// ======================================================================

export function openChange(number) {
  openTab({ key: "chg:" + number, kind: "change", number: number, title: number });
}

export function loadChange(tab) {
  if (tab.data) return Promise.resolve(tab.data);
  return api("GET", "/changes/" + enc(tab.number)).then(function (r) {
    tab.data = r;
    tab.title = r.change.number + " " + r.change.title;
    renderTabs();
    return r;
  });
}

/**
 * Suggested reviewers of a draft change (GET …/suggested-reviewers): one
 * Add button per person with the reasons as text. Loads after the tab.
 */
export function suggestedReviewers(number) {
  var box = h("div.suggestions");
  api("GET", "/changes/" + enc(number) + "/suggested-reviewers").then(function (r) {
    if (!r.suggestions.length) return;
    box.appendChild(h("h3", { text: "Suggested reviewers" }));
    r.suggestions.forEach(function (s, i) {
      var whyId = "suggested-why-" + number + "-" + i;
      var why = h("p.hint", { id: whyId, text: s.reasons.join("; ") });
      if (s.cannot_review) {
        box.appendChild(h("div.suggestion", null, [
          h("p", null, [h("strong", { text: s.display_name }), " ", h("span.badge", { text: "knows the item, cannot review" })]),
          why,
        ]));
        return;
      }
      var add = h("button.small", { type: "button", "aria-describedby": whyId, on: { click: function () {
        add.disabled = true;
        run("Adding " + s.display_name, function () {
          return api("POST", "/changes/" + enc(number) + "/reviewers", { username: s.username });
        }, changeInputs({ number: number }, [["reviewer", s.username]])).then(function () { toast("Added reviewer " + s.display_name); reloadActive(); }, function () { add.disabled = false; });
      } } }, "Add " + s.display_name);
      box.appendChild(h("div.suggestion", null, [add, why]));
    });
  }, function () { /* suggestions are optional */ });
  return box;
}

// What one dependent of the impact analysis depends on: every item on its
// level (`paths`, #525), the first three spelled out; older servers
// send only `relation` and `via_number`.
function dependsOnText(dep) {
  var paths = dep.paths && dep.paths.length ? dep.paths : [{ relation: dep.relation, via_number: dep.via_number }];
  var text = paths.slice(0, 3).map(function (p) { return relationText(p.relation, p.via_number); }).join("; ");
  return paths.length > 3 ? text + " and " + (paths.length - 3) + " more" : text;
}

/**
 * "Add reviewer" of a draft: any CAD editor or administrator by user name
 * (several separated by commas), not only the suggestions – with nobody
 * suggested a draft would otherwise be a dead end (#291). The own name is
 * refused here already (four-eyes principle); the server checks the rest.
 */
function addReviewerForm(number) {
  var input = h("input", { type: "text", maxlength: "200", spellcheck: "false", placeholder: "user name",
    "aria-label": "User name of the reviewer to add" });
  var add = h("button.small", { type: "submit" }, "Add reviewer");
  return h("form.row.add-reviewer", { on: { submit: function (e) {
    e.preventDefault();
    var p = reviewerProblem([], input.value, state.me.username, false);
    if (p.error) { toast(p.error, "error"); input.focus(); return; }
    if (!p.reviewers.length) { input.focus(); return; }
    add.disabled = true;
    run("Adding " + p.reviewers.join(", "), function () {
      return p.reviewers.reduce(function (done, username) {
        return done.then(function () { return api("POST", "/changes/" + enc(number) + "/reviewers", { username: username }); });
      }, Promise.resolve());
    }, changeInputs({ number: number }, [["reviewers", p.reviewers.join(", ")]])).then(function () { toast("Added " + p.reviewers.join(", ")); reloadActive(); },
      function () { add.disabled = false; reloadActive(); });
  } } }, [input, add]);
}

/** "Remove" next to a reviewer of a draft. */
function removeReviewerButton(number, r) {
  var b = h("button.small", { type: "button", title: "Remove " + r.reviewer_name + " from " + number, on: { click: function () {
    b.disabled = true;
    run("Removing " + r.reviewer_name, function () {
      return api("DELETE", "/changes/" + enc(number) + "/reviewers/" + enc(r.reviewer_username));
    }, changeInputs({ number: number }, [["reviewer", r.reviewer_username]])).then(function () { toast("Removed reviewer " + r.reviewer_name); reloadActive(); }, function () { b.disabled = false; });
  } } }, "Remove");
  return b;
}

export function renderChange(tab, body, toolbar) {
  var c = tab.data.change, me = state.me;
  var owner = canEdit() && (c.created_by === me.id || me.role === "admin");
  var myReview = tab.data.reviews.filter(function (r) { return r.reviewer_id === me.id; })[0];
  var canReview = c.status === "in_review" && myReview && myReview.decision === "pending";
  function action(path, text) {
    run(text, function () { return api("POST", "/changes/" + enc(c.number) + "/" + path); }, changeInputs(c))
      .then(function () { toast(text + " – done"); reloadActive(); refreshWorklistCount(); }, function () {});
  }
  var notOwner = owner ? null : "Only " + c.created_by_name + " (who created it) or an administrator can do this";
  var isDraft = c.status === "draft";
  // a draft without revision or reviewer cannot be submitted: say why instead
  // of letting every click fail on the server (#291)
  var blocker = isDraft ? draftBlocker(tab.data.items.length, tab.data.reviews.length) : null;
  var canSubmit = owner && isDraft && !blocker;
  // Withdraw is the creator's action; an administrator's override sits in
  // More ▾ with a confirmation, never next to Approve change (#517)
  var withdraw = withdrawAction(c, me, canEdit(), label(c.status));
  function doWithdraw() {
    if (!withdraw.confirm) { action("withdraw", "Withdrawing " + c.number); return; }
    confirmDialog("Withdraw " + c.number, "Withdraw " + c.number + " from review as an administrator? " + c.created_by_name +
      " created it; the review ends and its revisions return to WIP.", "Withdraw", true).then(function (ok) {
      if (ok) action("withdraw", "Withdrawing " + c.number);
    });
  }
  var canCancel = owner && (isDraft || c.status === "rejected");
  // The review decision lives in the toolbar – the comment field below goes with it.
  // A visible label and hint as in the web review form (a placeholder would vanish once you type).
  var comment = canReview ? h("textarea.review-comment", { id: "review-comment", maxlength: "4000", "aria-describedby": "review-comment-hint" }) : null;
  function decide(decision) {
    if (decision === "reject" && !comment.value.trim()) {
      toast("Write in the comment why you reject " + c.number, "error");
      comment.focus();
      return;
    }
    run(decision === "approve" ? "Approving" : "Rejecting", function () {
      return api("POST", "/changes/" + enc(c.number) + "/review", { decision: decision, comment: comment.value });
    }, changeInputs(c, [["decision", decision]])).then(function (r) {
      toast(c.number + " – " + (decision === "approve" ? "approved" : "rejected") + "; status " + label(r.change.status));
      reloadActive(); refreshWorklistCount();
    }, function () {});
  }
  append(toolbar, [
    canReview ? toolGroup("Review", [
      tool("Approve change", "approve", function () { decide("approve"); }, { primary: true, title: "Approve " + c.number + " (with the comment below, if any)" }),
      tool("Reject", "cancel", function () { decide("reject"); }, { danger: true, title: "Reject " + c.number + " – the comment below says why; the revisions go back to WIP" })
    ]) : null,
    toolGroup("Lifecycle", [
      tool("Submit for review", "checkin", function () { action("submit", "Submitting " + c.number); }, {
        primary: canSubmit, disabled: !canSubmit,
        title: canSubmit ? "Send it to the reviewers" : notOwner || blocker || "Only a draft is submitted – this one is " + label(c.status)
      }),
      withdraw.place === "toolbar" ? tool(withdraw.label, "cancel", doWithdraw, { disabled: !withdraw.enabled, title: withdraw.title }) : null,
      // acts at once, like the web UI: cancelling one's own change needs no confirmation
      tool("Cancel change", "obsolete", function () { action("cancel", "Cancelling " + c.number); }, {
        danger: true, disabled: !canCancel,
        title: canCancel ? "Cancel " + c.number + " at once (cannot be undone) – its revisions are free for another change"
          : notOwner || "Only a draft or a rejected change can be cancelled – this one is " + label(c.status)
      })
    ]),
    toolGroup("Collaborate", [
      watchTool(tab, "changes", c.number, !!tab.data.watching),
      agentButton(tab)
    ]),
    moreMenu([
      withdraw.place === "more" ? { label: withdraw.label, icon: "cancel", danger: true, disabled: !withdraw.enabled, run: doWithdraw, title: withdraw.title } : null,
      { label: "Open in browser", separated: withdraw.place === "more", icon: "web", run: function () { call("open_web", { path: "/changes/" + c.id }).catch(function (e) { toast(errorText(e), "error"); }); }, title: "Open this change in the web UI (items, reopen …)" },
      { label: "Refresh", icon: "refresh", run: reloadActive, shortcut: "F5", title: "Load this change again" }
    ])
  ]);
  body.appendChild(h("div.object-head", null, [h("div", null, [
    h("h1", null, [h("span.mono", { text: c.number }), " · " + c.title]),
    h("div.meta", null, [badge(c.status), priorityBadge(c.priority), h("span", { text: "created " + fmtDate(c.created_at) + " by " + c.created_by_name })])
  ])]));
  var hints = {
    draft: "Draft: add the revisions to release and at least one reviewer – somebody other than you – then submit.",
    in_review: "In Review: all reviewers must approve; a single rejection sends the revisions back to WIP.",
    rejected: "Rejected: fix the revisions, then reopen and resubmit in the web UI – or cancel the change here.",
    released: "Released: all revisions of this change are released."
  };
  if (hints[c.status]) body.appendChild(h("p.notice.info", { text: hints[c.status] }));

  if (canReview) {
    body.appendChild(h("section.panel", null, [h("h2", { text: "Your review" }),
      h("label", { for: "review-comment" }, ["Comment", comment]),
      h("p.hint.review-hint", { id: "review-comment-hint", text: "A rejection needs a comment: what has to change. Approve change and Reject are in the toolbar above." })]));
  }

  append(body, h("div.grid-2", null, [
    h("section.panel", null, [h("h2", { text: "Details" }), h("dl.props", null, [
      dt("Description"), dd(h("span.pre", { text: c.description || "—" })),
      dt("Submitted"), dd(fmtDate(c.submitted_at)),
      dt("Closed"), dd(fmtDate(c.closed_at)),
      dt("Last update"), dd(fmtDate(c.updated_at))
    ])]),
    h("section.panel", null, [h("h2", { text: "Reviewers" }),
      tab.data.reviews.length ? table(["Reviewer", "Decision", "Comment"].concat(owner && isDraft ? [""] : []), tab.data.reviews.map(function (r) {
        var cells = [r.reviewer_name, badge(r.decision), h("span.pre", { text: r.comment || "" })];
        if (owner && isDraft) cells.push(removeReviewerButton(c.number, r));
        return { cells: cells };
      }), { nowrap: [0, 1] }) : h("p.empty", { text: "No reviewers assigned yet." }),
      owner && isDraft ? addReviewerForm(c.number) : null,
      owner && isDraft ? suggestedReviewers(c.number) : null])
  ]));
  body.appendChild(h("section.panel", null, [h("h2", { text: "Affected revisions" }),
    tab.data.items.length ? table(["Revision", "Name", "State"], tab.data.items.map(function (i) {
      var open = function () { openRevision(i.item_number, i.code); };
      return { cells: [recordLink(i.item_number + "/" + i.code, open), i.name, h("span", null, [badge(i.state), i.checked_out_by_name ? lockBadge(i.checked_out_by_name, i.checked_out_by === state.me.id) : null])], open: open };
    })) : h("p.empty", { text: "No revisions yet." })]));
  // Release readiness: unreleased dependencies block the submit.
  if ((c.status === "draft" || c.status === "rejected") && tab.data.items.length) {
    var readyPanel = h("section.panel", null, [h("h2", { text: "Release readiness" }), h("p.muted.small", { text: "Checking …" })]);
    body.appendChild(readyPanel);
    api("GET", "/changes/" + enc(c.number) + "/readiness").then(function (r) {
      clear(readyPanel);
      var rows = r.blockers.concat(r.warnings);
      var addable = r.blockers.some(function (d) { return d.addable; });
      append(readyPanel, [
        h("div.row", null, [h("h2", { text: "Release readiness" }),
          owner && c.status === "draft" && addable ? h("button.primary.small", { type: "button", on: { click: function () {
            run("Adding the dependencies", function () { return api("POST", "/changes/" + enc(c.number) + "/dependencies"); }, changeInputs(c))
              .then(function (res) { toast(res.added.length ? "Added " + res.added.join(", ") : "Nothing to add"); reloadActive(); }, function () {});
          } } }, "Add all to this change") : null]),
        r.ready
          ? h("p.notice.ok", { text: "Ready: every BOM line and reference is released or part of this change." })
          : h("p.notice.error", { text: r.blockers.length + (r.blockers.length === 1 ? " dependency is" : " dependencies are") + " not released – add them to this change or release them first, then submit." }),
        rows.length ? table(["Dependency", "Name", "Needed by", "State"], rows.map(function (d) {
          var note = d.other_change ? " in " + d.other_change : d.problem === "obsolete" ? " – obsolete, replace it" : !d.blocking ? " – found by the CAD file scan" : "";
          return {
            cells: [h("span.mono", { text: d.item_number + "/" + d.latest_code }), d.item_name,
              h("span", null, [h("span.mono", { text: d.revision_label }), " " + relationText(d.relation)]),
              h("span", null, [badge(d.latest_state), h("span.dim", { text: note })])],
            open: function () { openRevision(d.item_number, d.latest_code); }
          };
        })) : null
      ]);
    }, function (err) { clear(readyPanel); append(readyPanel, [h("h2", { text: "Release readiness" }),
      errorNotice("p.notice.error", errorText(err), "Checking the release readiness of " + c.number, changeInputs(c))]); });
  }
  // Impact analysis: the items that depend on the revisions of this change.
  var impactPanel = h("section.panel", null, [h("h2", { text: "Impact analysis" }), h("p.muted.small", { text: "Loading …" })]);
  body.appendChild(impactPanel);
  api("GET", "/changes/" + enc(c.number) + "/impact").then(function (list) {
    clear(impactPanel);
    append(impactPanel, [h("h2", { text: "Impact analysis" }),
      list.length ? h("p.hint", { text: c.status === "released"
        ? "These items outside the change depended on the released revisions and were flagged for a check."
        : "These items outside the change depend on its revisions, each on its nearest level; on approval they are flagged for a check." }) : null,
      list.length ? table(["Level", "Revision", "Name", "Depends on", "State"], list.map(function (dep) {
        return {
          cells: [String(dep.level), h("span.mono", { text: dep.item_number + "/" + dep.revision_code }),
            // servers before #525 still list the change's own revisions, marked in_change
            dep.in_change ? h("span", null, [dep.item_name, h("span.badge.badge-member", { text: " in this change" })]) : dep.item_name,
            dependsOnText(dep), badge(dep.revision_state)],
          open: function () { openRevision(dep.item_number, dep.revision_code); }
        };
      }), { numeric: [0] }) : h("p.empty", { text: "No other item uses or references the revisions of this change." })]);
  }, function () {
    clear(impactPanel);
    append(impactPanel, [h("h2", { text: "Impact analysis" }), h("p.empty", { text: "Not available on this server." })]);
  });
}
