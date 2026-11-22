//! Concurrency management around check-outs: the sync state of the files of
//! a revision as a client's workspace sees them, and the bulk check-in of
//! everything a user has checked out.
//!
//! The lock itself (`revisions::checkout` / `cancel_checkout`) and the
//! optimistic base-version guard of uploads (`files::store_uploads`) live in
//! their own services; this module combines them for the workspace views.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::Actor,
    domain::{self, BaseVersion, FileSyncState, ServerFile},
    error::{AppError, AppResult},
    models::Revision,
    services::{access, audit, files, projects, revisions},
};

/// A file of the client's workspace, as the client reports it.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ClientFile {
    pub filename: String,
    /// SHA-256 of the local content; `None` = not in the workspace.
    #[serde(default)]
    pub sha256: Option<String>,
    /// The version the local file was checked out as (number) …
    #[serde(default)]
    pub base_version: Option<i32>,
    /// … or its content hash at check-out time.
    #[serde(default)]
    pub base_sha256: Option<String>,
}

impl ClientFile {
    fn base(&self) -> AppResult<Option<BaseVersion>> {
        if let Some(sha) = self.base_sha256.as_deref().filter(|s| !s.trim().is_empty()) {
            return match BaseVersion::parse(sha) {
                Some(b @ BaseVersion::Sha256(_)) => Ok(Some(b)),
                _ => Err(AppError::BadRequest(format!(
                    "{}: base_sha256 must be a SHA-256 in hex.",
                    self.filename
                ))),
            };
        }
        match self.base_version {
            Some(v) if v < 0 => Err(AppError::BadRequest(format!(
                "{}: base_version must not be negative.",
                self.filename
            ))),
            Some(v) => Ok(Some(BaseVersion::Version(v))),
            None => Ok(None),
        }
    }
}

/// One file of a revision with its server state and – if the client
/// reported its workspace – the sync state of the local copy.
#[derive(Clone, Debug, Serialize)]
pub struct FileStatus {
    pub filename: String,
    /// `None` for files that exist only in the client's workspace.
    pub file_id: Option<Uuid>,
    pub version: Option<i32>,
    pub sha256: Option<String>,
    pub size_bytes: Option<i64>,
    /// Folder inside the project group (`None` for files only in the workspace).
    pub folder: Option<String>,
    pub uploaded_by_name: Option<String>,
    pub uploaded_at: Option<DateTime<Utc>>,
    /// The local file as the client reported it.
    pub local_sha256: Option<String>,
    /// The server version the local content equals, if any.
    pub local_version: Option<i32>,
    /// `up_to_date`, `modified`, `outdated`, `conflict`, `new`, `missing`;
    /// `None` when the client reported no files.
    pub state: Option<FileSyncState>,
}

/// The files of a revision and who holds its lock.
#[derive(Clone, Debug, Serialize)]
pub struct RevisionStatus {
    pub revision_id: Uuid,
    pub item_number: String,
    pub code: String,
    pub state: String,
    pub locked_by: Option<Uuid>,
    pub locked_by_name: Option<String>,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by_me: bool,
    /// `true` if somebody else holds the lock: local changes cannot be
    /// checked in right now.
    pub locked_by_other: bool,
    /// Somebody else released the actor's check-out of this revision and
    /// nobody has checked it out since (issue #173, step 10): the desktop
    /// app tells the former holder that their local changes stay but need
    /// a new check-out. `None` otherwise.
    pub released: Option<ReleasedCheckout>,
    /// The item's folder in its group (`items.folder`): where a revision
    /// without files has its place in the working folder.
    pub item_folder: String,
    pub files: Vec<FileStatus>,
}

/// A forced release of the actor's own check-out (`revision.checkout_forced_release`).
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ReleasedCheckout {
    /// Who released it (display name; the user name if the account is gone).
    pub by_name: String,
    pub at: DateTime<Utc>,
    /// The reason a manager gave, if any.
    pub reason: Option<String>,
    /// The sentence to show the former holder ([`released_message`]).
    pub message: String,
}

/// What the former holder of a released check-out is told in the working
/// folder: their local files are not touched, but checking them in needs
/// the lock again.
pub fn released_message(by_name: &str) -> String {
    format!("Released by {by_name}; your local changes are kept and need a new check-out.")
}

