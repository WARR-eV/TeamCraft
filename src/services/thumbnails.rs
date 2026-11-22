//! The main thumbnail of a revision (#412, #420): the one picture that
//! stands for a revision on its page, in the item list, the search results,
//! the vault explorer and the desktop views.
//!
//! The rule is pure ([`pick_main`]); [`main_for_revisions`] applies it to a
//! whole page of revisions in one query, so a list never asks per row.

use std::collections::HashMap;

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{cadformat::FormatKind, error::AppResult, models::FileRecord};

/// The main thumbnail of a revision: its file and the content hash of the
/// file's current version (the cache key, `?v=` of the image URL).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MainThumb {
    pub file_id: Uuid,
    pub sha256: String,
}

/// Rank of a stored format for the main thumbnail: the assembly before the
/// part before a model file before the drawing – the same order as the
/// desktop app's main CAD file, then a visualisation file (JT). `None` for
/// formats without a preview image (stored in the file or rendered from its 3D preview).
pub fn rank(format: &str) -> Option<u8> {
    let info = crate::cadformat::info(format).filter(|i| i.thumbnail)?;
    match info.kind {
        FormatKind::Assembly => Some(0),
        FormatKind::Part => Some(1),
        FormatKind::Model => Some(2),
        FormatKind::Drawing => Some(3),
        // a stand-alone JT file: after the native files it was exported from
        FormatKind::Visualization => Some(4),
        _ => None,
    }
}

/// The file whose picture stands for a revision: the best [`rank`], the
/// first by name (case-insensitive) on a tie. `key` gives a file's stored
/// format id and its name.
pub fn pick_main<T>(files: &[T], key: impl Fn(&T) -> (&str, &str)) -> Option<&T> {
    files
        .iter()
        .filter_map(|f| {
            let (format, filename) = key(f);
            Some((rank(format)?, filename.to_lowercase(), f))
        })
        .min_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)))
        .map(|(_, _, f)| f)
}

/// [`pick_main`] over the files of one revision.
pub fn pick_main_file(files: &[FileRecord]) -> Option<&FileRecord> {
    pick_main(files, |f| (f.format.as_str(), f.filename.as_str()))
}

/// One file row of [`main_for_revisions`].
#[derive(sqlx::FromRow)]
struct FileRow {
    revision_id: Uuid,
    id: Uuid,
    filename: String,
    format: String,
    sha256: String,
}

/// The main thumbnail of each of `revision_ids` that has one, in one query
/// (whatever the number of revisions). The caller has already checked that
/// the actor may read these revisions (they come from a permission-checked
/// listing).
pub async fn main_for_revisions(
    pool: &PgPool,
    revision_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, MainThumb>> {
    if revision_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, FileRow>(
        "SELECT revision_id, id, filename, COALESCE(format, '') AS format, sha256
         FROM files WHERE revision_id = ANY($1)",
    )
    .bind(revision_ids)
    .fetch_all(pool)
    .await?;
    let mut by_revision: HashMap<Uuid, Vec<FileRow>> = HashMap::new();
    for row in rows {
        by_revision.entry(row.revision_id).or_default().push(row);
    }
    Ok(by_revision
        .into_iter()
        .filter_map(|(revision_id, files)| {
            let main = pick_main(&files, |f| (f.format.as_str(), f.filename.as_str()))?;
            Some((
                revision_id,
                MainThumb {
                    file_id: main.id,
                    sha256: main.sha256.clone(),
                },
            ))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick<'a>(files: &'a [(&'a str, &'a str)]) -> Option<&'a str> {
        pick_main(files, |f| (f.1, f.0)).map(|f| f.0)
    }

    #[test]
    fn formats_without_a_picture_have_no_rank() {
        assert_eq!(rank(""), None);
        assert_eq!(rank("step"), None);
        assert_eq!(rank("no-such-format"), None);
        assert_eq!(rank("inventor-assembly"), Some(0));
        assert_eq!(rank("inventor-part"), Some(1));
        assert_eq!(rank("inventor-drawing"), Some(3));
        assert_eq!(rank("jt"), Some(4));
    }

    #[test]
    fn the_main_thumbnail_is_the_assembly_before_the_part_before_the_drawing() {
        assert_eq!(pick(&[]), None);
        assert_eq!(pick(&[("bracket.stp", "step"), ("notes.txt", "")]), None);
        let files = [
            ("Bracket.idw", "inventor-drawing"),
            ("bracket.stp", "step"),
            ("Bracket.ipt", "inventor-part"),
        ];
        assert_eq!(pick(&files), Some("Bracket.ipt"));
        let with_assembly = [
            ("Bracket.idw", "inventor-drawing"),
            ("Frame.iam", "inventor-assembly"),
            ("Bracket.ipt", "inventor-part"),
        ];
        assert_eq!(pick(&with_assembly), Some("Frame.iam"));
        // a tie goes to the first by name, whatever the case
        assert_eq!(
            pick(&[("b.ipt", "inventor-part"), ("A.ipt", "inventor-part")]),
            Some("A.ipt")
        );
        // only the drawing has a picture: it stands for the revision
        assert_eq!(
            pick(&[("Bracket.idw", "inventor-drawing"), ("bracket.stp", "step")]),
            Some("Bracket.idw")
        );
    }
}
