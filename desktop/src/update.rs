//! Update notice: is a newer TeamCraft release out?
//!
//! At most once a day (and only while the user leaves it switched on in the
//! settings) the app asks the GitHub releases API for the latest release and
//! compares its version with its own. A newer one shows a notice with a link
//! to the release page – nothing is downloaded or installed by the app.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The releases, newest first. Not `releases/latest`: GitHub leaves
/// pre-releases out of it, and every 0.x release is published as one, so it
/// never had an answer (#493). The first page is enough – a release with 30
/// newer ones is never the one to offer.
const RELEASES_API: &str = "https://api.github.com/repos/WARR-eV/TeamCraft/releases?per_page=30";
/// The list of releases, for *Release page* without a known version.
pub const RELEASES_PAGE: &str = "https://github.com/WARR-eV/TeamCraft/releases";

/// The page of the release of `version` (a version from [`tag_version`]).
pub fn release_page(version: &str) -> String {
    format!("https://github.com/WARR-eV/TeamCraft/releases/tag/v{version}")
}
/// Time between two checks.
pub const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
const TIMEOUT: Duration = Duration::from_secs(10);

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A check is due a day after the last one (or when the clock went back).
pub fn due(last_check: Option<u64>, now: u64) -> bool {
    match last_check {
        None => true,
        Some(last) => now < last || now - last >= CHECK_INTERVAL_SECS,
    }
}

/// `major.minor.patch` and whether it is a pre-release (`1.2.0-rc.1`);
/// `None` for anything else.
fn parse(version: &str) -> Option<([u64; 3], bool)> {
    let version = version.trim().trim_start_matches('v');
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, !pre.is_empty()),
        None => (version, false),
    };
    let core = core.split('+').next()?;
    let mut parts = core.split('.');
    let mut numbers = [0u64; 3];
    for n in &mut numbers {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *n = part.parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some((numbers, pre))
}

/// The version of a release tag (`v0.3.0` → `0.3.0`) if it is a plain
/// version; anything else from the network is ignored.
pub fn tag_version(tag: &str) -> Option<String> {
    let version = tag.trim().trim_start_matches('v');
    let plain = version.len() <= 32
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'+');
    (plain && parse(version).is_some()).then(|| version.to_string())
}

/// Is `latest` newer than `current`? A release is newer than its own
/// pre-releases (`0.3.0` > `0.3.0-rc.1`); unreadable versions never are.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse(latest), parse(current)) {
        (Some((l, l_pre)), Some((c, c_pre))) => l > c || (l == c && c_pre && !l_pre),
        _ => false,
    }
}

/// The release to offer to app version `current` from GitHub's list of
/// releases (#493): the newest that is not a draft. GitHub's `prerelease`
/// flag is not looked at – every 0.x release carries it. A version with a
/// suffix (`0.3.0-rc.1`, a release candidate) is offered only to an app that
/// is such a version itself (a tester); among equal versions the first in
/// the list (the newest) wins.
pub fn pick_release(releases: &serde_json::Value, current: &str) -> Option<String> {
    let tester = parse(current).is_some_and(|(_, pre)| pre);
    let mut best: Option<(([u64; 3], bool), String)> = None;
    for release in releases.as_array()? {
        if release["draft"] == true {
            continue;
        }
        let Some(version) = release["tag_name"].as_str().and_then(tag_version) else {
            continue;
        };
        let Some((numbers, pre)) = parse(&version) else {
            continue;
        };
        if pre && !tester {
            continue;
        }
        // a release is newer than its own candidates
        let key = (numbers, !pre);
        if best.as_ref().is_none_or(|(b, _)| key > *b) {
            best = Some((key, version));
        }
    }
    best.map(|(_, v)| v)
}

/// Asks GitHub for the release to offer ([`pick_release`]); `Ok(None)` when
/// there is no (readable) release yet.
pub async fn latest_release() -> Result<Option<String>, String> {
    // its own client: a short timeout and no redirects (a moved repository
    // shows no notice rather than following an answer to another host)
    let client = reqwest::Client::builder()
        .user_agent(concat!("TeamCraft-Desktop/", env!("CARGO_PKG_VERSION")))
        .timeout(TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None); // no release published yet
    }
    if !response.status().is_success() {
        return Err(format!("GitHub answered {}", response.status()));
    }
    let releases: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(pick_release(&releases, env!("CARGO_PKG_VERSION")))
}

