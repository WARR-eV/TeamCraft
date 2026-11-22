//! Who holds which lock in a project group (#428): the check-out overview of
//! administrators and the group's managers – the locks grouped per holder,
//! and the graph of holders and checked-out revisions that the web page
//! (`projects/checkouts.html`) draws. The desktop app draws the same graph
//! from the REST answer (`desktop/ui/js/lock-layout.js`, the same rules).
//! Read-only: releasing a lock stays on the revision page.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::Actor,
    error::{AppError, AppResult},
    services::{access, projects},
};

/// One checked-out revision of the group.
#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Lock {
    pub revision_id: Uuid,
    pub item_number: String,
    pub code: String,
    pub name: String,
    pub item_type: String,
    pub holder_id: Uuid,
    pub holder_name: String,
    pub holder_active: bool,
    pub checked_out_at: DateTime<Utc>,
}

/// A person holding locks in the group, with their locks (oldest first).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Holder {
    pub id: Uuid,
    pub name: String,
    pub active: bool,
    pub locks: Vec<Lock>,
}

/// The overview of one project group.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Overview {
    pub project_id: Uuid,
    pub project_code: String,
    pub project_name: String,
    /// Number of checked-out revisions.
    pub total: usize,
    /// Holders by name.
    pub holders: Vec<Holder>,
}

/// The overview, for administrators and the group's managers. A group the
/// actor cannot see answers like a missing one – also for an administrator
/// whose need-to-know policy hides it: the overview shows item numbers and
/// names, the group's content.
pub async fn for_project(pool: &PgPool, actor: &Actor, project_id: Uuid) -> AppResult<Overview> {
    access::ensure_visible(actor, Some(project_id), access::not_found_project())?;
    let mut conn = pool.acquire().await?;
    if !projects::can_manage(&mut conn, actor, project_id).await? {
        return Err(AppError::Forbidden(
            "Only the managers of this project group and administrators see its check-out overview."
                .into(),
        ));
    }
    let project = projects::get_for_management(pool, actor, project_id).await?;
    let locks = sqlx::query_as::<_, Lock>(
        "SELECT r.id AS revision_id, i.number AS item_number, r.code, r.name, i.item_type,
                u.id AS holder_id, u.display_name AS holder_name, u.active AS holder_active,
                COALESCE(r.checked_out_at, r.updated_at) AS checked_out_at
         FROM revisions r
         JOIN items i ON i.id = r.item_id
         JOIN users u ON u.id = r.checked_out_by
         WHERE i.project_id = $1 AND r.checked_out_by IS NOT NULL
         ORDER BY lower(u.display_name), u.id, COALESCE(r.checked_out_at, r.updated_at), i.number",
    )
    .bind(project_id)
    .fetch_all(&mut *conn)
    .await?;
    let total = locks.len();
    Ok(Overview {
        project_id: project.id,
        project_code: project.code,
        project_name: project.name,
        total,
        holders: group(locks),
    })
}

/// Locks sorted by holder → one [`Holder`] per person, in that order.
fn group(locks: Vec<Lock>) -> Vec<Holder> {
    let mut holders: Vec<Holder> = Vec::new();
    for lock in locks {
        match holders.last_mut() {
            Some(h) if h.id == lock.holder_id => h.locks.push(lock),
            _ => holders.push(Holder {
                id: lock.holder_id,
                name: lock.holder_name.clone(),
                active: lock.holder_active,
                locks: vec![lock],
            }),
        }
    }
    holders
}

// ------------------------------------------------------------------ graph

/// Height of one row (one checked-out revision) in the graph.
pub const ROW: u32 = 34;
/// Space above the first and below the last row.
pub const PAD: u32 = 12;
/// Height of a node box.
pub const NODE_H: u32 = 26;
/// Left edge and width of the holder column.
pub const HOLDER_X: u32 = 8;
pub const HOLDER_W: u32 = 190;
/// Left edge and width of the revision column.
pub const LOCK_X: u32 = 300;
pub const LOCK_W: u32 = 330;
/// Longest label in characters; longer ones end in "…".
pub const LABEL_MAX: usize = 40;

/// How long a lock has been held, as the class of its node and line.
pub fn age_class(checked_out_at: DateTime<Utc>, now: DateTime<Utc>) -> &'static str {
    let hours = (now - checked_out_at).num_hours();
    if hours < 24 {
        "today"
    } else if hours < 24 * 7 {
        "week"
    } else {
        "old"
    }
}

/// A label cut to [`LABEL_MAX`] characters.
pub fn label(text: &str) -> String {
    if text.chars().count() <= LABEL_MAX {
        text.to_string()
    } else {
        let cut: String = text.chars().take(LABEL_MAX - 1).collect();
        format!("{}…", cut.trim_end())
    }
}

/// A holder node: vertically centred on its revisions.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct HolderNode {
    pub y: u32,
    pub label: String,
    pub count: usize,
    pub active: bool,
}

/// A revision node, one row each, with the line to its holder.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LockNode {
    pub y: u32,
    pub revision_id: Uuid,
    pub label: String,
    pub sub: String,
    pub age: &'static str,
    /// SVG path of the line from the holder to this revision.
    pub edge: String,
}

/// The graph: holders on the left, revisions on the right, in the order of
/// the overview, so a holder's revisions sit next to each other and the
/// lines never cross.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Graph {
    pub width: u32,
    pub height: u32,
    pub holder_x: u32,
    pub holder_w: u32,
    pub lock_x: u32,
    pub lock_w: u32,
    pub node_h: u32,
    pub holders: Vec<HolderNode>,
    pub locks: Vec<LockNode>,
}

