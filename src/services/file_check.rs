//! Duplicate checks for files that are about to join a project group (issue
//! #359). TeamCraft Desktop asks before it creates an item from a file of the
//! working folder whether the group already has a file at that place, of that
//! name or with that content ([`check`], `POST /projects/{code}/file-check`),
//! and an upload can ask the server to refuse such a duplicate atomically,
//! in the transaction that stores it ([`ClashGuard`], `refuse_clash`).
//!
//! Both look at the **current** files of the group only: the files of the
//! latest revision of every item – what the file browser
//! (`services::folders`) shows and a new working folder gets. Folders and
//! file names are compared case-insensitively, names in Unicode NFC and as an
//! upload would store them (`files::stored_name`).
//!
//! # Invariants
//!
//! * **Permission:** [`check`] reads a group the actor may read; a hidden
//!   group answers "not found" and nothing of it is told. The guard runs
//!   inside the transaction of an upload into a revision the caller locked
//!   and checked as editable; it only refuses, it never reads out data.
//! * **Locks:** the guard takes a transaction-scoped advisory lock per
//!   project group ([`CLASH_LOCK`] and the hash of the group id), so two
//!   uploads that would create the same file cannot both pass the check.
//! * **Audit / notifications / webhooks:** none – nothing is written here.

use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    auth::Actor,
    domain,
    error::{AppError, AppResult},
    models::Revision,
    services::{access, files},
    util,
};

/// Most files one [`check`] call takes.
pub const MAX_FILES: usize = 200;

/// Most files of the same name or content one checked file lists.
pub const MAX_MATCHES: i64 = 20;

/// Longest file name (characters) – what `util::sanitize_filename` keeps.
pub const MAX_NAME_CHARS: usize = 200;

/// The first key of the advisory lock of the duplicate guard (the two-key
/// form `pg_advisory_xact_lock(int, int)`, the second key is the hash of the
/// project group id). Its own key space: the one-key locks of the server
/// (`users::create_first_admin`, the migrations) never meet it.
pub const CLASH_LOCK: i32 = 0x7463_3539; // "tc59"

/// The current files of the group bound at `$1` – the files of the latest
/// revision of each item, as in `services::folders` – as the `FROM … WHERE`
/// of a query, `f` = the file, `r` = its revision, `i` = its item. `guard`
/// is an additional condition (the actor's visibility guard, or `TRUE`).
fn current_files(guard: &str) -> String {
    format!(
        "files f
         JOIN revisions r ON r.id = f.revision_id
         JOIN items i ON i.id = r.item_id
         WHERE i.project_id = $1 AND {guard}
           AND r.seq = (SELECT max(lr.seq) FROM revisions lr WHERE lr.item_id = i.id)"
    )
}

// ---------------------------------------------------------------------------
// The check before an item is created
// ---------------------------------------------------------------------------

/// One file to check.
#[derive(Clone, Debug, Deserialize)]
pub struct Wanted {
    /// Folder inside the group (`Design/Struktur`); empty, null or absent =
    /// the top. Normalised like the folder of an upload.
    #[serde(default)]
    pub folder: Option<String>,
    /// The file name, without a folder.
    pub filename: String,
    /// SHA-256 of the content (64 hexadecimal digits); null or absent = no
    /// content check.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// The item that has a current file at the checked place.
#[derive(Clone, Debug, Serialize)]
pub struct Owner {
    pub item_number: String,
    /// Code of the item's latest revision.
    pub code: String,
    /// Name of the item's latest revision.
    pub name: String,
}

/// A current file of the group with the same name or the same content.
#[derive(Clone, Debug, Serialize)]
pub struct Found {
    pub item_number: String,
    /// Code of the item's latest revision.
    pub code: String,
    pub folder: String,
    pub filename: String,
}

/// What the group already has for one checked file.
#[derive(Clone, Debug, Serialize)]
pub struct Checked {
    /// The checked folder, normalised.
    pub folder: String,
    /// The checked file name as sent, in NFC.
    pub filename: String,
    /// A current file with the same folder and name, if any.
    pub same_place: Option<Owner>,
    /// Current files with the same name in another folder (at most
    /// [`MAX_MATCHES`], by folder, name and item number).
    pub same_name: Vec<Found>,
    /// Current files with the same content, the `same_place` file left out
    /// (at most [`MAX_MATCHES`], by folder, name and item number; empty
    /// without a `sha256`).
    pub same_content: Vec<Found>,
}

/// A wanted file, checked: the answer's folder and name, the name an upload
/// would store and the content hash (`""` = none).
struct Key {
    folder: String,
    filename: String,
    stored: String,
    sha256: String,
}

/// Refuses an empty file name, one with a folder, control characters or
/// only dots, and one longer than [`MAX_NAME_CHARS`].
fn check_name(name: &str) -> AppResult<()> {
    let trimmed = name.trim();
    let bad = |why: &str| {
        AppError::BadRequest(format!(
            "File name '{}': {why}",
            domain::truncate_chars(trimmed, 80)
        ))
    };
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("A file name is empty.".into()));
    }
    if trimmed.contains(['/', '\\']) {
        return Err(bad(
            "give the folder in 'folder', the file name without one.",
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(bad("control characters are not allowed."));
    }
    if trimmed.trim_start_matches('.').is_empty() {
        return Err(bad("this is not a file name."));
    }
    if trimmed.chars().count() > MAX_NAME_CHARS {
        return Err(bad(format!("at most {MAX_NAME_CHARS} characters.").as_str()));
    }
    Ok(())
}

/// A content hash: 64 hexadecimal digits, answered in lower case.
fn check_sha256(raw: &str) -> AppResult<String> {
    let s = raw.trim();
    if s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(s.to_ascii_lowercase())
    } else {
        Err(AppError::BadRequest(
            "sha256 must be 64 hexadecimal digits (or null).".into(),
        ))
    }
}

