//! Change propagation to dependents.
//!
//! When a revision is **released**, TeamCraft
//!
//! 1. freezes the configuration of the released revisions: every BOM line and
//!    item reference remembers the revision of its target at that moment
//!    (revision rule *as released*);
//! 2. resolves the open impacts of the released items themselves – a new
//!    release was reviewed with the current components (*superseded*);
//! 3. flags every **dependent** for a check: items whose latest revision uses
//!    the released item in its BOM, references it (drawing, specification,
//!    reference), has a CAD file that references one of its files, or is a
//!    simulation based on another revision of it (or on a file version that
//!    is no longer current; see `simulations`). Items released in the same
//!    change are not flagged.
//!
//! When the latest revision of an item is made **obsolete**, its dependents
//! are flagged the same way and its own open impacts are closed.
//!
//! An impact stays open until a CAD editor of the dependent's project group
//! acknowledges it with a note ("form, fit and function unchanged"), a newer
//! revision of the dependent is released, or the dependent becomes obsolete.
//! Propagation is one level per event: when the dependent is revised and
//! released, its own dependents are flagged in turn.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    auth::Actor,
    domain::{self, NotificationKind, Role},
    error::{AppError, AppResult},
    models::{Impact, Revision},
    services::{
        access, audit,
        notifications::{self, Notice},
        revisions,
    },
};

const IMPACT_SELECT: &str = "
    SELECT im.id, im.item_id, i.number AS item_number, i.project_id, p.code AS project_code,
           r.id AS revision_id, r.code AS revision_code, r.state AS revision_state, r.name AS item_name,
           im.source_item_id, si.number AS source_number, sr.id AS source_revision_id,
           sr.code AS source_code, sr.name AS source_name, im.event, im.relation, im.detail,
           im.change_id, c.number AS change_number, im.created_at, im.resolved_at, im.resolution,
           u.display_name AS resolved_by_name, im.note, si.project_id AS source_project_id
    FROM impacts im
    JOIN items i ON i.id = im.item_id
    LEFT JOIN projects p ON p.id = i.project_id
    JOIN revisions r ON r.id = im.revision_id
    JOIN items si ON si.id = im.source_item_id
    JOIN revisions sr ON sr.id = im.source_revision_id
    LEFT JOIN changes c ON c.id = im.change_id
    LEFT JOIN users u ON u.id = im.resolved_by";

/// Latest, not obsolete revision of an item (SQL condition on `pr`).
const LATEST: &str = "pr.seq = (SELECT MAX(x.seq) FROM revisions x WHERE x.item_id = pr.item_id)
                      AND pr.state <> 'obsolete'";

/// Redacts impacts for a reader – every impact that leaves this module goes
/// through here. Sources the reader may not see lose their identity, and a
/// releasing change order the reader may not see (it also holds restricted
/// items, see [`access::change_visible`]) loses its number and id.
pub async fn redact_all(
    conn: &mut PgConnection,
    actor: &Actor,
    rows: Vec<Impact>,
) -> AppResult<Vec<Impact>> {
    let mut changes: Vec<Uuid> = rows.iter().filter_map(|i| i.change_id).collect();
    changes.sort_unstable();
    changes.dedup();
    let visible: Vec<Uuid> =
        if changes.is_empty() || (actor.is_admin() && actor.hidden_projects.is_empty()) {
            changes
        } else {
            sqlx::query_scalar(&format!(
                "SELECT c.id FROM changes c WHERE c.id = ANY($1) AND {}",
                access::change_visible("c", 2, 3)
            ))
            .bind(&changes)
            .bind(actor.id)
            .bind(&actor.hidden_projects)
            .fetch_all(&mut *conn)
            .await?
        };
    Ok(rows
        .into_iter()
        .map(|i| {
            let mut i = i.redact(actor);
            if i.change_id.is_some_and(|c| !visible.contains(&c)) {
                i.hide_change();
            }
            i
        })
        .collect())
}

