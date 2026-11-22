// SPDX-License-Identifier: MPL-2.0
// TeamCraft Desktop – pure rules of the empty states (#423): which message
// an empty list shows and which next step it offers, by role. The answers
// are message IDs of shared/i18n/ and action names; navigator.js turns them
// into text (shared/js/i18n.js t()) and buttons.
// No imports and no DOM: scripts/desktop-empty-rules.test.mjs tests it with
// plain Node.js.

/**
 * The hint of an empty list: `lead` (message ID of what is missing, or
 * null), `text` (message ID of the next step), the `actions` it offers in
 * order (`create_group`, `import_folder`, `new_item`, `get`) and, for a
 * viewer, the `note` that says why there are no editing buttons.
 */
function emptyHintOf(text, actions, note, lead) {
  return { lead: lead || null, text: text, actions: actions || [], note: note || null };
}

/** No project group to see. CAD editors create one (they become its
 *  manager); viewers ask a manager to add them. */
export function noGroupsHint(editor) {
  return editor
    ? emptyHintOf("empty.no_groups.editor", ["create_group"])
    : emptyHintOf("empty.no_groups.viewer", [], "empty.viewer_note");
}

/** A project group without items. `editor` = the user may create items in
 *  it (CAD editor and member, or administrator). */
export function emptyGroupHint(editor) {
  return editor
    ? emptyHintOf("empty.group_empty.editor", ["import_folder", "new_item"], null, "empty.group_empty")
    : emptyHintOf("empty.group_empty.viewer", [], null, "empty.group_empty");
}

/** The worklist section *New on this computer* without suggestions: the
 *  working folder has no new models – *Get…* copies a group's files there. */
export function noNewFilesHint() {
  return emptyHintOf("empty.no_new_files.get", ["get"], null, "empty.no_new_files");
}

/** An empty worklist section (`checkouts`, `reviews`, `changes`): one line
 *  on how work arrives there; viewers get the variant that says why they
 *  have none. Unknown sections: null. */
export function worklistHint(section, editor) {
  switch (section) {
    case "checkouts": return emptyHintOf(editor ? "empty.no_checkouts" : "empty.no_checkouts.viewer");
    case "reviews": return emptyHintOf("empty.nothing_to_review");
    case "changes": return emptyHintOf(editor ? "empty.no_open_changes" : "empty.no_open_changes.viewer");
    default: return null;
  }
}

/** The message ID of an action's button; null for an unknown action. */
export function emptyActionLabel(action) {
  return {
    create_group: "empty.action.create_group",
    import_folder: "empty.action.import_folder",
    new_item: "empty.action.new_item",
    get: "empty.action.get"
  }[action] || null;
}

/** Every message ID these rules can answer (the test checks the catalogs). */
export var EMPTY_MESSAGE_IDS = [
  "empty.no_groups.editor", "empty.no_groups.viewer", "empty.viewer_note",
  "empty.group_empty", "empty.group_empty.editor", "empty.group_empty.viewer",
  "empty.no_new_files", "empty.no_new_files.get",
  "empty.no_checkouts", "empty.no_checkouts.viewer", "empty.nothing_to_review",
  "empty.no_open_changes", "empty.no_open_changes.viewer",
  "empty.action.create_group", "empty.action.import_folder", "empty.action.new_item", "empty.action.get"
];
