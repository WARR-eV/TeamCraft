//! Anonymous usage counts for the user's own TeamCraft server (#383).
//!
//! The app counts a fixed list of actions ([`ACTIONS`], the server's
//! `services::usage::ACTIONS` – a server test compares the two) and the
//! minutes its window had the focus, per day. When a day is over, its report
//! goes to the server the app is signed in to (`POST /api/v1/usage`) – the
//! organisation's own server, which shares an aggregate with the TeamCraft
//! developers only if its administrator turns that on. The report holds
//! exactly the fields of [`report`]: a random installation id (made here,
//! not derived from anything), versions, OS, architecture, package type,
//! active minutes and counts – never names, numbers, paths or anything
//! typed. *Settings → Privacy → Show what is sent* shows the report being
//! counted and the one sent last.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The actions counted – nothing else is ever counted or sent.
pub const ACTIONS: &[&str] = &[
    "checkout",
    "checkin",
    "park",
    "cancel_checkout",
    "get_files",
    "open_cad",
    "import_bom",
    "import_folder",
    "new_item",
    "release",
    "open_3d_preview",
    "search",
    "assistant",
];

/// The CAD programs whose STEP exports are counted.
pub const CAD_EXPORTS: &[&str] = &["inventor", "solid-edge", "solidcraft"];

/// The counts of one day, kept in the settings until they are sent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Random id of this installation (a UUID made here).
    #[serde(default)]
    pub install_id: Option<String>,
    /// The day being counted (`YYYY-MM-DD`).
    #[serde(default)]
    pub day: Option<String>,
    #[serde(default)]
    pub active_minutes: u32,
    #[serde(default)]
    pub actions: BTreeMap<String, u64>,
    #[serde(default)]
    pub cad_exports: BTreeMap<String, u64>,
    /// A finished day's report waiting to be sent.
    #[serde(default)]
    pub pending: Option<Value>,
    /// The report sent last (*Show what is sent*).
    #[serde(default)]
    pub last_sent: Option<Value>,
}

/// A new random installation id (version 4 UUID from the OS random source).
pub fn new_install_id() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex::encode(b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// The package type of this installation, as the report names it:
/// `appimage`, `flatpak`, `msi`, `setup`, `portable`, `app`, `package`
/// (`.deb`/`.rpm`) or `other` (built from source).
pub fn package() -> &'static str {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().replace('\\', "/").to_ascii_lowercase())
        .unwrap_or_default();
    package_of(
        std::env::consts::OS,
        &exe,
        std::env::var_os("APPIMAGE").is_some(),
        std::env::var_os("FLATPAK_ID").is_some(),
    )
}

fn package_of(os: &str, exe: &str, appimage: bool, flatpak: bool) -> &'static str {
    match os {
        _ if exe.contains("/target/") => "other",
        "windows" if exe.contains("/program files") => "msi",
        "windows" if exe.contains("/appdata/local/") => "setup",
        "windows" => "portable",
        "macos" if exe.contains(".app/contents/macos/") => "app",
        "linux" if flatpak => "flatpak",
        "linux" if appimage => "appimage",
        "linux" if exe.starts_with("/usr/") || exe.starts_with("/opt/") => "package",
        _ => "other",
    }
}

/// Today in UTC as `YYYY-MM-DD` (civil date from the days since 1970).
pub fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_date(secs / 86_400)
}