/// An impact as stored, without a read check (the caller checks; the
/// audit trail names the real source).
async fn fetch(conn: &mut PgConnection, id: Uuid) -> AppResult<Impact> {
    let impact = sqlx::query_as::<_, Impact>(&format!("{IMPACT_SELECT} WHERE im.id = $1"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| AppError::not_found("Impact not found."))?;
    Ok(impact)
}

/// An impact on an item the actor may read, without redaction.
async fn fetch_visible(conn: &mut PgConnection, actor: &Actor, id: Uuid) -> AppResult<Impact> {
    let impact = fetch(conn, id).await?;
    access::ensure_visible(
        actor,
        impact.project_id,
        AppError::not_found("Impact not found."),
    )?;
    Ok(impact)
}

/// An impact on an item the actor may read; a hidden source is redacted.
pub async fn get(pool: &PgPool, actor: &Actor, id: Uuid) -> AppResult<Impact> {
    let mut conn = pool.acquire().await?;
    let impact = fetch_visible(&mut conn, actor, id).await?;
    Ok(redact_all(&mut conn, actor, vec![impact]).await?.remove(0))
}

/// Impacts on an item: open ones first, then the 20 most recently resolved.
/// Sources the actor may not see are redacted.
pub async fn for_item(pool: &PgPool, actor: &Actor, item_id: Uuid) -> AppResult<Vec<Impact>> {
    let mut conn = pool.acquire().await?;
    let rows = sqlx::query_as::<_, Impact>(&format!(
        "({IMPACT_SELECT} WHERE im.item_id = $1 AND im.resolved_at IS NULL ORDER BY im.created_at DESC)
         UNION ALL
         ({IMPACT_SELECT} WHERE im.item_id = $1 AND im.resolved_at IS NOT NULL
          ORDER BY im.resolved_at DESC LIMIT 20)"
    ))
    .bind(item_id)
    .fetch_all(&mut *conn)
    .await?;
    if let Some(first) = rows.first() {
        access::ensure_visible(actor, first.project_id, access::not_found_item())?;
    }
    redact_all(&mut conn, actor, rows).await
}

/// Open impacts the actor can resolve: on items of their project groups
/// (all for admins, none for viewers), newest first.
pub async fn open_for(pool: &PgPool, actor: &Actor) -> AppResult<Vec<Impact>> {
    if !actor.can_edit() {
        return Ok(Vec::new());
    }
    let mut conn = pool.acquire().await?;
    let projects: Vec<Uuid> = actor.projects.to_vec();
    let rows = sqlx::query_as::<_, Impact>(&format!(
        "{IMPACT_SELECT} WHERE im.resolved_at IS NULL AND ($1 OR i.project_id = ANY($2))
           AND {}
         ORDER BY im.created_at DESC LIMIT 200",
        access::item_visible("i", 3)
    ))
    .bind(actor.is_admin())
    .bind(&projects)
    .bind(&actor.hidden_projects)
    .fetch_all(&mut *conn)
    .await?;
    redact_all(&mut conn, actor, rows).await
}

/// Marks an open impact as checked: the dependent needs no change.
///
/// # Invariants
///
/// * **Permission:** CAD editor; the impact is on an item the actor sees ("not found"
///   otherwise) and in a group the actor may edit (403); a note is needed (400); the impact is
///   open (409).
/// * **Locks:** the impact row, through the `UPDATE ... WHERE resolved_at IS NULL`; when it was
///   resolved in the meantime the actor gets a 409.
/// * **Audit:** `impact.acknowledge`.
/// * **Notifications / webhooks:** none.
/// * **Atomic:** one transaction.
pub async fn acknowledge(pool: &PgPool, actor: &Actor, id: Uuid, note: &str) -> AppResult<Impact> {
    actor.require(Role::Editor)?;
    let note = domain::require_text("Note", note, 2000).map_err(|_| {
        AppError::BadRequest(
            "Say why the dependent needs no change (e.g. form, fit and function unchanged).".into(),
        )
    })?;
    // Unredacted: the audit entry names the real source (it is redacted
    // when read by somebody who may not see it).
    let impact = {
        let mut conn = pool.acquire().await?;
        fetch_visible(&mut conn, actor, id).await?
    };
    if impact.resolved_at.is_some() {
        return Err(AppError::Conflict(format!(
            "This impact on {} is already resolved.",
            impact.item_number
        )));
    }
    if !actor.can_edit_project(impact.project_id) {
        return Err(AppError::Forbidden(format!(
            "Only CAD editors of the project group {} (or admins) can acknowledge impacts on {}.",
            impact.project_code.as_deref().unwrap_or("—"),
            impact.item_number
        )));
    }
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE impacts SET resolved_at = now(), resolved_by = $2, resolution = 'acknowledged', note = $3
         WHERE id = $1 AND resolved_at IS NULL",
    )
    .bind(id)
    .bind(actor.id)
    .bind(&note)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(AppError::Conflict(
            "The impact was resolved in the meantime.".into(),
        ));
    }
    audit::record_related(
        &mut tx,
        Some(actor),
        "impact.acknowledge",
        "item",
        Some(impact.item_id),
        Some(impact.source_item_id),
        format!(
            "{} checked against {}/{} ({}): {note}",
            impact.item_number, impact.source_number, impact.source_code, impact.event
        ),
        json!({ "impact_id": id, "source_revision_id": impact.source_revision_id }),
    )
    .await?;
    tx.commit().await?;
    get(pool, actor, id).await
}

/// The result of [`acknowledge_all`].
#[derive(Clone, Debug, Serialize)]
pub struct Acknowledged {
    pub item_number: String,
    pub revision_code: String,
    /// The impacts acknowledged now (resolved, with the note).
    pub acknowledged: Vec<Impact>,
    /// Requested impacts that were resolved in the meantime (skipped).
    pub skipped: usize,
}