/// Lays the overview out as a [`Graph`]; `now` decides the age classes.
pub fn graph(overview: &Overview, now: DateTime<Utc>) -> Graph {
    let mut holders = Vec::with_capacity(overview.holders.len());
    let mut locks = Vec::with_capacity(overview.total);
    let mut row = 0u32;
    for h in &overview.holders {
        let first = PAD + row * ROW;
        let last = PAD + (row + h.locks.len().saturating_sub(1) as u32) * ROW;
        let hy = (first + last) / 2;
        for l in &h.locks {
            let y = PAD + row * ROW;
            let (y1, y2) = (hy + NODE_H / 2, y + NODE_H / 2);
            let (x1, x2) = (HOLDER_X + HOLDER_W, LOCK_X);
            let mid = (x1 + x2) / 2;
            locks.push(LockNode {
                y,
                revision_id: l.revision_id,
                label: label(&format!("{}/{} {}", l.item_number, l.code, l.name)),
                sub: l.item_type.clone(),
                age: age_class(l.checked_out_at, now),
                edge: format!("M{x1},{y1} C{mid},{y1} {mid},{y2} {x2},{y2}"),
            });
            row += 1;
        }
        holders.push(HolderNode {
            y: hy,
            label: label(&h.name),
            count: h.locks.len(),
            active: h.active,
        });
    }
    Graph {
        width: LOCK_X + LOCK_W + HOLDER_X,
        height: if row == 0 {
            0
        } else {
            2 * PAD + (row - 1) * ROW + NODE_H
        },
        holder_x: HOLDER_X,
        holder_w: HOLDER_W,
        lock_x: LOCK_X,
        lock_w: LOCK_W,
        node_h: NODE_H,
        holders,
        locks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn lock(holder: u128, name: &str, number: &str, hours_ago: i64, now: DateTime<Utc>) -> Lock {
        Lock {
            revision_id: Uuid::new_v4(),
            item_number: number.into(),
            code: "A".into(),
            name: "Bracket".into(),
            item_type: "part".into(),
            holder_id: Uuid::from_u128(holder),
            holder_name: name.into(),
            holder_active: holder != 3,
            checked_out_at: now - Duration::hours(hours_ago),
        }
    }

    fn overview(locks: Vec<Lock>) -> Overview {
        Overview {
            project_id: Uuid::nil(),
            project_code: "ROVER".into(),
            project_name: "Rover".into(),
            total: locks.len(),
            holders: group(locks),
        }
    }

    #[test]
    fn locks_are_grouped_per_holder_in_order() {
        let now = Utc::now();
        let o = overview(vec![
            lock(1, "Erika", "PRT-1", 1, now),
            lock(1, "Erika", "PRT-2", 2, now),
            lock(2, "Leo", "PRT-3", 3, now),
        ]);
        assert_eq!(o.holders.len(), 2);
        assert_eq!(o.holders[0].name, "Erika");
        assert_eq!(o.holders[0].locks.len(), 2);
        assert_eq!(o.holders[1].locks[0].item_number, "PRT-3");
    }

    #[test]
    fn the_graph_centres_each_holder_on_its_revisions() {
        let now = Utc::now();
        let o = overview(vec![
            lock(1, "Erika", "PRT-1", 1, now),
            lock(1, "Erika", "PRT-2", 30, now),
            lock(1, "Erika", "PRT-3", 24 * 9, now),
            lock(3, "Gone", "PRT-4", 2, now),
        ]);
        let g = graph(&o, now);
        assert_eq!(g.locks.len(), 4);
        let ys: Vec<u32> = g.locks.iter().map(|l| l.y).collect();
        assert_eq!(ys, vec![PAD, PAD + ROW, PAD + 2 * ROW, PAD + 3 * ROW]);
        // Erika in the middle of her three rows, the second holder on its one
        assert_eq!(g.holders[0].y, PAD + ROW);
        assert_eq!(g.holders[0].count, 3);
        assert_eq!(g.holders[1].y, PAD + 3 * ROW);
        assert!(!g.holders[1].active, "deactivated holder");
        // ages: today, this week, older
        let ages: Vec<&str> = g.locks.iter().map(|l| l.age).collect();
        assert_eq!(ages, vec!["today", "week", "old", "today"]);
        // every line runs from the holder column to the revision column
        let x1 = HOLDER_X + HOLDER_W;
        assert!(g.locks[0].edge.starts_with(&format!("M{x1},")));
        assert!(g.locks[0]
            .edge
            .ends_with(&format!(" {LOCK_X},{}", PAD + NODE_H / 2)));
        assert_eq!(g.height, 2 * PAD + 3 * ROW + NODE_H);
        assert_eq!(g.width, LOCK_X + LOCK_W + HOLDER_X);
    }

    #[test]
    fn an_empty_group_has_an_empty_graph_and_long_labels_are_cut() {
        let g = graph(&overview(vec![]), Utc::now());
        assert_eq!((g.height, g.holders.len(), g.locks.len()), (0, 0, 0));
        let long = "x".repeat(60);
        let cut = label(&long);
        assert_eq!(cut.chars().count(), LABEL_MAX);
        assert!(cut.ends_with('…'));
        assert_eq!(label("Bracket"), "Bracket");
    }
}