// ------------------------------------------------- update window (#377)

/// *Remind me later* hides the notice for three days.
pub const REMIND_LATER_SECS: u64 = 3 * 24 * 60 * 60;
/// Longest release notes shown (characters).
const MAX_NOTES: usize = 20_000;
/// Largest download accepted.
pub const MAX_DOWNLOAD: u64 = 1024 * 1024 * 1024;
/// Every asset is downloaded from this repository's releases only.
const DOWNLOAD_PREFIX: &str = "https://github.com/WARR-eV/TeamCraft/releases/download/";
/// The release API of one version.
const RELEASE_BY_TAG_API: &str = "https://api.github.com/repos/WARR-eV/TeamCraft/releases/tags/v";
/// The checksum list every release publishes.
pub const SUMS_FILE: &str = "SHA256SUMS.txt";

/// Should the notice of `latest` show now? Only for a newer version that
/// was not skipped (*Skip this version*) and not while *Remind me later*
/// runs (`remind_until`, seconds since the epoch).
pub fn notice_due(
    latest: &str,
    current: &str,
    skipped: Option<&str>,
    remind_until: Option<u64>,
    now: u64,
) -> bool {
    is_newer(latest, current) && skipped != Some(latest) && remind_until.is_none_or(|t| now >= t)
}

/// How this copy of the app was installed – what *Update now* can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallKind {
    /// Windows MSI (Program Files): download, verify, start the installer.
    WindowsMsi,
    /// Windows setup.exe (per user, AppData): download, verify, start it.
    WindowsSetup,
    /// Windows portable zip: download and verify; unpack it yourself.
    WindowsPortable,
    /// macOS app: download and verify the `.dmg` and open it.
    MacDmg,
    /// Linux AppImage: download and verify next to the old one.
    AppImage,
    /// Flatpak, `.deb`, `.rpm`: the package manager updates the app.
    Package,
    /// Built from source or unknown: the release page only.
    Other,
}

/// The install kind from the operating system, the app's own path and the
/// environment (`APPIMAGE`, `FLATPAK_ID`).
pub fn install_kind(os: &str, exe: &str, appimage: bool, flatpak: bool) -> InstallKind {
    let exe_lower = exe.replace('\\', "/").to_ascii_lowercase();
    match os {
        "windows" if exe_lower.contains("/program files") => InstallKind::WindowsMsi,
        "windows" if exe_lower.contains("/appdata/local/") => InstallKind::WindowsSetup,
        "windows" if exe_lower.contains("/target/") => InstallKind::Other,
        "windows" => InstallKind::WindowsPortable,
        "macos" if exe.contains(".app/Contents/MacOS/") => InstallKind::MacDmg,
        "linux" if flatpak => InstallKind::Package,
        "linux" if appimage => InstallKind::AppImage,
        "linux" if exe.starts_with("/usr/") || exe.starts_with("/opt/") => InstallKind::Package,
        _ => InstallKind::Other,
    }
}

/// The install kind of this running app.
pub fn this_install() -> InstallKind {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    install_kind(
        std::env::consts::OS,
        &exe,
        std::env::var_os("APPIMAGE").is_some(),
        std::env::var_os("FLATPAK_ID").is_some(),
    )
}