/// Acknowledges every open impact on a dependent revision with one note –
/// or only those in `only` (the ones the user saw, so a check flagged a
/// moment ago is not acknowledged unseen). Impacts resolved in the meantime
/// are skipped; if none is left, nothing changes (conflict). Same rules as
/// [`acknowledge`]; one transaction, one audit entry per impact.
///
/// # Invariants
///
/// * **Permission:** CAD editor; the revision is visible ("not found" otherwise) and in a group
///   the actor may edit (403); a note is needed (400); a given list is not empty and names only
///   impacts on this revision (400); at least one impact is still open (409).
/// * **Locks:** the open impact rows (`FOR UPDATE`), before they are resolved.
/// * **Audit:** `impact.acknowledge` per impact.
/// * **Notifications / webhooks:** none.
/// * **Atomic:** one transaction; impacts resolved in the meantime are skipped and counted.
pub async fn acknowledge_all(
    pool: &PgPool,
    actor: &Actor,
    revision_id: Uuid,
    only: Option<&[Uuid]>,
    note: &str,
) -> AppResult<Acknowledged> {
    actor.require(Role::Editor)?;
    let note = domain::require_text("Note", note, 2000).map_err(|_| {
        AppError::BadRequest(
            "Say why the dependent needs no change (e.g. form, fit and function unchanged).".into(),
        )
    })?;
    let rev = revisions::get(pool, actor, revision_id).await?;
    if !actor.can_edit_project(rev.project_id) {
        return Err(AppError::Forbidden(format!(
            "Only CAD editors of the project group {} (or admins) can acknowledge impacts on {}.",
            rev.project_code.as_deref().unwrap_or("—"),
            rev.item_number
        )));
    }
    // The requested ids, without duplicates; an empty list is a mistake of
    // the caller, not "resolved in the meantime".
    let wanted = only.map(|o| {
        let mut wanted = o.to_vec();
        wanted.sort_unstable();
        wanted.dedup();
        wanted
    });
    if wanted.as_ref().is_some_and(Vec::is_empty) {
        return Err(AppError::BadRequest(
            "List the impacts to acknowledge (or leave the list out for all).".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    if let Some(wanted) = &wanted {
        // Every listed impact has to be one on this revision; otherwise the
        // answer would count foreign or unknown ids as "resolved meanwhile".
        let known = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM impacts WHERE revision_id = $1 AND id = ANY($2)",
        )
        .bind(revision_id)
        .bind(wanted)
        .fetch_one(&mut *tx)
        .await?;
        if known != wanted.len() as i64 {
            return Err(AppError::BadRequest(format!(
                "Only impacts on {} can be acknowledged here.",
                revisions::label(&rev)
            )));
        }
    }
    let ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM impacts
         WHERE revision_id = $1 AND resolved_at IS NULL AND ($2::uuid[] IS NULL OR id = ANY($2))
         ORDER BY created_at
         FOR UPDATE",
    )
    .bind(revision_id)
    .bind(wanted.as_deref())
    .fetch_all(&mut *tx)
    .await?;
    if ids.is_empty() {
        return Err(AppError::Conflict(format!(
            "The checks on {} were resolved in the meantime.",
            revisions::label(&rev)
        )));
    }
    let skipped = wanted.map_or(0, |w| w.len().saturating_sub(ids.len()));
    sqlx::query(
        "UPDATE impacts SET resolved_at = now(), resolved_by = $2, resolution = 'acknowledged', note = $3
         WHERE id = ANY($1) AND resolved_at IS NULL",
    )
    .bind(&ids)
    .bind(actor.id)
    .bind(&note)
    .execute(&mut *tx)
    .await?;
    // Unredacted for the audit entries; the answer is redacted below.
    let acknowledged = sqlx::query_as::<_, Impact>(&format!(
        "{IMPACT_SELECT} WHERE im.id = ANY($1) ORDER BY si.number, im.id"
    ))
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;
    for impact in &acknowledged {
        audit::record_related(
            &mut tx,
            Some(actor),
            "impact.acknowledge",
            "item",
            Some(impact.item_id),
            Some(impact.source_item_id),
            format!(
                "{} checked against {}/{} ({}): {note}",
                impact.item_number, impact.source_number, impact.source_code, impact.event
            ),
            json!({ "impact_id": impact.id, "source_revision_id": impact.source_revision_id }),
        )
        .await?;
    }
    let mut acknowledged = redact_all(&mut tx, actor, acknowledged).await?;
    // by source number; hidden sources last (their place would bound it)
    acknowledged.sort_by_key(|i| i.source_restricted);
    tx.commit().await?;
    Ok(Acknowledged {
        item_number: rev.item_number,
        revision_code: rev.code,
        acknowledged,
        skipped,
    })
}

// ---------------------------------------------------------------------------
// Grouping by dependent (worklists)
// ---------------------------------------------------------------------------

/// The open impacts on one dependent revision: everything that has to be
/// checked on one assembly, drawing or simulation.
#[derive(Clone, Debug, Serialize)]
pub struct DependentGroup {
    pub item_id: Uuid,
    pub item_number: String,
    pub item_name: String,
    pub project_id: Option<Uuid>,
    pub project_code: Option<String>,
    pub revision_id: Uuid,
    pub revision_code: String,
    pub revision_state: String,
    /// When the newest of its impacts was flagged.
    pub latest_at: DateTime<Utc>,
    /// The impact ids, comma-separated (the hidden field of the web form
    /// that acknowledges exactly the impacts shown).
    pub impact_ids: String,
    /// The impacts, ordered by source item number.
    pub impacts: Vec<Impact>,
}

/// Groups impacts by the dependent revision. Groups keep the order in which
/// they first appear (newest first for [`open_for`]); inside a group the
/// sources are ordered by item number and revision.
pub fn group_by_dependent(impacts: Vec<Impact>) -> Vec<DependentGroup> {
    let mut groups: Vec<DependentGroup> = Vec::new();
    for impact in impacts {
        if let Some(g) = groups
            .iter_mut()
            .find(|g| g.revision_id == impact.revision_id)
        {
            g.latest_at = g.latest_at.max(impact.created_at);
            g.impacts.push(impact);
            continue;
        }
        groups.push(DependentGroup {
            item_id: impact.item_id,
            item_number: impact.item_number.clone(),
            item_name: impact.item_name.clone(),
            project_id: impact.project_id,
            project_code: impact.project_code.clone(),
            revision_id: impact.revision_id,
            revision_code: impact.revision_code.clone(),
            revision_state: impact.revision_state.clone(),
            latest_at: impact.created_at,
            impact_ids: String::new(),
            impacts: vec![impact],
        });
    }
    for g in &mut groups {
        // hidden sources last (their place among the numbers would bound them)
        g.impacts.sort_by(|a, b| {
            (a.source_restricted, &a.source_number, &a.source_code).cmp(&(
                b.source_restricted,
                &b.source_number,
                &b.source_code,
            ))
        });
        g.impact_ids = g
            .impacts
            .iter()
            .map(|i| i.id.to_string())
            .collect::<Vec<_>>()
            .join(",");
    }
    groups
}

/// Parses the comma-separated impact ids of a web form (see
/// [`DependentGroup::impact_ids`]); empty means all open impacts.
pub fn parse_ids(text: &str) -> AppResult<Option<Vec<Uuid>>> {
    let ids = text
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            Uuid::parse_str(s).map_err(|_| AppError::BadRequest("Invalid impact list.".into()))
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok((!ids.is_empty()).then_some(ids))
}

/// Flags the dependents of `source_revision` (see the module docs) and
/// returns their item ids. Items in `exclude` are not flagged.
async fn flag_dependents(
    conn: &mut PgConnection,
    source: &Revision,
    event: &str,
    change_id: Option<Uuid>,
    exclude: &[Uuid],
) -> AppResult<Vec<Uuid>> {
    // On conflict the newer event replaces the open impact of the same source.
    const UPSERT: &str = "
        ON CONFLICT (item_id, source_item_id) WHERE resolved_at IS NULL
        DO UPDATE SET revision_id = EXCLUDED.revision_id, source_revision_id = EXCLUDED.source_revision_id,
                      event = EXCLUDED.event, relation = EXCLUDED.relation, detail = EXCLUDED.detail,
                      change_id = EXCLUDED.change_id, created_at = now()
        RETURNING item_id";
    // CAD files of dependents that reference a file of the source revision.
    let by_file = format!(
        "INSERT INTO impacts (id, item_id, revision_id, source_item_id, source_revision_id, event,
                              relation, detail, change_id)
         SELECT DISTINCT ON (pr.item_id) gen_random_uuid(), pr.item_id, pr.id, $1, $2, $3, 'file',
                df.filename || ' references ' || sf.filename, $4
         FROM files sf
         JOIN file_references fr ON lower(fr.name) = lower(sf.filename)
         JOIN file_versions dv ON dv.id = fr.file_version_id
         JOIN files df ON df.id = dv.file_id AND dv.version = df.current_version
         JOIN revisions pr ON pr.id = df.revision_id
         JOIN items di ON di.id = pr.item_id
         WHERE sf.revision_id = $2 AND pr.item_id <> $1 AND {LATEST} AND NOT (pr.item_id = ANY($5))
           -- a file-name match alone never crosses the boundary of a
           -- restricted project group (it would tell one side about the other)
           AND (di.project_id IS NOT DISTINCT FROM (SELECT si.project_id FROM items si WHERE si.id = $1)
                OR NOT EXISTS (SELECT 1 FROM projects xp
                               WHERE xp.visibility = 'restricted'
                                 AND (xp.id = di.project_id
                                      OR xp.id = (SELECT si.project_id FROM items si WHERE si.id = $1))))
         ORDER BY pr.item_id, df.filename
         {UPSERT}"
    );
    // Drawings, specifications and references to the source item.
    let by_reference = format!(
        "INSERT INTO impacts (id, item_id, revision_id, source_item_id, source_revision_id, event,
                              relation, detail, change_id)
         SELECT DISTINCT ON (pr.item_id) gen_random_uuid(), pr.item_id, pr.id, $1, $2, $3, ir.kind,
                ir.note, $4
         FROM item_references ir
         JOIN revisions pr ON pr.id = ir.from_revision_id
         WHERE ir.to_item_id = $1 AND {LATEST} AND NOT (pr.item_id = ANY($5))
         ORDER BY pr.item_id, ir.kind
         {UPSERT}"
    );
    // Simulations based on a revision of the source item: flagged when a
    // different revision is released, when the simulated file version is no
    // longer the current one, or when the source becomes obsolete.
    let by_simulation = format!(
        "INSERT INTO impacts (id, item_id, revision_id, source_item_id, source_revision_id, event,
                              relation, detail, change_id)
         SELECT DISTINCT ON (pr.item_id) gen_random_uuid(), pr.item_id, pr.id, $1, $2, $3, 'simulation',
                'based on ' || br.code ||
                CASE WHEN sb.file_name <> '' THEN ', ' || sb.file_name || ' v' || sb.file_version ELSE '' END,
                $4
         FROM simulation_bases sb
         JOIN revisions br ON br.id = sb.basis_revision_id
         JOIN revisions pr ON pr.id = sb.revision_id
         LEFT JOIN file_versions fv ON fv.id = sb.file_version_id
         LEFT JOIN files f ON f.id = fv.file_id
         WHERE br.item_id = $1 AND {LATEST} AND NOT (pr.item_id = ANY($5))
           AND ($3 = 'obsolete' OR br.id <> $2
                OR (sb.file_name <> '' AND (fv.id IS NULL OR fv.version <> f.current_version)))
         ORDER BY pr.item_id, sb.role
         {UPSERT}"
    );
    // Assemblies using the source item (last, so the BOM wins if several apply).
    let by_bom = format!(
        "INSERT INTO impacts (id, item_id, revision_id, source_item_id, source_revision_id, event,
                              relation, detail, change_id)
         SELECT DISTINCT ON (pr.item_id) gen_random_uuid(), pr.item_id, pr.id, $1, $2, $3, 'bom',
                'find no. ' || bl.find_number, $4
         FROM bom_lines bl
         JOIN revisions pr ON pr.id = bl.parent_revision_id
         WHERE bl.child_item_id = $1 AND {LATEST} AND NOT (pr.item_id = ANY($5))
         ORDER BY pr.item_id, bl.find_number
         {UPSERT}"
    );
    let mut flagged = Vec::new();
    for sql in [by_file, by_simulation, by_reference, by_bom] {
        let ids = sqlx::query_scalar::<_, Uuid>(&sql)
            .bind(source.item_id)
            .bind(source.id)
            .bind(event)
            .bind(change_id)
            .bind(exclude)
            .fetch_all(&mut *conn)
            .await?;
        for id in ids {
            if !flagged.contains(&id) {
                flagged.push(id);
            }
        }
    }
    Ok(flagged)
}

