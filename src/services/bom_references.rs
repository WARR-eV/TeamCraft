//! A BOM from the CAD references the server found.
//!
//! The scan of the CAD files (`cadrefs`, matched by
//! [`references::files_of_revision`]) knows which part files an assembly
//! uses. This module turns that into a **plan** (what the BOM of the revision
//! would be, with quantities) and applies the lines the user ticked.
//!
//! # Which references count
//!
//! * Files of an **assembly** format (SOLIDWORKS `.SLDASM`, Inventor, …),
//!   NX files and SolidCraft designs **that have component instances** (an
//!   NX `.prt` or a SolidCraft `.sc` is a part or an assembly by its content,
//!   not by its extension), and STEP assemblies whose occurrences lead to
//!   **external files**. Drawings and every other file do not count: a
//!   drawing's link to its model is a *drawing of* reference.
//! * **Quantity:** NX – the placed instances of the component (hidden layers
//!   left out, as in the preview); SOLIDWORKS – the instances of the
//!   component tree without `swExcludeFromBOM` (a component with only
//!   excluded instances is left out); STEP – the assembly occurrences of the
//!   external file; Inventor – the occurrences in the file descriptor table
//!   (suppressed ones left out; a part with only suppressed ones is left
//!   out); SolidCraft – the placements of each linked design; otherwise 1,
//!   flagged *assumed*.
//! * A child is listed once per item. Unmatched names are listed as *not in
//!   TeamCraft* (never created). Children in project groups the actor may not
//!   see are only counted.
//!
//! # Invariants of [`apply`]
//!
//! * **Permission:** the revision is visible ("not found" otherwise); it is
//!   checked out by the actor and editable (`revisions::ensure_editable`, 409
//!   otherwise).
//! * **Lines:** only children of the plan can be chosen; an existing line is
//!   never duplicated or replaced, its quantity is changed only when
//!   `update_quantities` is set; a child that would make the BOM circular is
//!   refused per line.
//! * **Locks:** the parent revision row.
//! * **Audit:** `bom.add` per line (as `bom::add_tx`), `bom.update` per
//!   changed quantity and one `bom.from_references` with the counts.
//! * **Atomic:** one transaction.
//!
//! # At check-in (#407)
//!
//! A check-in with *Update the BOM from the CAD file*
//! (`bom_from_references`) runs `apply_on_checkin` in the check-in's
//! transaction, after the new versions and their references are stored:
//! new children are added, CAD-counted quantities corrected, circular
//! children refused, nothing removed (see its invariants).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    auth::Actor,
    cadformat::FormatKind,
    error::{AppError, AppResult},
    models::Revision,
    services::{
        access, audit, bom,
        references::{self, FileWithReferences},
        revisions,
    },
    util, AppState,
};

/// Note of a line whose quantity the CAD file does not tell.
pub const ASSUMED_NOTE: &str = "quantity not known from the CAD file – check it";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantitySource {
    /// Instances counted in the CAD file.
    CadInstances,
    /// The file does not tell: 1.
    Assumed,
}

