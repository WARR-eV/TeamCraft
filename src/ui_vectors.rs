//! The rules the web UI renders in Rust and the desktop app in JavaScript
//! (`shared/js/`), pinned to the same test vectors in `shared/test-vectors/`.
//! `scripts/shared-vectors.test.mjs` runs the JavaScript side against the
//! same files, so a rule changed on one side only fails one of the two.

use serde_json::Value;

use crate::{assistant::render_markdown, domain, templates, util};

fn vectors(json: &str) -> Value {
    serde_json::from_str(json).expect("test vector JSON")
}

/// `[[value, digits], "text"]` cases of format.json.
fn number_cases(v: &Value, key: &str) -> Vec<(f64, usize, String)> {
    v[key]
        .as_array()
        .unwrap_or_else(|| panic!("format.json: {key}"))
        .iter()
        .map(|case| {
            (
                case[0][0].as_f64().unwrap(),
                case[0][1].as_u64().unwrap() as usize,
                case[1].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// `[integer, "text"]` cases of format.json.
fn integer_cases(v: &Value, key: &str) -> Vec<(i64, String)> {
    v[key]
        .as_array()
        .unwrap_or_else(|| panic!("format.json: {key}"))
        .iter()
        .map(|case| {
            (
                case[0].as_i64().unwrap(),
                case[1].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn labels() {
    let v = vectors(include_str!("../shared/test-vectors/labels.json"));
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 10);
    for case in cases {
        let code = case[0].as_str().map(str::to_string);
        assert_eq!(
            templates::filter_label(code.clone()),
            case[1].as_str().unwrap(),
            "label of {code:?}"
        );
    }
}

#[test]
fn numbers_sizes_and_ages() {
    let v = vectors(include_str!("../shared/test-vectors/format.json"));
    for (value, digits, text) in number_cases(&v, "num") {
        assert_eq!(
            util::format_number(value, digits),
            text,
            "num {value} {digits}"
        );
    }
    for (value, digits, text) in number_cases(&v, "fixed") {
        assert_eq!(
            templates::filter_fixed(Some(value), Some(digits)),
            text,
            "fixed {value} {digits}"
        );
    }
    for (bytes, text) in integer_cases(&v, "bytes") {
        assert_eq!(util::format_bytes(bytes), text, "bytes {bytes}");
    }
    for (seconds, text) in integer_cases(&v, "age") {
        assert_eq!(domain::format_age(seconds), text, "age {seconds}");
    }
}

#[test]
fn state_glyphs() {
    let v = vectors(include_str!("../shared/test-vectors/states.json"));
    let glyphs = v["glyphs"].as_object().unwrap();
    assert!(glyphs.len() >= 25);
    for (state, glyph) in glyphs {
        assert_eq!(
            domain::state_glyph(state),
            glyph.as_str().unwrap(),
            "{state}"
        );
    }
    for state in v["unknown"].as_array().unwrap() {
        assert_eq!(domain::state_glyph(state.as_str().unwrap()), "", "{state}");
    }
}

/// The macros emit the markup contract with the glyph of the table: a
/// badge is `span.badge.badge-<state>` with an `aria-hidden` glyph span
/// (when the state has one) and the label.
#[test]
fn badge_macro_uses_the_glyph_table() {
    let env = templates::environment("TeamCraft Test").expect("templates compile");
    let render = |state: &str| {
        env.render_str(
            "{% import \"macros.html\" as m %}{{ m.badge(s) }}|{{ m.state_glyph(s) }}",
            minijinja::context! { s => state },
        )
        .unwrap()
    };
    let v = vectors(include_str!("../shared/test-vectors/states.json"));
    for (state, glyph) in v["glyphs"].as_object().unwrap() {
        let glyph = glyph.as_str().unwrap();
        let label = templates::filter_label(Some(state.clone()));
        let html = render(state);
        let (badge, alone) = html.split_once('|').unwrap();
        assert_eq!(
            badge,
            format!(
                "<span class=\"badge badge-{state}\"><span class=\"glyph\" aria-hidden=\"true\">{glyph}</span>{label}</span>"
            ),
            "badge {state}"
        );
        assert!(
            alone.contains(&format!(">{glyph}</span>")),
            "state glyph {state}: {alone}"
        );
    }
    // an unknown state: the label alone, and the neutral dot in dense lists
    let html = render("state_from_a_newer_server");
    let (badge, alone) = html.split_once('|').unwrap();
    assert_eq!(
        badge,
        "<span class=\"badge badge-state_from_a_newer_server\">State From A Newer Server</span>"
    );
    assert!(alone.contains(">•</span>"));
}

/// Markdown-lite: the HTML of `render_markdown` without line breaks between
/// elements and without class attributes – the structure the vectors pin.
fn normalized(html: &str) -> String {
    let mut out = html.replace(">\n", ">");
    while let Some(start) = out.find(" class=\"") {
        let end = out[start + 8..]
            .find('"')
            .map(|e| start + 8 + e + 1)
            .unwrap();
        out.replace_range(start..end, "");
    }
    out
}

#[test]
fn markdown_lite() {
    let v = vectors(include_str!("../shared/test-vectors/markdown.json"));
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 15);
    for case in cases {
        let input = case[0].as_str().unwrap();
        assert_eq!(
            normalized(&render_markdown(input)),
            case[1].as_str().unwrap(),
            "markdown {input:?}"
        );
    }
}