/// Writes an audit entry that lists the flagged dependents.
async fn record_flags(
    conn: &mut PgConnection,
    actor: &Actor,
    source: &Revision,
    event: &str,
    flagged: &[Uuid],
) -> AppResult<()> {
    if flagged.is_empty() {
        return Ok(());
    }
    // One entry per project group of the dependents, each naming only its own
    // items: a reader who may not see a restricted group's dependents still
    // reads the entries about the others.
    let rows = sqlx::query_as::<_, (Option<Uuid>, Uuid, String)>(
        "SELECT project_id, id, number FROM items WHERE id = ANY($1)
         ORDER BY project_id NULLS FIRST, number",
    )
    .bind(flagged)
    .fetch_all(&mut *conn)
    .await?;
    let mut groups: Vec<(Option<Uuid>, Vec<Uuid>, Vec<String>)> = Vec::new();
    for (project, id, number) in rows {
        match groups.last_mut() {
            Some((p, ids, numbers)) if *p == project => {
                ids.push(id);
                numbers.push(number);
            }
            _ => groups.push((project, vec![id], vec![number])),
        }
    }
    for (_, ids, numbers) in groups {
        audit::record_mentioning(
            conn,
            Some(actor),
            "impact.flag",
            "revision",
            Some(source.id),
            &audit::Mentions::items(ids.iter().copied()),
            format!(
                "{} {event}: check {}",
                revisions::label(source),
                numbers.join(", ")
            ),
            json!({ "item_id": source.item_id, "dependents": ids }),
        )
        .await?;
    }
    notify_flagged(conn, actor, source, flagged).await
}

