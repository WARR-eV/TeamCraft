//! Usage statistics (#383).
//!
//! * The **desktop apps** report one small, anonymous report per day of use to
//!   their own TeamCraft server (`POST /api/v1/usage`): a random installation
//!   id, versions, OS, package type, active minutes and counts of the fixed
//!   [`ACTIONS`] and [`CAD_EXPORTS`]. Nothing else is accepted: unknown
//!   fields are refused, unknown action names dropped. Kept in
//!   `usage_reports` for [`KEEP_DAYS`] days; *Admin → Usage statistics*
//!   shows the aggregate.
//! * The **server** shares an aggregate with the TeamCraft developers only
//!   when an administrator turns it on (off by default) or
//!   `TEAMCRAFT_USAGE_STATISTICS=1` fixes it on – and only when a destination
//!   is configured (`TEAMCRAFT_USAGE_STATISTICS_URL`). `DO_NOT_TRACK=1`,
//!   `TEAMCRAFT_NO_TELEMETRY=1`, the local test server and a live demo never
//!   send. At most once a day, 10 s timeout, no retries before the next day.
//!
//! Never content: no user, item, file, group or server names, numbers,
//! paths, search terms or anything typed. Sizes are sent in buckets
//! ([`bucket`]).

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::Actor,
    config::{Config, UsageConfig},
    domain::Role,
    error::{AppError, AppResult},
    services::audit,
};

/// The action names a desktop report may count – the one list, checked by a
/// test against the desktop app's own (`desktop/src/usage.rs`).
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

/// The CAD programs whose STEP exports a desktop report counts.
pub const CAD_EXPORTS: &[&str] = &["inventor", "solid-edge", "solidcraft"];

/// Desktop reports are kept this long.
pub const KEEP_DAYS: i64 = 400;
/// A report may be this many days old when it arrives (sent late).
const MAX_AGE_DAYS: i64 = 7;
/// Largest count of one action per day.
const MAX_COUNT: i64 = 100_000;

/// A desktop report, checked.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DesktopReport {
    pub day: NaiveDate,
    pub install_id: Uuid,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub package: String,
    pub active_minutes: i32,
    pub actions: BTreeMap<String, i64>,
    pub cad_exports: BTreeMap<String, i64>,
}

/// The fields a desktop report has – exactly these.
const FIELDS: &[&str] = &[
    "v",
    "install_id",
    "day",
    "version",
    "os",
    "arch",
    "package",
    "server_version",
    "active_minutes",
    "actions",
    "cad_exports",
];

fn bad(why: &str) -> AppError {
    AppError::BadRequest(format!("Not a usage report: {why}."))
}

/// A short token (`0.2.9`, `msi`): letters, digits, `.`, `-`, `+`, `_`.
fn token(value: &Value, name: &str, max: usize) -> AppResult<String> {
    let s = value
        .as_str()
        .ok_or_else(|| bad(&format!("{name} is not text")))?;
    let fits = !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+' | b'_'));
    if fits {
        Ok(s.to_string())
    } else {
        Err(bad(&format!("{name} is not a short name")))
    }
}

/// Counts of known names; unknown names are dropped, zero counts left out.
fn counts(value: &Value, known: &[&str]) -> AppResult<BTreeMap<String, i64>> {
    let Some(map) = value.as_object() else {
        return Err(bad("counts are not an object"));
    };
    let mut out = BTreeMap::new();
    for (name, n) in map {
        let n = n
            .as_i64()
            .filter(|n| (0..=MAX_COUNT).contains(n))
            .ok_or_else(|| bad("a count is not a number"))?;
        if n > 0 && known.contains(&name.as_str()) {
            out.insert(name.clone(), n);
        }
    }
    Ok(out)
}

