//! Static files embedded into the binary – stylesheets, scripts, logos and
//! fonts – and how they are served.
//!
//! Two sources: the explicit list below (`static/`, `media/`: files of the web
//! UI) and `shared/`, the files the web UI shares with the desktop app.
//! `build.rs` walks `shared/` and writes its table into `OUT_DIR`; every file
//! there is served at `/static/shared/<path>` (README, tests and test vectors
//! are skipped – the same rule as in `desktop/build.rs`).
//!
//! Cache busting: pages link every asset with `?v=<assets version>`, a hash
//! over all embedded files. A request with the current version may be cached
//! for a year (`immutable`); any other request – an outdated link, a font
//! referenced from CSS – has to be revalidated (`no-cache` + `ETag`). So a
//! browser never shows an old stylesheet or logo after the server was updated.
//!
//! Shared scripts are linked through a versioned path instead,
//! `/static/shared/v/<assets version>/js/…`: ES modules import each other with
//! relative URLs, which drop a `?v=` query but stay inside the versioned
//! directory. The same rules apply – the current version is `immutable`, any
//! other version is still served, with `no-cache`, so an outdated page works.

use std::sync::OnceLock;

use axum::{
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};

const CSS: &str = "text/css; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";
const SVG: &str = "image/svg+xml";
const WOFF2: &str = "font/woff2";
const TXT: &str = "text/plain; charset=utf-8";
const JSON: &str = "application/json";

/// URL prefix of the files from `shared/`.
pub const SHARED_PREFIX: &str = "/static/shared/";

/// One embedded file and the URL path it is served at.
pub struct Asset {
    pub url: &'static str,
    pub content_type: &'static str,
    pub body: &'static [u8],
}

macro_rules! assets {
    ($( $url:literal => $file:literal, $ct:expr; )*) => {
        &[$( Asset { url: $url, content_type: $ct, body: include_bytes!(concat!("../", $file)) } ),*]
    };
}

/// Files of the web UI only.
static LISTED: &[Asset] = assets![
    "/static/style.css" => "static/style.css", CSS;
    "/static/app.js" => "static/app.js", JS;
    "/static/media/logos/TeamCraft_Icon.svg" => "media/logos/TeamCraft_Icon.svg", SVG;
    "/static/media/logos/TeamCraft_Logo_Blue.svg" => "media/logos/TeamCraft_Logo_Blue.svg", SVG;
    "/static/media/logos/TeamCraft_Logo_White.svg" => "media/logos/TeamCraft_Logo_White.svg", SVG;
    "/static/media/icons/Green_tick.svg" => "media/icons/Green_tick.svg", SVG;
    "/static/media/icons/Red_cross.svg" => "media/icons/Red_cross.svg", SVG;
    "/static/media/icons/White_plus.svg" => "media/icons/White_plus.svg", SVG;
    "/static/media/icons/Yellow_circle.svg" => "media/icons/Yellow_circle.svg", SVG;
];

/// Files from `shared/`, listed by `build.rs`.
const SHARED: &[Asset] = include!(concat!(env!("OUT_DIR"), "/shared_assets.rs"));
const _: () = assert!(!SHARED.is_empty(), "build.rs found no files in shared/");

/// Every embedded asset.
pub fn all() -> impl Iterator<Item = &'static Asset> {
    LISTED.iter().chain(SHARED.iter())
}

/// URL of the favicon (the TeamCraft icon).
pub const FAVICON: &str = "/static/media/logos/TeamCraft_Icon.svg";

fn hashes() -> &'static (String, Vec<String>) {
    static HASHES: OnceLock<(String, Vec<String>)> = OnceLock::new();
    HASHES.get_or_init(|| {
        let mut whole = Sha256::new();
        let per_asset = all()
            .map(|a| {
                whole.update(a.url.as_bytes());
                whole.update(a.body);
                hex::encode(&Sha256::digest(a.body)[..8])
            })
            .collect();
        (hex::encode(&whole.finalize()[..6]), per_asset)
    })
}