/// Tells the creator of each flagged item, the author of its latest revision
/// and its watchers that it has to be checked.
async fn notify_flagged(
    conn: &mut PgConnection,
    actor: &Actor,
    source: &Revision,
    flagged: &[Uuid],
) -> AppResult<()> {
    #[derive(sqlx::FromRow)]
    struct Row {
        item_id: Uuid,
        number: String,
        item_creator: Uuid,
        revision_id: Uuid,
        code: String,
        revision_creator: Uuid,
        event: String,
        relation: String,
        detail: String,
    }
    let rows = sqlx::query_as::<_, Row>(
        "SELECT i.id AS item_id, i.number, i.created_by AS item_creator, lr.id AS revision_id,
                lr.code, lr.created_by AS revision_creator, im.event, im.relation, im.detail
         FROM impacts im
         JOIN items i ON i.id = im.item_id
         CROSS JOIN LATERAL (SELECT r.id, r.code, r.created_by FROM revisions r
                             WHERE r.item_id = i.id ORDER BY r.seq DESC LIMIT 1) lr
         WHERE im.source_item_id = $1 AND im.item_id = ANY($2) AND im.resolved_at IS NULL",
    )
    .bind(source.item_id)
    .bind(flagged)
    .fetch_all(&mut *conn)
    .await?;
    for row in rows {
        let mut recipients = notifications::watchers(conn, "item", &[row.item_id]).await?;
        recipients.extend([row.item_creator, row.revision_creator]);
        let how = match row.relation.as_str() {
            "bom" => "uses it in its BOM",
            "drawing" => "is a drawing of it",
            "specification" => "is a specification for it",
            "reference" => "references it",
            "simulation" => "is a simulation based on it",
            _ => "has a CAD file that references its files",
        };
        let notice = Notice::revision_ref(
            NotificationKind::Impact,
            row.revision_id,
            row.item_id,
            &row.number,
            &row.code,
            format!(
                "Check needed: {} – {} {}",
                row.number,
                revisions::label(source),
                if row.event == "released" {
                    "was released"
                } else {
                    "is obsolete"
                }
            ),
            format!(
                "{} {how}{}. Acknowledge with a note if it needs no change, or revise it.",
                row.number,
                if row.detail.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", row.detail)
                }
            ),
        )
        // Recipients who may not see the source learn only that something
        // restricted changed.
        .redacted_for(
            source.item_id,
            format!(
                "Check needed: {} – a restricted item this depends on {}",
                row.number,
                if row.event == "released" {
                    "was released"
                } else {
                    "is obsolete"
                }
            ),
            format!(
                "{} {how}. Acknowledge with a note if it needs no change, or revise it.",
                row.number
            ),
        );
        notifications::notify(conn, Some(actor), &recipients, &notice).await?;
    }
    Ok(())
}

/// Change propagation after revisions were released together (one change).
/// Runs inside the releasing transaction, after the state change.
pub(crate) async fn on_release(
    conn: &mut PgConnection,
    actor: &Actor,
    change_id: Option<Uuid>,
    released: &[Uuid],
) -> AppResult<()> {
    if released.is_empty() {
        return Ok(());
    }
    // 1. As-released configuration of the released revisions.
    sqlx::query(
        "UPDATE bom_lines bl SET released_child_revision_id = (
             SELECT r.id FROM revisions r
             WHERE r.item_id = bl.child_item_id AND r.state = 'released'
             ORDER BY r.seq DESC LIMIT 1)
         WHERE bl.parent_revision_id = ANY($1)",
    )
    .bind(released)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE item_references ir SET released_to_revision_id = (
             SELECT r.id FROM revisions r
             WHERE r.item_id = ir.to_item_id AND r.state = 'released'
             ORDER BY r.seq DESC LIMIT 1)
         WHERE ir.from_revision_id = ANY($1)",
    )
    .bind(released)
    .execute(&mut *conn)
    .await?;
    // 2. The released items were reviewed with the current components.
    sqlx::query(
        "UPDATE impacts im SET resolved_at = now(), resolved_by = $2, resolution = 'superseded',
                note = 'Released as ' || r.code
         FROM revisions r
         WHERE r.id = ANY($1) AND im.item_id = r.item_id AND im.resolved_at IS NULL",
    )
    .bind(released)
    .bind(actor.id)
    .execute(&mut *conn)
    .await?;
    // 3. Flag the dependents.
    let mut revs = Vec::with_capacity(released.len());
    for id in released {
        revs.push(revisions::lock(conn, *id).await?);
    }
    let batch: Vec<Uuid> = revs.iter().map(|r| r.item_id).collect();
    for rev in &revs {
        let flagged = flag_dependents(conn, rev, "released", change_id, &batch).await?;
        record_flags(conn, actor, rev, "released", &flagged).await?;
    }
    Ok(())
}

/// Change propagation after a revision was made obsolete: if it was the
/// latest revision of its item, its dependents have to replace it.
pub(crate) async fn on_obsolete(
    conn: &mut PgConnection,
    actor: &Actor,
    rev: &Revision,
) -> AppResult<()> {
    let is_latest = sqlx::query_scalar::<_, bool>(
        "SELECT $2 = (SELECT MAX(seq) FROM revisions WHERE item_id = $1)",
    )
    .bind(rev.item_id)
    .bind(rev.seq)
    .fetch_one(&mut *conn)
    .await?;
    if !is_latest {
        return Ok(());
    }
    sqlx::query(
        "UPDATE impacts SET resolved_at = now(), resolved_by = $2, resolution = 'obsolete',
                note = 'The item is obsolete'
         WHERE item_id = $1 AND resolved_at IS NULL",
    )
    .bind(rev.item_id)
    .bind(actor.id)
    .execute(&mut *conn)
    .await?;
    let flagged = flag_dependents(conn, rev, "obsolete", None, &[rev.item_id]).await?;
    record_flags(conn, actor, rev, "is obsolete", &flagged).await
}

// ---------------------------------------------------------------------------
// Impact analysis (before a change is approved)
// ---------------------------------------------------------------------------

/// An item that depends – directly (level 1) or through other dependents –
/// on a revision of a change: one row per dependent, on the lowest level it
/// is reached on. Items that are part of the change are not listed (the walk
/// still goes on through them).
#[derive(Clone, Debug, Serialize)]
pub struct Dependent {
    /// The first of `root_item_ids` (kept for clients before #525).
    pub root_item_id: Uuid,
    /// Every item of the change it is reached from on its level, sorted
    /// (at least one): a row can merge the walks from several of them.
    pub root_item_ids: Vec<Uuid>,
    pub level: i32,
    pub item_id: Uuid,
    pub item_number: String,
    pub item_name: String,
    pub revision_id: Uuid,
    pub revision_code: String,
    pub revision_state: String,
    /// `bom`, `drawing`, `specification`, `reference`, `simulation` or
    /// `file` – the first of `paths`.
    pub relation: String,
    /// The item it uses or references – the first of `paths`.
    pub via_number: String,
    /// Every item it depends on at its level, with the relation (sorted by
    /// number, then relation; at least one).
    pub paths: Vec<DependsOn>,
}