/// The audit actions that take or give back the lock of a revision; the
/// newest of them says what happened to the lock last.
const LOCK_ACTIONS: [&str; 4] = [
    "revision.checkout",
    "revision.checkin",
    "revision.checkout_cancel",
    "revision.checkout_forced_release",
];

/// The forced release of `actor`'s check-out of `rev`, if that is the last
/// thing that happened to its lock and it is not checked out now. Read from
/// the audit log (`details.holder`, written by `revisions::cancel_checkout`),
/// so it needs no state of its own; releases audited before the holder was
/// recorded are not reported.
pub async fn released_from(
    pool: &PgPool,
    actor: &Actor,
    rev: &Revision,
) -> AppResult<Option<ReleasedCheckout>> {
    if rev.checked_out_by.is_some() {
        return Ok(None);
    }
    let row: Option<(String, DateTime<Utc>, String, serde_json::Value)> = sqlx::query_as(
        "SELECT a.action, a.at, COALESCE(u.display_name, a.username), a.details
           FROM audit_log a LEFT JOIN users u ON u.id = a.user_id
          WHERE a.entity_id = $1 AND a.entity_type = 'revision' AND a.action = ANY($2)
          ORDER BY a.at DESC, a.id DESC
          LIMIT 1",
    )
    .bind(rev.id)
    .bind(&LOCK_ACTIONS[..])
    .fetch_optional(pool)
    .await?;
    let Some((action, at, by_name, details)) = row else {
        return Ok(None);
    };
    let holder = details["holder"]
        .as_str()
        .and_then(|h| Uuid::parse_str(h).ok());
    if action != "revision.checkout_forced_release" || holder != Some(actor.id) {
        return Ok(None);
    }
    Ok(Some(ReleasedCheckout {
        message: released_message(&by_name),
        by_name,
        at,
        reason: details["reason"].as_str().map(str::to_string),
    }))
}

/// The status of every file of a revision for one workspace. With an empty
/// `client` list only the server side is returned (`state` = `None`).
pub async fn file_status(
    pool: &PgPool,
    actor: &Actor,
    revision_id: Uuid,
    client: &[ClientFile],
) -> AppResult<RevisionStatus> {
    let rev = revisions::get(pool, actor, revision_id).await?;
    let (list, versions) = tokio::try_join!(
        files::list(pool, actor, revision_id),
        files::versions_of_revision(pool, revision_id)
    )?;
    let mut history: HashMap<Uuid, Vec<(i32, String)>> = HashMap::new();
    for v in versions {
        history
            .entry(v.file_id)
            .or_default()
            .push((v.version, v.sha256));
    }
    let mut reported: HashMap<String, &ClientFile> = HashMap::new();
    for c in client {
        reported.insert(files::stored_name(&c.filename).to_lowercase(), c);
    }
    let with_client = !client.is_empty();
    let mut out = Vec::with_capacity(list.len());
    for f in &list {
        let key = f.filename.to_lowercase();
        let local = reported.remove(&key);
        let hist: Vec<(i32, &str)> = history
            .get(&f.id)
            .map(|h| h.iter().map(|(v, s)| (*v, s.as_str())).collect())
            .unwrap_or_default();
        let local_sha = local
            .and_then(|c| c.sha256.as_deref())
            .map(str::to_ascii_lowercase);
        let state = if with_client {
            let base = local.map(ClientFile::base).transpose()?.flatten();
            Some(domain::sync_state(
                Some(ServerFile {
                    version: f.version,
                    sha256: &f.sha256,
                }),
                &hist,
                local_sha.as_deref(),
                base.as_ref(),
            ))
        } else {
            None
        };
        let local_version = local_sha.as_deref().and_then(|sha| {
            hist.iter()
                .filter(|(_, s)| s.eq_ignore_ascii_case(sha))
                .map(|(v, _)| *v)
                .max()
        });
        out.push(FileStatus {
            filename: f.filename.clone(),
            file_id: Some(f.id),
            version: Some(f.version),
            sha256: Some(f.sha256.clone()),
            size_bytes: Some(f.size_bytes),
            folder: Some(f.folder.clone()),
            uploaded_by_name: Some(f.uploaded_by_name.clone()),
            uploaded_at: Some(f.uploaded_at),
            local_sha256: local_sha,
            local_version,
            state,
        });
    }
    // Files only in the workspace.
    let mut only_local: Vec<&ClientFile> = reported
        .into_values()
        .filter(|c| c.sha256.is_some())
        .collect();
    only_local.sort_by_key(|c| c.filename.to_lowercase());
    for c in only_local {
        let base = c.base()?;
        out.push(FileStatus {
            filename: files::stored_name(&c.filename),
            file_id: None,
            version: None,
            sha256: None,
            size_bytes: None,
            folder: None,
            uploaded_by_name: None,
            uploaded_at: None,
            local_sha256: c.sha256.as_deref().map(str::to_ascii_lowercase),
            local_version: None,
            // A local file that was checked out from the server but is gone there.
            state: Some(match base {
                Some(BaseVersion::Version(v)) if v > 0 => FileSyncState::Conflict,
                Some(BaseVersion::Sha256(_)) => FileSyncState::Conflict,
                _ => FileSyncState::New,
            }),
        });
    }
    let released = released_from(pool, actor, &rev).await?;
    Ok(RevisionStatus {
        released,
        revision_id: rev.id,
        item_number: rev.item_number.clone(),
        code: rev.code.clone(),
        state: rev.state.clone(),
        locked_by: rev.checked_out_by,
        locked_by_name: rev.checked_out_by_name.clone(),
        locked_at: rev.checked_out_at,
        locked_by_me: rev.checked_out_by == Some(actor.id),
        locked_by_other: rev.checked_out_by.is_some_and(|id| id != actor.id),
        item_folder: rev.item_folder.clone(),
        files: out,
    })
}