/// Checks a desktop report: exactly the documented fields, version 1, a day
/// in the last [`MAX_AGE_DAYS`] days, known OS and architecture.
pub fn check_report(report: &Value, today: NaiveDate) -> AppResult<DesktopReport> {
    let Some(map) = report.as_object() else {
        return Err(bad("not an object"));
    };
    if let Some(extra) = map.keys().find(|k| !FIELDS.contains(&k.as_str())) {
        return Err(bad(&format!("unknown field {extra}")));
    }
    if map.get("v").and_then(Value::as_i64) != Some(1) {
        return Err(bad("unknown version"));
    }
    let install_id = map
        .get("install_id")
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| bad("install_id is not a UUID"))?;
    let day = map
        .get("day")
        .and_then(Value::as_str)
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .ok_or_else(|| bad("day is not a date"))?;
    if day > today || (today - day).num_days() > MAX_AGE_DAYS {
        return Err(bad("day is out of range"));
    }
    let os = token(map.get("os").unwrap_or(&Value::Null), "os", 16)?;
    let os = if ["windows", "macos", "linux"].contains(&os.as_str()) {
        os
    } else {
        "other".to_string()
    };
    let arch = token(map.get("arch").unwrap_or(&Value::Null), "arch", 16)?;
    let arch = if ["x86_64", "aarch64"].contains(&arch.as_str()) {
        arch
    } else {
        "other".to_string()
    };
    let active_minutes = map
        .get("active_minutes")
        .and_then(Value::as_i64)
        .filter(|m| (0..=1440).contains(m))
        .ok_or_else(|| bad("active_minutes is out of range"))? as i32;
    let empty = Value::Object(Map::new());
    Ok(DesktopReport {
        day,
        install_id,
        version: token(map.get("version").unwrap_or(&Value::Null), "version", 32)?,
        os,
        arch,
        package: token(map.get("package").unwrap_or(&Value::Null), "package", 32)?,
        active_minutes,
        actions: counts(map.get("actions").unwrap_or(&empty), ACTIONS)?,
        cad_exports: counts(map.get("cad_exports").unwrap_or(&empty), CAD_EXPORTS)?,
    })
}

/// Stores a desktop report (a second one for the same day replaces the
/// first). Any signed-in user may report; nothing names the user.
pub async fn record(pool: &PgPool, report: &DesktopReport) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO usage_reports
            (day, install_id, version, os, arch, package, active_minutes, actions, cad_exports)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (day, install_id) DO UPDATE
            SET version = EXCLUDED.version, os = EXCLUDED.os, arch = EXCLUDED.arch,
                package = EXCLUDED.package, active_minutes = EXCLUDED.active_minutes,
                actions = EXCLUDED.actions, cad_exports = EXCLUDED.cad_exports,
                received_at = now()",
    )
    .bind(report.day)
    .bind(report.install_id)
    .bind(&report.version)
    .bind(&report.os)
    .bind(&report.arch)
    .bind(&report.package)
    .bind(report.active_minutes)
    .bind(serde_json::to_value(&report.actions).unwrap_or_default())
    .bind(serde_json::to_value(&report.cad_exports).unwrap_or_default())
    .execute(pool)
    .await?;
    Ok(())
}

/// A number as a size class: `0`, `1-10`, `11-100`, `101-1000`,
/// `1001-10000`, `10001+`.
pub fn bucket(n: i64) -> &'static str {
    match n {
        i64::MIN..=0 => "0",
        1..=10 => "1-10",
        11..=100 => "11-100",
        101..=1000 => "101-1000",
        1001..=10000 => "1001-10000",
        _ => "10001+",
    }
}

/// The desktop reports of the last `days` days, summed up.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DesktopSummary {
    pub days: i64,
    pub installations: i64,
    pub active_minutes: i64,
    /// Installations per app version, OS and package.
    pub versions: BTreeMap<String, i64>,
    pub os: BTreeMap<String, i64>,
    pub packages: BTreeMap<String, i64>,
    pub actions: BTreeMap<String, i64>,
    pub cad_exports: BTreeMap<String, i64>,
}

#[derive(sqlx::FromRow)]
struct Row {
    install_id: Uuid,
    version: String,
    os: String,
    package: String,
    active_minutes: i32,
    actions: Value,
    cad_exports: Value,
}

/// Sums the rows of one period: an installation counts once, with the
/// version of its newest report (rows come newest first).
fn summarize(rows: &[Row], days: i64) -> DesktopSummary {
    let mut s = DesktopSummary {
        days,
        ..DesktopSummary::default()
    };
    let mut seen = std::collections::HashSet::new();
    for r in rows {
        s.active_minutes += i64::from(r.active_minutes);
        for (map, field) in [
            (&mut s.actions, &r.actions),
            (&mut s.cad_exports, &r.cad_exports),
        ] {
            if let Some(obj) = field.as_object() {
                for (k, v) in obj {
                    *map.entry(k.clone()).or_default() += v.as_i64().unwrap_or(0);
                }
            }
        }
        if seen.insert(r.install_id) {
            *s.versions.entry(r.version.clone()).or_default() += 1;
            *s.os.entry(r.os.clone()).or_default() += 1;
            *s.packages.entry(r.package.clone()).or_default() += 1;
        }
    }
    s.installations = seen.len() as i64;
    s
}

