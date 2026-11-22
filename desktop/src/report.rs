//! *Report a problem* (#381): one reviewed report for any failure of the
//! desktop app – from *Help → Report a problem…* or the *Report…* link of an
//! error message.
//!
//! The report is prefilled from the failure: the title "<action>: <error>"
//! (a long error cut at a word, ended with "…"), a description with the
//! action, its key inputs (never file contents or item data), the error and
//! the versions, and the question "What were you trying to do?" (left out
//! when unanswered). The recent error messages of this session can be
//! attached; everything is redacted like crash reports (no home folder,
//! addresses or tokens, `crash::redact`). Nothing leaves the app without a
//! click: *Open issue form* (the text in the address when it is short, else
//! cut there and the full text on the clipboard), *Copy report*, *Save
//! report…*. Operators can point the form at their own tracker
//! (*Settings → Report problems to*).

use serde::Deserialize;

/// Longest title (characters).
pub const MAX_TITLE: usize = 100;
/// Longest value of one input (characters).
const MAX_INPUT: usize = 200;
/// Inputs shown at most.
const MAX_INPUTS: usize = 12;
/// Recent messages attached at most.
pub const MAX_LOG_LINES: usize = 50;

/// What the dialog collected; every text is redacted again here.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Problem {
    /// What the user did (`Check in PRT-000042/B`), may be empty.
    #[serde(default)]
    pub action: String,
    /// The error text, may be empty.
    #[serde(default)]
    pub error: String,
    /// Key inputs of the action (`file` → `bracket.ipt`).
    #[serde(default)]
    pub inputs: Vec<(String, String)>,
    /// The answer to "What were you trying to do?".
    #[serde(default)]
    pub answer: String,
    /// The version of the server the app is connected to, if known.
    #[serde(default)]
    pub server_version: Option<String>,
    /// Recent error messages of this session (the "log" attachment), when
    /// the user ticked it.
    #[serde(default)]
    pub log: Option<Vec<String>>,
}

/// `text` cut at a word boundary to at most `max` characters, ended with "…".
pub fn cut_at_word(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max {
        return text;
    }
    let head: String = text.chars().take(max.saturating_sub(1)).collect();
    let at_word = match head.rfind(' ') {
        Some(i) if i > max / 2 => &head[..i],
        _ => head.as_str(),
    };
    format!("{}…", at_word.trim_end_matches([' ', ',', ';', ':', '.']))
}

fn clean(text: &str, home: Option<&str>) -> String {
    crate::crash::redact(text, home)
}

/// The title: "<action>: <error>", either part alone when the other is
/// empty, "Problem in TeamCraft Desktop" without both.
pub fn title(p: &Problem, home: Option<&str>) -> String {
    let action = clean(p.action.trim(), home);
    let error = clean(p.error.trim(), home);
    let full = match (action.is_empty(), error.is_empty()) {
        (false, false) => format!("{action}: {error}"),
        (false, true) => action,
        (true, false) => error,
        (true, true) => "Problem in TeamCraft Desktop".to_string(),
    };
    cut_at_word(&full, MAX_TITLE)
}

/// The description, in GitHub-flavoured Markdown.
pub fn body(p: &Problem, home: Option<&str>) -> String {
    let mut out = String::new();
    if !p.action.trim().is_empty() {
        out.push_str(&format!("**Action:** {}\n\n", clean(p.action.trim(), home)));
    }
    let inputs: Vec<String> = p
        .inputs
        .iter()
        .filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
        .take(MAX_INPUTS)
        .map(|(k, v)| {
            format!(
                "- {}: `{}`",
                cut_at_word(&clean(k, home), 40),
                cut_at_word(&clean(v, home), MAX_INPUT).replace('`', "'")
            )
        })
        .collect();
    if !inputs.is_empty() {
        out.push_str(&format!("**Inputs:**\n{}\n\n", inputs.join("\n")));
    }
    if !p.error.trim().is_empty() {
        out.push_str(&format!(
            "**Error:**\n```text\n{}\n```\n\n",
            clean(p.error.trim(), home).replace("```", "'''")
        ));
    }
    out.push_str(&format!(
        "**Versions:** TeamCraft Desktop {} on {} ({}), server {}\n\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        p.server_version
            .as_deref()
            .map_or("unknown".to_string(), |v| clean(v, home))
    ));
    if !p.answer.trim().is_empty() {
        out.push_str(&format!(
            "**What were you trying to do?**\n{}\n\n",
            clean(p.answer.trim(), home)
        ));
    }
    if let Some(log) = p.log.as_ref().filter(|l| !l.is_empty()) {
        let start = log.len().saturating_sub(MAX_LOG_LINES);
        let lines: Vec<String> = log[start..]
            .iter()
            .map(|l| cut_at_word(&clean(l, home), 300).replace("```", "'''"))
            .collect();
        out.push_str(&format!(
            "<details><summary>Recent messages</summary>\n\n```text\n{}\n```\n</details>\n",
            lines.join("\n")
        ));
    }
    out.trim_end().to_string() + "\n"
}