/// Result of the bulk check-in for one revision.
#[derive(Clone, Debug, Serialize)]
pub struct CheckInOutcome {
    pub revision_id: Uuid,
    pub item_number: String,
    pub code: String,
    pub name: String,
    pub ok: bool,
    /// Why the check-in failed (`ok` = false).
    pub error: Option<String>,
}

impl CheckInOutcome {
    fn of(rev: &Revision, result: AppResult<()>) -> CheckInOutcome {
        CheckInOutcome {
            revision_id: rev.id,
            item_number: rev.item_number.clone(),
            code: rev.code.clone(),
            name: rev.name.clone(),
            ok: result.is_ok(),
            error: result.err().map(|e| match e {
                AppError::Database(_) | AppError::Io(_) | AppError::Other(_) => {
                    tracing::error!(revision = %rev.id, error = %e, "bulk check-in failed");
                    "Internal error – see the server log.".to_string()
                }
                e => e.to_string(),
            }),
        }
    }
}

/// The comment of every check-in of [`check_in_all`] (shown in the audit log).
pub const BULK_COMMENT: &str = "Bulk check-in: lock released without new files";

/// Checks in every revision the actor has checked out – or only those of
/// `only` – **without new files**: the lock is released, the revision keeps
/// its files. Each revision is checked in in its own transaction with its own
/// audit record (`revision.checkin`), so one failure does not block the
/// others; the outcome lists every revision. Clients with a local workspace
/// (desktop) pass only the revisions without local changes.
pub async fn check_in_all(
    pool: &PgPool,
    actor: &Actor,
    only: Option<&[Uuid]>,
) -> AppResult<Vec<CheckInOutcome>> {
    actor.require(domain::Role::Editor)?;
    let mine = revisions::checked_out_by(pool, actor).await?;
    let mut outcomes = Vec::new();
    if let Some(ids) = only {
        // Requested revisions that are not (or no longer) checked out by the actor.
        for id in ids {
            if mine.iter().any(|r| r.id == *id) {
                continue;
            }
            if let Ok(rev) = revisions::get(pool, actor, *id).await {
                outcomes.push(CheckInOutcome::of(
                    &rev,
                    Err(AppError::Conflict(format!(
                        "You have not checked out {}.",
                        revisions::label(&rev)
                    ))),
                ));
            }
        }
    }
    for rev in mine {
        if only.is_some_and(|ids| !ids.contains(&rev.id)) {
            continue;
        }
        let result = files::check_in(pool, actor, rev.id, &[], BULK_COMMENT)
            .await
            .map(|_| ());
        outcomes.push(CheckInOutcome::of(&rev, result));
    }
    Ok(outcomes)
}

/// Most revisions one [`checkout_many`] call takes (400 above it).
pub const MAX_CHECKOUT_BATCH: usize = 500;

/// One revision a client asks [`checkout_many`] to check out: by its id, or
/// by item number and revision code (`latest` = the newest revision).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct CheckoutTarget {
    #[serde(default)]
    pub id: Option<Uuid>,
    #[serde(default)]
    pub item: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}