/// One link of a [`Dependent`] to the item it depends on.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct DependsOn {
    pub via_number: String,
    pub relation: String,
}

/// A dependent reached by the walk of [`analysis`]: from the change's item
/// `root`, on `level`, through `via` (the item it depends on there).
#[derive(Clone, Debug, PartialEq)]
struct Found {
    root: Uuid,
    item: Uuid,
    level: i32,
    relation: String,
    via: Uuid,
    visible: bool,
}

/// One row of the impact analysis before the texts are added: a dependent on
/// its lowest level with every item of the change it is reached from there
/// (`roots`) and every `(relation, via)` link on that level.
#[derive(Debug, PartialEq)]
struct Nearest {
    roots: Vec<Uuid>,
    item: Uuid,
    level: i32,
    links: Vec<(String, Uuid)>,
}

/// One row per dependent: the walk reaches a dependent once per item of the
/// change (and an item of the change through another one), so the same
/// assembly could show up on several levels. Each dependent is kept on its
/// lowest level, with all its roots and links on that level (in the order
/// found, each once); items in `in_change` are left out – they are reviewed in the
/// change, not flagged. Order: first seen.
fn nearest_per_item(
    found: Vec<Found>,
    in_change: &std::collections::HashSet<Uuid>,
) -> Vec<Nearest> {
    use std::collections::{HashMap, HashSet};
    let mut lowest: HashMap<Uuid, i32> = HashMap::new();
    for f in found.iter().filter(|f| !in_change.contains(&f.item)) {
        let level = lowest.entry(f.item).or_insert(f.level);
        *level = (*level).min(f.level);
    }
    let mut index: HashMap<Uuid, usize> = HashMap::new();
    let mut linked: HashSet<(Uuid, Uuid, String)> = HashSet::new();
    let mut out: Vec<Nearest> = Vec::new();
    for f in found {
        if lowest.get(&f.item) != Some(&f.level) {
            continue;
        }
        let at = *index.entry(f.item).or_insert_with(|| {
            out.push(Nearest {
                roots: Vec::new(),
                item: f.item,
                level: f.level,
                links: Vec::new(),
            });
            out.len() - 1
        });
        let row = &mut out[at];
        if !row.roots.contains(&f.root) {
            row.roots.push(f.root);
        }
        if linked.insert((f.item, f.via, f.relation.clone())) {
            row.links.push((f.relation, f.via));
        }
    }
    out
}

/// Levels the impact analysis walks up.
const MAX_ANALYSIS_LEVELS: i32 = 12;

/// The direct dependents of a set of items: whose latest (not obsolete)
/// revision uses one of them in its BOM, references it or is a simulation
/// based on it – one query for a whole level of the analysis. `visible`
/// tells whether the reader (hidden groups bound at `$2`) may see the
/// dependent.
fn dependents_sql() -> String {
    format!(
        "SELECT e.target, e.dependent, e.relation, {} AS visible
         FROM (
             SELECT bl.child_item_id AS target, pr.item_id AS dependent, 'bom'::text AS relation
             FROM bom_lines bl JOIN revisions pr ON pr.id = bl.parent_revision_id
             WHERE bl.child_item_id = ANY($1) AND {LATEST}
             UNION
             SELECT ir.to_item_id, pr.item_id, ir.kind
             FROM item_references ir JOIN revisions pr ON pr.id = ir.from_revision_id
             WHERE ir.to_item_id = ANY($1) AND {LATEST}
             UNION
             SELECT br.item_id, pr.item_id, 'simulation'::text
             FROM revisions br
             JOIN simulation_bases sb ON sb.basis_revision_id = br.id
             JOIN revisions pr ON pr.id = sb.revision_id
             WHERE br.item_id = ANY($1) AND {LATEST}
         ) e
         JOIN items di ON di.id = e.dependent
         ORDER BY 1, 3, 2",
        access::item_visible("di", 2)
    )
}