fn key_of(wanted: &Wanted) -> AppResult<Key> {
    let folder = domain::normalize_folder(wanted.folder.as_deref().unwrap_or(""))?;
    let filename = util::nfc(&wanted.filename).into_owned();
    check_name(&filename)?;
    let sha256 = match wanted.sha256.as_deref() {
        Some(raw) => check_sha256(raw)?,
        None => String::new(),
    };
    Ok(Key {
        stored: files::stored_name(&filename),
        folder,
        filename,
        sha256,
    })
}

/// One match of [`check`]: `kind` = `place`, `name` or `content`, `ord` =
/// the 1-based position of the checked file; the rows come in the order of
/// each kind.
#[derive(sqlx::FromRow)]
struct Match {
    ord: i64,
    kind: String,
    item_number: String,
    code: String,
    name: String,
    folder: String,
    filename: String,
}

/// What the group already has for each of the `wanted` files (1 to
/// [`MAX_FILES`]), in request order: a current file at the same place, files
/// of the same name in other folders and files with the same content. One
/// query for all files, read-only.
///
/// # Invariants
///
/// * **Permission:** the group is visible to the actor ("not found"
///   otherwise, checked before anything else); viewers may ask. Only the
///   group's own current files are compared, under the visibility guard.
/// * **Data:** a bad folder (`domain::normalize_folder`), file name or
///   `sha256`, no file or more than [`MAX_FILES`] are a 400; nothing is
///   stored.
/// * **Locks / audit / notifications / webhooks:** none.
pub async fn check(
    pool: &PgPool,
    actor: &Actor,
    project_id: Uuid,
    wanted: &[Wanted],
) -> AppResult<Vec<Checked>> {
    access::ensure_visible(
        actor,
        Some(project_id),
        AppError::not_found("Project group not found."),
    )?;
    if wanted.is_empty() || wanted.len() > MAX_FILES {
        return Err(AppError::BadRequest(format!(
            "Send 1 to {MAX_FILES} files to check (got {}).",
            wanted.len()
        )));
    }
    let keys = wanted.iter().map(key_of).collect::<AppResult<Vec<Key>>>()?;
    let folders: Vec<String> = keys.iter().map(|k| k.folder.clone()).collect();
    let names: Vec<String> = keys.iter().map(|k| k.stored.clone()).collect();
    let hashes: Vec<String> = keys.iter().map(|k| k.sha256.clone()).collect();
    let current = current_files(&access::item_visible("i", 2));
    // per checked file w: the file at its place (sp), then the matches of the
    // three kinds, each in its own order (pos)
    let matches = sqlx::query_as::<_, Match>(&format!(
        "SELECT w.ord, m.kind, m.item_number, m.code, m.name, m.folder, m.filename
         FROM unnest($3::text[], $4::text[], $5::text[]) WITH ORDINALITY AS w(folder, name, sha256, ord)
         LEFT JOIN LATERAL (
             SELECT f.id, i.number AS item_number, r.code, r.name, f.folder, f.filename
             FROM {current}
               AND lower(f.filename) = lower(w.name) AND lower(f.folder) = lower(w.folder)
             ORDER BY i.number
             LIMIT 1) sp ON TRUE
         CROSS JOIN LATERAL (
             SELECT 'place' AS kind, sp.item_number, sp.code, sp.name, sp.folder, sp.filename,
                    0::bigint AS pos
             WHERE sp.id IS NOT NULL
             UNION ALL
             (SELECT 'name', i.number, r.code, r.name, f.folder, f.filename,
                     row_number() OVER (ORDER BY lower(f.folder), f.folder, lower(f.filename),
                                                 f.filename, i.number) AS pos
              FROM {current}
                AND lower(f.filename) = lower(w.name) AND lower(f.folder) <> lower(w.folder)
              ORDER BY pos
              LIMIT {MAX_MATCHES})
             UNION ALL
             (SELECT 'content', i.number, r.code, r.name, f.folder, f.filename,
                     row_number() OVER (ORDER BY lower(f.folder), f.folder, lower(f.filename),
                                                 f.filename, i.number) AS pos
              FROM {current}
                AND w.sha256 <> '' AND f.sha256 = w.sha256 AND f.id IS DISTINCT FROM sp.id
              ORDER BY pos
              LIMIT {MAX_MATCHES})) m
         ORDER BY w.ord, m.kind, m.pos"
    ))
    .bind(project_id)
    .bind(&actor.hidden_projects)
    .bind(&folders)
    .bind(&names)
    .bind(&hashes)
    .fetch_all(pool)
    .await?;
    let mut out: Vec<Checked> = keys
        .into_iter()
        .map(|k| Checked {
            folder: k.folder,
            filename: k.filename,
            same_place: None,
            same_name: Vec::new(),
            same_content: Vec::new(),
        })
        .collect();
    for m in matches {
        // ord counts from 1
        let index = usize::try_from(m.ord - 1).unwrap_or(usize::MAX);
        let Some(entry) = out.get_mut(index) else {
            continue;
        };
        let found = Found {
            item_number: m.item_number,
            code: m.code,
            folder: m.folder,
            filename: m.filename,
        };
        match m.kind.as_str() {
            "place" => {
                entry.same_place = Some(Owner {
                    item_number: found.item_number,
                    code: found.code,
                    name: m.name,
                })
            }
            "name" => entry.same_name.push(found),
            _ => entry.same_content.push(found),
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// The guard of an upload
// ---------------------------------------------------------------------------

/// The duplicate guard an upload asks for (multipart fields `refuse_clash` /
/// `refuse_same_name`, the same fields of `POST /uploads`). It applies only to
/// a file that is **new in the revision** – a new version of a file of the
/// revision is never refused – and only to items in a project group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClashGuard {
    /// Refuse a new file when another item of the group has a current file
    /// with the same folder and name.
    pub same_place: bool,
    /// Also refuse it when another item has a current file of the same name
    /// in another folder (CAD programs find referenced files by name).
    pub same_name: bool,
}

impl ClashGuard {
    /// The guard of the two flags; `refuse_same_name` includes
    /// `refuse_clash` (a name taken in another folder is refused, the same
    /// name in the same folder all the more).
    pub fn new(refuse_clash: bool, refuse_same_name: bool) -> Self {
        ClashGuard {
            same_place: refuse_clash || refuse_same_name,
            same_name: refuse_same_name,
        }
    }

    /// The guard of the multipart text fields (`true`, `1`, `on` or `yes`
    /// switch a flag on; absent or anything else leaves it off).
    pub fn from_fields(refuse_clash: Option<&str>, refuse_same_name: Option<&str>) -> Self {
        let on =
            |v: Option<&str>| v.is_some_and(|v| matches!(v.trim(), "true" | "1" | "on" | "yes"));
        ClashGuard::new(on(refuse_clash), on(refuse_same_name))
    }

    /// Both flags of `self` and `other`.
    pub fn or(self, other: ClashGuard) -> Self {
        ClashGuard {
            same_place: self.same_place || other.same_place,
            same_name: self.same_name || other.same_name,
        }
    }

    pub fn is_off(self) -> bool {
        !self.same_place && !self.same_name
    }
}

/// Where a file is, for a message.
fn place(folder: &str) -> String {
    if folder.is_empty() {
        "at the top of the project group".to_string()
    } else {
        format!("in {folder}")
    }
}

/// The current file of another item that a new file would duplicate.
#[derive(sqlx::FromRow)]
struct Clash {
    item_number: String,
    code: String,
    folder: String,
    same_folder: bool,
}

/// Refuses (409) a new file `filename` in `folder` of an item of the group
/// when another item has a current file at the same place – or, with
/// `guard.same_name`, of the same name anywhere in the group.
async fn ensure_no_clash(
    conn: &mut PgConnection,
    project_id: Uuid,
    item_id: Uuid,
    filename: &str,
    folder: &str,
    guard: ClashGuard,
) -> AppResult<()> {
    let clash = sqlx::query_as::<_, Clash>(&format!(
        "SELECT i.number AS item_number, r.code, f.folder,
                lower(f.folder) = lower($4) AS same_folder
         FROM {}
           AND i.id <> $2 AND lower(f.filename) = lower($3)
           AND ($5 OR lower(f.folder) = lower($4))
         ORDER BY same_folder DESC, lower(f.folder), f.folder, i.number
         LIMIT 1",
        current_files("TRUE")
    ))
    .bind(project_id)
    .bind(item_id)
    .bind(filename)
    .bind(folder)
    .bind(guard.same_name)
    .fetch_optional(&mut *conn)
    .await?;
    match clash {
        None => Ok(()),
        Some(c) if c.same_folder => Err(AppError::Conflict(format!(
            "{filename} {} already belongs to {}/{} – open that item instead of creating a \
             duplicate. Nothing was stored.",
            place(&c.folder),
            c.item_number,
            c.code
        ))),
        Some(c) => Err(AppError::Conflict(format!(
            "{filename} already exists {} as a file of {}/{} – CAD programs find files by \
             their name, so a second {filename} in this project group would be mistaken for \
             it. Nothing was stored.",
            place(&c.folder),
            c.item_number,
            c.code
        ))),
    }
}

/// The duplicate guard of an upload, inside the transaction that stores it:
/// called for a file that is new in `rev` (`folder` = the folder it really
/// gets). Takes the group's advisory lock, then refuses (409) the file as
/// [`ClashGuard`] asks – the caller's transaction then rolls back and nothing
/// is stored. Without a guard or a project group it does nothing.
pub(crate) async fn refuse_clash(
    conn: &mut PgConnection,
    rev: &Revision,
    filename: &str,
    folder: &str,
    guard: ClashGuard,
) -> AppResult<()> {
    let Some(project_id) = rev.project_id else {
        return Ok(());
    };
    if guard.is_off() {
        return Ok(());
    }
    // held until the transaction ends: a parallel guarded upload of the
    // group waits here and then sees this file
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext($2::uuid::text))")
        .bind(CLASH_LOCK)
        .bind(project_id)
        .execute(&mut *conn)
        .await?;
    ensure_no_clash(conn, project_id, rev.item_id, filename, folder, guard).await
}

/// The guard checked when a chunked upload starts, so a duplicate is refused
/// before its bytes are sent; `folder` = the folder of the session (`None`:
/// the item's folder for a new file). A file the revision already has is a
/// new version and passes. The binding check runs again when the file is
/// stored ([`refuse_clash`]).
pub(crate) async fn precheck(
    pool: &PgPool,
    rev: &Revision,
    filename: &str,
    folder: Option<&str>,
    guard: ClashGuard,
) -> AppResult<()> {
    let Some(project_id) = rev.project_id else {
        return Ok(());
    };
    if guard.is_off() {
        return Ok(());
    }
    let mut conn = pool.acquire().await?;
    let version = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM files WHERE revision_id = $1 AND lower(filename) = lower($2))",
    )
    .bind(rev.id)
    .bind(filename)
    .fetch_one(&mut *conn)
    .await?;
    if version {
        return Ok(());
    }
    let folder = folder.unwrap_or(&rev.item_folder);
    ensure_no_clash(&mut conn, project_id, rev.item_id, filename, folder, guard).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_checked() {
        assert!(check_name("Bracket.ipt").is_ok());
        assert!(check_name(" Lager ä.ipt ").is_ok());
        for bad in [
            "",
            "  ",
            "Design/Bracket.ipt",
            "a\\b.ipt",
            "a\u{7}.ipt",
            ".",
            "..",
        ] {
            assert!(
                matches!(check_name(bad), Err(AppError::BadRequest(_))),
                "{bad:?}"
            );
        }
        assert!(check_name(&"a".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(check_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn hashes_are_checked() {
        assert_eq!(check_sha256(&"AB".repeat(32)).unwrap(), "ab".repeat(32));
        let not_hex = "zz".repeat(32);
        let too_long = "ab".repeat(33);
        for bad in ["", "abc", not_hex.as_str(), too_long.as_str()] {
            assert!(check_sha256(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_guard_flags() {
        assert!(ClashGuard::default().is_off());
        assert!(ClashGuard::from_fields(None, None).is_off());
        assert!(ClashGuard::from_fields(Some("false"), Some("")).is_off());
        let clash = ClashGuard::from_fields(Some(" true "), None);
        assert!(clash.same_place && !clash.same_name);
        // refuse_same_name includes refuse_clash
        let name = ClashGuard::from_fields(None, Some("1"));
        assert!(name.same_place && name.same_name);
        assert_eq!(ClashGuard::default().or(clash), clash);
        assert_eq!(clash.or(name), name);
    }
}