/// Why [`checkout_many`] did not check out one revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutRefusal {
    /// Missing, or hidden from the actor (answered alike, nothing leaks).
    NotFound,
    /// Released or obsolete: revise it first.
    Released,
    /// In review: the change has to come back to *in work* first.
    InReview,
    /// Checked out by somebody else (`checked_out_by_name`).
    Locked,
    /// Not a member of the item's project group.
    Forbidden,
}

/// The result of [`checkout_many`] for one requested revision.
#[derive(Clone, Debug, Serialize)]
pub struct CheckoutOutcome {
    /// The requested id, or the revision found by item and code (never for
    /// a revision the actor may not read).
    pub revision_id: Option<Uuid>,
    /// Item number (as requested when the revision was not found).
    pub item: String,
    pub code: String,
    pub name: Option<String>,
    pub ok: bool,
    /// The actor already held the check-out (no change, no audit record).
    pub already: bool,
    pub reason: Option<CheckoutRefusal>,
    /// The sentence to show for a refusal.
    pub message: Option<String>,
    /// Who holds the check-out now: the actor when `ok`, the other user
    /// when `reason` = `locked`.
    pub checked_out_by_name: Option<String>,
}

impl CheckoutOutcome {
    fn not_found(slot: &BatchSlot) -> CheckoutOutcome {
        CheckoutOutcome {
            revision_id: slot.requested_id,
            item: slot.item.clone(),
            code: slot.code.clone(),
            name: None,
            ok: false,
            already: false,
            reason: Some(CheckoutRefusal::NotFound),
            message: Some(access::not_found_revision().to_string()),
            checked_out_by_name: None,
        }
    }

    fn of(rev: &Revision) -> CheckoutOutcome {
        CheckoutOutcome {
            revision_id: Some(rev.id),
            item: rev.item_number.clone(),
            code: rev.code.clone(),
            name: Some(rev.name.clone()),
            ok: false,
            already: false,
            reason: None,
            message: None,
            checked_out_by_name: rev.checked_out_by_name.clone(),
        }
    }
}

/// One entry of the request after resolving item and code.
struct BatchSlot {
    requested_id: Option<Uuid>,
    item: String,
    code: String,
    id: Option<Uuid>,
}

/// Why `actor` may not check out `rev` – the rules of
/// `revisions::ensure_checkout_possible`, sorted into a [`CheckoutRefusal`]
/// instead of an error, so one refusal does not end the batch.
fn refusal(rev: &Revision, actor: &Actor) -> Option<(CheckoutRefusal, String)> {
    let err = revisions::ensure_checkout_possible(rev, actor).err()?;
    let state = revisions::state_of(rev);
    Some(match err {
        AppError::NotFound(m) => (CheckoutRefusal::NotFound, m),
        AppError::Conflict(_) if state == domain::LifecycleState::InReview => (
            CheckoutRefusal::InReview,
            format!(
                "{} is in review; it can be checked out again once the change returns it to in work.",
                revisions::label(rev)
            ),
        ),
        AppError::Conflict(_) if !state.is_editable() => (
            CheckoutRefusal::Released,
            format!(
                "{} is {}: revise it first.",
                revisions::label(rev),
                state.label()
            ),
        ),
        AppError::Conflict(m) => (CheckoutRefusal::Locked, m),
        other => (CheckoutRefusal::Forbidden, other.to_string()),
    })
}

/// Looks up the id of a revision the actor may read, by item number and
/// code (`latest` = the newest revision). Read-only; a hidden revision is
/// `None`, exactly like a missing one, so the caller deduplicates and locks
/// only visible rows (nothing about a hidden revision reaches the answer or
/// its timing).
async fn visible_revision_id_by_code(
    pool: &PgPool,
    actor: &Actor,
    item: &str,
    code: &str,
) -> AppResult<Option<Uuid>> {
    Ok(sqlx::query_scalar(&format!(
        "SELECT r.id FROM revisions r JOIN items i ON i.id = r.item_id
          WHERE i.number = $1 AND ($2 = 'LATEST' OR r.code = $2) AND {}
          ORDER BY r.seq DESC
          LIMIT 1",
        access::item_visible("i", 3)
    ))
    .bind(item)
    .bind(code)
    .bind(&actor.hidden_projects)
    .fetch_optional(pool)
    .await?)
}

