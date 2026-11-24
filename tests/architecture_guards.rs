//! Guards for the architecture rules of `CLAUDE.md` / `CONTRIBUTING.md`, read
//! from the sources (no database). They walk directories, not fixed file
//! names, so they keep working when modules are split or moved.
//!
//! * no SQL in the handlers: `src/web`, `src/api` and `src/mcp` never touch
//!   `sqlx` – mutations and queries live in `services::*`;
//! * every mutation in `services::*` writes an audit entry: a public service
//!   function that writes to the database (itself or through the service
//!   functions it calls) also records an audit entry (itself or through them),
//!   unless it is on the short list of documented exceptions below;
//! * every template file is in `TEMPLATES` and rendered by the
//!   `every_template_renders` test (or extended/imported by one that is);
//! * every `TEAMCRAFT_*` variable the code reads is documented in
//!   `docs/src/admin/configuration.md`;
//! * every language file of the desktop Group Policy template
//!   (`deploy/desktop-policies/windows/*/TeamCraft.adml`) defines the string and
//!   presentation IDs of en-US and every one the ADMX references.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use regex::Regex;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every file below `dir` with the extension `ext`.
fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            files(&path, ext, out);
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
}

/// The Rust sources below `dir`, without unit-test files (`tests.rs`,
/// `*_tests.rs`).
fn rust_files(dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    files(&root().join(dir), "rs", &mut out);
    out.retain(|p| {
        let stem = p.file_stem().unwrap().to_string_lossy();
        stem != "tests" && !stem.ends_with("_tests")
    });
    out.sort();
    out
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .replace('\r', "")
}

/// The source without its unit-test module (`#[cfg(test)] mod tests { … }`
/// at the end of a file).
fn without_tests(text: &str) -> &str {
    match text.find("\n#[cfg(test)]\nmod tests") {
        Some(at) => &text[..at],
        None => text,
    }
}

fn relative(path: &Path) -> String {
    path.strip_prefix(root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

// ---------------------------------------------------------------------------
// (a) no SQL in the handlers
// ---------------------------------------------------------------------------

/// Lines of `text` that use `sqlx` (code only: comments are skipped).
fn sqlx_lines(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let code = line.split("//").next().unwrap_or("");
            code.contains("sqlx")
        })
        .map(|(n, line)| (n + 1, line.trim().to_string()))
        .collect()
}