/// The release file for this install: the name the release workflow gives
/// it (`TeamCraft_0.3.0_x64_en-US.msi`, `TeamCraft_0.3.0_x64-setup.exe`,
/// `TeamCraft_0.3.0_x64_portable.zip`, `TeamCraft_0.3.0_aarch64.dmg`,
/// `TeamCraft_0.3.0_amd64.AppImage`; the *with local server* downloads as
/// `TeamCraft-with-local-server_…`).
pub fn pick_asset<'a>(
    kind: InstallKind,
    arch: &str,
    version: &str,
    local_server: bool,
    names: &'a [String],
) -> Option<&'a str> {
    let prefix = if local_server {
        format!("TeamCraft-with-local-server_{version}_")
    } else {
        format!("TeamCraft_{version}_")
    };
    let win = match arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => return None,
    };
    names
        .iter()
        .map(String::as_str)
        .filter(|n| n.starts_with(&prefix))
        .find(|n| {
            let rest = &n[prefix.len()..];
            match kind {
                InstallKind::WindowsMsi => {
                    rest.starts_with(&format!("{win}_")) && rest.ends_with(".msi")
                }
                InstallKind::WindowsSetup => rest == format!("{win}-setup.exe"),
                InstallKind::WindowsPortable => rest == format!("{win}_portable.zip"),
                InstallKind::MacDmg => arch == "aarch64" && rest == "aarch64.dmg",
                InstallKind::AppImage => arch == "x86_64" && rest == "amd64.AppImage",
                InstallKind::Package | InstallKind::Other => false,
            }
        })
}

/// The command that updates a package installation, for the update window.
pub fn package_hint(flatpak: bool) -> &'static str {
    if flatpak {
        "flatpak update io.github.rudalabs.teamcraft"
    } else {
        "Update TeamCraft with your package manager (apt, dnf, zypper …) or install the new .deb/.rpm from the release page."
    }
}

/// The SHA-256 of `name` in a `sha256sum` list (`<hex>  <name>` or
/// `<hex> *<name>`); `None` without exactly one well-formed line for it.
pub fn checksum_of(sums: &str, name: &str) -> Option<String> {
    let mut found = None;
    for line in sums.lines() {
        let line = line.trim_end_matches('\r');
        let Some((hex, file)) = line.split_once(' ') else {
            continue;
        };
        let file = file.trim_start_matches(' ').trim_start_matches('*');
        if file != name {
            continue;
        }
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) || found.is_some() {
            return None;
        }
        found = Some(hex.to_ascii_lowercase());
    }
    found
}

/// A release as the update window shows it.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Release {
    pub version: String,
    /// The release notes (plain text, cut at [`MAX_NOTES`] characters).
    pub notes: String,
    /// Asset name → download address (this repository's releases only).
    #[serde(skip)]
    pub assets: Vec<(String, String)>,
}

/// The release of one version from the GitHub release JSON; assets with an
/// address outside this repository's downloads are left out.
pub fn release_of(json: &serde_json::Value) -> Option<Release> {
    let version = tag_version(json["tag_name"].as_str()?)?;
    let notes: String = json["body"]
        .as_str()
        .unwrap_or("")
        .chars()
        .take(MAX_NOTES)
        .collect();
    let assets = json["assets"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    let name = a["name"].as_str()?;
                    let url = a["browser_download_url"].as_str()?;
                    let expected = format!("{DOWNLOAD_PREFIX}v{version}/{name}");
                    (url == expected).then(|| (name.to_string(), url.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Release {
        version,
        notes,
        assets,
    })
}

fn client(redirects: bool) -> Result<reqwest::Client, String> {
    let policy = if redirects {
        // GitHub's downloads answer with a redirect to its storage host:
        // followed over https only; the checksum proves the content
        reqwest::redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() == "https" && attempt.previous().len() < 5 {
                attempt.follow()
            } else {
                attempt.stop()
            }
        })
    } else {
        reqwest::redirect::Policy::none()
    };
    reqwest::Client::builder()
        .user_agent(concat!("TeamCraft-Desktop/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(TIMEOUT)
        .redirect(policy)
        .build()
        .map_err(|e| e.to_string())
}

/// The release of `version` from GitHub (notes and assets).
pub async fn release(version: &str) -> Result<Release, String> {
    let version = tag_version(version).ok_or("not a version")?;
    let response = client(false)?
        .get(format!("{RELEASE_BY_TAG_API}{version}"))
        .header("Accept", "application/vnd.github+json")
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub answered {}", response.status()));
    }
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    release_of(&json).ok_or_else(|| "GitHub sent an unreadable release.".to_string())
}

/// Downloads `name` of `release` into `dir` and checks it against the
/// release's [`SUMS_FILE`]: a file that does not match, or has no line
/// there, is deleted and nothing changes. Returns the verified file.
pub async fn download_verified(
    release: &Release,
    name: &str,
    dir: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncWriteExt;
    let url_of = |n: &str| {
        release
            .assets
            .iter()
            .find(|(a, _)| a == n)
            .map(|(_, u)| u.clone())
    };
    let sums_url =
        url_of(SUMS_FILE).ok_or("The release has no SHA256SUMS.txt – nothing was downloaded.")?;
    let url = url_of(name).ok_or_else(|| format!("The release has no {name}."))?;
    let http = client(true)?;
    let sums = http
        .get(&sums_url)
        .timeout(TIMEOUT)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("SHA256SUMS.txt could not be loaded: {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let expected = checksum_of(&sums, name).ok_or_else(|| {
        format!("SHA256SUMS.txt has no checksum for {name} – nothing was downloaded.")
    })?;
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| e.to_string())?;
    let target = dir.join(name);
    let part = dir.join(format!("{name}.part"));
    let result = async {
        let mut response = http
            .get(&url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("The download failed: {e}"))?;
        let mut file = tokio::fs::File::create(&part)
            .await
            .map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut size = 0u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("The download failed: {e}"))?
        {
            size += chunk.len() as u64;
            if size > MAX_DOWNLOAD {
                return Err("The download is larger than 1 GiB – stopped.".to_string());
            }
            hash.update(&chunk);
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())?;
        drop(file);
        verify_digest(&hex::encode(hash.finalize()), &expected, name)
    }
    .await;
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(e);
    }
    tokio::fs::rename(&part, &target)
        .await
        .map_err(|e| e.to_string())?;
    Ok(target)
}