/// `id` if it names a revision the actor may read, else `None` (missing and
/// hidden alike).
async fn visible_revision_id(pool: &PgPool, actor: &Actor, id: Uuid) -> AppResult<Option<Uuid>> {
    Ok(sqlx::query_scalar(&format!(
        "SELECT r.id FROM revisions r JOIN items i ON i.id = r.item_id
          WHERE r.id = $1 AND {}",
        access::item_visible("i", 2)
    ))
    .bind(id)
    .bind(&actor.hidden_projects)
    .fetch_optional(pool)
    .await?)
}

/// Checks out several revisions in one step – the parts of an assembly a
/// CAD user wants to edit in its context (issue #415). The result has one
/// entry per requested revision (duplicates once, in request order): the
/// ones the actor may take are checked out, the rest are reported with a
/// [`CheckoutRefusal`] instead of failing the whole request, so a released
/// part or a part somebody else holds does not stop the others.
///
/// # Invariants
///
/// * **Permission:** CAD editor (403 for the whole request otherwise). Per
///   revision the rules of `revisions::checkout`: visible ("not found"
///   otherwise, the same answer as a missing revision, without its item or
///   name), in a group the actor may edit (`forbidden`), in work
///   (`released`, `in_review`) and not checked out by someone else
///   (`locked`, with the holder's name).
/// * **Bounds:** at most [`MAX_CHECKOUT_BATCH`] entries (400); every entry
///   names an id or an item and a code (400).
/// * **Locks:** the requested revisions the actor may read (resolved and
///   deduplicated under the visibility guard, so a hidden row is neither
///   locked nor collapsed with another entry) in one `SELECT … FOR UPDATE`
///   ordered by id, so two batches over overlapping sets cannot deadlock:
///   the second waits, then reports the revisions the first took as
///   `locked` – each revision has one winner.
/// * **Audit:** `revision.checkout` for every revision whose lock changed;
///   nothing for one the actor already held.
/// * **Notifications / webhooks:** none.
/// * **Atomic:** one transaction for the whole batch.
pub async fn checkout_many(
    pool: &PgPool,
    actor: &Actor,
    targets: &[CheckoutTarget],
) -> AppResult<Vec<CheckoutOutcome>> {
    actor.require(domain::Role::Editor)?;
    if targets.len() > MAX_CHECKOUT_BATCH {
        return Err(AppError::BadRequest(format!(
            "At most {MAX_CHECKOUT_BATCH} revisions can be checked out at once."
        )));
    }
    let mut slots: Vec<BatchSlot> = Vec::with_capacity(targets.len());
    let mut seen: HashSet<String> = HashSet::new();
    for t in targets {
        let slot = if let Some(id) = t.id {
            BatchSlot {
                requested_id: Some(id),
                item: String::new(),
                code: String::new(),
                id: visible_revision_id(pool, actor, id).await?,
            }
        } else {
            let item = t.item.as_deref().unwrap_or("").trim().to_ascii_uppercase();
            let code = t.code.as_deref().unwrap_or("").trim().to_ascii_uppercase();
            if item.is_empty() || code.is_empty() {
                return Err(AppError::BadRequest(
                    "Every revision needs an id or an item and a code.".into(),
                ));
            }
            let id = visible_revision_id_by_code(pool, actor, &item, &code).await?;
            BatchSlot {
                requested_id: None,
                item,
                code,
                id,
            }
        };
        // Only visible revisions are deduplicated by their id; a missing or
        // hidden one by what was sent, so both collapse (or not) alike.
        let key = match (slot.id, slot.requested_id) {
            (Some(id), _) | (None, Some(id)) => id.to_string(),
            (None, None) => format!("{}/{}", slot.item, slot.code),
        };
        if seen.insert(key) {
            slots.push(slot);
        }
    }
    let mut ids: Vec<Uuid> = slots.iter().filter_map(|s| s.id).collect();
    ids.sort();
    if ids.is_empty() {
        return Ok(slots.iter().map(CheckoutOutcome::not_found).collect());
    }

    let mut tx = pool.begin().await?;
    // Lock first, in a fixed order, then read: the read is a new statement
    // and sees what a batch we waited for committed, including the joined
    // holder name (a locking read with joins could return a stale one).
    sqlx::query("SELECT id FROM revisions WHERE id = ANY($1) ORDER BY id FOR UPDATE")
        .bind(&ids)
        .execute(&mut *tx)
        .await?;
    let rows: Vec<Revision> = sqlx::query_as::<_, Revision>(&format!(
        "{} WHERE r.id = ANY($1)",
        revisions::REVISION_SELECT
    ))
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;
    let by_id: HashMap<Uuid, &Revision> = rows.iter().map(|r| (r.id, r)).collect();

    let mut outcomes = Vec::with_capacity(slots.len());
    let mut take: Vec<&Revision> = Vec::new();
    for slot in &slots {
        let Some(rev) = slot.id.and_then(|id| by_id.get(&id).copied()) else {
            outcomes.push(CheckoutOutcome::not_found(slot));
            continue;
        };
        if !actor.can_see_project(rev.project_id) {
            outcomes.push(CheckoutOutcome::not_found(slot));
            continue;
        }
        let mut outcome = CheckoutOutcome::of(rev);
        match refusal(rev, actor) {
            Some((CheckoutRefusal::NotFound, _)) => {
                outcomes.push(CheckoutOutcome::not_found(slot));
                continue;
            }
            Some((reason, message)) => {
                outcome.reason = Some(reason);
                outcome.message = Some(message);
                if reason != CheckoutRefusal::Locked {
                    outcome.checked_out_by_name = None;
                }
            }
            None => {
                outcome.ok = true;
                outcome.already = rev.checked_out_by == Some(actor.id);
                outcome.checked_out_by_name = Some(actor.display_name.clone());
                if !outcome.already {
                    take.push(rev);
                }
            }
        }
        outcomes.push(outcome);
    }

    if !take.is_empty() {
        let take_ids: Vec<Uuid> = take.iter().map(|r| r.id).collect();
        sqlx::query(
            "UPDATE revisions SET checked_out_by = $2, checked_out_at = now()
              WHERE id = ANY($1)",
        )
        .bind(&take_ids)
        .bind(actor.id)
        .execute(&mut *tx)
        .await?;
        for rev in &take {
            audit::record(
                &mut tx,
                Some(actor),
                "revision.checkout",
                "revision",
                Some(rev.id),
                format!("Checked out {}", revisions::label(rev)),
                json!({ "item_id": rev.item_id, "batch": true }),
            )
            .await?;
        }
    }
    tx.commit().await?;
    Ok(outcomes)
}