#[test]
fn handlers_never_use_sqlx() {
    let mut handlers = Vec::new();
    for dir in ["src/web", "src/api", "src/mcp"] {
        handlers.extend(rust_files(dir));
    }
    assert!(
        handlers.len() > 20,
        "found the handler sources: {}",
        handlers.len()
    );
    let offenders: Vec<String> = handlers
        .iter()
        .flat_map(|file| {
            sqlx_lines(&read(file))
                .into_iter()
                .map(move |(n, line)| format!("{}:{n}: {line}", relative(file)))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "handlers call services::*, never sqlx (move the query into a service):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_sqlx_guard_sees_sqlx() {
    // the scanner itself: a use is found, a comment is not
    assert_eq!(sqlx_lines("let x = sqlx::query(\"SELECT 1\");").len(), 1);
    assert!(sqlx_lines("// sqlx is used by the services").is_empty());
    assert!(sqlx_lines("let a = 1; // not sqlx").is_empty());
}

// ---------------------------------------------------------------------------
// (b) every mutation writes an audit entry
// ---------------------------------------------------------------------------

/// Public service functions that write without an audit entry on purpose:
/// personal settings and marks of the user, sessions and tokens of the
/// sign-in itself, staging areas and caches, and housekeeping. Each entry
/// names why.
const NOT_AUDITED: &[(&str, &str)] = &[
    (
        "access_log::record",
        "the read log itself: one entry per read, separate from the audit log",
    ),
    (
        "access_log::record_revision",
        "the read log itself: one entry per read, separate from the audit log",
    ),
    (
        "access_log::record_structure",
        "the read log itself: one entry per group of a read structure, separate from the audit log",
    ),
    (
        "access_log::record_tool_read",
        "the read log itself: one entry per MCP or assistant tool read, separate from the audit log",
    ),
    (
        "package::build",
        "a read (Pack and Go); its only write is the read log entry (access_log::record_structure)",
    ),
    (
        "assistant_threads::append",
        "the user's own conversation; the question is audited by services::assistant",
    ),
    (
        "assistant_threads::load",
        "trims the user's own conversation to its limits",
    ),
    (
        "assistant_threads::purge_expired",
        "housekeeping of expired conversations",
    ),
    (
        "desktop::purge_expired",
        "housekeeping of expired one-time codes",
    ),
    (
        "files::classify_unclassified",
        "background classification of the file format (derived data)",
    ),
    (
        "files::rescan_references",
        "maintenance command: recomputes the CAD file references (derived data)",
    ),
    (
        "imports::discard",
        "the user's own staged upload, nothing imported",
    ),
    ("imports::purge_stale", "housekeeping of staged uploads"),
    (
        "imports::stage",
        "the user's own staged upload before the import (which is audited)",
    ),
    (
        "notifications::mark_read",
        "read marks of the user's own notifications",
    ),
    (
        "notifications::open",
        "read mark of the user's own notification",
    ),
    ("notifications::set_watch", "the user's own watch list"),
    ("personal::delete_search", "the user's own saved search"),
    ("personal::save_search", "the user's own saved search"),
    ("personal::set_favorite", "the user's own favourites"),
    ("similar::store", "cache of shape signatures (derived data)"),
    (
        "uploads::abort",
        "the user's own upload session; nothing was stored",
    ),
    (
        "uploads::complete",
        "the user's own upload session; storing it (finish) is audited",
    ),
    (
        "uploads::complete_in",
        "uploads::complete with a scope: the user's own upload session; storing it (finish) is audited",
    ),
    ("uploads::create", "the user's own upload session"),
    ("uploads::forget", "removes finished upload sessions"),
    ("uploads::purge_expired", "housekeeping of upload sessions"),
    ("users::actor_for_api_token", "last-used time of a token"),
    ("users::purge_expired", "housekeeping of sessions"),
    ("users::set_preferred_cad", "the user's own display preference"),
    ("users::set_skin", "the user's own display preference"),
    (
        "webhooks::emit",
        "outbox rows written inside the audited transaction of the event",
    ),
    (
        "files::scan_unscanned_references",
        "background scan of CAD file references (derived data)",
    ),
    (
        "notifications::send_test",
        "a test notification to the user themself",
    ),
    (
        "similar::ensure",
        "cache of shape signatures (derived data)",
    ),
    (
        "similar::for_files",
        "fills the cache of shape signatures while searching",
    ),
    (
        "similar::for_revision",
        "fills the cache of shape signatures while searching",
    ),
    (
        "similar::spawn_ensure",
        "fills the cache of shape signatures in the background",
    ),
    (
        "uploads::append",
        "a chunk of the user's own upload session; storing it (finish) is audited",
    ),
    (
        "request_digests::run_once",
        "background e-mail digest: notifications derived from waiting requests (each audited when \
         raised) and the bookkeeping of what the digests covered; no actor",
    ),
    (
        "request_digests::spawn",
        "starts the background task of request_digests::run_once",
    ),
    (
        "usage::record",
        "an anonymous desktop usage report (#383): no actor, nothing names the user",
    ),
    (
        "usage::overview",
        "a read for the admin page; its only write creates the one usage_sharing row (default off)",
    ),
    (
        "usage::spawn",
        "housekeeping of old usage reports and the stamp of what was shared last; no actor",
    ),
];

#[derive(Debug, Default, Clone)]
struct ServiceFn {
    public: bool,
    writes: bool,
    audits: bool,
    calls: BTreeSet<String>,
}

/// The service module a file below `src/services` belongs to: the file stem
/// (`items.rs` → `items`, `mod.rs` → `services`), or for a module split into
/// a directory its name (`requests/triage.rs` → `requests`), so that
/// `requests::decide` keeps its name whichever file of the split it is in.
fn service_module(file: &Path) -> String {
    let services = root().join("src/services");
    let rel = file.strip_prefix(&services).unwrap_or(file);
    let mut parts = rel.components();
    let first = Path::new(parts.next().unwrap().as_os_str());
    if parts.next().is_some() {
        return first.to_string_lossy().to_string();
    }
    match first.file_stem().unwrap().to_string_lossy().as_ref() {
        "mod" => "services".to_string(),
        stem => stem.to_string(),
    }
}

/// `module::function` → what it does, for every top-level function in
/// `src/services` (the module as [`service_module`] names it).
fn service_functions() -> BTreeMap<String, ServiceFn> {
    let item = Regex::new(r"(?m)^(pub(?:\([a-z]+\))? )?(?:async )?fn (\w+)").unwrap();
    let write = Regex::new(r"(?i)\b(INSERT INTO|DELETE FROM|UPDATE\s+\w+\s+SET)\b").unwrap();
    let call = Regex::new(r"\b(?:(\w+)::)?(\w+)\s*\(").unwrap();
    let mut functions = BTreeMap::new();
    for file in rust_files("src/services") {
        let module = service_module(&file);
        let text = read(&file);
        let text = without_tests(&text);
        let starts: Vec<(usize, bool, String)> = item
            .captures_iter(text)
            .map(|c| {
                (
                    c.get(0).unwrap().start(),
                    c.get(1).is_some_and(|v| v.as_str() == "pub "),
                    c[2].to_string(),
                )
            })
            .collect();
        for (i, (start, public, name)) in starts.iter().enumerate() {
            let end = starts.get(i + 1).map_or(text.len(), |s| s.0);
            let body = &text[*start..end];
            let calls = call
                .captures_iter(body)
                .map(|c| {
                    let callee = c[2].to_string();
                    match c.get(1).map(|m| m.as_str()) {
                        Some(m) if m != "self" && m != "Self" && m != "super" => {
                            format!("{m}::{callee}")
                        }
                        _ => format!("{module}::{callee}"),
                    }
                })
                .collect();
            functions.insert(
                format!("{module}::{name}"),
                ServiceFn {
                    public: *public,
                    writes: write.is_match(body),
                    audits: body.contains("audit::record") || module == "audit",
                    calls,
                },
            );
        }
    }
    // writing and auditing pass up through the calls between services
    loop {
        let mut changed = false;
        let snapshot = functions.clone();
        for f in functions.values_mut() {
            for callee in &f.calls {
                if let Some(c) = snapshot.get(callee) {
                    if c.writes && !f.writes {
                        f.writes = true;
                        changed = true;
                    }
                    if c.audits && !f.audits {
                        f.audits = true;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return functions;
        }
    }
}

#[test]
fn every_service_mutation_writes_an_audit_entry() {
    let functions = service_functions();
    assert!(
        functions.len() > 200,
        "found the services: {}",
        functions.len()
    );
    let exempt: BTreeMap<&str, &str> = NOT_AUDITED.iter().copied().collect();
    let missing: Vec<&String> = functions
        .iter()
        .filter(|(name, f)| {
            f.public && f.writes && !f.audits && !exempt.contains_key(name.as_str())
        })
        .map(|(name, _)| name)
        .collect();
    assert!(
        missing.is_empty(),
        "these service functions write to the database without an audit entry – record one \
         (audit::record in the same transaction) or, for personal data, caches and \
         housekeeping, add them to NOT_AUDITED with a reason:\n{}",
        missing
            .iter()
            .map(|m| format!("  {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    // the exceptions are real and still exceptions (no stale entries)
    for (name, why) in NOT_AUDITED {
        let f = functions
            .get(*name)
            .unwrap_or_else(|| panic!("NOT_AUDITED names {name}, which is gone ({why})"));
        assert!(
            f.writes && !f.audits,
            "{name} is on NOT_AUDITED but {} – remove it from the list",
            if f.audits {
                "records an audit entry"
            } else {
                "does not write"
            }
        );
    }
}

#[test]
fn the_audit_guard_follows_calls_between_services() {
    let functions = service_functions();
    // a check-in writes through store_uploads and audits itself
    let check_in = &functions["files::check_in"];
    assert!(check_in.public && check_in.writes && check_in.audits);
    // accepting a result proposal audits through simulations::write_results
    let accept = &functions["result_proposals::accept"];
    assert!(accept.writes && accept.audits, "{accept:?}");
    // a module split into a directory keeps its name, and calls between its
    // files are followed: on_change_released (requests/implementation.rs)
    // writes through set_status (requests/triage.rs)
    let released = &functions["requests::on_change_released"];
    assert!(released.writes && released.audits, "{released:?}");
    assert!(functions["requests::decide"].public);
}

// ---------------------------------------------------------------------------
// (b2) every read of items, revisions and files for an actor is guarded (#312)
// ---------------------------------------------------------------------------

/// Service functions that query items, revisions or files for an actor
/// without a visibility guard in their own body, on purpose. Each entry names
/// why it cannot leak what the actor may not read. Reviewed for #312.
const NOT_GUARDED: &[(&str, &str)] = &[
    (
        "access_log::record_structure",
        "writes the read log for revision ids its callers have already read through guarded queries",
    ),
    (
        "bom_references::load_files",
        "reads the files of one revision; both callers (the preview and the check-in step, #407) \
         have checked that revision with access::ensure_visible first",
    ),
    (
        "changes::stands_in_for_author",
        "returns only whether the actor manages every group of a change the caller has loaded \
         visibly; nothing of the items leaves the function",
    ),
    (
        "files::store_uploads",
        "runs inside the caller's transaction on a revision the caller locked and checked with \
         revisions::ensure_editable",
    ),
    (
        "impacts::record_flags",
        "change propagation inside a release: writes one audit entry per project group, each \
         naming only that group's items",
    ),
    (
        "impacts::notify_flagged",
        "notifications::notify filters the recipients with access::user_sees_project and \
         sends the redacted text to those who may not see the source",
    ),
    (
        "impacts::on_release",
        "change propagation inside the releasing transaction; returns nothing to the actor",
    ),
    (
        "impacts::on_obsolete",
        "change propagation inside the transaction that made the revision obsolete; returns \
         nothing to the actor",
    ),
    (
        "interchange::tell_supersession_watchers",
        "notifications::notify filters the watchers with access::user_sees_project",
    ),
    (
        "offboarding::idle_sso_users",
        "administrators only; returns counts per user (management data), no item content",
    ),
    (
        "projects::former_holders",
        "managers of the group only (require_manager); returns people and check-out counts, \
         no item content",
    ),
    (
        "revisions::has_checked_in",
        "the actor's own uploads only (uploaded_by = actor): one yes/no for the Getting \
         started wizard, no item, revision or file data reaches the actor",
    ),
    (
        "season::members",
        "managers of the group only (require_manager); returns members and check-out counts, \
         no item content",
    ),
    (
        "simulations::insert_basis_tx",
        "the caller passes a basis revision it read through a guarded query; the function \
         only locks that item and its files",
    ),
];

/// Service functions that take an actor, query items, revisions or files
/// and contain no guard in their own body (`module::function`, sorted).
///
/// Guards are the SQL fragments of `services::access`, the Rust checks
/// (`ensure_visible`, `can_see_project`, `ensure_editable`) and the guarded
/// getters that load the object for the actor (`revisions::get`,
/// `get(pool, actor, …)`, `lock_visible`, …). Calls are not followed: a
/// function that only hands data to a guarded one is listed, so each such
/// case is looked at once and named in [`NOT_GUARDED`].
fn unguarded_reads() -> Vec<String> {
    let item = Regex::new(r"(?m)^(pub(?:\([a-z]+\))? )?(?:async )?fn (\w+)").unwrap();
    let actor = Regex::new(r"(actor|me|viewer): &Actor").unwrap();
    let touches =
        Regex::new(r"(?m)(FROM|JOIN) (items|revisions|files|file_versions)( |$|\))").unwrap();
    let guard = Regex::new(
        r"item_visible|project_visible|ensure_visible|hidden_projects|change_visible|request_visible|user_sees_project|can_see_project|ensure_editable|revisions::get\(|items::get\(|files::get\(| get\(pool, actor|ensure_revision_visible|lock_visible| list\(pool, actor|bom::tree\(pool, actor",
    )
    .unwrap();
    let mut found = Vec::new();
    for file in rust_files("src/services") {
        if file.ends_with("access.rs") {
            continue; // writes the guards
        }
        let module = service_module(&file);
        let text = read(&file);
        let text = without_tests(&text);
        let starts: Vec<(usize, String)> = item
            .captures_iter(text)
            .map(|c| (c.get(0).unwrap().start(), c[2].to_string()))
            .collect();
        for (i, (start, name)) in starts.iter().enumerate() {
            let end = starts.get(i + 1).map_or(text.len(), |s| s.0);
            let body = &text[*start..end];
            if actor.is_match(body) && touches.is_match(body) && !guard.is_match(body) {
                found.push(format!("{module}::{name}"));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn every_read_for_an_actor_applies_the_visibility_guard() {
    let found = unguarded_reads();
    let exempt: BTreeMap<&str, &str> = NOT_GUARDED.iter().copied().collect();
    let missing: Vec<&String> = found
        .iter()
        .filter(|name| !exempt.contains_key(name.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "these service functions query items, revisions or files for an actor without a \
         visibility guard – add access::item_visible / project_visible to the SQL, check the \
         loaded row with access::ensure_visible, load it through a guarded getter, or, when \
         nothing of a hidden group can reach the actor, add them to NOT_GUARDED with the \
         reason:\n{}",
        missing
            .iter()
            .map(|m| format!("  {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    // the exceptions are still exceptions (no stale entries)
    for (name, why) in NOT_GUARDED {
        assert!(
            found.iter().any(|f| f == name),
            "NOT_GUARDED names {name} ({why}), which is gone or guarded now – remove it from \
             the list"
        );
    }
}

#[test]
fn the_visibility_guard_scanner_sees_reads_and_guards() {
    let found = unguarded_reads();
    // a guarded read is not listed (read_text checks through files::list)
    assert!(
        !found.contains(&"files::read_text".to_string()),
        "{found:?}"
    );
    // the exceptions are found by the scanner, so it parses the services
    assert!(
        found.contains(&"impacts::notify_flagged".to_string()),
        "{found:?}"
    );
    assert!(found.contains(&"season::members".to_string()), "{found:?}");
}

// ---------------------------------------------------------------------------
// (c) every template registered and rendered
// ---------------------------------------------------------------------------

#[test]
fn every_template_is_registered_and_rendered() {
    let dir = root().join("templates");
    let mut paths = Vec::new();
    files(&dir, "html", &mut paths);
    let names: BTreeSet<String> = paths
        .iter()
        .map(|p| {
            p.strip_prefix(&dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert!(names.len() > 20, "found the templates: {}", names.len());

    let source = read(&root().join("src/templates.rs"));
    let registry_start = source.find("pub const TEMPLATES").expect("TEMPLATES");
    let registry_end = registry_start
        + source[registry_start..]
            .find("];")
            .expect("end of TEMPLATES");
    let registry = &source[registry_start..registry_end];
    let test = &source[source
        .find("fn every_template_renders")
        .expect("the render test")..];

    // a template extended or imported by another one is rendered with it
    let include = Regex::new(r#"\{%-?\s*(?:extends|import|include|from)\s+"([^"]+)""#).unwrap();
    let mut included = BTreeSet::new();
    for path in &paths {
        for c in include.captures_iter(&read(path)) {
            included.insert(c[1].to_string());
        }
    }
    let mut problems = Vec::new();
    for name in &names {
        let quoted = format!("\"{name}\"");
        if !registry.contains(&quoted) {
            problems.push(format!("{name}: not in TEMPLATES (src/templates.rs)"));
        }
        if !test.contains(&quoted) && !included.contains(name) {
            problems.push(format!("{name}: not rendered by every_template_renders"));
        }
    }
    // and nothing registered that is gone
    let registered = Regex::new(r#""([\w/]+\.html)""#).unwrap();
    for c in registered.captures_iter(registry) {
        if !names.contains(&c[1]) {
            problems.push(format!("{}: in TEMPLATES, but no such file", &c[1]));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

// ---------------------------------------------------------------------------
// (d) every configuration variable documented
// ---------------------------------------------------------------------------

/// The variables named in the documentation: full names, the shorthand
/// `TEAMCRAFT_DB_HOST` / `_PORT` (same name with another last part) and the
/// wildcard rows (`TEAMCRAFT_OIDC_*`, the variables are then documented on the
/// page the row links to – any page of the book).
fn documented_variables() -> (BTreeSet<String>, Vec<String>) {
    let config = read(&root().join("docs/src/admin/configuration.md"));
    let full = Regex::new(r"TEAMCRAFT_[A-Z0-9_]*[A-Z0-9]").unwrap();
    let suffix = Regex::new(r"`(_[A-Z0-9_]+)`").unwrap();
    let wildcard = Regex::new(r"`(TEAMCRAFT_[A-Z0-9_]*)\*`").unwrap();
    let mut names = BTreeSet::new();
    let mut wildcards = Vec::new();
    for line in config.lines() {
        let on_line: Vec<String> = full
            .find_iter(line)
            .map(|m| m.as_str().to_string())
            .collect();
        for c in wildcard.captures_iter(line) {
            wildcards.push(c[1].to_string());
        }
        if let Some(first) = on_line.first() {
            for c in suffix.captures_iter(line) {
                if let Some(cut) = first.rfind('_') {
                    names.insert(format!("{}{}", &first[..cut], &c[1]));
                }
            }
        }
        names.extend(on_line);
    }
    (names, wildcards)
}

#[test]
fn every_configuration_variable_is_documented() {
    let literal = Regex::new(r#""(TEAMCRAFT_[A-Z0-9_]+)""#).unwrap();
    let mut read_by_code = BTreeSet::new();
    for file in rust_files("src") {
        let text = read(&file);
        for c in literal.captures_iter(without_tests(&text)) {
            read_by_code.insert(c[1].to_string());
        }
    }
    assert!(
        read_by_code.len() > 30,
        "found the variables: {}",
        read_by_code.len()
    );
    let (documented, wildcards) = documented_variables();
    let mut book = String::new();
    let mut pages = Vec::new();
    files(&root().join("docs/src"), "md", &mut pages);
    for page in &pages {
        book.push_str(&read(page));
    }
    let missing: Vec<&String> = read_by_code
        .iter()
        // `X_FILE` is documented with `X` (every variable has a `_FILE` form)
        .filter(|name| {
            !documented.contains(*name)
                && !name
                    .strip_suffix("_FILE")
                    .is_some_and(|base| documented.contains(base))
        })
        .filter(|name| {
            // a wildcard row in configuration.md, and the page it points to
            !wildcards
                .iter()
                .any(|w| name.starts_with(w.as_str()) && book.contains(name.as_str()))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "document these variables in docs/src/admin/configuration.md (every one also as \
         NAME_FILE):\n{}",
        missing
            .iter()
            .map(|m| format!("  {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_documentation_scanner_understands_the_shorthands() {
    let (names, wildcards) = documented_variables();
    assert!(
        names.contains("TEAMCRAFT_DB_PORT"),
        "`TEAMCRAFT_DB_HOST` / `_PORT`"
    );
    assert!(names.contains("TEAMCRAFT_BOOTSTRAP_ADMIN_PASSWORD"));
    assert!(
        wildcards.iter().any(|w| w == "TEAMCRAFT_OIDC_"),
        "{wildcards:?}"
    );
}

// ---------------------------------------------------------------------------
// Desktop policy templates (#378): every language file of the Group Policy
// template defines exactly the string and presentation IDs of en-US, and
// every `$(string.X)` / `$(presentation.X)` the ADMX references. A policy
// added to the ADMX and en-US but not to de-DE would make the Group Policy
// editor refuse the whole template on German systems.

/// The `<string id>` and `<presentation id>` sets of an ADML file.
fn adml_ids(text: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let collect = |re: &str| -> BTreeSet<String> {
        Regex::new(re)
            .expect("regex")
            .captures_iter(text)
            .map(|c| c[1].to_string())
            .collect()
    };
    (
        collect(r#"<string\s+id="([^"]+)""#),
        collect(r#"<presentation\s+id="([^"]+)""#),
    )
}

/// The `$(string.X)` and `$(presentation.X)` references of the ADMX file.
fn admx_refs(text: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut strings = BTreeSet::new();
    let mut presentations = BTreeSet::new();
    let re = Regex::new(r"\$\((string|presentation)\.([A-Za-z0-9_]+)\)").expect("regex");
    for c in re.captures_iter(text) {
        if &c[1] == "string" {
            strings.insert(c[2].to_string());
        } else {
            presentations.insert(c[2].to_string());
        }
    }
    (strings, presentations)
}

#[test]
fn every_policy_language_file_matches_the_admx() {
    let dir = root().join("deploy/desktop-policies/windows");
    let (refs_strings, refs_presentations) = admx_refs(&read(&dir.join("TeamCraft.admx")));
    assert!(!refs_strings.is_empty(), "no $(string.X) in TeamCraft.admx");
    let (en_strings, en_presentations) = adml_ids(&read(&dir.join("en-US/TeamCraft.adml")));
    let mut languages = Vec::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.expect("entry").path().join("TeamCraft.adml");
        if path.is_file() {
            languages.push(path);
        }
    }
    assert!(
        languages.len() >= 2,
        "en-US and de-DE expected: {languages:?}"
    );
    let mut problems = Vec::new();
    for path in &languages {
        let (strings, presentations) = adml_ids(&read(path));
        let name = relative(path);
        for missing in refs_strings.difference(&strings) {
            problems.push(format!(
                "{name}: string `{missing}` referenced by the ADMX is missing"
            ));
        }
        for missing in refs_presentations.difference(&presentations) {
            problems.push(format!(
                "{name}: presentation `{missing}` referenced by the ADMX is missing"
            ));
        }
        if strings != en_strings {
            problems.push(format!(
                "{name}: string IDs differ from en-US: {:?}",
                strings
                    .symmetric_difference(&en_strings)
                    .collect::<Vec<_>>()
            ));
        }
        if presentations != en_presentations {
            problems.push(format!(
                "{name}: presentation IDs differ from en-US: {:?}",
                presentations
                    .symmetric_difference(&en_presentations)
                    .collect::<Vec<_>>()
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_policy_template_scanner_sees_ids_and_references() {
    let (strings, presentations) = adml_ids(
        r#"<string id="A">x</string><string  id="B_Explain">y</string><presentation id="A"><textBox refId="A"/></presentation>"#,
    );
    assert_eq!(strings.into_iter().collect::<Vec<_>>(), ["A", "B_Explain"]);
    assert_eq!(presentations.into_iter().collect::<Vec<_>>(), ["A"]);
    let (strings, presentations) = admx_refs(
        r#"displayName="$(string.A)" explainText="$(string.A_Explain)" presentation="$(presentation.A)""#,
    );
    assert_eq!(strings.into_iter().collect::<Vec<_>>(), ["A", "A_Explain"]);
    assert_eq!(presentations.into_iter().collect::<Vec<_>>(), ["A"]);
}