impl QuantitySource {
    pub fn label(self) -> &'static str {
        match self {
            QuantitySource::CadInstances => "CAD instances",
            QuantitySource::Assumed => "assumed 1",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildStatus {
    /// Not in the BOM yet.
    New,
    /// A line with the same quantity exists.
    AlreadyInBom,
    /// A line exists with another quantity.
    Differs,
    /// Adding it would make the BOM circular.
    Circular,
}

impl ChildStatus {
    pub fn label(self) -> &'static str {
        match self {
            ChildStatus::New => "new line",
            ChildStatus::AlreadyInBom => "already in the BOM",
            ChildStatus::Differs => "differs",
            ChildStatus::Circular => "circular",
        }
    }
}

/// A child found in the CAD references.
#[derive(Clone, Debug, Serialize)]
pub struct PlannedChild {
    pub item_id: Uuid,
    pub item_number: String,
    pub item_name: String,
    pub quantity: f64,
    pub quantity_source: QuantitySource,
    pub quantity_label: &'static str,
    /// The quantities as shown (`4`, not `4.0`).
    pub quantity_text: String,
    pub bom_quantity_text: Option<String>,
    /// The CAD file that references it and the referenced file.
    pub via: String,
    pub status: ChildStatus,
    pub status_label: &'static str,
    /// The quantity of the existing BOM line.
    pub bom_quantity: Option<f64>,
    /// Ticked by default (new lines).
    pub selected: bool,
}

/// A referenced file TeamCraft does not have (or not visible to the actor).
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Unmatched {
    pub name: String,
    pub via: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Plan {
    pub revision_id: Uuid,
    pub children: Vec<PlannedChild>,
    pub not_in_teamcraft: Vec<Unmatched>,
    /// Referenced items in project groups the actor may not see (counted,
    /// not named).
    pub hidden: i64,
    /// Components of the CAD files that are excluded from the BOM there.
    pub excluded: Vec<String>,
    /// The revision has a file whose references describe its structure.
    #[serde(skip)]
    pub structure: bool,
    /// Items the structure files reference only through components excluded
    /// from the BOM there (`excluded`): still in the CAD file.
    #[serde(skip)]
    pub excluded_items: Vec<Uuid>,
}

/// A child before it is compared with the BOM.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Candidate {
    pub item_id: Uuid,
    pub item_number: String,
    pub item_name: String,
    pub quantity: f64,
    pub source: QuantitySource,
    pub via: String,
}

/// One CAD file of the revision with what is known about its components.
pub(crate) struct SourceFile<'a> {
    pub file: &'a FileWithReferences,
    /// Format id (`nx-part`, …).
    pub format: &'a str,
    /// Instance counts by lower case file name (`None`: the file tells none).
    pub counts: Option<&'a HashMap<String, u32>>,
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct Collected {
    pub children: Vec<Candidate>,
    pub unmatched: Vec<Unmatched>,
    pub excluded: Vec<String>,
    /// The items behind `excluded` (other revisions' files only).
    pub excluded_items: Vec<Uuid>,
}

/// The structure rule: a file with references describes the structure of
/// its revision when it is of an assembly format, or an NX part, SolidCraft
/// design or STEP file whose references carry instance counts (`counted`:
/// an assembly by content, not by extension).
fn structure_rule(kind: Option<FormatKind>, format: &str, counted: bool) -> bool {
    match kind {
        Some(FormatKind::Assembly) => true,
        // NX, SolidCraft: an assembly by content (it has component instances)
        Some(FormatKind::Model) if matches!(format, "nx-part" | "solidcraft-design") => counted,
        // STEP: an assembly with external references (occurrences counted)
        Some(FormatKind::Exchange) if format == "step" => counted,
        _ => false,
    }
}

/// Does this file's references describe the structure of the revision?
fn is_structure(src: &SourceFile<'_>) -> bool {
    structure_rule(
        src.file.format_kind,
        src.format,
        src.counts.is_some_and(|c| !c.is_empty()),
    )
}

/// Has the revision a structure file, by what the reference scan stored
/// (`file_references.instances`)? Cheap – no file is parsed and no name is
/// matched; the caller has checked that the actor may read the revision.
/// Offers *Update the BOM from the CAD file* at check-in (web form, desktop
/// `file-status`).
pub async fn has_structure(pool: &PgPool, revision_id: Uuid) -> AppResult<bool> {
    let rows = sqlx::query_as::<_, (String, bool)>(
        "SELECT COALESCE(f.format, ''), COALESCE(bool_or(fr.instances >= 0), FALSE)
         FROM files f
         JOIN file_versions fv ON fv.file_id = f.id AND fv.version = f.current_version
         JOIN file_references fr ON fr.file_version_id = fv.id
         WHERE f.revision_id = $1
         GROUP BY f.id, f.format",
    )
    .bind(revision_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().any(|(format, counted)| {
        structure_rule(
            crate::cadformat::by_id(format).map(|f| f.kind),
            format,
            *counted,
        )
    }))
}

/// The children of the structure files: the first match of every referenced
/// name that is another item, once per item.
pub(crate) fn collect(files: &[SourceFile<'_>]) -> Collected {
    let mut out = Collected::default();
    let mut seen: HashSet<Uuid> = HashSet::new();
    let mut seen_unmatched: HashSet<String> = HashSet::new();
    for src in files.iter().filter(|s| is_structure(s)) {
        for name in &src.file.references {
            let key = name.name.to_lowercase();
            let counted = src.counts.and_then(|c| c.get(&key)).copied();
            if counted == Some(0) {
                if !out.excluded.contains(&name.name) {
                    out.excluded.push(name.name.clone());
                }
                // still in the CAD file, only not in its BOM
                for m in name.matches.iter().filter(|m| !m.same_revision) {
                    if !out.excluded_items.contains(&m.item_id) {
                        out.excluded_items.push(m.item_id);
                    }
                }
                continue;
            }
            if name.matches.is_empty() {
                if seen_unmatched.insert(key) {
                    out.unmatched.push(Unmatched {
                        name: name.name.clone(),
                        via: src.file.filename.clone(),
                    });
                }
                continue;
            }
            // the revision's own files are no children
            let Some(m) = name.matches.iter().find(|m| !m.same_revision) else {
                continue;
            };
            if !seen.insert(m.item_id) {
                continue;
            }
            let (quantity, source) = match counted {
                Some(n) => (f64::from(n), QuantitySource::CadInstances),
                None => (1.0, QuantitySource::Assumed),
            };
            out.children.push(Candidate {
                item_id: m.item_id,
                item_number: m.item_number.clone(),
                item_name: m.item_name.clone(),
                quantity,
                source,
                via: format!("{} → {}", src.file.filename, m.filename),
            });
        }
    }
    out
}

/// Compares a candidate with the existing BOM line (`bom_quantity`) and the
/// cycle check.
pub(crate) fn status_of(bom_quantity: Option<f64>, quantity: f64, circular: bool) -> ChildStatus {
    match bom_quantity {
        Some(q) if (q - quantity).abs() < 1e-9 => ChildStatus::AlreadyInBom,
        Some(_) => ChildStatus::Differs,
        None if circular => ChildStatus::Circular,
        None => ChildStatus::New,
    }
}

/// The note of a line created from a candidate.
pub(crate) fn line_note(c: &Candidate) -> String {
    match c.source {
        QuantitySource::CadInstances => format!(
            "From the CAD references ({} instance{})",
            util::format_number(c.quantity, 4),
            if (c.quantity - 1.0).abs() < 1e-9 {
                ""
            } else {
                "s"
            }
        ),
        QuantitySource::Assumed => ASSUMED_NOTE.to_string(),
    }
}

#[derive(sqlx::FromRow)]
struct FileRow {
    id: Uuid,
    sha256: String,
    format: String,
}

/// The stored instance counts of one file, by lower case name (`None`: the
/// file told none).
fn stored_counts(file: &FileWithReferences) -> Option<HashMap<String, u32>> {
    let counts: HashMap<String, u32> = file
        .references
        .iter()
        .filter_map(|r| {
            r.instances
                .and_then(|n| u32::try_from(n).ok())
                .map(|n| (r.name.to_lowercase(), n))
        })
        .collect();
    (!counts.is_empty()).then_some(counts)
}

/// Instance counts of the components of one file, read off the async runtime.
async fn read_counts(
    st: &AppState,
    sha256: &str,
    format: &str,
) -> AppResult<Option<HashMap<String, u32>>> {
    let path = st.vault.object_path(sha256)?;
    let format = format.to_string();
    // the same counts the reference scan stores (`file_references.instances`)
    tokio::task::spawn_blocking(move || crate::cadrefs::instance_counts(&path, &format))
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("reading the CAD file failed: {e}")))
}