/// The downloaded file's SHA-256 must be the listed one.
fn verify_digest(actual: &str, expected: &str, name: &str) -> Result<(), String> {
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "{name} does not match its checksum in SHA256SUMS.txt – it was deleted and nothing was installed."
        ))
    }
}

// ------------------------------------------- swap in place (#466)

/// The files of the Windows portable zip that an update replaces, by name
/// (the release workflow packs them into one folder `TeamCraft_<v>_<arch>_portable`).
pub const PORTABLE_FILES: [&str; 3] = ["TeamCraft.exe", "LICENSE.txt", "README.txt"];
/// The suffix of a file the swap moved aside; removed at the next start.
pub const OLD_SUFFIX: &str = ".old";
/// The folder next to the app where the zip is unpacked first.
pub const STAGING_DIR: &str = ".teamcraft-update";

/// The file of the portable folder an entry of the unpacked zip replaces:
/// `TeamCraft_0.3.0_x64_portable/TeamCraft.exe` → `TeamCraft.exe`. Only
/// [`PORTABLE_FILES`] one folder deep; anything else (other names, deeper or
/// shallower paths, `..`) is never copied.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn portable_target(entry: &str) -> Option<&'static str> {
    let parts: Vec<&str> = entry.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        [folder, name] if !folder.starts_with('.') && *folder != ".." => PORTABLE_FILES
            .iter()
            .copied()
            .find(|f| f.eq_ignore_ascii_case(name)),
        _ => None,
    }
}

/// The name a replaced file is moved to: `TeamCraft.exe` → `TeamCraft.exe.old`.
pub fn old_name(name: &str) -> String {
    format!("{name}{OLD_SUFFIX}")
}

/// What the start after a swap removes from the app's folder: the files the
/// swap moved aside (only those of [`PORTABLE_FILES`]) and a staging folder
/// left by an interrupted update.
pub fn leftovers(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|n| {
            n.as_str() == STAGING_DIR
                || PORTABLE_FILES
                    .iter()
                    .any(|f| n.eq_ignore_ascii_case(&old_name(f)))
        })
        .cloned()
        .collect()
}

/// One step of a swap: move `from` to `to` (both in the app's folder).
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: std::path::PathBuf,
    pub to: std::path::PathBuf,
}

