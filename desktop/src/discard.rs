//! *Discard my changes* (#559): puts the version a working-folder file was
//! copied from (its *base*, `.teamcraft/state.json`) back in place of a local
//! change – typically a file a CAD program saved although it was not checked
//! out (Inventor's *Save All* replaces read-only files instead of writing into
//! them, so the read-only flag does not stop it).
//!
//! Per file, the cheapest safe source:
//!
//! 1. **The CAD program's backup folder.** Inventor moves the replaced file to
//!    `OldVersions/<stem>.<nnnn><ext>` next to it. A copy there whose SHA-256
//!    equals the base is the server version byte for byte – it is used and
//!    nothing is downloaded. Copies with another hash are never used; the
//!    newest matching one wins.
//! 2. **Otherwise the base version from the server** (`GET /files/{id}/versions`,
//!    then that version's content, checked by SHA-256) – the base, not the
//!    latest version, so a discard never updates a file as a side effect.
//!
//! Nothing is lost: the replacement is prepared next to the file first, then
//! the user's copy moves to `.teamcraft/recovered/<date>/<path>` and the
//! replacement takes its place. The restored file is read-only again unless
//! the user holds the check-out. Files that are not tracked, or that already
//! match their base, are reported and left alone.

use std::{
    future::Future,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use serde::Serialize;
use serde_json::Value;

use crate::{
    api::{Api, ApiError},
    layout::{self, Entry, State},
    workspace,
};

/// Folder names of CAD backup copies next to a file (compared ignoring case).
pub const BACKUP_DIRS: [&str; 1] = ["OldVersions"];
/// Where the user's copies go: `<workspace>/.teamcraft/recovered/<date>/…`.
pub const RECOVERED_DIR: &str = "recovered";

/// Where a discarded file's content came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A byte-identical copy in the CAD program's backup folder.
    Backup,
    /// Downloaded from the server.
    Server,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Restored {
    pub path: String,
    pub source: Source,
    /// Where the user's copy went (relative to the workspace).
    pub recovered: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Skipped {
    pub path: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Discarded {
    pub restored: Vec<Restored>,
    /// Already the server version: only the read-only flag was set again.
    pub unchanged: Vec<String>,
    pub skipped: Vec<Skipped>,
    /// `.teamcraft/recovered/<date>` when anything was moved there.
    pub recovered_dir: Option<String>,
    /// The sentence for the user ([`summary`]).
    pub summary: String,
}

/// The backup copies of `name` in the backup folders of `dir`, newest first:
/// `<stem>.<digits><ext>` (Inventor's `OldVersions/Bolt.0012.ipt`).
pub fn backup_candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    let mut found: Vec<(u64, PathBuf)> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    for sub in entries.flatten() {
        let sub_name = sub.file_name().to_string_lossy().to_string();
        if !BACKUP_DIRS
            .iter()
            .any(|d| d.eq_ignore_ascii_case(&sub_name))
            || !sub.path().is_dir()
        {
            continue;
        }
        let Ok(files) = std::fs::read_dir(sub.path()) else {
            continue;
        };
        for f in files.flatten() {
            let n = f.file_name().to_string_lossy().to_string();
            if !is_backup_name(&n, stem, ext) {
                continue;
            }
            let Ok(meta) = f.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            found.push((modified, f.path()));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    found.into_iter().map(|(_, p)| p).collect()
}

/// `<stem>.<digits><ext>`, stem and extension compared ignoring case.
fn is_backup_name(name: &str, stem: &str, ext: &str) -> bool {
    let lower = name.to_lowercase();
    let (stem, ext) = (stem.to_lowercase(), ext.to_lowercase());
    let Some(rest) = lower.strip_prefix(&stem) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix('.') else {
        return false;
    };
    let Some(digits) = rest.strip_suffix(&ext) else {
        return false;
    };
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// The free place for the user's copy of `rel` below `dir_rel`
/// (`.teamcraft/recovered/<date>`): `<rel>`, else `<stem> (2)<ext>`, …
pub fn recovered_rel(dir_rel: &str, rel: &str, taken: impl Fn(&str) -> bool) -> String {
    let first = format!("{dir_rel}/{rel}");
    if !taken(&first) {
        return first;
    }
    let (base, ext) = match rel.rfind('.') {
        Some(i) if i > rel.rfind('/').map_or(0, |s| s + 1) => (&rel[..i], &rel[i..]),
        _ => (rel, ""),
    };
    (2..)
        .map(|n| format!("{dir_rel}/{base} ({n}){ext}"))
        .find(|c| !taken(c))
        .unwrap_or(first)
}

/// The sentence the app shows after a discard.
pub fn summary(d: &Discarded) -> String {
    let backup = d
        .restored
        .iter()
        .filter(|r| r.source == Source::Backup)
        .count();
    let server = d.restored.len() - backup;
    let mut parts = Vec::new();
    if !d.restored.is_empty() {
        let mut s = format!(
            "{} file{} put back to the server version",
            d.restored.len(),
            if d.restored.len() == 1 { "" } else { "s" }
        );
        let mut from = Vec::new();
        if backup > 0 {
            from.push(format!("{backup} from the CAD program's backup folder"));
        }
        if server > 0 {
            from.push(format!("{server} downloaded"));
        }
        s.push_str(&format!(" ({})", from.join(", ")));
        parts.push(s);
    }
    if !d.unchanged.is_empty() {
        parts.push(format!(
            "{} already matched the server version",
            d.unchanged.len()
        ));
    }
    if !d.skipped.is_empty() {
        parts.push(format!("{} not changed", d.skipped.len()));
    }
    let mut out = parts.join("; ");
    if let Some(dir) = &d.recovered_dir {
        out.push_str(&format!(". Your copies are kept in {dir}"));
    }
    out
}

/// Discards the local changes of `rels` (workspace-relative paths of tracked
/// files). `fetch_base(entry, dest)` writes the base version of a file to
/// `dest` (checked by SHA-256) when no backup copy matches.
pub async fn discard_with<F, Fut>(
    root: &Path,
    rels: &[String],
    mut fetch_base: F,
) -> Result<Discarded, ApiError>
where
    F: FnMut(Entry, PathBuf) -> Fut,
    Fut: Future<Output = Result<(), ApiError>>,
{
    let mut state = State::load(root);
    let mut out = Discarded::default();
    let date: String = workspace::now_utc().chars().take(10).collect();
    let dir_rel = format!("{}/{RECOVERED_DIR}/{date}", layout::STATE_DIR);
    let skip = |out: &mut Discarded, path: &str, reason: &str| {
        out.skipped.push(Skipped {
            path: path.to_string(),
            reason: reason.to_string(),
        })
    };
    for rel in rels {
        let Some(entry) = state.get(rel).cloned() else {
            skip(&mut out, rel, "not a file from the server");
            continue;
        };
        let path = layout::join_rel(root, &entry.path);
        let read_only = layout::read_only_wanted(entry.checked_out);
        if entry.sha256.is_empty() {
            skip(
                &mut out,
                &entry.path,
                "the version it came from is not known – use Get latest",
            );
            continue;
        }
        if path.is_file() && workspace::sha256_of(&path).await? == entry.sha256 {
            let _ = workspace::set_read_only(&path, read_only);
            out.unchanged.push(entry.path.clone());
            continue;
        }
        let Some(dir) = path.parent() else {
            skip(&mut out, &entry.path, "no folder");
            continue;
        };
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        // 1. the replacement, next to the file (never in place of it yet)
        let staged = dir.join(format!(".{file_name}.teamcraft-discard"));
        let _ = tokio::fs::remove_file(&staged).await;
        let mut source = None;
        for candidate in backup_candidates(dir, &file_name) {
            if workspace::sha256_of(&candidate).await.ok().as_deref() == Some(entry.sha256.as_str())
            {
                tokio::fs::copy(&candidate, &staged).await?;
                source = Some(Source::Backup);
                break;
            }
        }
        if source.is_none() {
            match fetch_base(entry.clone(), staged.clone()).await {
                Ok(()) => source = Some(Source::Server),
                Err(e) => {
                    let _ = tokio::fs::remove_file(&staged).await;
                    skip(
                        &mut out,
                        &entry.path,
                        &format!("the server version could not be fetched: {e}"),
                    );
                    continue;
                }
            }
        }
        if workspace::sha256_of(&staged).await? != entry.sha256 {
            let _ = tokio::fs::remove_file(&staged).await;
            skip(
                &mut out,
                &entry.path,
                "the copy did not match the server version",
            );
            continue;
        }
        // 2. the user's copy to recovered/, 3. the replacement into its place
        let mut recovered = String::new();
        if path.exists() {
            recovered = recovered_rel(&dir_rel, &entry.path, |c| {
                layout::join_rel(root, c).exists()
            });
            let target = layout::join_rel(root, &recovered);
            if let Some(parent) = target.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            if let Err(e) = tokio::fs::rename(&path, &target).await {
                let _ = tokio::fs::remove_file(&staged).await;
                skip(
                    &mut out,
                    &entry.path,
                    &format!("your copy could not be moved aside: {e}"),
                );
                continue;
            }
        }
        if let Err(e) = tokio::fs::rename(&staged, &path).await {
            // put the user's copy back
            if !recovered.is_empty() {
                let _ = tokio::fs::rename(layout::join_rel(root, &recovered), &path).await;
            }
            let _ = tokio::fs::remove_file(&staged).await;
            skip(
                &mut out,
                &entry.path,
                &format!("the server version could not be put in place: {e}"),
            );
            continue;
        }
        let _ = workspace::set_read_only(&path, read_only);
        let (size, modified) = std::fs::metadata(&path)
            .map(|m| {
                let t = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                (m.len(), t)
            })
            .unwrap_or((entry.size, 0));
        let updated = Entry {
            size,
            modified_ms: modified,
            ..entry.clone()
        };
        state.insert(updated.clone());
        State::store(root, [updated])
            .map_err(|e| ApiError::local(format!("Workspace state: {e}")))?;
        if !recovered.is_empty() {
            out.recovered_dir = Some(dir_rel.clone());
        }
        out.restored.push(Restored {
            path: entry.path.clone(),
            source: source.unwrap_or(Source::Server),
            recovered,
        });
    }
    out.summary = summary(&out);
    Ok(out)
}

/// [`discard_with`] against the server: the base version's content by its id.
pub async fn discard(api: &Api, root: &Path, rels: &[String]) -> Result<Discarded, ApiError> {
    discard_with(root, rels, |entry, dest| async move {
        let versions = api
            .json(
                reqwest::Method::GET,
                &format!("/files/{}/versions", workspace::enc(&entry.file_id)),
                None,
            )
            .await?;
        let id = versions
            .as_array()
            .and_then(|list| {
                list.iter()
                    .find(|v| v.get("version").and_then(Value::as_i64) == Some(entry.version))
            })
            .and_then(|v| v.get("id").and_then(Value::as_str))
            .map(str::to_string)
            .ok_or_else(|| {
                ApiError::local(format!(
                    "version {} is no longer on the server",
                    entry.version
                ))
            })?;
        let cancel = std::sync::atomic::AtomicBool::new(false);
        api.download_verified(
            &format!("/file-versions/{id}/content"),
            &dest,
            &entry.sha256,
            &cancel,
            &mut |_| {},
        )
        .await
        .map(|_| ())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn sha(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn entry(rel: &str, content: &[u8], checked_out: bool) -> Entry {
        Entry {
            path: rel.to_string(),
            project: Some("ROVER".into()),
            item: "PRT-1".into(),
            revision: "A".into(),
            file_id: "f1".into(),
            filename: rel.rsplit('/').next().unwrap().to_string(),
            version: 3,
            sha256: sha(content),
            checked_out,
            size: content.len() as u64,
            modified_ms: 0,
        }
    }

    fn setup(
        rel: &str,
        base: &[u8],
        local: &[u8],
        checked_out: bool,
    ) -> (tempfile::TempDir, PathBuf) {
        let ws = tempfile::tempdir().unwrap();
        let path = layout::join_rel(ws.path(), rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, local).unwrap();
        State::store(ws.path(), [entry(rel, base, checked_out)]).unwrap();
        (ws, path)
    }

    fn no_server(_: Entry, _: PathBuf) -> std::future::Ready<Result<(), ApiError>> {
        std::future::ready(Err(ApiError::local("offline".to_string())))
    }

    #[test]
    fn backup_names_are_stem_dot_digits_ext() {
        assert!(is_backup_name("Bolzen.0029.ipt", "Bolzen", ".ipt"));
        assert!(is_backup_name("bolzen.12.IPT", "Bolzen", ".ipt"));
        assert!(!is_backup_name("Bolzen.ipt", "Bolzen", ".ipt"));
        assert!(!is_backup_name("Bolzen2.0001.ipt", "Bolzen", ".ipt"));
        assert!(!is_backup_name("Bolzen.0001.iam", "Bolzen", ".ipt"));
        assert!(!is_backup_name("Bolzen.v1.ipt", "Bolzen", ".ipt"));
    }

    #[test]
    fn recovered_names_never_overwrite() {
        let dir = ".teamcraft/recovered/2026-11-28";
        assert_eq!(
            recovered_rel(dir, "ROVER/a/b.ipt", |_| false),
            format!("{dir}/ROVER/a/b.ipt")
        );
        let taken = |c: &str| c.ends_with("/b.ipt") || c.ends_with("/b (2).ipt");
        assert_eq!(
            recovered_rel(dir, "ROVER/a/b.ipt", taken),
            format!("{dir}/ROVER/a/b (3).ipt")
        );
        assert_eq!(
            recovered_rel(dir, "ROVER/a.d/README", |c| c.ends_with("README")),
            format!("{dir}/ROVER/a.d/README (2)")
        );
    }

    #[tokio::test]
    async fn a_matching_backup_copy_is_used_and_the_users_copy_kept() {
        let rel = "ROVER/parts/Bolzen.ipt";
        let (ws, path) = setup(rel, b"server v3", b"saved by Save All", false);
        let old = path.parent().unwrap().join("OldVersions");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("Bolzen.0007.ipt"), b"something else").unwrap();
        std::fs::write(old.join("Bolzen.0029.ipt"), b"server v3").unwrap();
        let d = discard_with(ws.path(), &[rel.to_string()], no_server)
            .await
            .unwrap();
        assert_eq!(d.restored.len(), 1, "{d:?}");
        assert_eq!(d.restored[0].source, Source::Backup);
        assert_eq!(std::fs::read(&path).unwrap(), b"server v3");
        assert!(
            workspace::is_read_only(&path),
            "not checked out: read-only again"
        );
        let kept = layout::join_rel(ws.path(), &d.restored[0].recovered);
        assert_eq!(std::fs::read(kept).unwrap(), b"saved by Save All");
        assert!(d
            .recovered_dir
            .as_deref()
            .unwrap()
            .starts_with(".teamcraft/recovered/"));
        // the backup copy stays where it was
        assert_eq!(
            std::fs::read(old.join("Bolzen.0029.ipt")).unwrap(),
            b"server v3"
        );
        assert!(
            d.summary.contains(
                "1 file put back to the server version (1 from the CAD program's backup folder)"
            ),
            "{}",
            d.summary
        );
        assert!(d
            .summary
            .contains("Your copies are kept in .teamcraft/recovered/"));
    }

    #[tokio::test]
    async fn without_a_matching_backup_the_base_version_is_fetched() {
        let rel = "ROVER/parts/Plate.ipt";
        let (ws, path) = setup(rel, b"base", b"local edit", true);
        let mut asked = Vec::new();
        let d = discard_with(ws.path(), &[rel.to_string()], |e, dest| {
            asked.push(e.version);
            std::fs::write(&dest, b"base").unwrap();
            std::future::ready(Ok(()))
        })
        .await
        .unwrap();
        assert_eq!(asked, vec![3], "the base version, not the latest");
        assert_eq!(d.restored[0].source, Source::Server);
        assert_eq!(std::fs::read(&path).unwrap(), b"base");
        assert!(
            !workspace::is_read_only(&path),
            "my check-out: stays writable"
        );
    }

    #[tokio::test]
    async fn a_failed_or_wrong_download_leaves_the_users_file_alone() {
        let rel = "ROVER/parts/Shaft.ipt";
        let (ws, path) = setup(rel, b"base", b"local edit", false);
        let d = discard_with(ws.path(), &[rel.to_string()], no_server)
            .await
            .unwrap();
        assert!(d.restored.is_empty());
        assert!(d.skipped[0].reason.contains("could not be fetched"));
        assert_eq!(std::fs::read(&path).unwrap(), b"local edit");
        let d = discard_with(ws.path(), &[rel.to_string()], |_, dest| {
            std::fs::write(&dest, b"tampered").unwrap();
            std::future::ready(Ok(()))
        })
        .await
        .unwrap();
        assert!(d.skipped[0].reason.contains("did not match"));
        assert_eq!(std::fs::read(&path).unwrap(), b"local edit");
        assert!(d.recovered_dir.is_none());
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(leftovers.len(), 1, "no staged file left behind");
    }

    #[tokio::test]
    async fn unchanged_and_untracked_files_are_reported() {
        let rel = "ROVER/parts/Same.ipt";
        let (ws, path) = setup(rel, b"same", b"same", false);
        let d = discard_with(
            ws.path(),
            &[rel.to_string(), "ROVER/parts/new.step".to_string()],
            no_server,
        )
        .await
        .unwrap();
        assert_eq!(d.unchanged, vec![rel.to_string()]);
        assert!(workspace::is_read_only(&path));
        assert_eq!(d.skipped[0].path, "ROVER/parts/new.step");
        assert!(d.recovered_dir.is_none());
    }
}
