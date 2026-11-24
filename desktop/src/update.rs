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

/// One dot-separated identifier of a pre-release (`rc`, `2`): numbers rank
/// below words and compare as numbers (SemVer 2.0.0, §11).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Ident {
    Num(u64),
    Text(String),
}

/// A readable version: `major.minor.patch` and its pre-release identifiers
/// (empty for a release). Build metadata (`+…`) is dropped – it never counts.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    core: [u64; 3],
    pre: Vec<Ident>,
}

/// The version of `version` (`v` prefix allowed); `None` for anything else.
fn parse(version: &str) -> Option<Version> {
    let version = version.trim().trim_start_matches('v');
    // the metadata first: its `-` is no pre-release (`0.3.0+build-1`)
    let version = version.split('+').next()?;
    let (core, pre) = version.split_once('-').unwrap_or((version, ""));
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
    let mut idents = Vec::new();
    if !pre.is_empty() {
        for id in pre.split('.') {
            if id.is_empty() {
                return None;
            }
            idents.push(match id.parse::<u64>() {
                Ok(n) if id.bytes().all(|b| b.is_ascii_digit()) => Ident::Num(n),
                _ => Ident::Text(id.to_string()),
            });
        }
    }
    Some(Version {
        core: numbers,
        pre: idents,
    })
}

/// SemVer precedence: the numbers first; at equal numbers a release is
/// greater than its pre-releases, and two pre-releases compare identifier by
/// identifier (`rc.10` > `rc.9` > `rc.2`; the longer list wins a tie).
fn cmp_version(a: &Version, b: &Version) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    a.core
        .cmp(&b.core)
        .then_with(|| match (a.pre.is_empty(), b.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => a.pre.cmp(&b.pre),
        })
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

/// The page *Release page* opens: the release of `version` when it is a
/// plain version ([`tag_version`]), the list of releases otherwise.
pub fn release_url(version: Option<&str>) -> String {
    version
        .and_then(tag_version)
        .map_or_else(|| RELEASES_PAGE.to_string(), |v| release_page(&v))
}

/// Is `latest` newer than `current` by SemVer precedence? A release is newer
/// than its own pre-releases (`0.3.0` > `0.3.0-rc.1`), a later candidate than
/// an earlier one (`0.3.0-rc.2` > `0.3.0-rc.1`); build metadata never counts
/// and unreadable versions never are.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => cmp_version(&l, &c) == std::cmp::Ordering::Greater,
        _ => false,
    }
}