/// Hidden items (project groups the actor may not see) among the names.
async fn hidden_names(
    conn: &mut PgConnection,
    actor: &Actor,
    revision_id: Uuid,
    names: &[String],
) -> AppResult<Vec<(String, Uuid)>> {
    if names.is_empty() || actor.hidden_projects.is_empty() {
        return Ok(Vec::new());
    }
    let keys: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    Ok(sqlx::query_as::<_, (String, Uuid)>(
        "SELECT DISTINCT lower(f.filename), i.id
         FROM files f
         JOIN revisions r ON r.id = f.revision_id
         JOIN items i ON i.id = r.item_id
         WHERE lower(f.filename) = ANY($1)
           AND r.item_id <> (SELECT item_id FROM revisions WHERE id = $2)
           AND r.seq = (SELECT MAX(x.seq) FROM revisions x WHERE x.item_id = r.item_id)
           AND i.project_id = ANY($3)",
    )
    .bind(&keys)
    .bind(revision_id)
    .bind(&actor.hidden_projects)
    .fetch_all(&mut *conn)
    .await?)
}

/// What the BOM of the revision would be, from the CAD references.
///
/// Read-only; the actor must be able to read the revision. The files are
/// parsed for their instance counts with no pooled connection held: a plan
/// of a large assembly never keeps a connection busy while it reads.
pub async fn plan(st: &AppState, actor: &Actor, revision_id: Uuid) -> AppResult<Plan> {
    let rev = revisions::get(&st.db, actor, revision_id).await?;
    access::ensure_visible(actor, rev.project_id, access::not_found_revision())?;
    let (files, rows) = {
        let mut conn = st.db.acquire().await?;
        load_files(&mut conn, actor, revision_id).await?
    };
    let meta: HashMap<Uuid, &FileRow> = rows.iter().map(|r| (r.id, r)).collect();
    let mut counts: HashMap<Uuid, HashMap<String, u32>> = HashMap::new();
    for f in &files {
        if let Some(row) = meta.get(&f.file_id) {
            if let Some(c) = read_counts(st, &row.sha256, &row.format).await? {
                counts.insert(f.file_id, c);
            }
        }
    }
    let mut conn = st.db.acquire().await?;
    compare_on(&mut conn, actor, &rev, &files, &meta, &counts).await
}

/// The plan of `rev` with the stored instance counts, read on one
/// connection: inside a transaction it sees the references and BOM lines
/// that transaction wrote (the check-in, which never parses a file while it
/// holds the revision lock).
async fn plan_on(conn: &mut PgConnection, actor: &Actor, rev: &Revision) -> AppResult<Plan> {
    access::ensure_visible(actor, rev.project_id, access::not_found_revision())?;
    let (files, rows) = load_files(conn, actor, rev.id).await?;
    let meta: HashMap<Uuid, &FileRow> = rows.iter().map(|r| (r.id, r)).collect();
    let counts: HashMap<Uuid, HashMap<String, u32>> = files
        .iter()
        .filter_map(|f| stored_counts(f).map(|c| (f.file_id, c)))
        .collect();
    compare_on(conn, actor, rev, &files, &meta, &counts).await
}