/// The moves that put the unpacked files `found` (name → path in the
/// staging folder) into the app folder `dir`: each existing file aside to
/// `.old` first, then the new one in place. A file the zip does not have is
/// kept. Without `TeamCraft.exe` in the zip there is nothing to do.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn swap_moves(
    dir: &std::path::Path,
    found: &[(&'static str, std::path::PathBuf)],
    exists: impl Fn(&std::path::Path) -> bool,
) -> Vec<Move> {
    if !found.iter().any(|(n, _)| *n == "TeamCraft.exe") {
        return Vec::new();
    }
    let mut moves = Vec::new();
    for (name, new) in found {
        let target = dir.join(name);
        if exists(&target) {
            moves.push(Move {
                from: target.clone(),
                to: dir.join(old_name(name)),
            });
        }
        moves.push(Move {
            from: new.clone(),
            to: target,
        });
    }
    moves
}

/// Runs `moves` in order; when one fails, the ones done are undone in
/// reverse order and the error names the file.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn run_moves(moves: &[Move]) -> Result<(), String> {
    for (i, m) in moves.iter().enumerate() {
        if let Err(e) = std::fs::rename(&m.from, &m.to) {
            for done in moves[..i].iter().rev() {
                let _ = std::fs::rename(&done.to, &done.from);
            }
            return Err(format!("{} could not be replaced: {e}", m.to.display()));
        }
    }
    Ok(())
}