/// One line about a [`checkout_many`] result: how many were checked out and
/// which were refused, with the reason.
pub fn checkout_summary(outcomes: &[CheckoutOutcome]) -> String {
    let ok = outcomes.iter().filter(|o| o.ok).count();
    let refused: Vec<String> = outcomes
        .iter()
        .filter(|o| !o.ok)
        .map(|o| {
            let what = if o.item.is_empty() {
                o.revision_id.map(|id| id.to_string()).unwrap_or_default()
            } else {
                format!("{}/{}", o.item, o.code)
            };
            format!("{what} ({})", o.message.as_deref().unwrap_or("refused"))
        })
        .collect();
    let head = match ok {
        0 if refused.is_empty() => return "Nothing to check out.".into(),
        1 => "Checked out 1 revision".to_string(),
        n => format!("Checked out {n} revisions"),
    };
    if refused.is_empty() {
        format!("{head}.")
    } else {
        format!("{head}; {} refused: {}", refused.len(), refused.join(", "))
    }
}

/// Most check-outs one [`release_held`] run releases.
pub const MAX_RELEASE: i64 = 1000;

/// Releases the check-outs `holder` still has – in every group, or only in
/// `project` – for a leaver (issue #173). Each release goes through
/// `revisions::cancel_checkout` in its own transaction, so the permission
/// rule stays in one place, every release is audited
/// (`revision.checkout_forced_release`) and one refusal does not stop the
/// others; the outcome lists every revision.
///
/// # Invariants
///
/// * **Permission:** CAD editor. Administrators release in every group they
///   can read; anyone else needs to manage `project` ("not found" for a hidden
///   group, 403 otherwise) or, without `project`, at least one group (403),
///   and only check-outs in the groups they manage are considered. Per
///   revision `cancel_checkout` decides: a manager only releases the locks of
///   users who are no longer members or are deactivated; the rest are listed
///   as refused (an active member's lock needs a reason, one at a time).
/// * **Visibility:** revisions the actor may not read are left out, as if
///   they did not exist (a need-to-know group with `admin_read = none` hides
///   its revisions from administrators too).
/// * **Bounds:** at most [`MAX_RELEASE`] revisions per run.
/// * **Audit / notifications:** those of `cancel_checkout`, per revision.
/// * **Atomic:** one transaction per revision.
pub async fn release_held(
    pool: &PgPool,
    actor: &Actor,
    holder: Uuid,
    project: Option<Uuid>,
) -> AppResult<Vec<CheckInOutcome>> {
    release_held_because(pool, actor, holder, project, None).await
}