/// The desktop reports of the last `days` days (*Admin → Usage statistics*).
pub async fn desktop_summary(pool: &PgPool, days: i64) -> AppResult<DesktopSummary> {
    let rows = sqlx::query_as::<_, Row>(
        "SELECT install_id, version, os, package, active_minutes, actions, cad_exports
           FROM usage_reports
          WHERE day > CURRENT_DATE - $1::int
          ORDER BY day DESC",
    )
    .bind(days as i32)
    .fetch_all(pool)
    .await?;
    Ok(summarize(&rows, days))
}

/// Whether the aggregate leaves the server, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Sharing {
    /// Sharing is on (the setting or `TEAMCRAFT_USAGE_STATISTICS=1`).
    pub on: bool,
    /// Why the administrator cannot change it here, if so.
    pub fixed_by: Option<&'static str>,
    /// A destination is configured: without one nothing is sent.
    pub destination: bool,
}

impl Sharing {
    /// Something is sent (on, and somewhere to send it).
    pub fn sends(&self) -> bool {
        self.on && self.destination
    }
}

/// The effective switch: never with `DO_NOT_TRACK`, `TEAMCRAFT_NO_TELEMETRY`
/// or a live demo; else `TEAMCRAFT_USAGE_STATISTICS`; else the setting
/// (off by default).
pub fn sharing(usage: &UsageConfig, demo: bool, setting: bool) -> Sharing {
    let (on, fixed_by) = if usage.never {
        (false, Some("DO_NOT_TRACK or TEAMCRAFT_NO_TELEMETRY"))
    } else if demo {
        (false, Some("the live demo"))
    } else if let Some(forced) = usage.forced {
        (forced, Some("TEAMCRAFT_USAGE_STATISTICS"))
    } else {
        (setting, None)
    };
    Sharing {
        on,
        fixed_by,
        destination: usage.url.is_some(),
    }
}

#[derive(sqlx::FromRow)]
struct SharingRow {
    share: bool,
    instance_id: Uuid,
    last_sent_at: Option<chrono::DateTime<Utc>>,
    last_sent: Option<Value>,
}

async fn sharing_row(pool: &PgPool) -> AppResult<SharingRow> {
    sqlx::query(
        "INSERT INTO usage_sharing (id, instance_id) VALUES (true, gen_random_uuid())
         ON CONFLICT (id) DO NOTHING",
    )
    .execute(pool)
    .await?;
    Ok(sqlx::query_as::<_, SharingRow>(
        "SELECT share, instance_id, last_sent_at, last_sent FROM usage_sharing",
    )
    .fetch_one(pool)
    .await?)
}

/// The state for the admin page.
#[derive(Clone, Debug, Serialize)]
pub struct Overview {
    pub sharing: Sharing,
    pub desktop: DesktopSummary,
    /// The exact JSON that would be sent now (*Show what is sent*).
    pub next: Value,
    /// The JSON sent last, and when.
    pub last: Option<Value>,
    pub last_sent_at: Option<chrono::DateTime<Utc>>,
}

pub async fn overview(pool: &PgPool, config: &Config) -> AppResult<Overview> {
    let row = sharing_row(pool).await?;
    Ok(Overview {
        sharing: sharing(&config.usage, config.demo.is_some(), row.share),
        desktop: desktop_summary(pool, 30).await?,
        next: server_report(pool, config, row.instance_id).await?,
        last: row.last_sent,
        last_sent_at: row.last_sent_at,
    })
}