/// Removes [`leftovers`] from the app folder `dir` (at start; failures are
/// ignored – the file may still be in use, the next start tries again).
pub fn remove_leftovers(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    for name in leftovers(&names) {
        let path = dir.join(&name);
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Whether the app may write into `dir` (a test file is created and removed).
pub fn writable(dir: &std::path::Path) -> bool {
    let probe = dir.join(".teamcraft-write-test");
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// The AppImage swap: the verified download is copied next to the running
/// AppImage `target` (`<name>.new`, same file system), made executable and
/// renamed over it – a running AppImage keeps its mounted image.
pub fn swap_appimage(download: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    let dir = target
        .parent()
        .ok_or_else(|| "The AppImage has no folder.".to_string())?;
    if !writable(dir) {
        return Err(format!("{} is not writable.", dir.display()));
    }
    let staged = target.with_extension("new");
    std::fs::copy(download, &staged)
        .map_err(|e| format!("{} could not be written: {e}", staged.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)) {
            let _ = std::fs::remove_file(&staged);
            return Err(format!(
                "{} could not be made executable: {e}",
                staged.display()
            ));
        }
    }
    std::fs::rename(&staged, target).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        format!("{} could not be replaced: {e}", target.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_to_offer_is_picked_from_the_list() {
        use serde_json::json;
        let rel = |tag: &str, draft: bool, pre: bool| json!({ "tag_name": tag, "draft": draft, "prerelease": pre });
        // #493: every 0.x release is a GitHub pre-release – it still counts
        let list = json!([
            rel("v0.3.0-rc.1", false, true),
            rel("v0.2.13", false, true),
            rel("v0.2.12", false, true)
        ]);
        assert_eq!(pick_release(&list, "0.2.12").as_deref(), Some("0.2.13"));
        // a release candidate only for a tester
        assert_eq!(
            pick_release(&list, "0.2.13-rc.1").as_deref(),
            Some("0.3.0-rc.1")
        );
        // drafts and unreadable tags never
        let odd = json!([
            rel("v0.4.0", true, false),
            rel("nightly", false, true),
            rel("v0.2.13", false, false)
        ]);
        assert_eq!(pick_release(&odd, "0.2.12").as_deref(), Some("0.2.13"));
        // the list need not be sorted; a release beats its own candidate
        let mixed = json!([
            rel("v0.2.9", false, true),
            rel("v0.3.0-rc.2", false, true),
            rel("v0.3.0", false, true)
        ]);
        assert_eq!(pick_release(&mixed, "0.3.0-rc.1").as_deref(), Some("0.3.0"));
        // nothing usable
        assert_eq!(pick_release(&json!([]), "0.2.12"), None);
        assert_eq!(
            pick_release(&json!({"message": "Not Found"}), "0.2.12"),
            None
        );
        assert_eq!(
            release_page("0.2.13"),
            "https://github.com/WARR-eV/TeamCraft/releases/tag/v0.2.13"
        );
    }

    #[test]
    fn only_the_portable_files_one_folder_deep_are_swapped() {
        // #466: the release workflow's zip layout
        assert_eq!(
            portable_target("TeamCraft_0.3.0_x64_portable/TeamCraft.exe"),
            Some("TeamCraft.exe")
        );
        assert_eq!(
            portable_target(r"TeamCraft_0.3.0_x64_portable\README.txt"),
            Some("README.txt")
        );
        assert_eq!(
            portable_target("TeamCraft_0.3.0_x64_portable/teamcraft.EXE"),
            Some("TeamCraft.exe")
        );
        // anything else never
        assert_eq!(portable_target("TeamCraft.exe"), None, "not in the folder");
        assert_eq!(portable_target("a/b/TeamCraft.exe"), None, "too deep");
        assert_eq!(portable_target("../TeamCraft.exe"), None);
        assert_eq!(portable_target(".hidden/TeamCraft.exe"), None);
        assert_eq!(
            portable_target("TeamCraft_0.3.0_x64_portable/evil.dll"),
            None
        );
    }

    #[test]
    fn a_swap_moves_the_old_files_aside_first() {
        let dir = std::path::Path::new("/app");
        let staged = |n: &str| std::path::PathBuf::from(format!("/app/.teamcraft-update/T/{n}"));
        let found = vec![
            ("TeamCraft.exe", staged("TeamCraft.exe")),
            ("README.txt", staged("README.txt")),
        ];
        // README.txt was deleted by the user: it just comes back
        let moves = swap_moves(dir, &found, |p| p.ends_with("TeamCraft.exe"));
        let pairs: Vec<(String, String)> = moves
            .iter()
            .map(|m| {
                (
                    m.from.to_string_lossy().into_owned(),
                    m.to.to_string_lossy().into_owned(),
                )
            })
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("/app/TeamCraft.exe".into(), "/app/TeamCraft.exe.old".into()),
                (
                    "/app/.teamcraft-update/T/TeamCraft.exe".into(),
                    "/app/TeamCraft.exe".into()
                ),
                (
                    "/app/.teamcraft-update/T/README.txt".into(),
                    "/app/README.txt".into()
                ),
            ]
        );
        // a zip without the app: nothing at all
        assert!(swap_moves(dir, &[("README.txt", staged("README.txt"))], |_| true).is_empty());
    }

    #[test]
    fn a_failed_move_undoes_the_ones_before() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("TeamCraft.exe");
        let new = dir.path().join("new.exe");
        std::fs::write(&app, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        let moves = vec![
            Move {
                from: app.clone(),
                to: dir.path().join("TeamCraft.exe.old"),
            },
            Move {
                from: new.clone(),
                to: app.clone(),
            },
            Move {
                from: dir.path().join("missing"),
                to: dir.path().join("README.txt"),
            },
        ];
        assert!(run_moves(&moves).unwrap_err().contains("README.txt"));
        assert_eq!(std::fs::read(&app).unwrap(), b"old", "the old app is back");
        assert_eq!(std::fs::read(&new).unwrap(), b"new");
        assert!(!dir.path().join("TeamCraft.exe.old").exists());
        // the same moves without the broken one go through
        run_moves(&moves[..2]).unwrap();
        assert_eq!(std::fs::read(&app).unwrap(), b"new");
        assert_eq!(
            std::fs::read(dir.path().join("TeamCraft.exe.old")).unwrap(),
            b"old"
        );
    }

    #[test]
    fn the_next_start_removes_only_the_swap_leftovers() {
        let names: Vec<String> = [
            "TeamCraft.exe",
            "TeamCraft.exe.old",
            "README.txt.OLD",
            ".teamcraft-update",
            "notes.old",
            "data",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            leftovers(&names),
            vec!["TeamCraft.exe.old", "README.txt.OLD", ".teamcraft-update"]
        );
        let dir = tempfile::tempdir().unwrap();
        for n in &names[..5] {
            std::fs::write(dir.path().join(n), b"x").unwrap();
        }
        remove_leftovers(dir.path());
        assert!(dir.path().join("TeamCraft.exe").exists());
        assert!(dir.path().join("notes.old").exists(), "not ours");
        assert!(!dir.path().join("TeamCraft.exe.old").exists());
    }

    #[test]
    fn an_appimage_is_replaced_by_a_rename() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("TeamCraft.AppImage");
        let download = dir.path().join("download.AppImage");
        std::fs::write(&target, b"old").unwrap();
        std::fs::write(&download, b"new").unwrap();
        swap_appimage(&download, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert!(!target.with_extension("new").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
        assert!(swap_appimage(&dir.path().join("gone"), &target).is_err());
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"new",
            "a failed swap keeps the app"
        );
    }

    #[test]
    fn newer_versions() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.0"), "numeric, not text order");
        assert!(is_newer("0.3.0", "0.3.0-rc.1"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("0.3.0-rc.1", "0.3.0"));
        assert!(!is_newer("garbage", "0.2.0"));
        assert!(!is_newer("0.3", "0.2.0"));
        assert!(!is_newer("0.3.0.1", "0.2.0"));
    }

    #[test]
    fn tags() {
        assert_eq!(tag_version("v0.3.0").as_deref(), Some("0.3.0"));
        assert_eq!(tag_version("1.2.3-rc.1").as_deref(), Some("1.2.3-rc.1"));
        assert_eq!(tag_version("v1.2"), None);
        assert_eq!(tag_version("v1.2.3<script>"), None);
        assert_eq!(tag_version(&format!("v1.2.3-{}", "x".repeat(40))), None);
        assert_eq!(tag_version("nightly"), None);
    }

    #[test]
    fn at_most_daily() {
        let day = CHECK_INTERVAL_SECS;
        assert!(due(None, 1_000));
        assert!(!due(Some(1_000), 1_000 + day - 1));
        assert!(due(Some(1_000), 1_000 + day));
        assert!(due(Some(1_000 + day), 1_000), "the clock went back");
    }

    #[test]
    fn skipped_and_remind_later() {
        let now = 1_000_000;
        assert!(notice_due("0.3.0", "0.2.0", None, None, now));
        assert!(
            !notice_due("0.3.0", "0.2.0", Some("0.3.0"), None, now),
            "skipped"
        );
        assert!(
            notice_due("0.3.1", "0.2.0", Some("0.3.0"), None, now),
            "a newer one shows again"
        );
        let later = now + REMIND_LATER_SECS;
        assert!(
            !notice_due("0.3.0", "0.2.0", None, Some(later), now + 10),
            "remind me later"
        );
        assert!(
            notice_due("0.3.0", "0.2.0", None, Some(later), later),
            "three days on"
        );
        assert!(!notice_due("0.2.0", "0.2.0", None, None, now));
    }

    #[test]
    fn install_kinds() {
        use InstallKind::*;
        assert_eq!(
            install_kind(
                "windows",
                r"C:\Program Files\TeamCraft\TeamCraft.exe",
                false,
                false
            ),
            WindowsMsi
        );
        assert_eq!(
            install_kind(
                "windows",
                r"C:\Users\kim\AppData\Local\TeamCraft\teamcraft-desktop.exe",
                false,
                false
            ),
            WindowsSetup
        );
        assert_eq!(
            install_kind(
                "windows",
                r"D:\Tools\TeamCraft_0.2.9_x64_portable\TeamCraft.exe",
                false,
                false
            ),
            WindowsPortable
        );
        assert_eq!(
            install_kind(
                "macos",
                "/Applications/TeamCraft.app/Contents/MacOS/teamcraft-desktop",
                false,
                false
            ),
            MacDmg
        );
        assert_eq!(
            install_kind(
                "linux",
                "/tmp/.mount_TeamCr/usr/bin/teamcraft-desktop",
                true,
                false
            ),
            AppImage
        );
        assert_eq!(
            install_kind("linux", "/usr/bin/teamcraft-desktop", false, false),
            Package
        );
        assert_eq!(
            install_kind("linux", "/app/bin/teamcraft-desktop", false, true),
            Package
        );
        assert_eq!(
            install_kind(
                "linux",
                "/home/kim/src/desktop/target/debug/teamcraft-desktop",
                false,
                false
            ),
            Other
        );
    }

    #[test]
    fn the_asset_fits_the_install() {
        use InstallKind::*;
        let names: Vec<String> = [
            "TeamCraft_0.3.0_x64_en-US.msi",
            "TeamCraft_0.3.0_arm64_en-US.msi",
            "TeamCraft_0.3.0_x64-setup.exe",
            "TeamCraft_0.3.0_x64_portable.zip",
            "TeamCraft_0.3.0_aarch64.dmg",
            "TeamCraft_0.3.0_amd64.AppImage",
            "TeamCraft-with-local-server_0.3.0_x64-setup.exe",
            "SHA256SUMS.txt",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let pick = |k, arch, local| pick_asset(k, arch, "0.3.0", local, &names);
        assert_eq!(
            pick(WindowsMsi, "x86_64", false),
            Some("TeamCraft_0.3.0_x64_en-US.msi")
        );
        assert_eq!(
            pick(WindowsMsi, "aarch64", false),
            Some("TeamCraft_0.3.0_arm64_en-US.msi")
        );
        assert_eq!(
            pick(WindowsSetup, "x86_64", false),
            Some("TeamCraft_0.3.0_x64-setup.exe")
        );
        assert_eq!(
            pick(WindowsSetup, "x86_64", true),
            Some("TeamCraft-with-local-server_0.3.0_x64-setup.exe")
        );
        assert_eq!(pick(WindowsSetup, "aarch64", false), None);
        assert_eq!(
            pick(WindowsPortable, "x86_64", false),
            Some("TeamCraft_0.3.0_x64_portable.zip")
        );
        assert_eq!(
            pick(MacDmg, "aarch64", false),
            Some("TeamCraft_0.3.0_aarch64.dmg")
        );
        assert_eq!(
            pick(AppImage, "x86_64", false),
            Some("TeamCraft_0.3.0_amd64.AppImage")
        );
        assert_eq!(pick(AppImage, "aarch64", false), None);
        assert_eq!(pick(Package, "x86_64", false), None);
        assert_eq!(
            pick_asset(WindowsMsi, "x86_64", "0.3.1", false, &names),
            None,
            "another version"
        );
    }

    #[test]
    fn checksums() {
        let a = "a".repeat(64);
        let b = "B".repeat(64);
        let sums = format!(
            "{a}  TeamCraft_0.3.0_x64-setup.exe\r\n{b} *TeamCraft_0.3.0_aarch64.dmg\nnot a line\n"
        );
        assert_eq!(
            checksum_of(&sums, "TeamCraft_0.3.0_x64-setup.exe"),
            Some(a.clone())
        );
        assert_eq!(
            checksum_of(&sums, "TeamCraft_0.3.0_aarch64.dmg"),
            Some("b".repeat(64))
        );
        assert_eq!(
            checksum_of(&sums, "TeamCraft_0.3.0_x64_en-US.msi"),
            None,
            "missing line"
        );
        assert_eq!(checksum_of("abc  x.msi\n", "x.msi"), None, "not a SHA-256");
        assert_eq!(
            checksum_of(&format!("{a}  x.msi\n{b}  x.msi\n"), "x.msi"),
            None,
            "two lines"
        );
        assert!(verify_digest(&a, &a.to_uppercase(), "x").is_ok());
        let err = verify_digest(&a, &b, "x.msi").unwrap_err();
        assert!(
            err.contains("does not match") && err.contains("deleted"),
            "{err}"
        );
    }

    #[test]
    fn release_assets_come_from_this_repository_only() {
        let json = serde_json::json!({
            "tag_name": "v0.3.0", "body": "Notes",
            "assets": [
                { "name": "SHA256SUMS.txt", "browser_download_url": "https://github.com/WARR-eV/TeamCraft/releases/download/v0.3.0/SHA256SUMS.txt" },
                { "name": "evil.msi", "browser_download_url": "https://example.org/evil.msi" },
                { "name": "TeamCraft_0.3.0_x64-setup.exe", "browser_download_url": "https://github.com/WARR-eV/TeamCraft/releases/download/v0.2.0/TeamCraft_0.3.0_x64-setup.exe" }
            ]
        });
        let r = release_of(&json).unwrap();
        assert_eq!(r.version, "0.3.0");
        assert_eq!(r.notes, "Notes");
        assert_eq!(r.assets.len(), 1);
        assert_eq!(r.assets[0].0, "SHA256SUMS.txt");
        assert!(release_of(&serde_json::json!({ "tag_name": "nightly" })).is_none());
    }
}