/// The release to offer to app version `current` from GitHub's list of
/// releases (#493): the newest by SemVer precedence that is not a draft –
/// the order of the list does not matter. GitHub's `prerelease` flag is not
/// looked at – every 0.x release carries it. A version with a suffix
/// (`0.3.0-rc.1`, a release candidate) is offered only to an app that is
/// such a version itself (a tester); among equal versions the first in the
/// list wins.
pub fn pick_release(releases: &serde_json::Value, current: &str) -> Option<String> {
    let tester = parse(current).is_some_and(|v| !v.pre.is_empty());
    let mut best: Option<(Version, String)> = None;
    for release in releases.as_array()? {
        if release["draft"] == true {
            continue;
        }
        let Some(version) = release["tag_name"].as_str().and_then(tag_version) else {
            continue;
        };
        let Some(parsed) = parse(&version) else {
            continue;
        };
        if !parsed.pre.is_empty() && !tester {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(b, _)| cmp_version(&parsed, b) == std::cmp::Ordering::Greater)
        {
            best = Some((parsed, version));
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

/// The notice of the status bar from the last known release `latest`
/// ([`notice_due`]): `None` without one, for a version that is not newer
/// (the app was updated since the last check), a skipped one or while
/// *Remind me later* runs.
pub fn notice_for(
    latest: Option<&str>,
    current: &str,
    skipped: Option<&str>,
    remind_until: Option<u64>,
    now: u64,
) -> Option<String> {
    latest
        .filter(|v| notice_due(v, current, skipped, remind_until, now))
        .map(str::to_string)
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
/// environment (`APPIMAGE`, `FLATPAK_ID`). `portable`: the app's folder
/// proves to be an unpacked portable zip ([`portable_folder`]) – any other
/// Windows folder (an MSI installed to `D:\Apps`, a `TeamCraft.exe` copied
/// to the desktop) is never updated in place.
pub fn install_kind(
    os: &str,
    exe: &str,
    appimage: bool,
    flatpak: bool,
    portable: bool,
) -> InstallKind {
    let exe_lower = exe.replace('\\', "/").to_ascii_lowercase();
    match os {
        "windows" if exe_lower.contains("/program files") => InstallKind::WindowsMsi,
        "windows" if exe_lower.contains("/appdata/local/") => InstallKind::WindowsSetup,
        "windows" if exe_lower.contains("/target/") => InstallKind::Other,
        "windows" if portable => InstallKind::WindowsPortable,
        "windows" => InstallKind::Other,
        "macos" if exe.contains(".app/Contents/MacOS/") => InstallKind::MacDmg,
        "linux" if flatpak => InstallKind::Package,
        "linux" if appimage => InstallKind::AppImage,
        "linux" if exe.starts_with("/usr/") || exe.starts_with("/opt/") => InstallKind::Package,
        _ => InstallKind::Other,
    }
}

/// The install kind of this running app.
pub fn this_install() -> InstallKind {
    let exe = std::env::current_exe().unwrap_or_default();
    let portable = std::env::consts::OS == "windows" && exe.parent().is_some_and(portable_folder);
    install_kind(
        std::env::consts::OS,
        &exe.to_string_lossy(),
        std::env::var_os("APPIMAGE").is_some(),
        std::env::var_os("FLATPAK_ID").is_some(),
        portable,
    )
}

/// The marker file the release workflow puts into the portable zip's folder.
pub const PORTABLE_MARKER: &str = ".teamcraft-portable";

/// Is the first line of a `README.txt` the one of the portable zip
/// (`TeamCraft Desktop 0.3.0 (x64) - portable`)? For copies unpacked before
/// the zip had [`PORTABLE_MARKER`].
pub fn readme_is_portable(text: &str) -> bool {
    let first = text
        .trim_start_matches('\u{feff}')
        .lines()
        .next()
        .unwrap_or("")
        .trim();
    first.starts_with("TeamCraft Desktop ") && first.ends_with(" - portable")
}

/// Whether `dir` is an unpacked portable zip: it holds [`PORTABLE_MARKER`],
/// or the zip's own `README.txt`.
pub fn portable_folder(dir: &std::path::Path) -> bool {
    dir.join(PORTABLE_MARKER).is_file() || readme_is_portable(&head_of(&dir.join("README.txt")))
}

/// The first bytes of a file as text (empty when it cannot be read).
fn head_of(path: &std::path::Path) -> String {
    use std::io::Read;
    let mut buf = Vec::new();
    if let Ok(file) = std::fs::File::open(path) {
        let _ = file.take(256).read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// The folder the start cleans up after a swap ([`remove_leftovers`]): the
/// app's own folder, for a portable copy only – never an installer's folder.
pub fn leftover_dir(kind: InstallKind, exe: &std::path::Path) -> Option<std::path::PathBuf> {
    if kind == InstallKind::WindowsPortable {
        exe.parent().map(std::path::Path::to_path_buf)
    } else {
        None
    }
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
pub const PORTABLE_FILES: [&str; 4] = [
    "TeamCraft.exe",
    "LICENSE.txt",
    "README.txt",
    PORTABLE_MARKER,
];
/// The suffix of a file the swap moved aside; removed at the next start.
pub const OLD_SUFFIX: &str = ".old";
/// The folder next to the app where the zip is unpacked first; each process
/// unpacks into its own `.teamcraft-update-<pid>` ([`staging_name`]).
pub const STAGING_DIR: &str = ".teamcraft-update";
/// The list of the files a swap moved aside – the only ones the next start
/// removes ([`remove_leftovers`]).
pub const ASIDE_LIST: &str = ".teamcraft-moved-aside";
/// A staging folder this old was left by an update that ended (an
/// interrupted one); a younger one may be in use by another instance.
pub const STALE_STAGING_SECS: u64 = 60 * 60;

/// The staging folder of process `pid`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn staging_name(pid: u32) -> String {
    format!("{STAGING_DIR}-{pid}")
}

/// Is `name` a staging folder ([`STAGING_DIR`] or [`staging_name`])?
fn is_staging(name: &str) -> bool {
    name == STAGING_DIR
        || name
            .strip_prefix(STAGING_DIR)
            .and_then(|r| r.strip_prefix('-'))
            .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
}

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

/// The free name to move `name` aside to: `TeamCraft.exe.old`, or – when
/// that is `taken` (an old copy is still there) – `TeamCraft.exe.1.old` …
/// `.9.old`; an existing file is never renamed over. `None` when all are taken.
pub fn aside_name(name: &str, taken: impl Fn(&str) -> bool) -> Option<String> {
    std::iter::once(old_name(name))
        .chain((1..10).map(|i| format!("{name}.{i}{OLD_SUFFIX}")))
        .find(|n| !taken(n.as_str()))
}

/// Is `name` one of the names [`aside_name`] gives a file of [`PORTABLE_FILES`]?
fn is_aside_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    PORTABLE_FILES.iter().any(|f| {
        lower
            .strip_prefix(&f.to_ascii_lowercase())
            .and_then(|rest| rest.strip_suffix(OLD_SUFFIX))
            .is_some_and(|mid| {
                mid.is_empty()
                    || (mid.len() == 2
                        && mid.starts_with('.')
                        && mid.as_bytes()[1].is_ascii_digit()
                        && mid != ".0")
            })
    })
}

/// What the start after a swap removes from the app's folder `names`: the
/// files the swap moved aside – only those it listed in [`ASIDE_LIST`]
/// (`listed`), never a user's `README.txt.old` – and the staging folders
/// that are `stale` (left by an interrupted update, not one another running
/// instance is unpacking into).
pub fn leftovers(names: &[String], listed: &[String], stale: impl Fn(&str) -> bool) -> Vec<String> {
    names
        .iter()
        .filter(|n| {
            (is_staging(n) && stale(n.as_str()))
                || (is_aside_name(n) && listed.iter().any(|l| l.eq_ignore_ascii_case(n)))
        })
        .cloned()
        .collect()
}

/// Whose file is at a target of the swap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Existing {
    /// Nothing there.
    Missing,
    /// TeamCraft's own file (an older version of it).
    Ours,
    /// Somebody else's file with the same name: left alone.
    Foreign,
}

/// Is the file `name` whose text starts with `head` TeamCraft's own? The
/// portable zip's `README.txt` starts with "TeamCraft Desktop", its
/// `LICENSE.txt` is the MPL-2.0; the app and the marker always are.
pub fn is_ours(name: &str, head: &str) -> bool {
    let head = head.trim_start_matches('\u{feff}').trim_start();
    match name {
        "README.txt" => head.starts_with("TeamCraft Desktop"),
        "LICENSE.txt" => head.starts_with("Mozilla Public License Version 2.0"),
        _ => true,
    }
}

/// What is at `path`, the target of the portable file `name`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn existing(name: &str, path: &std::path::Path) -> Existing {
    if !path.exists() {
        Existing::Missing
    } else if is_ours(name, &head_of(path)) {
        Existing::Ours
    } else {
        Existing::Foreign
    }
}

/// The files of [`PORTABLE_FILES`] in the unpacked zip `staging` (one
/// folder deep, [`portable_target`]). A zip with a file twice (two version
/// folders) is refused.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn found_in(
    staging: &std::path::Path,
) -> Result<Vec<(&'static str, std::path::PathBuf)>, String> {
    let mut found: Vec<(&'static str, std::path::PathBuf)> = Vec::new();
    let mut folders: Vec<_> = std::fs::read_dir(staging)
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    folders.sort_by_key(|e| e.file_name());
    for folder in folders {
        let folder_name = folder.file_name().to_string_lossy().into_owned();
        let mut files: Vec<_> = std::fs::read_dir(folder.path())
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        files.sort_by_key(|e| e.file_name());
        for file in files {
            let rel = format!("{folder_name}/{}", file.file_name().to_string_lossy());
            if let Some(name) = portable_target(&rel) {
                if found.iter().any(|(n, _)| *n == name) {
                    return Err(format!("The update has more than one {name}."));
                }
                found.push((name, file.path()));
            }
        }
    }
    Ok(found)
}

/// One step of a swap: move `from` to `to` (both in the app's folder).
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: std::path::PathBuf,
    pub to: std::path::PathBuf,
}

/// The moves that put the unpacked files `found` (name → path in the
/// staging folder) into the app folder `dir`: TeamCraft's own file aside
/// first (to a free [`aside_name`]; `taken` says whether a name in `dir` is
/// in use), then the new one in place. A `README.txt` or `LICENSE.txt` that
/// is not TeamCraft's (`existing` says [`Existing::Foreign`]) is left alone
/// and not replaced; a missing one is put in. A file the zip does not have
/// is kept. Without `TeamCraft.exe` in the zip there is nothing to do.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn swap_moves(
    dir: &std::path::Path,
    found: &[(&'static str, std::path::PathBuf)],
    existing: impl Fn(&'static str, &std::path::Path) -> Existing,
    taken: impl Fn(&std::path::Path) -> bool,
) -> Result<Vec<Move>, String> {
    if !found.iter().any(|(n, _)| *n == "TeamCraft.exe") {
        return Ok(Vec::new());
    }
    let mut moves = Vec::new();
    for (name, new) in found {
        let name: &'static str = *name;
        let target = dir.join(name);
        let app = name == "TeamCraft.exe";
        match existing(name, &target) {
            Existing::Foreign if !app => continue,
            Existing::Missing => {}
            Existing::Ours | Existing::Foreign => {
                let aside = aside_name(name, |n| taken(&dir.join(n))).ok_or_else(|| {
                    format!("{name} could not be moved aside – remove its old copies ({name}.*.old) first.")
                })?;
                moves.push(Move {
                    from: target.clone(),
                    to: dir.join(aside),
                });
            }
        }
        moves.push(Move {
            from: new.clone(),
            to: target,
        });
    }
    Ok(moves)
}

/// The names `moves` put aside in the app folder (a move within one folder):
/// what [`record_asides`] lists for the next start.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn asides(moves: &[Move]) -> Vec<String> {
    moves
        .iter()
        .filter(|m| m.from.parent() == m.to.parent())
        .filter_map(|m| m.to.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect()
}

/// Adds `names` to the [`ASIDE_LIST`] in `dir` (after a swap).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn record_asides(dir: &std::path::Path, names: &[String]) -> std::io::Result<()> {
    let path = dir.join(ASIDE_LIST);
    let mut list = read_list(&path);
    for n in names {
        if !list.iter().any(|l| l.eq_ignore_ascii_case(n)) {
            list.push(n.clone());
        }
    }
    std::fs::write(&path, list.join("\n") + "\n")
}

fn read_list(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
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

/// Removes [`leftovers`] from the app folder `dir` (at start, `now`; failures
/// are ignored – the file may still be in use, the next start tries again).
/// The [`ASIDE_LIST`] keeps the names that are still there and goes when
/// none is left.
pub fn remove_leftovers(dir: &std::path::Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    let list_path = dir.join(ASIDE_LIST);
    let listed = read_list(&list_path);
    let stale = |name: &str| {
        std::fs::metadata(dir.join(name))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age.as_secs() >= STALE_STAGING_SECS)
    };
    for name in leftovers(&names, &listed, stale) {
        let path = dir.join(&name);
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
    if !listed.is_empty() {
        let left: Vec<String> = listed
            .into_iter()
            .filter(|n| is_aside_name(n) && dir.join(n).exists())
            .collect();
        let _ = if left.is_empty() {
            std::fs::remove_file(&list_path)
        } else {
            std::fs::write(&list_path, left.join("\n") + "\n")
        };
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
    std::fs::copy(download, &staged).map_err(|e| {
        // a copy that failed half-way (disk full) leaves no partial file
        let _ = std::fs::remove_file(&staged);
        format!("{} could not be written: {e}", staged.display())
    })?;
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
        let present = |_: &'static str, p: &std::path::Path| {
            if p.ends_with("TeamCraft.exe") {
                Existing::Ours
            } else {
                Existing::Missing
            }
        };
        let moves = swap_moves(dir, &found, present, |_| false).unwrap();
        // paths compared as paths: Path::join writes `\` on Windows
        let pairs: Vec<(std::path::PathBuf, std::path::PathBuf)> = moves
            .iter()
            .map(|m| (m.from.clone(), m.to.clone()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                (dir.join("TeamCraft.exe"), dir.join("TeamCraft.exe.old")),
                (staged("TeamCraft.exe"), dir.join("TeamCraft.exe")),
                (staged("README.txt"), dir.join("README.txt")),
            ]
        );
        assert_eq!(asides(&moves), vec!["TeamCraft.exe.old"]);
        // a zip without the app: nothing at all
        assert!(swap_moves(
            dir,
            &[("README.txt", staged("README.txt"))],
            |_, _| Existing::Ours,
            |_| false
        )
        .unwrap()
        .is_empty());
    }

    fn pairs(moves: &[Move]) -> Vec<(String, String)> {
        moves
            .iter()
            .map(|m| {
                (
                    m.from.file_name().unwrap().to_string_lossy().into_owned(),
                    m.to.file_name().unwrap().to_string_lossy().into_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn a_users_own_readme_next_to_the_app_is_left_alone() {
        let dir = std::path::Path::new("/desk");
        let staged = |n: &str| std::path::PathBuf::from(format!("/desk/s/T/{n}"));
        let found = vec![
            ("TeamCraft.exe", staged("TeamCraft.exe")),
            ("LICENSE.txt", staged("LICENSE.txt")),
            ("README.txt", staged("README.txt")),
        ];
        // README.txt is the user's, LICENSE.txt is ours: only ours moves
        let whose = |name: &'static str, _: &std::path::Path| match name {
            "README.txt" => Existing::Foreign,
            _ => Existing::Ours,
        };
        let moves = swap_moves(dir, &found, whose, |_| false).unwrap();
        assert_eq!(
            pairs(&moves),
            vec![
                ("TeamCraft.exe".into(), "TeamCraft.exe.old".into()),
                ("TeamCraft.exe".into(), "TeamCraft.exe".into()),
                ("LICENSE.txt".into(), "LICENSE.txt.old".into()),
                ("LICENSE.txt".into(), "LICENSE.txt".into()),
            ]
        );
        assert!(!moves.iter().any(|m| m.to.ends_with("README.txt")));
        // only TeamCraft.exe in the zip: the other files are not touched
        let only = swap_moves(
            dir,
            &[("TeamCraft.exe", staged("TeamCraft.exe"))],
            |_, _| Existing::Ours,
            |_| false,
        )
        .unwrap();
        assert_eq!(only.len(), 2);
        assert!(only.iter().all(|m| !m.to.ends_with("README.txt")));
        // whose file it is: by its first line
        assert!(is_ours(
            "README.txt",
            "\u{feff}TeamCraft Desktop 0.3.0 (x64) - portable\n"
        ));
        assert!(!is_ours("README.txt", "My notes\n"));
        assert!(is_ours(
            "LICENSE.txt",
            "Mozilla Public License Version 2.0\n==="
        ));
        assert!(!is_ours("LICENSE.txt", "MIT License"));
        assert!(is_ours("TeamCraft.exe", "MZ"));
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("README.txt"), b"Shopping list").unwrap();
        assert_eq!(
            existing("README.txt", &tmp.path().join("README.txt")),
            Existing::Foreign
        );
        assert_eq!(
            existing("LICENSE.txt", &tmp.path().join("LICENSE.txt")),
            Existing::Missing
        );
    }

    #[test]
    fn an_old_file_that_is_there_is_never_renamed_over() {
        let dir = std::path::Path::new("/app");
        let found = vec![(
            "TeamCraft.exe",
            std::path::PathBuf::from("/app/s/T/TeamCraft.exe"),
        )];
        let taken = |p: &std::path::Path| {
            p.ends_with("TeamCraft.exe.old") || p.ends_with("TeamCraft.exe.1.old")
        };
        let moves = swap_moves(dir, &found, |_, _| Existing::Ours, taken).unwrap();
        assert_eq!(moves[0].to, dir.join("TeamCraft.exe.2.old"));
        assert_eq!(asides(&moves), vec!["TeamCraft.exe.2.old"]);
        // every name taken: refused, nothing moves
        let err = swap_moves(dir, &found, |_, _| Existing::Ours, |_| true).unwrap_err();
        assert!(err.contains("TeamCraft.exe"), "{err}");
        assert_eq!(
            aside_name("README.txt", |_| false).as_deref(),
            Some("README.txt.old")
        );
    }

    #[test]
    fn the_unpacked_zip_is_read_one_folder_deep() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path();
        let folder = staging.join("TeamCraft_0.3.0_x64_portable");
        std::fs::create_dir_all(&folder).unwrap();
        for n in ["teamcraft.EXE", "LICENSE.txt", "README.txt", "evil.dll"] {
            std::fs::write(folder.join(n), b"x").unwrap();
        }
        let found = found_in(staging).unwrap();
        let mut names: Vec<&str> = found.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["LICENSE.txt", "README.txt", "TeamCraft.exe"]);
        // two version folders from a stale staging: refused
        let second = staging.join("TeamCraft_0.2.9_x64_portable");
        std::fs::create_dir_all(&second).unwrap();
        std::fs::write(second.join("TeamCraft.exe"), b"y").unwrap();
        assert!(found_in(staging).unwrap_err().contains("more than one"));
        // an empty or missing staging folder: nothing
        let empty = tempfile::tempdir().unwrap();
        assert!(found_in(empty.path()).unwrap().is_empty());
        assert!(found_in(&tmp.path().join("gone")).unwrap().is_empty());
    }

    #[test]
    fn a_whole_swap_runs_in_a_real_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("TeamCraft.exe"), b"old app").unwrap();
        std::fs::write(
            dir.join("README.txt"),
            b"TeamCraft Desktop 0.2.9 (x64) - portable",
        )
        .unwrap();
        std::fs::write(
            dir.join("LICENSE.txt"),
            b"Mozilla Public License Version 2.0",
        )
        .unwrap();
        let staging = dir.join(staging_name(42));
        let folder = staging.join("TeamCraft_0.3.0_x64_portable");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("TeamCraft.exe"), b"new app").unwrap();
        std::fs::write(
            folder.join("README.txt"),
            b"TeamCraft Desktop 0.3.0 (x64) - portable",
        )
        .unwrap();
        std::fs::write(
            folder.join("LICENSE.txt"),
            b"Mozilla Public License Version 2.0",
        )
        .unwrap();
        std::fs::write(folder.join(PORTABLE_MARKER), b"").unwrap();
        let found = found_in(&staging).unwrap();
        let moves = swap_moves(dir, &found, existing, |p| p.exists()).unwrap();
        run_moves(&moves).unwrap();
        record_asides(dir, &asides(&moves)).unwrap();
        assert_eq!(
            std::fs::read(dir.join("TeamCraft.exe")).unwrap(),
            b"new app"
        );
        assert_eq!(
            std::fs::read(dir.join("TeamCraft.exe.old")).unwrap(),
            b"old app"
        );
        assert!(
            dir.join(PORTABLE_MARKER).is_file(),
            "the marker comes along"
        );
        assert!(portable_folder(dir));
        let mut listed = read_list(&dir.join(ASIDE_LIST));
        listed.sort_unstable();
        assert_eq!(
            listed,
            vec!["LICENSE.txt.old", "README.txt.old", "TeamCraft.exe.old"]
        );
        // the next start: the listed files and the list go
        std::fs::remove_dir_all(&staging).unwrap();
        remove_leftovers(dir, SystemTime::now());
        assert!(!dir.join("TeamCraft.exe.old").exists());
        assert!(!dir.join("README.txt.old").exists());
        assert!(!dir.join(ASIDE_LIST).exists());
        assert!(dir.join("TeamCraft.exe").exists());
    }

    #[test]
    fn a_failed_first_move_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("TeamCraft.exe");
        std::fs::write(&app, b"old").unwrap();
        let moves = vec![Move {
            from: dir.path().join("missing.exe"),
            to: app.clone(),
        }];
        assert!(run_moves(&moves).unwrap_err().contains("TeamCraft.exe"));
        assert_eq!(std::fs::read(&app).unwrap(), b"old");
    }

    #[test]
    fn a_rollback_brings_the_app_back_over_an_old_copy() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("TeamCraft.exe");
        let old = dir.path().join("TeamCraft.exe.old");
        let new = dir.path().join("new.exe");
        std::fs::write(&app, b"app").unwrap();
        std::fs::write(&old, b"older").unwrap();
        std::fs::write(&new, b"new").unwrap();
        let moves = vec![
            Move {
                from: app.clone(),
                to: old.clone(),
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
        assert!(run_moves(&moves).is_err());
        assert_eq!(
            std::fs::read(&app).unwrap(),
            b"app",
            "the app is back as it was"
        );
        assert_eq!(std::fs::read(&new).unwrap(), b"new");
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
        let listed: Vec<String> = vec!["TeamCraft.exe.old".into(), "readme.txt.old".into()];
        assert_eq!(
            leftovers(&names, &listed, |_| true),
            vec!["TeamCraft.exe.old", "README.txt.OLD", ".teamcraft-update"]
        );
        // without the list of the swap nothing of the user's goes
        assert_eq!(leftovers(&names, &[], |_| true), vec![".teamcraft-update"]);
        // a staging folder in use (young) stays; another process's too
        let more: Vec<String> = vec![
            staging_name(7),
            ".teamcraft-update-x".into(),
            ASIDE_LIST.into(),
        ];
        assert!(leftovers(&more, &[], |_| false).is_empty());
        assert_eq!(leftovers(&more, &[], |_| true), vec![staging_name(7)]);
        // a listed name that is not an aside name of ours is never removed
        let odd: Vec<String> = vec!["notes.old".into(), "TeamCraft.exe.0.old".into()];
        assert!(leftovers(&odd, &odd, |_| true).is_empty());

        let dir = tempfile::tempdir().unwrap();
        for n in &names[..5] {
            std::fs::write(dir.path().join(n), b"x").unwrap();
        }
        // no list: the user's own README.txt.OLD and TeamCraft.exe.old stay
        remove_leftovers(dir.path(), SystemTime::now());
        assert!(dir.path().join("README.txt.OLD").exists());
        assert!(dir.path().join("TeamCraft.exe.old").exists());
        // the list of the swap names TeamCraft.exe.old only
        std::fs::write(dir.path().join(ASIDE_LIST), "TeamCraft.exe.old\n").unwrap();
        remove_leftovers(dir.path(), SystemTime::now());
        assert!(dir.path().join("TeamCraft.exe").exists());
        assert!(dir.path().join("notes.old").exists(), "not ours");
        assert!(dir.path().join("README.txt.OLD").exists(), "not listed");
        assert!(!dir.path().join("TeamCraft.exe.old").exists());
        assert!(
            !dir.path().join(ASIDE_LIST).exists(),
            "the list goes when done"
        );
    }

    #[test]
    fn a_stale_staging_folder_is_removed_with_its_contents() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir
            .path()
            .join(STAGING_DIR)
            .join("TeamCraft_0.3.0_x64_portable");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("TeamCraft.exe"), b"x").unwrap();
        let own = dir.path().join(staging_name(9));
        std::fs::create_dir_all(&own).unwrap();
        // an hour later: both are stale
        let later = SystemTime::now() + Duration::from_secs(STALE_STAGING_SECS + 60);
        remove_leftovers(dir.path(), SystemTime::now());
        assert!(own.exists(), "a fresh staging folder may be in use");
        remove_leftovers(dir.path(), later);
        assert!(!dir.path().join(STAGING_DIR).exists());
        assert!(!own.exists());
        // a listed directory named like an old app is removed too
        let old_dir = dir.path().join("TeamCraft.exe.old");
        std::fs::create_dir_all(old_dir.join("x")).unwrap();
        record_asides(dir.path(), &["TeamCraft.exe.old".to_string()]).unwrap();
        remove_leftovers(dir.path(), SystemTime::now());
        assert!(!old_dir.exists());
        // a folder that is not there: nothing happens
        remove_leftovers(&dir.path().join("gone"), SystemTime::now());
    }

    #[test]
    fn a_second_swap_adds_to_the_list() {
        let dir = tempfile::tempdir().unwrap();
        record_asides(
            dir.path(),
            &["README.txt.old".into(), "TeamCraft.exe.old".into()],
        )
        .unwrap();
        record_asides(
            dir.path(),
            &["TeamCraft.exe.OLD".into(), "TeamCraft.exe.1.old".into()],
        )
        .unwrap();
        assert_eq!(
            read_list(&dir.path().join(ASIDE_LIST)),
            vec!["README.txt.old", "TeamCraft.exe.old", "TeamCraft.exe.1.old"],
            "no duplicates"
        );
        // only what is still there stays listed
        std::fs::write(dir.path().join("TeamCraft.exe.1.old"), b"x").unwrap();
        std::fs::write(dir.path().join("keep.txt"), b"x").unwrap();
        remove_leftovers(dir.path(), SystemTime::now());
        assert!(!dir.path().join("TeamCraft.exe.1.old").exists());
        assert!(!dir.path().join(ASIDE_LIST).exists());
        assert!(dir.path().join("keep.txt").exists());
    }

    #[test]
    fn only_a_portable_copy_cleans_its_folder() {
        use InstallKind::*;
        let exe = std::path::Path::new("/apps/TeamCraft_portable/TeamCraft.exe");
        assert_eq!(
            leftover_dir(WindowsPortable, exe).as_deref(),
            Some(std::path::Path::new("/apps/TeamCraft_portable"))
        );
        for kind in [WindowsMsi, WindowsSetup, AppImage, MacDmg, Package, Other] {
            assert_eq!(leftover_dir(kind, exe), None, "{kind:?}");
        }
    }

    #[test]
    fn a_portable_folder_is_proven_by_its_marker_or_readme() {
        assert!(readme_is_portable(
            "\u{feff}TeamCraft Desktop 0.2.13 (x64) - portable\r\n\r\nUnpack …"
        ));
        assert!(!readme_is_portable("TeamCraft Desktop 0.2.13 (x64)"));
        assert!(!readme_is_portable("My README - portable"));
        let dir = tempfile::tempdir().unwrap();
        assert!(!portable_folder(dir.path()), "an exe copied anywhere");
        std::fs::write(dir.path().join("README.txt"), "Notes").unwrap();
        assert!(!portable_folder(dir.path()));
        std::fs::write(
            dir.path().join("README.txt"),
            "TeamCraft Desktop 0.2.13 (x64) - portable\n",
        )
        .unwrap();
        assert!(portable_folder(dir.path()));
        let marked = tempfile::tempdir().unwrap();
        std::fs::write(marked.path().join(PORTABLE_MARKER), "").unwrap();
        assert!(portable_folder(marked.path()));
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
        // a .new left from an earlier try: a failed copy leaves none behind
        std::fs::write(target.with_extension("new"), b"partial").unwrap();
        assert!(swap_appimage(&dir.path().join("gone"), &target).is_err());
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"new",
            "a failed swap keeps the app"
        );
        assert!(
            !target.with_extension("new").exists(),
            "no partial copy stays"
        );
        // an existing .new is simply overwritten
        std::fs::write(target.with_extension("new"), b"stale").unwrap();
        std::fs::write(&download, b"newer").unwrap();
        swap_appimage(&download, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"newer");
        // a name without an extension is staged as <name>.new
        let plain = dir.path().join("TeamCraft");
        std::fs::write(&plain, b"old").unwrap();
        swap_appimage(&download, &plain).unwrap();
        assert_eq!(std::fs::read(&plain).unwrap(), b"newer");
        assert!(!dir.path().join("TeamCraft.new").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_folder_is_refused_and_left_as_it_is() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let app_dir = dir.path().join("apps");
        std::fs::create_dir(&app_dir).unwrap();
        let target = app_dir.join("TeamCraft.AppImage");
        let download = dir.path().join("download.AppImage");
        std::fs::write(&target, b"old").unwrap();
        std::fs::write(&download, b"new").unwrap();
        std::fs::set_permissions(&app_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        // root may write anyway: then there is nothing to check
        if !writable(&app_dir) {
            assert!(swap_appimage(&download, &target)
                .unwrap_err()
                .contains("not writable"));
            assert_eq!(std::fs::read(&target).unwrap(), b"old");
            assert!(!app_dir.join(".teamcraft-write-test").exists());
        }
        std::fs::set_permissions(&app_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn the_write_probe_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        assert!(writable(dir.path()));
        assert!(!dir.path().join(".teamcraft-write-test").exists());
        assert!(!writable(&dir.path().join("gone")));
        assert!(!dir.path().join("gone").exists());
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
        // the next candidate of the same version (#493)
        assert!(is_newer("0.3.0-rc.2", "0.3.0-rc.1"));
        assert!(!is_newer("0.3.0-rc.1", "0.3.0-rc.2"), "never a downgrade");
        assert!(is_newer("0.3.0-rc.10", "0.3.0-rc.9"), "numeric identifiers");
        assert!(!is_newer("0.3.0-rc.2", "0.3.0-rc.2"));
        assert!(is_newer("0.3.0-rc.1.1", "0.3.0-rc.1"), "the longer list");
        assert!(
            is_newer("0.3.0-rc.1", "0.3.0-beta.9"),
            "words in ASCII order"
        );
        assert!(is_newer("0.3.0-alpha", "0.3.0-1"), "words above numbers");
        // build metadata never counts – also with a hyphen in it
        assert!(!is_newer("0.3.0", "0.3.0+b-1"));
        assert!(!is_newer("0.3.0+b-2", "0.3.0+b-1"));
        assert!(is_newer("0.3.0", "0.3.0-rc.1+b-2"));
        assert_eq!(
            parse("0.3.0+b-1"),
            Some(Version {
                core: [0, 3, 0],
                pre: vec![]
            })
        );
        assert_eq!(
            parse("0.3.0-rc.1+b-2"),
            Some(Version {
                core: [0, 3, 0],
                pre: vec![Ident::Text("rc".into()), Ident::Num(1)]
            })
        );
        assert_eq!(parse("0.3.0-rc..1"), None);
    }

    #[test]
    fn testers_get_the_next_candidate() {
        use serde_json::json;
        let rel = |tag: &str| json!({ "tag_name": tag, "draft": false, "prerelease": true });
        // #493: rc.1 → rc.2, whatever the order of the list
        let list = json!([rel("v0.3.0-rc.1"), rel("v0.3.0-rc.2"), rel("v0.2.13")]);
        assert_eq!(
            pick_release(&list, "0.3.0-rc.1").as_deref(),
            Some("0.3.0-rc.2")
        );
        assert!(notice_due("0.3.0-rc.2", "0.3.0-rc.1", None, None, 0));
        let nine = json!([rel("v0.3.0-rc.9"), rel("v0.3.0-rc.10"), rel("v0.3.0-rc.2")]);
        assert_eq!(
            pick_release(&nine, "0.3.0-rc.1").as_deref(),
            Some("0.3.0-rc.10")
        );
        // a newer plain release beats every candidate
        let plain = json!([rel("v0.3.0-rc.2"), rel("v0.3.1")]);
        assert_eq!(pick_release(&plain, "0.3.0-rc.1").as_deref(), Some("0.3.1"));
        // a user never gets a candidate, even first in the list
        let first = json!([rel("v0.4.0-rc.1"), rel("v0.3.0")]);
        assert_eq!(pick_release(&first, "0.2.13").as_deref(), Some("0.3.0"));
        // numbers, not text; a tag without "v"; build metadata is no candidate
        let nums = json!([rel("v0.9.0"), rel("0.10.0")]);
        assert_eq!(pick_release(&nums, "0.2.13").as_deref(), Some("0.10.0"));
        let meta = json!([rel("v0.3.0+b-1"), rel("v0.2.13")]);
        assert_eq!(pick_release(&meta, "0.2.13").as_deref(), Some("0.3.0+b-1"));
        // among equal versions the first in the list wins
        let same = json!([rel("v0.3.0+b-2"), rel("v0.3.0+b-1")]);
        assert_eq!(pick_release(&same, "0.2.13").as_deref(), Some("0.3.0+b-2"));
        let same = json!([rel("v0.3.0+b-1"), rel("v0.3.0+b-2")]);
        assert_eq!(pick_release(&same, "0.2.13").as_deref(), Some("0.3.0+b-1"));
    }

    #[test]
    fn the_release_page_to_open() {
        assert_eq!(release_url(None), RELEASES_PAGE);
        assert_eq!(release_url(Some("nightly")), RELEASES_PAGE);
        assert_eq!(release_url(Some("0.3.0<script>")), RELEASES_PAGE);
        assert_eq!(
            release_url(Some("0.2")),
            RELEASES_PAGE,
            "major.minor is no tag"
        );
        let tag = "https://github.com/WARR-eV/TeamCraft/releases/tag/v0.3.0";
        assert_eq!(release_url(Some("v0.3.0")), tag);
        assert_eq!(release_url(Some("0.3.0")), tag);
        assert_eq!(
            release_url(Some("0.3.0-rc.1")),
            "https://github.com/WARR-eV/TeamCraft/releases/tag/v0.3.0-rc.1"
        );
    }

    #[test]
    fn the_notice_from_the_last_known_release() {
        let now = 1_000_000;
        assert_eq!(notice_for(None, "0.2.0", None, None, now), None);
        assert_eq!(
            notice_for(Some("0.3.0"), "0.2.0", None, None, now).as_deref(),
            Some("0.3.0")
        );
        // the app was updated since the last check
        assert_eq!(notice_for(Some("0.3.0"), "0.3.0", None, None, now), None);
        assert_eq!(notice_for(Some("0.2.9"), "0.3.0", None, None, now), None);
        assert_eq!(
            notice_for(Some("0.3.0"), "0.2.0", Some("0.3.0"), None, now),
            None
        );
        assert_eq!(
            notice_for(Some("0.3.0"), "0.2.0", None, Some(now + 1), now),
            None
        );
        // a tester's next candidate
        assert_eq!(
            notice_for(Some("0.3.0-rc.2"), "0.3.0-rc.1", None, None, now).as_deref(),
            Some("0.3.0-rc.2")
        );
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
        let kind = |os, exe, appimage, flatpak| install_kind(os, exe, appimage, flatpak, false);
        assert_eq!(
            kind(
                "windows",
                r"C:\Program Files\TeamCraft\TeamCraft.exe",
                false,
                false
            ),
            WindowsMsi
        );
        assert_eq!(
            kind(
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
                false,
                true
            ),
            WindowsPortable
        );
        // #466: a Windows folder without the proof of a portable zip (an MSI
        // installed to D:\Apps, an exe copied to the desktop) is no portable
        assert_eq!(
            kind("windows", r"D:\Apps\TeamCraft\TeamCraft.exe", false, false),
            Other
        );
        assert_eq!(
            kind(
                "windows",
                r"C:\Users\kim\Desktop\TeamCraft.exe",
                false,
                false
            ),
            Other
        );
        assert_eq!(
            install_kind(
                "windows",
                r"C:\Program Files\TeamCraft\TeamCraft.exe",
                false,
                false,
                true
            ),
            WindowsMsi,
            "an installer's folder never"
        );
        assert_eq!(
            kind(
                "macos",
                "/Applications/TeamCraft.app/Contents/MacOS/teamcraft-desktop",
                false,
                false
            ),
            MacDmg
        );
        assert_eq!(
            kind(
                "linux",
                "/tmp/.mount_TeamCr/usr/bin/teamcraft-desktop",
                true,
                false
            ),
            AppImage
        );
        assert_eq!(
            kind("linux", "/usr/bin/teamcraft-desktop", false, false),
            Package
        );
        assert_eq!(
            kind("linux", "/app/bin/teamcraft-desktop", false, true),
            Package
        );
        assert_eq!(
            kind(
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