/// *Share aggregated usage statistics with the TeamCraft developers*
/// (administrators; refused while the environment fixes it).
pub async fn set_share(pool: &PgPool, config: &Config, actor: &Actor, on: bool) -> AppResult<()> {
    actor.require(Role::Admin)?;
    if let Some(by) = sharing(&config.usage, config.demo.is_some(), false).fixed_by {
        return Err(AppError::BadRequest(format!(
            "Sharing is set by {by} on this server and cannot be changed here."
        )));
    }
    sharing_row(pool).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE usage_sharing SET share = $1, updated_by = $2, updated_at = now()")
        .bind(on)
        .bind(actor.id)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        Some(actor),
        "usage.sharing",
        "usage",
        None::<Uuid>,
        if on {
            "Turned on sharing usage statistics with the developers"
        } else {
            "Turned off sharing usage statistics with the developers"
        },
        json!({ "share": on }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// How this server was installed: Docker, or a binary/package.
fn install_type() -> &'static str {
    if std::path::Path::new("/.dockerenv").exists() {
        "docker"
    } else {
        "binary"
    }
}

/// The exact report the server would share: its own facts (random instance
/// id, versions, sizes in buckets, optional features on/off) and the
/// desktop summary of the last day with installations in buckets.
pub async fn server_report(pool: &PgPool, config: &Config, instance_id: Uuid) -> AppResult<Value> {
    let (items, files, users, pg, webhooks): (i64, i64, i64, i32, bool) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM items), (SELECT count(*) FROM files),
                (SELECT count(*) FROM users WHERE active),
                current_setting('server_version_num')::int / 10000,
                EXISTS (SELECT 1 FROM webhooks)",
    )
    .fetch_one(pool)
    .await?;
    // yesterday and today: a day's reports arrive the day after
    let desktop = desktop_summary(pool, 2).await?;
    let in_buckets = |m: &BTreeMap<String, i64>| -> Map<String, Value> {
        m.iter()
            .map(|(k, v)| (k.clone(), Value::from(bucket(*v))))
            .collect()
    };
    Ok(json!({
        "v": 1,
        "instance_id": instance_id,
        "day": Utc::now().date_naive().to_string(),
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "install": install_type(),
        "postgres": pg,
        "items": bucket(items),
        "files": bucket(files),
        "users": bucket(users),
        "features": {
            "oidc": config.oidc.is_some(),
            "assistant": config.assistant.is_some(),
            "webhooks": webhooks,
            "mattermost": config.mattermost.is_some(),
            "smtp": config.smtp.is_some(),
        },
        "desktop": {
            "installations": bucket(desktop.installations),
            "versions": in_buckets(&desktop.versions),
            "os": in_buckets(&desktop.os),
            "packages": in_buckets(&desktop.packages),
            "actions": desktop.actions,
            "cad_exports": desktop.cad_exports,
        },
    }))
}

/// Once an hour: drops old desktop reports and, when sharing sends and the
/// last report is a day old, sends the aggregate (10 s, no redirects, no
/// retry before the next day; a failure is only logged).
pub fn spawn(pool: PgPool, config: std::sync::Arc<Config>) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .user_agent(concat!("TeamCraft/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build();
        let Ok(client) = client else { return };
        let mut tick = tokio::time::interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            if let Err(e) = run_once(&pool, &config, &client).await {
                tracing::warn!(error = %e, "usage statistics");
            }
        }
    });
}