/// [`release_held`] with a reason for every release: a manager then also
/// releases the check-outs of an *active* member of their groups (issue #173,
/// the season handover); the reason is audited and told to the holder.
///
/// # Invariants
///
/// * **Permission / visibility / bounds / atomic:** as [`release_held`];
///   per revision `cancel_checkout` decides with the reason.
/// * **Data:** a reason has `revisions::RELEASE_REASON_MIN` to
///   `RELEASE_REASON_MAX` characters (400 before anything is released).
/// * **Audit / notifications:** those of `cancel_checkout`, per revision,
///   with the reason.
pub async fn release_held_because(
    pool: &PgPool,
    actor: &Actor,
    holder: Uuid,
    project: Option<Uuid>,
    reason: Option<&str>,
) -> AppResult<Vec<CheckInOutcome>> {
    actor.require(domain::Role::Editor)?;
    revisions::check_release_reason(reason)?;
    let managed: Option<Vec<Uuid>> = if actor.is_admin() {
        if let Some(p) = project {
            projects::get_for_management(pool, actor, p).await?;
        }
        None
    } else {
        if let Some(p) = project {
            let mut conn = pool.acquire().await?;
            projects::require_manager(&mut conn, actor, p).await?;
        }
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT project_id FROM project_members WHERE user_id = $1 AND manager",
        )
        .bind(actor.id)
        .fetch_all(pool)
        .await?;
        if ids.is_empty() {
            return Err(AppError::Forbidden(
                "Only managers of a project group and administrators can release other users' \
                 check-outs."
                    .into(),
            ));
        }
        Some(ids)
    };
    let held = sqlx::query_as::<_, Revision>(&format!(
        "{} WHERE r.checked_out_by = $1 AND {}
           AND ($3::uuid IS NULL OR i.project_id = $3)
           AND ($4::uuid[] IS NULL OR i.project_id = ANY($4))
         ORDER BY i.number, r.code LIMIT $5",
        revisions::REVISION_SELECT,
        access::item_visible("i", 2)
    ))
    .bind(holder)
    .bind(&actor.hidden_projects)
    .bind(project)
    .bind(managed.as_deref())
    .bind(MAX_RELEASE)
    .fetch_all(pool)
    .await?;
    let mut outcomes = Vec::with_capacity(held.len());
    for rev in held {
        let result = revisions::cancel_checkout(pool, actor, rev.id, reason)
            .await
            .map(|_| ());
        outcomes.push(CheckInOutcome::of(&rev, result));
    }
    Ok(outcomes)
}

/// How many revisions `holder` has checked out that the actor may read – the
/// number [`release_held`] would look at for an administrator.
pub async fn held_count(pool: &PgPool, actor: &Actor, holder: Uuid) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM revisions r JOIN items i ON i.id = r.item_id
         WHERE r.checked_out_by = $1 AND {}",
        access::item_visible("i", 2)
    ))
    .bind(holder)
    .bind(&actor.hidden_projects)
    .fetch_one(pool)
    .await?)
}

/// `"Released 4 check-outs; 1 refused: PRT-000003/A (…)"` for a flash message.
pub fn release_summary(outcomes: &[CheckInOutcome]) -> String {
    let ok = outcomes.iter().filter(|o| o.ok).count();
    let failed: Vec<String> = outcomes
        .iter()
        .filter(|o| !o.ok)
        .take(5)
        .map(|o| {
            format!(
                "{}/{} ({})",
                o.item_number,
                o.code,
                o.error.as_deref().unwrap_or("refused")
            )
        })
        .collect();
    let refused = outcomes.len() - ok;
    let head = match ok {
        0 if refused == 0 => return "No check-outs to release.".into(),
        1 => "Released 1 check-out".to_string(),
        n => format!("Released {n} check-outs"),
    };
    if refused == 0 {
        format!("{head}.")
    } else {
        let more = if refused > failed.len() { ", …" } else { "" };
        format!("{head}; {refused} refused: {}{more}", failed.join(", "))
    }
}

