//! Translated UI (#379): the message catalogs of `shared/i18n/` for the web
//! UI's templates and server code; the desktop app reads the same files
//! (`shared/js/i18n.js`). Background: `docs/src/research/translated-ui.md`.
//!
//! A message is a sentence with named placeholders (`{count}`, `{item}`), or
//! an object of CLDR plural categories chosen by `count`. A message missing
//! in a language falls back to English; an ID missing in English shows the
//! ID (the guard tests below keep that from shipping). Values are inserted
//! in one pass after the lookup, so a value containing `{x}` is never
//! expanded; the templates escape the result like every other value.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use serde_json::{Map, Value};

/// The languages TeamCraft ships, English first (the fallback).
pub const LANGUAGES: &[&str] = &["en", "de"];

static CATALOGS: LazyLock<BTreeMap<&'static str, Map<String, Value>>> = LazyLock::new(|| {
    let parse = |text: &str| match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    };
    BTreeMap::from([
        ("en", parse(include_str!("../shared/i18n/en.json"))),
        ("de", parse(include_str!("../shared/i18n/de.json"))),
    ])
});

/// The shipped language for a tag (`de-AT` → `de`), if any.
pub fn supported(tag: &str) -> Option<&'static str> {
    let tag = tag.trim().to_ascii_lowercase();
    let base = tag.split(['-', '_']).next().unwrap_or("");
    LANGUAGES
        .iter()
        .copied()
        .find(|l| *l == tag)
        .or_else(|| LANGUAGES.iter().copied().find(|l| *l == base))
}

/// The language for an `Accept-Language` header: the tags by their `q`
/// weight (RFC 9110), each matched exactly, then by its base language;
/// English when none fits.
pub fn negotiate(accept: &str) -> &'static str {
    let mut tags: Vec<(f32, usize, &str)> = accept
        .split(',')
        .enumerate()
        .filter_map(|(i, part)| {
            let mut pieces = part.split(';');
            let tag = pieces.next()?.trim();
            let q = pieces
                .find_map(|p| p.trim().strip_prefix("q="))
                .map_or(Some(1.0), |q| q.trim().parse::<f32>().ok())?;
            (!tag.is_empty() && tag != "*" && q > 0.0).then_some((q, i, tag))
        })
        .collect();
    tags.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    tags.iter()
        .find_map(|(_, _, tag)| supported(tag))
        .unwrap_or("en")
}

/// The CLDR plural category of `n` in `lang` (English and German: `one`
/// for 1, else `other`).
pub fn plural_category(lang: &str, n: i64) -> &'static str {
    match lang {
        "en" | "de" if n == 1 => "one",
        _ => "other",
    }
}