async fn run_once(pool: &PgPool, config: &Config, client: &reqwest::Client) -> AppResult<()> {
    sqlx::query("DELETE FROM usage_reports WHERE day < CURRENT_DATE - $1::int")
        .bind(KEEP_DAYS as i32)
        .execute(pool)
        .await?;
    let row = sharing_row(pool).await?;
    let state = sharing(&config.usage, config.demo.is_some(), row.share);
    let due = row
        .last_sent_at
        .is_none_or(|t| Utc::now() - t >= chrono::Duration::hours(24));
    let (true, true, Some(url)) = (state.sends(), due, config.usage.url.as_deref()) else {
        return Ok(());
    };
    let report = server_report(pool, config, row.instance_id).await?;
    // stamped before sending: a failure waits for the next day
    sqlx::query("UPDATE usage_sharing SET last_sent_at = now(), last_sent = $1")
        .bind(&report)
        .execute(pool)
        .await?;
    if let Err(e) = client
        .post(url)
        .json(&report)
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        tracing::info!(error = %e, "usage statistics not delivered");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 21).unwrap()
    }

    fn report() -> Value {
        json!({
            "v": 1,
            "install_id": "6f1c2a3b-4d5e-4f60-8a7b-9c0d1e2f3a4b",
            "day": "2026-11-21",
            "version": "0.2.9",
            "os": "windows", "arch": "x86_64",
            "package": "msi",
            "server_version": "0.2.9",
            "active_minutes": 37,
            "actions": { "checkout": 4, "checkin": 3, "import_bom": 1, "open_3d_preview": 7, "secret_thing": 9 },
            "cad_exports": { "inventor": 2, "catia": 1 }
        })
    }

    #[test]
    fn the_documented_report_is_read_and_unknown_actions_dropped() {
        let r = check_report(&report(), today()).unwrap();
        assert_eq!(r.version, "0.2.9");
        assert_eq!(r.active_minutes, 37);
        assert_eq!(r.actions.len(), 4);
        assert!(!r.actions.contains_key("secret_thing"));
        assert_eq!(r.cad_exports.keys().collect::<Vec<_>>(), vec!["inventor"]);
    }

    #[test]
    fn anything_but_the_documented_fields_is_refused() {
        let mut extra = report();
        extra["user"] = json!("kim");
        assert!(check_report(&extra, today()).is_err(), "an unknown field");
        let mut path = report();
        path["version"] = json!("C:/Users/kim");
        assert!(check_report(&path, today()).is_err(), "not a short name");
        let mut future = report();
        future["day"] = json!("2026-11-22");
        assert!(check_report(&future, today()).is_err());
        let mut old = report();
        old["day"] = json!("2026-11-01");
        assert!(check_report(&old, today()).is_err());
        let mut minutes = report();
        minutes["active_minutes"] = json!(1441);
        assert!(check_report(&minutes, today()).is_err());
        let mut v2 = report();
        v2["v"] = json!(2);
        assert!(check_report(&v2, today()).is_err());
        let mut negative = report();
        negative["actions"]["checkin"] = json!(-1);
        assert!(check_report(&negative, today()).is_err());
        let mut os = report();
        os["os"] = json!("freebsd");
        assert_eq!(check_report(&os, today()).unwrap().os, "other");
    }

    #[test]
    fn sizes_are_buckets() {
        assert_eq!(bucket(0), "0");
        assert_eq!(bucket(1), "1-10");
        assert_eq!(bucket(10), "1-10");
        assert_eq!(bucket(11), "11-100");
        assert_eq!(bucket(1000), "101-1000");
        assert_eq!(bucket(10000), "1001-10000");
        assert_eq!(bucket(10001), "10001+");
    }

    #[test]
    fn sharing_is_off_until_turned_on_and_never_with_do_not_track() {
        let url = Some("https://stats.example.org/v1".to_string());
        let open = UsageConfig {
            url: url.clone(),
            ..UsageConfig::default()
        };
        assert!(!sharing(&open, false, false).on, "off by default");
        assert!(sharing(&open, false, true).sends());
        let nowhere = UsageConfig::default();
        assert!(sharing(&nowhere, false, true).on && !sharing(&nowhere, false, true).sends());
        let forced = UsageConfig {
            forced: Some(false),
            url: url.clone(),
            ..UsageConfig::default()
        };
        let s = sharing(&forced, false, true);
        assert!(!s.on && s.fixed_by == Some("TEAMCRAFT_USAGE_STATISTICS"));
        let never = UsageConfig {
            forced: Some(true),
            never: true,
            url: url.clone(),
        };
        assert!(!sharing(&never, false, true).sends(), "DO_NOT_TRACK wins");
        assert!(
            !sharing(&open, true, true).sends(),
            "the live demo never sends"
        );
    }

    #[test]
    fn installations_count_once() {
        let id = Uuid::new_v4();
        let row = |id: Uuid, version: &str, n: i64| Row {
            install_id: id,
            version: version.into(),
            os: "linux".into(),
            package: "deb".into(),
            active_minutes: 10,
            actions: json!({ "checkin": n }),
            cad_exports: json!({}),
        };
        let s = summarize(
            &[
                row(id, "0.3.0", 2),
                row(id, "0.2.9", 3),
                row(Uuid::new_v4(), "0.2.9", 1),
            ],
            30,
        );
        assert_eq!(s.installations, 2);
        assert_eq!(
            s.versions.get("0.3.0"),
            Some(&1),
            "the newest report's version"
        );
        assert_eq!(s.versions.get("0.2.9"), Some(&1));
        assert_eq!(s.actions.get("checkin"), Some(&6));
        assert_eq!(s.active_minutes, 30);
    }
}

#[cfg(test)]
mod desktop_list {
    /// The desktop app counts exactly the server's actions and programs.
    #[test]
    fn the_desktop_app_counts_the_same_names() {
        let desktop = include_str!("../../desktop/src/usage.rs");
        for name in super::ACTIONS.iter().chain(super::CAD_EXPORTS) {
            assert!(
                desktop.contains(&format!("\"{name}\",")),
                "{name} missing in desktop/src/usage.rs"
            );
        }
        let start = desktop.find("pub const ACTIONS").unwrap();
        let list = &desktop[start..desktop[start..].find("];").unwrap() + start];
        assert_eq!(
            list.matches("\",").count(),
            super::ACTIONS.len(),
            "the same number of actions"
        );
    }
}
