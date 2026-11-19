//! The glyph of a state: colour is never the only signal of a badge or a
//! state glyph in a dense list.
//!
//! The desktop app has the same table in `shared/js/states.js`; both are
//! pinned to `shared/test-vectors/states.json` (`src/ui_vectors.rs`,
//! `scripts/shared-vectors.test.mjs`).

/// The glyph shown before a state's label, `""` for none or an unknown
/// state (a value of a newer version gets no glyph, never an error).
pub fn state_glyph(state: &str) -> &'static str {
    match state {
        "released" | "approved" | "accepted" | "implemented" | "pass" | "sent" | "up_to_date"
        | "unchanged" => "✓",
        "in_review" | "pending" | "in_triage" | "retrying" | "out_of_date" | "modified"
        | "outdated" => "○",
        "rejected" | "fail" | "failed" | "conflict" | "clash" => "×",
        "obsolete" | "cancelled" | "withdrawn" | "deferred" | "missing" => "–",
        "in_work" | "draft" | "new" => "◐",
        "high" => "▲",
        "critical" => "▲▲",
        _ => "",
    }
}