/// The home folder that redaction replaces with `~`.
pub fn home() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .filter(|h| h.len() > 1)
}

/// The address of the issue form with title and body, at most
/// [`crate::crash::MAX_ISSUE_URL`] long – a longer body is cut there with a
/// note (the UI copies the full text). `base` is GitHub's form or the
/// operator's own tracker (`title` and `body` query parameters).
pub fn form_url(base: &str, title: &str, body: &str) -> Result<(String, bool), String> {
    let base = url::Url::parse(base).map_err(|e| format!("Not an address: {e}"))?;
    if !matches!(base.scheme(), "https" | "http") {
        return Err("The report address must be http(s).".into());
    }
    let mut text = body.to_string();
    let mut cut = false;
    loop {
        let mut url = base.clone();
        url.query_pairs_mut()
            .append_pair("title", title)
            .append_pair("body", &text);
        let s = url.to_string();
        if s.len() <= crate::crash::MAX_ISSUE_URL || text.len() < 200 {
            return Ok((s, cut));
        }
        cut = true;
        let keep: String = text.chars().take(text.chars().count() * 3 / 4).collect();
        text =
            format!("{keep}\n\n[… cut to fit the link – paste the full report from the clipboard]");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem() -> Problem {
        Problem {
            action: "Check in PRT-000042/B".into(),
            error: "The server refused /home/kim/CAD/rover/bracket.ipt: version 3 was checked in by somebody else (409) at https://plm.example.org/api/v1/revisions/abc with tc_pat_abcdef123456".into(),
            inputs: vec![
                ("file".into(), "/home/kim/CAD/rover/bracket.ipt".into()),
                ("comment".into(), "".into()),
            ],
            answer: String::new(),
            server_version: Some("0.2.11".into()),
            log: None,
        }
    }

    #[test]
    fn the_title_is_cut_at_a_word_and_redacted() {
        let t = title(&problem(), Some("/home/kim"));
        assert!(
            t.starts_with("Check in PRT-000042/B: The server refused ~/CAD/rover/bracket.ipt"),
            "{t}"
        );
        assert!(t.chars().count() <= MAX_TITLE && t.ends_with('…'), "{t}");
        assert!(!t.contains("kim") && !t.contains("tc_pat_"), "{t}");
        assert_eq!(
            title(&Problem::default(), None),
            "Problem in TeamCraft Desktop"
        );
        assert_eq!(cut_at_word("short one", 20), "short one");
        assert_eq!(cut_at_word("alpha beta gamma delta", 15), "alpha beta…");
    }

    #[test]
    fn the_body_names_action_inputs_error_versions_and_drops_an_unanswered_question() {
        let b = body(&problem(), Some("/home/kim"));
        assert!(b.contains("**Action:** Check in PRT-000042/B"));
        assert!(b.contains("- file: `~/CAD/rover/bracket.ipt`"), "{b}");
        assert!(!b.contains("- comment"), "empty inputs are left out");
        assert!(
            b.contains("**Error:**") && b.contains("[url]") && b.contains("[token]"),
            "{b}"
        );
        assert!(
            !b.contains("plm.example.org")
                && !b.contains("abcdef123456")
                && !b.contains("/home/kim")
        );
        assert!(b.contains("server 0.2.11"));
        assert!(
            !b.contains("What were you trying to do?"),
            "unanswered: left out"
        );
        let mut answered = problem();
        answered.answer = "Check in the bracket after the wall change".into();
        assert!(
            body(&answered, None).contains("**What were you trying to do?**\nCheck in the bracket")
        );
    }

    #[test]
    fn the_log_is_redacted_and_capped() {
        let mut p = problem();
        p.log = Some(
            (0..80)
                .map(|i| format!("{i}: failed at /home/kim/x"))
                .collect(),
        );
        let b = body(&p, Some("/home/kim"));
        assert!(b.contains("Recent messages") && b.contains("79: failed at ~/x"));
        assert!(!b.contains("\n29: failed"), "only the last {MAX_LOG_LINES}");
    }

    #[test]
    fn the_form_address_fits_and_says_when_it_was_cut() {
        let (short, cut) = form_url(
            "https://github.com/WARR-eV/TeamCraft/issues/new",
            "T",
            "body",
        )
        .unwrap();
        assert!(!cut && short.contains("title=T") && short.contains("body=body"));
        let long = "x ".repeat(10_000);
        let (url, cut) = form_url("https://help.example.org/new?queue=cad", "T", &long).unwrap();
        assert!(cut && url.len() <= crate::crash::MAX_ISSUE_URL && url.contains("queue=cad"));
        assert!(form_url("javascript:alert(1)", "T", "b").is_err());
        assert!(form_url("file:///etc/passwd", "T", "b").is_err());
    }
}