/// Version of all embedded assets (12 hex digits), used as `?v=` in links
/// and in the versioned path of shared scripts.
pub fn version() -> &'static str {
    &hashes().0
}

pub fn find(url: &str) -> Option<&'static Asset> {
    all().find(|a| a.url == url)
}

/// The asset for a request path, and whether the request names the current
/// version (`?v=<version>`, or the path `/static/shared/v/<version>/…`).
pub fn resolve(path: &str, query: Option<&str>) -> Option<(&'static Asset, bool)> {
    let versioned = path
        .strip_prefix(SHARED_PREFIX)
        .and_then(|rest| rest.strip_prefix("v/"))
        .and_then(|rest| rest.split_once('/'));
    if let Some((v, rest)) = versioned {
        let asset = find(&format!("{SHARED_PREFIX}{rest}"))?;
        return Some((asset, v == version()));
    }
    let current = query.is_some_and(|q| q.split('&').any(|p| p == format!("v={}", version())));
    find(path).map(|asset| (asset, current))
}

/// The asset as a response, honouring `If-None-Match` and the cache rules
/// described at the top of this module (`current`: see [`resolve`]).
pub fn respond(asset: &'static Asset, current: bool, headers: &HeaderMap) -> Response {
    let index = all().position(|a| std::ptr::eq(a, asset)).unwrap_or(0);
    let etag = format!("\"{}\"", hashes().1[index]);
    let cache = if current {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let not_modified = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|t| t.trim() == etag || t.trim() == "*"));
    let mut response = if not_modified {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(asset.content_type),
            )],
            asset.body,
        )
            .into_response()
    };
    let h = response.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    if let Ok(v) = HeaderValue::from_str(&etag) {
        h.insert(header::ETAG, v);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_lookup() {
        assert_eq!(version().len(), 12);
        assert!(version().chars().all(|c| c.is_ascii_hexdigit()));
        let mut urls: Vec<_> = all().map(|a| a.url).collect();
        let count = urls.len();
        urls.sort();
        urls.dedup();
        assert_eq!(urls.len(), count, "unique URLs");
        assert!(find("/static/style.css").is_some());
        assert!(find("/static/shared/css/skins.css").is_some());
        assert!(find("/static/shared/js/viewer.js").is_some());
        assert!(find(FAVICON).is_some());
        assert!(find("/static/../Cargo.toml").is_none());
        assert!(find("/static/shared/../../Cargo.toml").is_none());
        // every skin logo is embedded
        for skin in crate::domain::SKINS {
            assert!(
                find(&format!("/static/media/{}", skin.logo)).is_some(),
                "{}",
                skin.logo
            );
        }
    }

    #[test]
    fn cache_rules() {
        let (asset, current) =
            resolve("/static/style.css", Some(&format!("v={}", version()))).unwrap();
        assert!(current);
        let fresh = respond(asset, current, &HeaderMap::new());
        assert_eq!(fresh.status(), StatusCode::OK);
        assert!(fresh.headers()[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable"));
        let (asset, current) = resolve("/static/style.css", Some("v=0.1.0")).unwrap();
        assert!(!current);
        let old = respond(asset, current, &HeaderMap::new());
        assert_eq!(old.headers()[header::CACHE_CONTROL], "no-cache");
        let etag = old.headers()[header::ETAG].clone();
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, etag);
        let again = respond(asset, false, &headers);
        assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    }

    /// Shared scripts under `/static/shared/v/<version>/…`: the current
    /// version is immutable, any other version is served for revalidation.
    #[test]
    fn versioned_shared_paths() {
        let viewer = find("/static/shared/js/viewer.js").unwrap();
        let path = format!("/static/shared/v/{}/js/viewer.js", version());
        let (asset, current) = resolve(&path, None).unwrap();
        assert!(std::ptr::eq(asset, viewer) && current);
        let (asset, current) = resolve("/static/shared/v/000000000000/js/viewer.js", None).unwrap();
        assert!(std::ptr::eq(asset, viewer) && !current);
        let (_, current) = resolve("/static/shared/js/viewer.js", None).unwrap();
        assert!(!current);
        assert!(resolve("/static/shared/v/x/js/missing.js", None).is_none());
        assert!(resolve("/static/shared/v/x/../../style.css", None).is_none());
        // only shared files have a versioned path
        assert!(resolve(
            &format!("/static/shared/v/{}/../style.css", version()),
            None
        )
        .is_none());
        assert_eq!(viewer.content_type, JS);
    }

    /// Every file of a directory below the repository root, recursively,
    /// as `/`-separated paths relative to it.
    fn files_below(dir: &str) -> Vec<String> {
        fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    let rel = path.strip_prefix(root).unwrap().to_str().unwrap();
                    out.push(rel.replace('\\', "/"));
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
        let mut out = Vec::new();
        walk(&root, &root, &mut out);
        out.sort();
        out
    }

    /// The README, tests and test vectors of `shared/` stay in the repository
    /// (same rule as `build.rs` and `desktop/build.rs`).
    fn shared_servable(rel: &str) -> bool {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        !(rel.starts_with("test-vectors/")
            || name == "README.md"
            || name.ends_with(".test.js")
            || name.starts_with('.'))
    }

    /// The server embeds exactly the servable files of `shared/`, byte for
    /// byte, at `/static/shared/<path>` (the desktop app has the mirror test
    /// for its copy in `desktop/src/lib.rs`).
    #[test]
    fn shared_files_are_served() {
        let expected: Vec<String> = files_below("shared")
            .into_iter()
            .filter(|rel| shared_servable(rel))
            .collect();
        assert!(expected.contains(&"css/skins.css".to_string()));
        let mut served: Vec<String> = all()
            .filter_map(|a| a.url.strip_prefix(SHARED_PREFIX))
            .map(str::to_string)
            .collect();
        served.sort();
        assert_eq!(served, expected, "shared/ and the embedded table differ");
        for rel in &expected {
            let disk =
                std::fs::read(format!("{}/shared/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap();
            assert_eq!(
                find(&format!("{SHARED_PREFIX}{rel}")).unwrap().body,
                disk,
                "{rel}"
            );
        }
    }

    /// Files in `shared/` are copied into another program's bundle, so every
    /// stylesheet, script and SVG names its licence in the first line.
    #[test]
    fn shared_files_carry_spdx_header() {
        let mut checked = 0;
        for rel in files_below("shared") {
            if !(rel.ends_with(".css") || rel.ends_with(".js") || rel.ends_with(".svg")) {
                continue;
            }
            let text =
                std::fs::read_to_string(format!("{}/shared/{rel}", env!("CARGO_MANIFEST_DIR")))
                    .unwrap();
            let first = text.lines().next().unwrap_or("");
            let ok = if rel.ends_with(".svg") {
                first.contains("<!-- SPDX-License-Identifier: MPL-2.0 -->")
            } else {
                first == "/* SPDX-License-Identifier: MPL-2.0 */"
            };
            assert!(
                ok,
                "shared/{rel}: the first line must be the SPDX MPL-2.0 header"
            );
            checked += 1;
        }
        assert!(checked >= 2);
    }

    /// Every design token the stylesheets use without a fallback is declared
    /// somewhere (skins.css, style.css, or the desktop app.css that loads
    /// skins.css): an undefined `var(--x)` silently drops the declaration.
    #[test]
    fn css_tokens_are_defined() {
        let sheets: Vec<(String, String)> = shared_sheets()
            .into_iter()
            .chain([
                ("static/style.css".into(), STYLE_CSS.into()),
                ("desktop/ui/app.css".into(), APP_CSS.into()),
            ])
            .collect();
        assert!(sheets.iter().any(|(f, _)| f == "shared/css/assistant.css"));
        let declared: std::collections::HashSet<&str> = sheets
            .iter()
            .flat_map(|(_, css)| css.match_indices("--").map(|(start, _)| &css[start..]))
            .filter_map(|s| {
                let (name, rest) = custom_property(s)?;
                rest.trim_start().starts_with(':').then_some(name)
            })
            .collect();
        assert!(declared.contains("--radius-sm") && declared.contains("--primary-from"));
        for (file, css) in &sheets {
            for (start, _) in css.match_indices("var(") {
                let after = &css[start + 4..];
                let Some((name, rest)) = custom_property(after) else {
                    continue;
                };
                if rest.trim_start().starts_with(',') {
                    continue; // has a fallback
                }
                assert!(declared.contains(name), "{file}: {name} is not declared");
            }
        }
    }

    const STYLE_CSS: &str = include_str!("../static/style.css");
    const APP_CSS: &str = include_str!("../desktop/ui/app.css");

    /// Every stylesheet of `shared/css/` as (path, text).
    fn shared_sheets() -> Vec<(String, String)> {
        files_below("shared")
            .into_iter()
            .filter(|rel| rel.starts_with("css/") && rel.ends_with(".css"))
            .map(|rel| {
                let text =
                    std::fs::read_to_string(format!("{}/shared/{rel}", env!("CARGO_MANIFEST_DIR")))
                        .unwrap();
                (format!("shared/{rel}"), text)
            })
            .collect()
    }

    /// The classes that *start* a rule of `css`: the first class of the
    /// first compound selector (`.badge`, `span.badge-x::before`,
    /// `.menu-pop > a`), for every selector of every rule, also inside
    /// `@media`. A class that only appears in a context (`.toolbar .badge`)
    /// is not counted: an app may adjust a shared component in its own
    /// context.
    fn leading_classes(css: &str) -> std::collections::BTreeSet<String> {
        let mut text = String::with_capacity(css.len());
        let mut rest = css;
        while let Some(start) = rest.find("/*") {
            text.push_str(&rest[..start]);
            rest = rest[start..]
                .find("*/")
                .map_or("", |end| &rest[start + end + 2..]);
        }
        text.push_str(rest);
        let mut out = std::collections::BTreeSet::new();
        let mut start = 0;
        for (i, c) in text.char_indices() {
            match c {
                '{' => {
                    let prelude = text[start..i].trim();
                    if !prelude.is_empty() && !prelude.starts_with('@') {
                        let mut depth = 0;
                        let mut selector = String::new();
                        let mut selectors = Vec::new();
                        for ch in prelude.chars() {
                            match ch {
                                '(' => depth += 1,
                                ')' => depth -= 1,
                                _ => {}
                            }
                            if ch == ',' && depth == 0 {
                                selectors.push(std::mem::take(&mut selector));
                            } else {
                                selector.push(ch);
                            }
                        }
                        selectors.push(selector);
                        for s in selectors {
                            let compound = s
                                .trim()
                                .split(|ch: char| ch.is_whitespace() || "+>~".contains(ch))
                                .next()
                                .unwrap_or("");
                            let element = compound
                                .find(|ch: char| !ch.is_ascii_lowercase())
                                .unwrap_or(compound.len());
                            if let Some(class) = compound[element..].strip_prefix('.') {
                                let len = class
                                    .find(|ch: char| {
                                        !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
                                    })
                                    .unwrap_or(class.len());
                                out.insert(class[..len].to_string());
                            }
                        }
                    }
                    start = i + 1;
                }
                '}' | ';' => start = i + 1,
                _ => {}
            }
        }
        out
    }

    #[test]
    fn leading_classes_of_a_stylesheet() {
        let css = "/* .commented { } */ .badge, span.badge-x::before { a: b; }\n\
                   @media (hover: hover) { .menu-pop > a:hover { c: d; } }\n\
                   .toolbar .in-context { e: f; } :is(.x, .y) .z { } button { }";
        let classes: Vec<String> = leading_classes(css).into_iter().collect();
        // .in-context only appears in a context: not counted
        assert_eq!(classes, ["badge", "badge-x", "menu-pop", "toolbar"]);
    }

    /// shared/ owns the components it styles: the app stylesheets may adjust
    /// a shared class in their own context (`.toolbar .badge`) but never
    /// start a rule with it – that is a second copy that drifts
    /// (shared/README.md, "One source per rule").
    #[test]
    fn shared_css_classes_are_not_redefined() {
        let mut owned = std::collections::BTreeMap::new();
        for (file, css) in shared_sheets() {
            if file.ends_with("/skins.css") {
                continue; // tokens and skins, no components
            }
            for class in leading_classes(&css) {
                owned.insert(class, file.clone());
            }
        }
        assert!(owned.contains_key("tc-md"), "assistant.css is checked");
        let mut problems = Vec::new();
        for (app, css) in [
            ("static/style.css", STYLE_CSS),
            ("desktop/ui/app.css", APP_CSS),
        ] {
            for class in leading_classes(css) {
                if let Some(file) = owned.get(&class) {
                    problems.push(format!("{app}: .{class} is styled by {file}"));
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// Classes both app stylesheets style today – each one a component
    /// written twice. The list may only shrink: a class moves into
    /// `shared/css/` (and out of this list) with its migration step
    /// (docs/src/research/shared-ui.md, step 3), and a new class styled in
    /// both apps belongs into `shared/css/` from the start.
    const STYLED_TWICE: &[&str] = &[
        // badges and state glyphs
        "badge",
        "badge-accepted",
        "badge-approved",
        "badge-cancelled",
        "badge-deferred",
        "badge-implemented",
        "badge-in_review",
        "badge-in_triage",
        "badge-obsolete",
        "badge-pending",
        "badge-rejected",
        "badge-released",
        "badge-up_to_date",
        "badge-withdrawn",
        // tables and trees
        "grid",
        "table-wrap",
        "tree",
        "num",
        "props",
        // cards, stats, empty states, notices
        "stat",
        "empty",
        "notice",
        "flash",
        "flash-error",
        "hint",
        "warn-text",
        // menus
        "menu",
        "menu-sep",
        // lifecycle track
        "lifecycle-track",
        // threads, impacts, notifications, assistant text
        "thread",
        "thread-entry",
        "thread-meta",
        "impact-count",
        "impact-detail",
        "impact-event",
        "impact-group",
        "impact-group-head",
        "impact-group-name",
        "impact-relation",
        "impact-rows",
        "impact-source",
        "impact-source-name",
        "n-body",
        "n-meta",
        "n-title",
        "assistant-text",
        "attachment-thumbs",
        "file-thumb",
        "suggestion",
        "suggestions",
        "other-formats",
        // 3D viewer bar and legend
        "viewer-bar",
        "viewer-canvas",
        "viewer-legend",
        "viewer-legend-bar",
        "viewer-legend-value",
        "viewer-measuring",
        "viewer-notes",
        "viewer-overlay",
        "viewer-section",
        "viewer-stage",
        // controls and text utilities
        "button",
        "primary",
        "danger",
        "ghost",
        "link",
        "icon",
        "check",
        "small",
        "muted",
        "dim",
        "mono",
        "pre",
        "plain",
        "right",
        "stack",
        "grid-2",
        "hidden",
        "fmt",
        // the apps' own frames: same names, different places (top bar, rail)
        "app-glow",
        "bell",
        "bell-count",
        "brand",
        "brand-name",
        "brand-sub",
        "rail",
        "skin-card",
        "skin-swatch",
        "user-name",
    ];

    #[test]
    fn no_new_component_styled_in_both_apps() {
        let web = leading_classes(STYLE_CSS);
        let desktop = leading_classes(APP_CSS);
        let twice: Vec<&String> = web.intersection(&desktop).collect();
        let new: Vec<&&String> = twice
            .iter()
            .filter(|c| !STYLED_TWICE.contains(&c.as_str()))
            .collect();
        assert!(
            new.is_empty(),
            "styled in both static/style.css and desktop/ui/app.css: {new:?} – \
             move the component into shared/css/ (shared/README.md)"
        );
        let gone: Vec<&&str> = STYLED_TWICE
            .iter()
            .filter(|c| !twice.iter().any(|t| t == *c))
            .collect();
        assert!(
            gone.is_empty(),
            "no longer styled in both apps – remove from STYLED_TWICE: {gone:?}"
        );
    }

    /// Colours exist only as tokens in skins.css; shared components read
    /// them, so every skin and both schemes apply.
    #[test]
    fn shared_components_use_no_literal_colours() {
        for (file, css) in shared_sheets() {
            if file.ends_with("/skins.css") {
                continue;
            }
            let lower = css.to_ascii_lowercase();
            for literal in ["rgb(", "rgba(", "hsl(", "hsla(", "oklch("] {
                assert!(!lower.contains(literal), "{file}: {literal} – use a token");
            }
            for (i, _) in css.match_indices('#') {
                let hex = css[i + 1..]
                    .chars()
                    .take_while(char::is_ascii_hexdigit)
                    .count();
                let next = css[i + 1 + hex..].chars().next().unwrap_or(' ');
                assert!(
                    !(matches!(hex, 3 | 4 | 6 | 8)
                        && !(next.is_ascii_alphanumeric() || next == '-' || next == '_')),
                    "{file}: literal colour #{} – use a token",
                    &css[i + 1..i + 1 + hex]
                );
            }
        }
    }

    /// One set of density tokens (shared/css/skins.css): the desktop app
    /// selects the compact set instead of declaring its own control and row
    /// heights or body text size.
    #[test]
    fn one_density_token_set() {
        let skins = include_str!("../shared/css/skins.css");
        let compact = skins
            .split(":root[data-density=\"compact\"] {")
            .nth(1)
            .and_then(|s| s.split('}').next())
            .expect("compact density block");
        for token in [
            "--fs-body",
            "--lh-body",
            "--ctl-h:",
            "--ctl-h-sm",
            "--row-h:",
        ] {
            assert!(compact.contains(token), "compact density sets {token}");
        }
        let app_css = include_str!("../desktop/ui/app.css");
        for own in ["--d-ctl", "--d-row", "--ctl-h:", "--row-h:", "--fs-body:"] {
            assert!(!app_css.contains(own), "desktop/ui/app.css declares {own}");
        }
        let index = include_str!("../desktop/ui/index.html");
        assert!(
            index.contains("<html lang=\"en\" data-skin=\"teamcraft\" data-density=\"compact\">")
        );
    }

    /// `--name` at the start of `s` and the text after it.
    fn custom_property(s: &str) -> Option<(&str, &str)> {
        let body = s.strip_prefix("--")?;
        let len = body
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(body.len());
        (len > 0).then(|| (&s[..len + 2], &body[len..]))
    }

    /// SVGs must scale (viewBox) and must not contain active content.
    #[test]
    fn media_svgs_scale_and_are_inert() {
        for asset in all().filter(|a| a.content_type == SVG) {
            let svg = std::str::from_utf8(asset.body).expect("SVG is UTF-8");
            let root = svg
                .find("<svg")
                .map(|start| &svg[start..])
                .and_then(|s| s.split_once('>'))
                .map(|(tag, _)| tag)
                .unwrap_or_else(|| panic!("{} has no <svg> element", asset.url));
            assert!(
                root.contains("viewBox"),
                "{}: without a viewBox the icon does not scale in every browser",
                asset.url
            );
            let lower = svg.to_ascii_lowercase();
            for active in [
                "<script",
                "javascript:",
                "onload=",
                "onclick=",
                "<foreignobject",
            ] {
                assert!(!lower.contains(active), "{} contains {active}", asset.url);
            }
        }
    }
}