/// The dependents of the given revisions that the actor may see, and how
/// many dependents were left out because they belong to restricted groups
/// (the walk does not go on through them): multi-level through BOMs and
/// item references of the latest revisions, plus CAD files that reference
/// their files.
///
/// The walk goes up level by level for all revisions at once (one query per
/// level, not one per path or per revision): every dependent is reported on
/// the lowest level it is reached on from each revision's item, with the
/// relation and the item it depends on there.
pub async fn analysis(
    pool: &PgPool,
    actor: &Actor,
    revision_ids: &[Uuid],
) -> AppResult<(Vec<Dependent>, i64)> {
    use std::collections::{HashMap, HashSet};
    if revision_ids.is_empty() {
        return Ok((Vec::new(), 0));
    }
    #[derive(sqlx::FromRow)]
    struct Root {
        revision_id: Uuid,
        item_id: Uuid,
        visible: bool,
    }
    let roots = sqlx::query_as::<_, Root>(&format!(
        "SELECT r.id AS revision_id, r.item_id, {} AS visible
         FROM revisions r JOIN items i ON i.id = r.item_id
         WHERE r.id = ANY($1)",
        access::item_visible("i", 2)
    ))
    .bind(revision_ids)
    .bind(&actor.hidden_projects)
    .fetch_all(pool)
    .await?;

    #[derive(sqlx::FromRow)]
    struct Edge {
        target: Uuid,
        dependent: Uuid,
        relation: String,
        visible: bool,
    }
    // CAD files of other items that reference the files (one level), for
    // all revisions in one query – run while the walk below goes up. Only
    // items the actor may see (hidden file-name matches are left out
    // without a count).
    #[derive(sqlx::FromRow)]
    struct FileReferrer {
        source_revision_id: Uuid,
        item_id: Uuid,
    }
    let file_referrers = async {
        Ok::<_, AppError>(
            sqlx::query_as::<_, FileReferrer>(&format!(
                "SELECT DISTINCT ON (sf.revision_id, i.number) sf.revision_id AS source_revision_id,
                        i.id AS item_id
                 FROM files sf
                 JOIN revisions sr ON sr.id = sf.revision_id
                 JOIN file_references fr ON lower(fr.name) = lower(sf.filename)
                 JOIN file_versions dv ON dv.id = fr.file_version_id
                 JOIN files df ON df.id = dv.file_id AND dv.version = df.current_version
                 JOIN revisions r ON r.id = df.revision_id
                 JOIN items i ON i.id = r.item_id
                 WHERE sf.revision_id = ANY($1)
                   AND r.item_id <> sr.item_id
                   AND r.seq = (SELECT MAX(x.seq) FROM revisions x WHERE x.item_id = r.item_id)
                   AND {}
                 ORDER BY sf.revision_id, i.number",
                access::item_visible("i", 2)
            ))
            .bind(revision_ids)
            .bind(&actor.hidden_projects)
            .fetch_all(pool)
            .await?,
        )
    };
    // (root item, item) pairs reached so far; the frontier holds the pairs
    // of the last level. Roots the actor may not see are not walked.
    let walk = async {
        let sql = dependents_sql();
        let mut reached: HashSet<(Uuid, Uuid)> =
            roots.iter().map(|r| (r.item_id, r.item_id)).collect();
        let mut frontier: Vec<(Uuid, Uuid)> = roots
            .iter()
            .filter(|r| r.visible)
            .map(|r| (r.item_id, r.item_id))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let mut found: Vec<Found> = Vec::new();
        for level in 1..=MAX_ANALYSIS_LEVELS {
            if frontier.is_empty() {
                break;
            }
            let targets: Vec<Uuid> = frontier
                .iter()
                .map(|(_, item)| *item)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            let edges = sqlx::query_as::<_, Edge>(&sql)
                .bind(&targets)
                .bind(&actor.hidden_projects)
                .fetch_all(pool)
                .await?;
            let mut by_target: HashMap<Uuid, Vec<&Edge>> = HashMap::new();
            for e in &edges {
                by_target.entry(e.target).or_default().push(e);
            }
            let mut next = Vec::new();
            for (root, item) in frontier {
                for e in by_target.get(&item).into_iter().flatten() {
                    if reached.insert((root, e.dependent)) {
                        // hidden dependents end the walk on their path
                        if e.visible {
                            next.push((root, e.dependent));
                        }
                        found.push(Found {
                            root,
                            item: e.dependent,
                            level,
                            relation: e.relation.clone(),
                            via: item,
                            visible: e.visible,
                        });
                    }
                }
            }
            frontier = next;
        }
        Ok::<_, AppError>(found)
    };
    let (found, file_referrers) = tokio::try_join!(walk, file_referrers)?;
    // Hidden dependents are dropped and counted.
    let hidden_items: HashSet<Uuid> = found
        .iter()
        .filter(|f| !f.visible)
        .map(|f| f.item)
        .collect();
    let mut found: Vec<Found> = found.into_iter().filter(|f| f.visible).collect();
    let root_of: HashMap<Uuid, Uuid> = roots
        .iter()
        .filter(|r| r.visible)
        .map(|r| (r.revision_id, r.item_id))
        .collect();
    // Checked per revision as before: a dependent found through BOM or
    // references keeps that relation.
    let mut by_pair: HashSet<(Uuid, Uuid)> = found.iter().map(|f| (f.root, f.item)).collect();
    for f in file_referrers {
        let Some(&root) = root_of.get(&f.source_revision_id) else {
            continue;
        };
        if by_pair.insert((root, f.item_id)) {
            found.push(Found {
                root,
                item: f.item_id,
                level: 1,
                relation: "file".into(),
                via: root,
                visible: true,
            });
        }
    }

    // One row per dependent, on its lowest level; the change's own items
    // are left out (#525).
    let in_change: HashSet<Uuid> = roots.iter().map(|r| r.item_id).collect();
    let nearest = nearest_per_item(found, &in_change);

    // Numbers, names and latest revisions of everything found, in one query.
    #[derive(sqlx::FromRow)]
    struct Latest {
        item_id: Uuid,
        number: String,
        revision_id: Uuid,
        code: String,
        state: String,
        name: String,
    }
    let ids: Vec<Uuid> = nearest
        .iter()
        .flat_map(|n| std::iter::once(n.item).chain(n.links.iter().map(|(_, via)| *via)))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let latest: HashMap<Uuid, Latest> = sqlx::query_as::<_, Latest>(
        "SELECT i.id AS item_id, i.number, lr.id AS revision_id, lr.code, lr.state, lr.name
         FROM items i
         CROSS JOIN LATERAL (
             SELECT r.id, r.code, r.state, r.name FROM revisions r
             WHERE r.item_id = i.id ORDER BY r.seq DESC LIMIT 1
         ) lr
         WHERE i.id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|l| (l.item_id, l))
    .collect();
    let mut out: Vec<Dependent> = nearest
        .into_iter()
        .filter_map(|n| {
            let item = latest.get(&n.item)?;
            let mut paths: Vec<DependsOn> = n
                .links
                .into_iter()
                .filter_map(|(relation, via)| {
                    Some(DependsOn {
                        via_number: latest.get(&via)?.number.clone(),
                        relation,
                    })
                })
                .collect();
            paths.sort();
            paths.dedup();
            let first = paths.first()?.clone();
            let mut roots = n.roots;
            roots.sort();
            Some(Dependent {
                root_item_id: *roots.first()?,
                root_item_ids: roots,
                level: n.level,
                item_id: n.item,
                item_number: item.number.clone(),
                item_name: item.name.clone(),
                revision_id: item.revision_id,
                revision_code: item.code.clone(),
                revision_state: item.state.clone(),
                relation: first.relation,
                via_number: first.via_number,
                paths,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        (a.level, &a.item_number, &a.via_number).cmp(&(b.level, &b.item_number, &b.via_number))
    });
    Ok((out, hidden_items.len() as i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn impact(n: u128, dependent: u128, source: &str, minute: u32) -> Impact {
        Impact {
            id: Uuid::from_u128(n),
            item_id: Uuid::from_u128(1000 + dependent),
            item_number: format!("ASM-{dependent:06}"),
            project_id: None,
            project_code: Some("ROVER".into()),
            revision_id: Uuid::from_u128(2000 + dependent),
            revision_code: "A".into(),
            revision_state: "released".into(),
            item_name: "Enclosure".into(),
            source_item_id: Uuid::from_u128(3000 + n),
            source_number: source.into(),
            source_revision_id: Uuid::from_u128(4000 + n),
            source_code: "A".into(),
            source_name: "Screw".into(),
            event: "released".into(),
            relation: "bom".into(),
            detail: String::new(),
            change_id: None,
            change_number: None,
            created_at: Utc.with_ymd_and_hms(2026, 11, 5, 9, minute, 0).unwrap(),
            resolved_at: None,
            resolution: None,
            resolved_by_name: None,
            note: String::new(),
            source_project_id: None,
            source_restricted: false,
        }
    }

    #[test]
    fn groups_by_dependent_revision_in_first_seen_order() {
        // newest first, as open_for returns them
        let groups = group_by_dependent(vec![
            impact(1, 2, "PRT-000008", 50),
            impact(2, 1, "PRT-000008", 40),
            impact(3, 2, "PRT-000005", 30),
            impact(4, 2, "PRT-000006", 20),
            impact(5, 1, "PRT-000006", 10),
        ]);
        let shape: Vec<(&str, Vec<&str>)> = groups
            .iter()
            .map(|g| {
                (
                    g.item_number.as_str(),
                    g.impacts.iter().map(|i| i.source_number.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(
            shape,
            vec![
                ("ASM-000002", vec!["PRT-000005", "PRT-000006", "PRT-000008"]),
                ("ASM-000001", vec!["PRT-000006", "PRT-000008"]),
            ]
        );
        assert_eq!(groups[0].latest_at.format("%M").to_string(), "50");
        assert_eq!(groups[0].revision_id, Uuid::from_u128(2002));
        assert_eq!(
            groups[1].impact_ids,
            format!("{},{}", Uuid::from_u128(5), Uuid::from_u128(2))
        );
        assert_eq!(parse_ids(&groups[1].impact_ids).unwrap().unwrap().len(), 2);
    }

    #[test]
    fn revisions_of_the_same_item_are_separate_groups() {
        let mut older = impact(1, 1, "PRT-000001", 1);
        older.revision_id = Uuid::from_u128(99);
        older.revision_code = "A".into();
        let mut newer = impact(2, 1, "PRT-000002", 2);
        newer.revision_code = "B".into();
        let groups = group_by_dependent(vec![newer, older]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].revision_code, "B");
        assert!(group_by_dependent(Vec::new()).is_empty());
    }

    #[test]
    fn parses_impact_id_lists() {
        assert_eq!(parse_ids("").unwrap(), None);
        assert_eq!(parse_ids(" , ").unwrap(), None);
        let id = Uuid::from_u128(7);
        assert_eq!(parse_ids(&format!(" {id} ,")).unwrap(), Some(vec![id]));
        assert!(parse_ids("nope").is_err());
    }

    fn found(root: u128, item: u128, level: i32, relation: &str, via: u128) -> Found {
        Found {
            root: Uuid::from_u128(root),
            item: Uuid::from_u128(item),
            level,
            relation: relation.into(),
            via: Uuid::from_u128(via),
            visible: true,
        }
    }

    /// The case of the UI review (#525): the change holds four parts and
    /// the assembly ASM-2 that uses them; ASM-1 uses ASM-2. The walk reaches
    /// ASM-2 from every part and ASM-1 on level 1 (from ASM-2) and level 2
    /// (from the parts through ASM-2).
    #[test]
    fn one_row_per_dependent_without_the_changes_own_items() {
        let (asm1, asm2) = (1, 2);
        let parts = [11, 12, 13, 14];
        let mut walk = Vec::new();
        for p in parts {
            walk.push(found(p, asm2, 1, "bom", p));
        }
        walk.push(found(asm2, asm1, 1, "bom", asm2));
        for p in parts {
            walk.push(found(p, asm1, 2, "bom", asm2));
        }
        let in_change: std::collections::HashSet<Uuid> = [asm2, 11, 12, 13, 14]
            .into_iter()
            .map(Uuid::from_u128)
            .collect();
        assert_eq!(
            nearest_per_item(walk, &in_change),
            vec![Nearest {
                roots: vec![Uuid::from_u128(asm2)],
                item: Uuid::from_u128(asm1),
                level: 1,
                links: vec![("bom".into(), Uuid::from_u128(asm2))],
            }]
        );
    }

    /// A dependent that uses several items of the change keeps every link
    /// and every root of its lowest level, each once; links on higher levels
    /// are dropped.
    #[test]
    fn keeps_all_links_of_the_lowest_level() {
        let walk = vec![
            found(10, 1, 2, "bom", 5),
            found(10, 1, 1, "bom", 10),
            found(11, 1, 1, "drawing", 11),
            found(11, 1, 1, "drawing", 11),
            found(10, 2, 1, "file", 10),
            found(11, 2, 1, "file", 11),
        ];
        let rows = nearest_per_item(walk, &std::collections::HashSet::new());
        let shape: Vec<(u128, i32, Vec<(&str, u128)>)> = rows
            .iter()
            .map(|r| {
                (
                    r.item.as_u128(),
                    r.level,
                    r.links
                        .iter()
                        .map(|(rel, via)| (rel.as_str(), via.as_u128()))
                        .collect(),
                )
            })
            .collect();
        assert_eq!(
            shape,
            vec![
                (1, 1, vec![("bom", 10), ("drawing", 11)]),
                (2, 1, vec![("file", 10), ("file", 11)]),
            ]
        );
        let roots: Vec<Vec<u128>> = rows
            .iter()
            .map(|r| r.roots.iter().map(|u| u.as_u128()).collect())
            .collect();
        assert_eq!(roots, vec![vec![10, 11], vec![10, 11]]);
        assert!(nearest_per_item(Vec::new(), &std::collections::HashSet::new()).is_empty());
    }
}