fn civil_date(days: u64) -> String {
    // Howard Hinnant's days_from_civil, inverted
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

impl Usage {
    /// Starts `today`: a finished day becomes the pending report (an older
    /// pending one that was never sent is replaced).
    pub fn roll(&mut self, today: &str, package: &str, server_version: Option<&str>) {
        if self.install_id.is_none() {
            self.install_id = Some(new_install_id());
        }
        if self.day.as_deref() == Some(today) {
            return;
        }
        let used = self.active_minutes > 0 || !self.actions.is_empty();
        if self.day.is_some() && used {
            let finished = report(self, package, server_version);
            self.pending = Some(finished);
        }
        self.reset(today);
    }

    fn reset(&mut self, today: &str) {
        self.day = Some(today.to_string());
        self.active_minutes = 0;
        self.actions.clear();
        self.cad_exports.clear();
    }

    /// Counts one of the [`ACTIONS`]; other names are ignored.
    pub fn count(&mut self, action: &str) {
        if ACTIONS.contains(&action) {
            *self.actions.entry(action.to_string()).or_default() += 1;
        }
    }

    /// Counts a STEP export with one of the [`CAD_EXPORTS`].
    pub fn count_export(&mut self, program: &str) {
        if CAD_EXPORTS.contains(&program) {
            *self.cad_exports.entry(program.to_string()).or_default() += 1;
        }
    }

    /// One more minute with the window in front (at most a day's worth).
    pub fn tick(&mut self) {
        self.active_minutes = (self.active_minutes + 1).min(1440);
    }
}

/// The report of the day counted in `u` – exactly these fields.
pub fn report(u: &Usage, package: &str, server_version: Option<&str>) -> Value {
    let os = match std::env::consts::OS {
        os @ ("windows" | "macos" | "linux") => os,
        _ => "other",
    };
    let arch = match std::env::consts::ARCH {
        arch @ ("x86_64" | "aarch64") => arch,
        _ => "other",
    };
    json!({
        "v": 1,
        "install_id": u.install_id,
        "day": u.day,
        "version": env!("CARGO_PKG_VERSION"),
        "os": os,
        "arch": arch,
        "package": package,
        "server_version": server_version,
        "active_minutes": u.active_minutes,
        "actions": u.actions,
        "cad_exports": u.cad_exports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_has_exactly_the_documented_fields() {
        let mut u = Usage::default();
        u.roll("2026-11-21", "msi", Some("0.2.9"));
        u.count("checkin");
        u.count("checkin");
        u.count("not_an_action");
        u.count_export("inventor");
        u.count_export("catia");
        u.tick();
        let r = report(&u, "msi", Some("0.2.9"));
        let mut keys: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "actions",
                "active_minutes",
                "arch",
                "cad_exports",
                "day",
                "install_id",
                "os",
                "package",
                "server_version",
                "v",
                "version"
            ]
        );
        assert_eq!(
            r["actions"],
            json!({ "checkin": 2 }),
            "unknown names are never counted"
        );
        assert_eq!(r["cad_exports"], json!({ "inventor": 1 }));
        assert_eq!(r["active_minutes"], 1);
        assert_eq!(r["install_id"].as_str().unwrap().len(), 36);
    }

    #[test]
    fn a_finished_day_becomes_the_pending_report() {
        let mut u = Usage::default();
        u.roll("2026-11-20", "deb", None);
        assert!(u.pending.is_none());
        u.count("checkout");
        u.roll("2026-11-20", "deb", None);
        assert!(u.pending.is_none(), "the same day");
        u.roll("2026-11-21", "deb", None);
        let pending = u.pending.clone().unwrap();
        assert_eq!(pending["day"], "2026-11-20");
        assert_eq!(pending["actions"], json!({ "checkout": 1 }));
        assert!(u.actions.is_empty() && u.day.as_deref() == Some("2026-11-21"));
        // a day without use sends nothing
        u.pending = None;
        u.roll("2026-11-22", "deb", None);
        assert!(u.pending.is_none());
    }

    #[test]
    fn install_ids_are_random_v4_uuids() {
        let a = new_install_id();
        assert_ne!(a, new_install_id());
        assert_eq!(&a[14..15], "4");
        assert!(a.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    }

    #[test]
    fn every_action_is_counted_and_unknown_names_are_dropped() {
        let mut u = Usage::default();
        u.roll("2026-11-21", "deb", None);
        for a in ACTIONS {
            u.count(a);
        }
        for unknown in ["", "Checkout", "export", "search terms", "checkout "] {
            u.count(unknown);
        }
        assert_eq!(u.actions.len(), ACTIONS.len());
        assert!(ACTIONS.iter().all(|a| u.actions.get(*a) == Some(&1)));
        let r = report(&u, "deb", None);
        assert_eq!(r["actions"].as_object().unwrap().len(), ACTIONS.len());
    }

    /// Every action of [`ACTIONS`] has a place in the app that counts it:
    /// `u.count("…")` in the commands or `usage_count` with `action: "…"`
    /// in the UI – a listed action nobody counts would always report zero.
    #[test]
    fn every_action_has_a_count_site() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut sources = vec![std::fs::read_to_string(dir.join("src/commands.rs")).unwrap()];
        for entry in std::fs::read_dir(dir.join("ui/js")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) == Some("js") {
                sources.push(std::fs::read_to_string(path).unwrap());
            }
        }
        for a in ACTIONS {
            let rust = format!("u.count(\"{a}\")");
            let js = format!("call(\"usage_count\", {{ action: \"{a}\" }})");
            assert!(
                sources.iter().any(|s| s.contains(&rust) || s.contains(&js)),
                "nothing counts the usage action {a}"
            );
        }
    }

    #[test]
    fn minutes_stop_at_a_day() {
        let mut u = Usage {
            active_minutes: 1440,
            ..Usage::default()
        };
        u.tick();
        assert_eq!(u.active_minutes, 1440);
    }
}

#[cfg(test)]
mod date_tests {
    use super::*;

    #[test]
    fn civil_dates_and_packages() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(20_413), "2025-11-21");
        assert_eq!(civil_date(11_016), "2000-02-29");
        assert_eq!(
            package_of(
                "windows",
                "c:/program files/teamcraft/teamcraft.exe",
                false,
                false
            ),
            "msi"
        );
        assert_eq!(
            package_of(
                "windows",
                "c:/users/kim/appdata/local/teamcraft/t.exe",
                false,
                false
            ),
            "setup"
        );
        assert_eq!(
            package_of("linux", "/tmp/.mount_x/usr/bin/t", true, false),
            "appimage"
        );
        assert_eq!(
            package_of("linux", "/usr/bin/teamcraft-desktop", false, false),
            "package"
        );
        assert_eq!(
            package_of("linux", "/home/kim/desktop/target/debug/t", false, false),
            "other"
        );
    }
}