/// `"Checked in 4 revisions; 1 failed: PRT-000003/A (…)"` for a flash message.
pub fn summary(outcomes: &[CheckInOutcome]) -> String {
    let ok = outcomes.iter().filter(|o| o.ok).count();
    let failed: Vec<String> = outcomes
        .iter()
        .filter(|o| !o.ok)
        .map(|o| {
            format!(
                "{}/{} ({})",
                o.item_number,
                o.code,
                o.error.as_deref().unwrap_or("failed")
            )
        })
        .collect();
    let head = match ok {
        0 if failed.is_empty() => return "Nothing was checked out.".into(),
        1 => "Checked in 1 revision".to_string(),
        n => format!("Checked in {n} revisions"),
    };
    if failed.is_empty() {
        format!("{head}.")
    } else {
        format!("{head}; {} failed: {}", failed.len(), failed.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(number: &str, error: Option<&str>) -> CheckInOutcome {
        CheckInOutcome {
            revision_id: Uuid::nil(),
            item_number: number.into(),
            code: "A".into(),
            name: "x".into(),
            ok: error.is_none(),
            error: error.map(str::to_string),
        }
    }

    #[test]
    fn summaries() {
        assert_eq!(summary(&[]), "Nothing was checked out.");
        assert_eq!(summary(&[outcome("PRT-1", None)]), "Checked in 1 revision.");
        assert_eq!(
            summary(&[
                outcome("PRT-1", None),
                outcome("PRT-2", None),
                outcome("PRT-3", Some("no access"))
            ]),
            "Checked in 2 revisions; 1 failed: PRT-3/A (no access)"
        );
        assert_eq!(
            summary(&[outcome("PRT-3", Some("no access"))]),
            "Checked in 0 revisions; 1 failed: PRT-3/A (no access)"
        );
    }

    fn taken(number: &str, refusal: Option<(CheckoutRefusal, &str)>) -> CheckoutOutcome {
        CheckoutOutcome {
            revision_id: None,
            item: number.into(),
            code: "A".into(),
            name: None,
            ok: refusal.is_none(),
            already: false,
            reason: refusal.map(|r| r.0),
            message: refusal.map(|r| r.1.to_string()),
            checked_out_by_name: None,
        }
    }

    #[test]
    fn checkout_summaries() {
        assert_eq!(checkout_summary(&[]), "Nothing to check out.");
        assert_eq!(
            checkout_summary(&[taken("PRT-1", None)]),
            "Checked out 1 revision."
        );
        assert_eq!(
            checkout_summary(&[
                taken("PRT-1", None),
                taken("PRT-2", None),
                taken(
                    "PRT-3",
                    Some((
                        CheckoutRefusal::Released,
                        "PRT-3/A is Released: revise it first."
                    ))
                ),
            ]),
            "Checked out 2 revisions; 1 refused: PRT-3/A (PRT-3/A is Released: revise it first.)"
        );
        let hidden = CheckoutOutcome {
            revision_id: Some(Uuid::nil()),
            item: String::new(),
            ..taken("", Some((CheckoutRefusal::NotFound, "Revision not found.")))
        };
        assert_eq!(
            checkout_summary(&[hidden]),
            format!(
                "Checked out 0 revisions; 1 refused: {} (Revision not found.)",
                Uuid::nil()
            )
        );
        assert_eq!(
            serde_json::to_value(CheckoutRefusal::InReview).unwrap(),
            "in_review"
        );
    }

    #[test]
    fn client_bases() {
        let c = ClientFile {
            filename: "a.step".into(),
            base_version: Some(3),
            ..Default::default()
        };
        assert_eq!(c.base().unwrap(), Some(BaseVersion::Version(3)));
        let c = ClientFile {
            filename: "a.step".into(),
            base_version: Some(3),
            base_sha256: Some("CD".repeat(32)),
            ..Default::default()
        };
        assert_eq!(
            c.base().unwrap(),
            Some(BaseVersion::Sha256("cd".repeat(32))),
            "the hash wins"
        );
        let c = ClientFile {
            filename: "a.step".into(),
            base_sha256: Some("12".into()),
            ..Default::default()
        };
        assert!(c.base().is_err());
        assert_eq!(ClientFile::default().base().unwrap(), None);
    }
}