/// The files of the revision with their matched references, and their
/// content hash and format.
async fn load_files(
    conn: &mut PgConnection,
    actor: &Actor,
    revision_id: Uuid,
) -> AppResult<(Vec<FileWithReferences>, Vec<FileRow>)> {
    let files = references::files_of_revision_on(conn, actor, revision_id).await?;
    let ids: Vec<Uuid> = files.iter().map(|f| f.file_id).collect();
    let rows = sqlx::query_as::<_, FileRow>(
        "SELECT id, sha256, COALESCE(format, '') AS format FROM files WHERE id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(&mut *conn)
    .await?;
    Ok((files, rows))
}

/// The plan from the loaded files and their counts, compared with the BOM
/// of `rev` on `conn`.
async fn compare_on(
    conn: &mut PgConnection,
    actor: &Actor,
    rev: &Revision,
    files: &[FileWithReferences],
    meta: &HashMap<Uuid, &FileRow>,
    counts: &HashMap<Uuid, HashMap<String, u32>>,
) -> AppResult<Plan> {
    let revision_id = rev.id;
    let sources: Vec<SourceFile<'_>> = files
        .iter()
        .map(|f| SourceFile {
            file: f,
            format: meta.get(&f.file_id).map_or("", |r| r.format.as_str()),
            counts: counts.get(&f.file_id),
        })
        .collect();
    let structure = sources.iter().any(is_structure);
    let mut collected = collect(&sources);

    let existing: HashMap<Uuid, f64> = sqlx::query_as::<_, (Uuid, f64)>(
        "SELECT child_item_id, quantity FROM bom_lines WHERE parent_revision_id = $1",
    )
    .bind(revision_id)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .collect();
    let mut children = Vec::new();
    for c in collected.children {
        let bom_quantity = existing.get(&c.item_id).copied();
        let circular =
            bom_quantity.is_none() && bom::creates_cycle(conn, rev.item_id, c.item_id).await?;
        let status = status_of(bom_quantity, c.quantity, circular);
        children.push(PlannedChild {
            item_id: c.item_id,
            item_number: c.item_number,
            item_name: c.item_name,
            quantity: c.quantity,
            quantity_source: c.source,
            quantity_label: c.source.label(),
            quantity_text: util::format_number(c.quantity, 4),
            bom_quantity_text: bom_quantity.map(|q| util::format_number(q, 4)),
            via: c.via,
            status,
            status_label: status.label(),
            bom_quantity,
            selected: status == ChildStatus::New,
        });
    }

    // names no visible item has: some may belong to hidden items
    let names: Vec<String> = collected.unmatched.iter().map(|u| u.name.clone()).collect();
    let hidden = hidden_names(conn, actor, revision_id, &names).await?;
    let hidden_keys: HashSet<&str> = hidden.iter().map(|(k, _)| k.as_str()).collect();
    collected
        .unmatched
        .retain(|u| !hidden_keys.contains(u.name.to_lowercase().as_str()));
    let hidden_items: HashSet<Uuid> = hidden.iter().map(|(_, id)| *id).collect();
    Ok(Plan {
        revision_id,
        children,
        not_in_teamcraft: collected.unmatched,
        hidden: hidden_items.len() as i64,
        excluded: collected.excluded,
        structure,
        excluded_items: collected.excluded_items,
    })
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ApplyRequest {
    /// Item numbers of the children to add.
    #[serde(default)]
    pub children: Vec<String>,
    /// Also change the quantity of existing lines that differ.
    #[serde(default)]
    pub update_quantities: bool,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct Refused {
    pub child: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ApplyResult {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    /// Chosen but left as they are (already in the BOM, quantity differs
    /// without `update_quantities`).
    pub skipped: Vec<String>,
    pub refused: Vec<Refused>,
}

/// Adds the chosen children of the plan as BOM lines (see the module docs).
pub async fn apply(
    st: &AppState,
    actor: &Actor,
    revision_id: Uuid,
    req: &ApplyRequest,
) -> AppResult<ApplyResult> {
    let rev = revisions::get(&st.db, actor, revision_id).await?;
    revisions::ensure_editable(&rev, actor)?;
    if req.children.is_empty() {
        return Err(AppError::BadRequest(
            "Choose at least one child to add.".into(),
        ));
    }
    let plan = plan(st, actor, revision_id).await?;
    let by_number: HashMap<&str, &PlannedChild> = plan
        .children
        .iter()
        .map(|c| (c.item_number.as_str(), c))
        .collect();
    let mut chosen: Vec<&PlannedChild> = Vec::new();
    let mut seen = HashSet::new();
    for number in &req.children {
        let number = number.trim();
        let Some(child) = by_number.get(number) else {
            return Err(AppError::BadRequest(format!(
                "{number} is not a part the CAD references of this revision lead to."
            )));
        };
        if seen.insert(number.to_string()) {
            chosen.push(child);
        }
    }

    let mut tx = st.db.begin().await?;
    let parent = revisions::lock(&mut tx, revision_id).await?;
    revisions::ensure_editable(&parent, actor)?;
    let result =
        apply_children(&mut tx, actor, &parent, &chosen, |_| req.update_quantities).await?;
    audit::record_related(
        &mut tx,
        Some(actor),
        "bom.from_references",
        "revision",
        Some(parent.id),
        None,
        format!(
            "Created BOM lines of {} from the CAD references: {} added, {} updated",
            revisions::label(&parent),
            result.added.len(),
            result.updated.len()
        ),
        json!({
            "item_id": parent.item_id,
            "source": "cad_references",
            "added": result.added.len(),
            "updated": result.updated.len(),
            "skipped": result.skipped.len(),
            "refused": result.refused.len(),
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

/// The shared step of [`apply`] and [`apply_on_checkin`], on the locked
/// parent: adds the chosen children that have no line yet, changes the
/// quantity of a differing line where `update` says so, refuses a child that
/// would make the BOM circular. Never removes a line.
async fn apply_children(
    tx: &mut PgConnection,
    actor: &Actor,
    parent: &Revision,
    chosen: &[&PlannedChild],
    update: impl Fn(&PlannedChild) -> bool,
) -> AppResult<ApplyResult> {
    // decided under the lock: another session may have added lines meanwhile
    let existing: HashMap<Uuid, (Uuid, f64)> = sqlx::query_as::<_, (Uuid, Uuid, f64)>(
        "SELECT child_item_id, id, quantity FROM bom_lines WHERE parent_revision_id = $1",
    )
    .bind(parent.id)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|(child, id, q)| (child, (id, q)))
    .collect();
    let mut result = ApplyResult::default();
    for child in chosen.iter().copied() {
        let candidate = Candidate {
            item_id: child.item_id,
            item_number: child.item_number.clone(),
            item_name: child.item_name.clone(),
            quantity: child.quantity,
            source: child.quantity_source,
            via: child.via.clone(),
        };
        match existing.get(&child.item_id) {
            Some((line_id, q)) if (q - child.quantity).abs() >= 1e-9 && update(child) => {
                sqlx::query("UPDATE bom_lines SET quantity = $2 WHERE id = $1")
                    .bind(line_id)
                    .bind(child.quantity)
                    .execute(&mut *tx)
                    .await?;
                bom::touch(&mut *tx, parent.id).await?;
                audit::record_related(
                    &mut *tx,
                    Some(actor),
                    "bom.update",
                    "revision",
                    Some(parent.id),
                    Some(child.item_id),
                    format!(
                        "Changed the quantity of {} in {} from {} to {} (CAD references)",
                        child.item_number,
                        revisions::label(parent),
                        util::format_number(*q, 4),
                        util::format_number(child.quantity, 4)
                    ),
                    json!({ "item_id": parent.item_id, "line_id": line_id }),
                )
                .await?;
                result.updated.push(child.item_number.clone());
            }
            Some(_) => result.skipped.push(child.item_number.clone()),
            None => {
                match bom::add_tx(
                    &mut *tx,
                    actor,
                    parent,
                    child.item_id,
                    &child.item_number,
                    child.quantity,
                    None,
                    &line_note(&candidate),
                )
                .await
                {
                    Ok(_) => result.added.push(child.item_number.clone()),
                    Err(AppError::Conflict(reason)) if child.status == ChildStatus::Circular => {
                        result.refused.push(Refused {
                            child: child.item_number.clone(),
                            reason,
                        })
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }
    Ok(result)
}

/// What the BOM step of a check-in changed (#407). Item numbers, except
/// `not_in_teamcraft` (referenced file names).
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct CheckinBom {
    /// New lines (children with the status *new*).
    pub added: Vec<String>,
    /// Lines whose quantity was corrected to the counted CAD instances.
    pub updated: Vec<String>,
    /// Lines whose child the CAD files no longer reference – kept.
    pub kept_not_in_cad: Vec<String>,
    /// Referenced files TeamCraft does not have – never created.
    pub not_in_teamcraft: Vec<String>,
    /// Children refused because they would make the BOM circular.
    pub refused: Vec<Refused>,
}

impl CheckinBom {
    /// Nothing changed and nothing to report.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.updated.is_empty()
            && self.kept_not_in_cad.is_empty()
            && self.not_in_teamcraft.is_empty()
            && self.refused.is_empty()
    }

    /// One line for the check-in result: `BOM: 4 lines added, 1 quantity
    /// changed · not in TeamCraft: Lager.ipt · no longer in the CAD file:
    /// PRT-000012 (kept)`; `None` when no line was added, changed or refused
    /// (nothing changed: no line, even with lines kept or files missing).
    pub fn summary(&self) -> Option<String> {
        if self.added.is_empty() && self.updated.is_empty() && self.refused.is_empty() {
            return None;
        }
        fn plural(n: usize, one: &str, many: &str) -> String {
            format!("{n} {}", if n == 1 { one } else { many })
        }
        let mut changes = Vec::new();
        if !self.added.is_empty() {
            changes.push(format!(
                "{} added",
                plural(self.added.len(), "line", "lines")
            ));
        }
        if !self.updated.is_empty() {
            changes.push(format!(
                "{} changed",
                plural(self.updated.len(), "quantity", "quantities")
            ));
        }
        let mut parts = vec![if changes.is_empty() {
            "BOM unchanged".to_string()
        } else {
            format!("BOM: {}", changes.join(", "))
        }];
        if !self.refused.is_empty() {
            let names: Vec<&str> = self.refused.iter().map(|r| r.child.as_str()).collect();
            parts.push(format!("refused (circular): {}", names.join(", ")));
        }
        if !self.not_in_teamcraft.is_empty() {
            parts.push(format!(
                "not in TeamCraft: {}",
                self.not_in_teamcraft.join(", ")
            ));
        }
        if !self.kept_not_in_cad.is_empty() {
            parts.push(format!(
                "no longer in the CAD file: {} (kept)",
                self.kept_not_in_cad.join(", ")
            ));
        }
        Some(parts.join(" · "))
    }
}

/// The visible children of existing lines that are not among `children`
/// (item numbers, sorted).
async fn lines_not_in(
    conn: &mut PgConnection,
    actor: &Actor,
    parent_revision_id: Uuid,
    children: &[Uuid],
) -> AppResult<Vec<String>> {
    Ok(sqlx::query_scalar::<_, String>(&format!(
        "SELECT DISTINCT i.number
         FROM bom_lines bl
         JOIN items i ON i.id = bl.child_item_id
         WHERE bl.parent_revision_id = $1 AND NOT (bl.child_item_id = ANY($2))
           AND {}
         ORDER BY i.number",
        access::item_visible("i", 3)
    ))
    .bind(parent_revision_id)
    .bind(children)
    .bind(&actor.hidden_projects)
    .fetch_all(&mut *conn)
    .await?)
}

/// The BOM step of a check-in with *Update the BOM from the CAD file* (#407):
/// the plan of the version being checked in, applied inside the check-in's
/// transaction (`files::check_in_with`).
///
/// # Invariants
///
/// * **Permission:** that of the check-in – `parent` is the revision row the
///   caller locked, checked out by the actor and editable
///   (`revisions::ensure_editable`).
/// * **Counts:** the instance counts the reference scan stored with the
///   references of the new versions (no file is parsed under the lock).
/// * **Lines:** children with the status *new* are added; the quantity of a
///   differing line is changed only when the CAD file counts it (*CAD
///   instances*, never an *assumed 1*); a circular child is refused; no line
///   is ever removed – lines whose child is no longer referenced are reported
///   as `kept_not_in_cad` (a child whose components are only excluded from
///   the BOM in the CAD file is still referenced: kept, not reported),
///   referenced files TeamCraft does not have as `not_in_teamcraft`.
/// * **No structure:** a revision without a structure file (assembly, NX or
///   STEP assembly with counts) is left alone: `None`.
/// * **Audit:** `bom.add` / `bom.update` per line and one
///   `bom.from_references` with `"source": "checkin"`, only when a line was
///   added or changed.
/// * **Atomic:** runs in the caller's transaction.
pub(crate) async fn apply_on_checkin(
    tx: &mut PgConnection,
    actor: &Actor,
    parent: &Revision,
) -> AppResult<Option<CheckinBom>> {
    revisions::ensure_editable(parent, actor)?;
    let plan = plan_on(&mut *tx, actor, parent).await?;
    if !plan.structure {
        return Ok(None);
    }
    let chosen: Vec<&PlannedChild> = plan
        .children
        .iter()
        .filter(|c| c.status != ChildStatus::AlreadyInBom)
        .collect();
    let applied = apply_children(&mut *tx, actor, parent, &chosen, |c| {
        c.quantity_source == QuantitySource::CadInstances
    })
    .await?;
    // children still in the CAD file: counted ones and those only excluded
    // from its BOM there (not "no longer in the CAD file")
    let mut referenced: Vec<Uuid> = plan.children.iter().map(|c| c.item_id).collect();
    referenced.extend(plan.excluded_items.iter().copied());
    let kept_not_in_cad = lines_not_in(&mut *tx, actor, parent.id, &referenced).await?;
    let result = CheckinBom {
        added: applied.added,
        updated: applied.updated,
        kept_not_in_cad,
        not_in_teamcraft: plan.not_in_teamcraft.into_iter().map(|u| u.name).collect(),
        refused: applied.refused,
    };
    if !result.added.is_empty() || !result.updated.is_empty() {
        audit::record_related(
            &mut *tx,
            Some(actor),
            "bom.from_references",
            "revision",
            Some(parent.id),
            None,
            format!(
                "Updated the BOM of {} from the CAD file at check-in: {} added, {} updated",
                revisions::label(parent),
                result.added.len(),
                result.updated.len()
            ),
            json!({
                "item_id": parent.item_id,
                "source": "checkin",
                "added": result.added.len(),
                "updated": result.updated.len(),
                "kept_not_in_cad": result.kept_not_in_cad.len(),
                "not_in_teamcraft": result.not_in_teamcraft.len(),
                "refused": result.refused.len(),
            }),
        )
        .await?;
    }
    Ok(Some(result))
}

/// Number of matched, not yet used structure children (for the hint on the
/// revision page): children found in the CAD references with the status new.
pub fn new_children(plan: &Plan) -> usize {
    plan.children
        .iter()
        .filter(|c| c.status == ChildStatus::New)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::references::{FileMatch, ReferencedName};

    fn m(item: u128, number: &str, filename: &str, same: bool) -> FileMatch {
        FileMatch {
            key: filename.to_lowercase(),
            item_id: Uuid::from_u128(item),
            item_number: number.into(),
            item_name: format!("{number} name"),
            revision_id: Uuid::from_u128(item + 1000),
            revision_code: "A".into(),
            revision_state: "in_work".into(),
            file_id: Uuid::from_u128(item + 2000),
            filename: filename.into(),
            same_revision: same,
            export_name: false,
        }
    }

    fn file(name: &str, kind: FormatKind, refs: Vec<(&str, Vec<FileMatch>)>) -> FileWithReferences {
        FileWithReferences {
            file_id: Uuid::from_u128(9),
            filename: name.into(),
            format_kind: Some(kind),
            references: refs
                .into_iter()
                .map(|(n, matches)| ReferencedName {
                    name: n.into(),
                    instances: None,
                    matches,
                })
                .collect(),
        }
    }

    #[test]
    fn dedupes_skips_self_and_lists_unmatched() {
        let asm = file(
            "A.SLDASM",
            FormatKind::Assembly,
            vec![
                ("bolt.sldprt", vec![m(1, "PRT-1", "bolt.sldprt", false)]),
                // the same item through a second name
                ("bolt2.sldprt", vec![m(1, "PRT-1", "bolt2.sldprt", false)]),
                // only the revision's own file
                ("own.sldprt", vec![m(2, "ASM-1", "own.sldprt", true)]),
                ("ghost.sldprt", vec![]),
                ("Ghost.SLDPRT", vec![]),
            ],
        );
        let c = collect(&[SourceFile {
            file: &asm,
            format: "solidworks-assembly",
            counts: None,
        }]);
        assert_eq!(c.children.len(), 1);
        assert_eq!(c.children[0].item_number, "PRT-1");
        assert_eq!(c.children[0].quantity, 1.0);
        assert_eq!(c.children[0].source, QuantitySource::Assumed);
        assert_eq!(c.unmatched.len(), 1, "unmatched names once, any case");
        assert_eq!(c.unmatched[0].name, "ghost.sldprt");
        assert!(line_note(&c.children[0]).contains("check it"));
    }

    #[test]
    fn quantities_come_from_the_instance_counts() {
        let asm = file(
            "A.SLDASM",
            FormatKind::Assembly,
            vec![
                ("bolt.sldprt", vec![m(1, "PRT-1", "bolt.sldprt", false)]),
                ("glue.sldprt", vec![m(3, "PRT-3", "glue.sldprt", false)]),
                ("nut.sldprt", vec![m(4, "PRT-4", "nut.sldprt", false)]),
            ],
        );
        let counts: HashMap<String, u32> = [
            ("bolt.sldprt".to_string(), 4),
            ("glue.sldprt".to_string(), 0),
        ]
        .into();
        let c = collect(&[SourceFile {
            file: &asm,
            format: "solidworks-assembly",
            counts: Some(&counts),
        }]);
        let numbers: Vec<_> = c.children.iter().map(|c| c.item_number.as_str()).collect();
        assert_eq!(numbers, ["PRT-1", "PRT-4"]);
        assert_eq!(c.children[0].quantity, 4.0);
        assert_eq!(c.children[0].source, QuantitySource::CadInstances);
        assert_eq!(
            line_note(&c.children[0]),
            "From the CAD references (4 instances)"
        );
        assert_eq!(c.children[1].source, QuantitySource::Assumed);
        assert_eq!(c.excluded, ["glue.sldprt"]);
        // still in the assembly, only excluded from its BOM: never "no
        // longer in the CAD file" at check-in
        assert_eq!(c.excluded_items, [Uuid::from_u128(3)]);
    }

    #[test]
    fn excluded_components_of_the_own_revision_are_no_items() {
        let asm = file(
            "A.iam",
            FormatKind::Assembly,
            vec![
                ("own.ipt", vec![m(2, "ASM-1", "own.ipt", true)]),
                ("ghost.ipt", vec![]),
            ],
        );
        let counts: HashMap<String, u32> =
            [("own.ipt".to_string(), 0), ("ghost.ipt".to_string(), 0)].into();
        let c = collect(&[SourceFile {
            file: &asm,
            format: "inventor-assembly",
            counts: Some(&counts),
        }]);
        assert_eq!(c.excluded, ["own.ipt", "ghost.ipt"]);
        assert!(c.excluded_items.is_empty());
        assert!(c.children.is_empty() && c.unmatched.is_empty());
    }

    #[test]
    fn the_structure_rule_needs_counts_for_models_and_step() {
        assert!(structure_rule(
            Some(FormatKind::Assembly),
            "inventor-assembly",
            false
        ));
        assert!(!structure_rule(Some(FormatKind::Model), "nx-part", false));
        assert!(structure_rule(Some(FormatKind::Model), "nx-part", true));
        assert!(structure_rule(
            Some(FormatKind::Model),
            "solidcraft-design",
            true
        ));
        assert!(!structure_rule(Some(FormatKind::Model), "creo-part", true));
        assert!(!structure_rule(Some(FormatKind::Exchange), "step", false));
        assert!(structure_rule(Some(FormatKind::Exchange), "step", true));
        assert!(!structure_rule(
            Some(FormatKind::Drawing),
            "inventor-drawing",
            true
        ));
        assert!(!structure_rule(None, "", true));
    }

    #[test]
    fn drawings_and_parts_do_not_count_and_nx_needs_instances() {
        let refs = || vec![("bolt.prt", vec![m(1, "PRT-1", "bolt.prt", false)])];
        let drawing = file("D.SLDDRW", FormatKind::Drawing, refs());
        let part = file("P.SLDPRT", FormatKind::Part, refs());
        let nx = file("N.prt", FormatKind::Model, refs());
        let sources = |nx_counts| {
            vec![
                SourceFile {
                    file: &drawing,
                    format: "solidworks-drawing",
                    counts: None,
                },
                SourceFile {
                    file: &part,
                    format: "solidworks-part",
                    counts: None,
                },
                SourceFile {
                    file: &nx,
                    format: "nx-part",
                    counts: nx_counts,
                },
            ]
        };
        assert!(collect(&sources(None)).children.is_empty());
        let empty = HashMap::new();
        assert!(collect(&sources(Some(&empty))).children.is_empty());
        let counts: HashMap<String, u32> = [("bolt.prt".to_string(), 6)].into();
        let c = collect(&sources(Some(&counts)));
        assert_eq!(c.children.len(), 1);
        assert_eq!(c.children[0].quantity, 6.0);
    }

    #[test]
    fn step_files_count_only_with_external_occurrences() {
        let step = file(
            "pump.stp",
            FormatKind::Exchange,
            vec![("bolt.stp", vec![m(1, "PRT-1", "bolt.stp", false)])],
        );
        let source = |counts| SourceFile {
            file: &step,
            format: "step",
            counts,
        };
        // a STEP file that only names other files is no structure
        assert!(collect(&[source(None)]).children.is_empty());
        let counts: HashMap<String, u32> = [("bolt.stp".to_string(), 4)].into();
        let c = collect(&[source(Some(&counts))]);
        assert_eq!(c.children.len(), 1);
        assert_eq!(c.children[0].quantity, 4.0);
        assert_eq!(c.children[0].source, QuantitySource::CadInstances);
    }

    #[test]
    fn solidcraft_designs_count_only_with_placements() {
        let design = file(
            "frame.sc",
            FormatKind::Model,
            vec![("wheel.sc", vec![m(1, "PRT-1", "wheel.sc", false)])],
        );
        let source = |counts| SourceFile {
            file: &design,
            format: "solidcraft-design",
            counts,
        };
        assert!(collect(&[source(None)]).children.is_empty());
        let counts: HashMap<String, u32> = [("wheel.sc".to_string(), 2)].into();
        let c = collect(&[source(Some(&counts))]);
        assert_eq!(c.children.len(), 1);
        assert_eq!(c.children[0].quantity, 2.0);
    }

    #[test]
    fn stored_counts_come_from_the_references() {
        let mut asm = file(
            "A.iam",
            FormatKind::Assembly,
            vec![
                ("Bolt.ipt", vec![m(1, "PRT-1", "Bolt.ipt", false)]),
                ("glue.ipt", vec![]),
            ],
        );
        assert_eq!(stored_counts(&asm), None, "no counts stored: none known");
        asm.references[0].instances = Some(4);
        asm.references[1].instances = Some(0);
        let counts = stored_counts(&asm).unwrap();
        assert_eq!(counts.get("bolt.ipt"), Some(&4));
        assert_eq!(counts.get("glue.ipt"), Some(&0));
    }

    #[test]
    fn the_checkin_summary_says_what_changed() {
        assert_eq!(CheckinBom::default().summary(), None);
        let bom = CheckinBom {
            added: vec![
                "PRT-1".into(),
                "PRT-2".into(),
                "PRT-3".into(),
                "PRT-4".into(),
            ],
            updated: vec!["PRT-5".into()],
            kept_not_in_cad: vec!["PRT-000012".into()],
            not_in_teamcraft: vec!["Lager.ipt".into()],
            refused: Vec::new(),
        };
        assert_eq!(
            bom.summary().unwrap(),
            "BOM: 4 lines added, 1 quantity changed · not in TeamCraft: Lager.ipt · \
             no longer in the CAD file: PRT-000012 (kept)"
        );
        // nothing changed: no line, even with something to report
        let only_report = CheckinBom {
            not_in_teamcraft: vec!["Lager.ipt".into()],
            kept_not_in_cad: vec!["PRT-000012".into()],
            ..CheckinBom::default()
        };
        assert_eq!(only_report.summary(), None);
        let refused = CheckinBom {
            refused: vec![Refused {
                child: "ASM-1".into(),
                reason: "circular".into(),
            }],
            ..CheckinBom::default()
        };
        assert_eq!(
            refused.summary().unwrap(),
            "BOM unchanged · refused (circular): ASM-1"
        );
    }

    #[test]
    fn status_compares_with_the_bom() {
        assert_eq!(status_of(None, 2.0, false), ChildStatus::New);
        assert_eq!(status_of(None, 2.0, true), ChildStatus::Circular);
        assert_eq!(status_of(Some(2.0), 2.0, false), ChildStatus::AlreadyInBom);
        assert_eq!(status_of(Some(2.0), 4.0, false), ChildStatus::Differs);
        // an existing line is never "circular"
        assert_eq!(status_of(Some(2.0), 2.0, true), ChildStatus::AlreadyInBom);
    }
}