/// Inserts `params` into `text` in one pass: `{name}` with a known name is
/// replaced, everything else (also inside inserted values) stays as it is.
pub fn fill(text: &str, params: &[(String, String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let value = after.find('}').and_then(|close| {
            let name = &after[..close];
            params
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| (v, close))
        });
        match value {
            Some((v, close)) => {
                out.push_str(v);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The text of a message in a language, plural chosen by `count`.
fn text_of(lang: &str, id: &str, count: Option<i64>) -> Option<String> {
    let message = CATALOGS.get(lang)?.get(id)?;
    match message {
        Value::String(s) => Some(s.clone()),
        Value::Object(forms) => {
            let category = plural_category(lang, count.unwrap_or(0));
            forms
                .get(category)
                .or_else(|| forms.get("other"))
                .and_then(Value::as_str)
                .map(str::to_string)
        }
        _ => None,
    }
}

/// Message `id` in `lang` with `params` filled in; English when the
/// language lacks it, the ID when English lacks it too.
pub fn t(lang: &str, id: &str, params: &[(String, String)]) -> String {
    let count = params
        .iter()
        .find(|(k, _)| k == "count")
        .and_then(|(_, v)| v.trim().parse::<i64>().ok());
    let text = text_of(lang, id, count)
        .or_else(|| text_of("en", id, count))
        .unwrap_or_else(|| id.to_string());
    fill(&text, params)
}

/// The placeholder names of a text (`{count}` → `count`).
pub fn placeholders(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close)
                if !after[..close].is_empty()
                    && after[..close]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_') =>
            {
                out.insert(after[..close].to_string());
                rest = &after[close + 1..];
            }
            _ => rest = after,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn languages_are_negotiated_exactly_then_by_base_then_english() {
        assert_eq!(negotiate("de-DE,de;q=0.9,en;q=0.8"), "de");
        assert_eq!(negotiate("de-AT"), "de");
        assert_eq!(negotiate("fr-FR,fr;q=0.9,de;q=0.5"), "de");
        assert_eq!(negotiate("en;q=0.4,de;q=0.6"), "de", "by weight");
        assert_eq!(negotiate("fr"), "en");
        assert_eq!(negotiate(""), "en");
        assert_eq!(negotiate("de;q=0,en"), "en", "q=0 means not wanted");
        assert_eq!(negotiate("*"), "en");
    }

    #[test]
    fn messages_fall_back_to_english_then_to_the_id() {
        assert_eq!(t("de", "nav.items", &[]), "Artikel");
        assert_eq!(t("en", "nav.items", &[]), "Items");
        assert_eq!(t("fr", "nav.items", &[]), "Items");
        assert_eq!(t("de", "no.such.message", &[]), "no.such.message");
    }

    #[test]
    fn plurals_and_placeholders() {
        let one = t("de", "nav.notifications.unread", &p(&[("count", "1")]));
        assert_eq!(one, "1 ungelesene Benachrichtigung");
        let many = t("en", "nav.notifications.unread", &p(&[("count", "3")]));
        assert_eq!(many, "3 unread notifications");
        assert_eq!(plural_category("en", 0), "other");
    }

    #[test]
    fn inserted_values_are_never_expanded() {
        let params = p(&[("a", "{b}"), ("b", "B")]);
        assert_eq!(fill("x {a} {b} {c} {", &params), "x {b} B {c} {");
        assert_eq!(fill("{}", &params), "{}");
    }

    /// Every catalog has only IDs of the English one, with the same
    /// placeholders and the plural categories of its language.
    #[test]
    fn catalogs_match_english() {
        let en = &CATALOGS["en"];
        assert!(!en.is_empty(), "shared/i18n/en.json is read");
        let texts = |v: &Value| -> Vec<String> {
            match v {
                Value::String(s) => vec![s.clone()],
                Value::Object(forms) => forms
                    .values()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
                _ => panic!("a message is text or plural forms: {v}"),
            }
        };
        let names = |v: &Value| -> BTreeSet<String> {
            texts(v).iter().flat_map(|t| placeholders(t)).collect()
        };
        for lang in LANGUAGES {
            let catalog = &CATALOGS[lang];
            for (id, message) in catalog {
                let english = en
                    .get(id)
                    .unwrap_or_else(|| panic!("{lang}: {id} is not in en.json"));
                assert_eq!(names(message), names(english), "{lang}: {id} placeholders");
                if let Value::Object(forms) = message {
                    assert!(forms.contains_key("other"), "{lang}: {id} has no `other`");
                    for category in forms.keys() {
                        assert!(
                            ["one", "other"].contains(&category.as_str()),
                            "{lang}: {id}: {category} is not a plural category of {lang}"
                        );
                    }
                }
            }
        }
    }

    /// The IDs `t("…")` in the templates and the shared scripts.
    fn used_ids(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let bytes = text.as_bytes();
        let mut at = 0;
        while let Some(found) = text[at..].find("t(\"") {
            let start = at + found;
            let before = start.checked_sub(1).map(|i| bytes[i]);
            let ident = before
                .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'$');
            let id_start = start + 3;
            if let Some(len) = text[id_start..].find('"') {
                let id = &text[id_start..id_start + len];
                let looks_like_id = !id.is_empty()
                    && id.contains('.')
                    && id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
                if !ident && looks_like_id {
                    out.push(id.to_string());
                }
            }
            at = id_start;
        }
        out
    }

    #[test]
    fn every_id_used_exists_in_english() {
        let root = env!("CARGO_MANIFEST_DIR");
        let mut files = Vec::new();
        // the desktop scripts are checked by scripts/shared-i18n.test.mjs
        for dir in ["templates", "shared/js"] {
            let mut stack = vec![std::path::PathBuf::from(root).join(dir)];
            while let Some(d) = stack.pop() {
                for entry in std::fs::read_dir(&d).unwrap().flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if path.extension().is_some_and(|e| e == "html" || e == "js") {
                        files.push(path);
                    }
                }
            }
        }
        let en = &CATALOGS["en"];
        let mut used = 0;
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            for id in used_ids(&text) {
                used += 1;
                assert!(
                    en.contains_key(&id),
                    "{}: {id} is not in shared/i18n/en.json",
                    file.display()
                );
            }
        }
        assert!(used > 0, "the templates use t(\"…\")");
        assert_eq!(
            used_ids(r#"alert("x.y") t("a.b") format("c.d")"#),
            vec!["a.b"]
        );
    }
}
