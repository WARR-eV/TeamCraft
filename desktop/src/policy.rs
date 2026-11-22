//! Managed deployment policies (#378): settings an IT department fixes for
//! every computer (or user) with the operating system's usual tools. The app
//! only reads them – once at start – and never sends them anywhere.
//!
//! | Policy               | Value                                                        |
//! |----------------------|--------------------------------------------------------------|
//! | `ServerUrl`          | the TeamCraft server (`https://…`); other servers are refused |
//! | `AllowOtherServers`  | 1: other servers may be added although `ServerUrl` is set     |
//! | `CheckForUpdates`    | 1: on, cannot be turned off; 0: no checks at all              |
//! | `CrashReportPrompt`  | 0: reports stay on this computer, never offered for sharing; 1: always offered |
//! | `WorkspaceRoot`      | default working-folder root (an absolute path)               |
//! | `OrganizationName`   | shown as *Managed by …* (one line, at most 64 characters)     |
//! | `UsageStatistics`    | 0: never sent, kept or asked; 1: sent, cannot be turned off (#383) |
//!
//! Where they come from:
//!
//! * **Windows:** registry values under `Software\Policies\TeamCraft` –
//!   `HKEY_LOCAL_MACHINE` wins over `HKEY_CURRENT_USER`, value by value
//!   (`REG_DWORD` for the switches, `REG_SZ` for the texts), read with
//!   `reg query` like the CAD program search.
//! * **macOS:** a configuration profile for the preference domain
//!   [`MAC_DOMAIN`] – the computer's managed preferences win over the user's
//!   (`/Library/Managed Preferences/[<user>/]<domain>.plist`, read with
//!   `plutil -convert json`).
//! * **Linux (and others):** the JSON object in [`LINUX_FILE`].
//!
//! A value of the wrong type or out of range counts as not set; removing a
//! policy brings back the user's own choice (the settings file keeps it).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use serde_json::Value;

/// The Linux (and fallback) policy file.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
pub const LINUX_FILE: &str = "/etc/teamcraft/policies.json";
/// The macOS preference domain of the configuration profile.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const MAC_DOMAIN: &str = "io.github.rudalabs.teamcraft";
/// The registry key below `HKLM` and `HKCU`.
#[cfg_attr(not(windows), allow(dead_code))]
pub const REGISTRY_KEY: &str = r"Software\Policies\TeamCraft";
/// Longest organisation name shown.
pub const MAX_ORGANIZATION: usize = 64;

/// The policies in force; every field `None` = not set.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Policies {
    pub server_url: Option<String>,
    pub allow_other_servers: Option<bool>,
    pub check_for_updates: Option<bool>,
    pub crash_report_prompt: Option<bool>,
    pub workspace_root: Option<PathBuf>,
    pub organization_name: Option<String>,
    pub usage_statistics: Option<bool>,
}

/// One raw value as a source stores it.
#[cfg_attr(windows, allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Raw {
    Number(i64),
    Bool(bool),
    Text(String),
}

/// A switch: `0`/`1` (or `false`/`true` in a JSON file or profile).
fn switch(raw: Option<&Raw>) -> Option<bool> {
    match raw? {
        Raw::Number(0) | Raw::Bool(false) => Some(false),
        Raw::Number(1) | Raw::Bool(true) => Some(true),
        _ => None,
    }
}

fn text(raw: Option<&Raw>) -> Option<&str> {
    match raw? {
        Raw::Text(t) => Some(t.as_str()),
        _ => None,
    }
}

/// `OrganizationName`: one line of at most [`MAX_ORGANIZATION`] characters,
/// no control characters, not blank.
fn organization(raw: Option<&Raw>) -> Option<String> {
    let name = text(raw)?.trim();
    let fits = !name.is_empty()
        && name.chars().count() <= MAX_ORGANIZATION
        && !name.chars().any(char::is_control);
    fits.then(|| name.to_string())
}

/// `WorkspaceRoot`: an absolute path (Windows: with a drive or a UNC share).
fn workspace_root(raw: Option<&Raw>) -> Option<PathBuf> {
    let path = text(raw)?.trim();
    let absolute = Path::new(path).is_absolute()
        || (path.len() > 2
            && path.as_bytes()[1] == b':'
            && path.as_bytes()[0].is_ascii_alphabetic())
        || path.starts_with(r"\\");
    (absolute && !path.contains('\0') && path.len() <= 1024).then(|| PathBuf::from(path))
}

impl Policies {
    /// The policies from a lookup of raw values by policy name; anything of
    /// the wrong type or out of range is left out.
    pub fn from_lookup(get: impl Fn(&str) -> Option<Raw>) -> Policies {
        Policies {
            server_url: text(get("ServerUrl").as_ref())
                .and_then(|u| crate::settings::normalize_server_url(u).ok()),
            allow_other_servers: switch(get("AllowOtherServers").as_ref()),
            check_for_updates: switch(get("CheckForUpdates").as_ref()),
            crash_report_prompt: switch(get("CrashReportPrompt").as_ref()),
            workspace_root: workspace_root(get("WorkspaceRoot").as_ref()),
            organization_name: organization(get("OrganizationName").as_ref()),
            usage_statistics: switch(get("UsageStatistics").as_ref()),
        }
    }

    /// The policies of a JSON object (the Linux file, a macOS profile as
    /// JSON); anything else counts as no policies.
    #[cfg_attr(windows, allow(dead_code))]
    pub fn from_json(text: &str) -> Policies {
        let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) else {
            return Policies::default();
        };
        Policies::from_lookup(|name| json_raw(map.get(name)?))
    }

    /// Takes every policy `other` sets that this one does not.
    #[cfg_attr(not(any(windows, target_os = "macos", test)), allow(dead_code))]
    fn or(self, other: Policies) -> Policies {
        Policies {
            server_url: self.server_url.or(other.server_url),
            allow_other_servers: self.allow_other_servers.or(other.allow_other_servers),
            check_for_updates: self.check_for_updates.or(other.check_for_updates),
            crash_report_prompt: self.crash_report_prompt.or(other.crash_report_prompt),
            workspace_root: self.workspace_root.or(other.workspace_root),
            organization_name: self.organization_name.or(other.organization_name),
            usage_statistics: self.usage_statistics.or(other.usage_statistics),
        }
    }

    /// Whether any policy is set (the app is managed).
    pub fn any(&self) -> bool {
        self != &Policies::default()
    }

    /// The server is fixed: no other server may be added or switched to.
    pub fn server_locked(&self) -> bool {
        self.server_url.is_some() && self.allow_other_servers != Some(true)
    }

    /// Refuses a server other than the one the policy fixes.
    pub fn check_server(&self, base: &str) -> Result<(), String> {
        match &self.server_url {
            Some(fixed) if self.server_locked() && fixed != base => Err(format!(
                "Your organization set the TeamCraft server to {fixed} – other servers cannot be used."
            )),
            _ => Ok(()),
        }
    }

    /// The daily release check: the policy, else the user's choice (`None` = on).
    pub fn update_check(&self, user: Option<bool>) -> bool {
        self.check_for_updates.unwrap_or(user != Some(false))
    }

    /// The question after a crash: the policy, else the user's choice.
    pub fn crash_prompt(&self, user: Option<bool>) -> bool {
        self.crash_report_prompt.unwrap_or(user != Some(false))
    }

    /// What the UI shows: the organisation and which settings are fixed.
    pub fn view(&self) -> PolicyView {
        let mut managed = Vec::new();
        if self.server_locked() {
            managed.push("server");
        }
        if self.check_for_updates.is_some() {
            managed.push("update_check");
        }
        if self.crash_report_prompt.is_some() {
            managed.push("crash_prompt");
        }
        if self.usage_statistics.is_some() {
            managed.push("usage_statistics");
        }
        PolicyView {
            managed: self.any(),
            organization: self.organization_name.clone(),
            server_url: self.server_url.clone(),
            fixed: managed,
        }
    }
}

/// The policy state as the UI gets it (part of the session).
#[derive(Clone, Debug, Serialize)]
pub struct PolicyView {
    /// Any policy is set.
    pub managed: bool,
    /// *Managed by …*
    pub organization: Option<String>,
    /// The server the policy names (prefilled on the connect screen).
    pub server_url: Option<String>,
    /// Settings fixed by a policy: shown as *Set by your organization*, disabled.
    pub fixed: Vec<&'static str>,
}

#[cfg_attr(windows, allow(dead_code))]
fn json_raw(value: &Value) -> Option<Raw> {
    match value {
        Value::Bool(b) => Some(Raw::Bool(*b)),
        Value::Number(n) => n.as_i64().map(Raw::Number),
        Value::String(s) => Some(Raw::Text(s.clone())),
        _ => None,
    }
}

/// The values of a `reg query <key>` answer: `REG_DWORD` as numbers,
/// `REG_SZ`/`REG_EXPAND_SZ` as text, everything else left out.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_reg_query(output: &str) -> Vec<(String, Raw)> {
    let mut out = Vec::new();
    for line in output.lines() {
        // "    Name    REG_TYPE    data" – the name may hold single spaces
        let Some(line) = line.strip_prefix("    ") else {
            continue;
        };
        let mut parts = line.splitn(3, "    ");
        let (Some(name), Some(kind), data) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let data = data.unwrap_or("").trim_end_matches('\r');
        let raw = match kind.trim() {
            "REG_DWORD" | "REG_QWORD" => data
                .trim()
                .strip_prefix("0x")
                .and_then(|hex| i64::from_str_radix(hex, 16).ok())
                .map(Raw::Number),
            "REG_SZ" | "REG_EXPAND_SZ" => Some(Raw::Text(data.to_string())),
            _ => None,
        };
        if let Some(raw) = raw {
            out.push((name.trim().to_string(), raw));
        }
    }
    out
}

#[cfg_attr(not(windows), allow(dead_code))]
fn from_pairs(pairs: Vec<(String, Raw)>) -> Policies {
    Policies::from_lookup(|name| {
        pairs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, raw)| raw.clone())
    })
}

/// Reads the policies of this computer and user (see the module docs).
pub fn read() -> Policies {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let query = |hive: &str| {
            std::process::Command::new("reg")
                .args(["query", &format!(r"{hive}\{REGISTRY_KEY}")])
                .creation_flags(0x0800_0000)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| from_pairs(parse_reg_query(&String::from_utf8_lossy(&o.stdout))))
                .unwrap_or_default()
        };
        query("HKLM").or(query("HKCU"))
    }
    #[cfg(target_os = "macos")]
    {
        let profile = |path: PathBuf| {
            if !path.is_file() {
                return Policies::default();
            }
            std::process::Command::new("/usr/bin/plutil")
                .args(["-convert", "json", "-o", "-"])
                .arg(&path)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| Policies::from_json(&String::from_utf8_lossy(&o.stdout)))
                .unwrap_or_default()
        };
        let managed = PathBuf::from("/Library/Managed Preferences");
        let user = std::env::var("USER").unwrap_or_default();
        let computer = profile(managed.join(format!("{MAC_DOMAIN}.plist")));
        let for_user = if user.is_empty() || user.contains('/') {
            Policies::default()
        } else {
            profile(managed.join(&user).join(format!("{MAC_DOMAIN}.plist")))
        };
        computer.or(for_user)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::fs::read_to_string(LINUX_FILE)
            .map(|text| Policies::from_json(&text))
            .unwrap_or_default()
    }
}

static CURRENT: OnceLock<Policies> = OnceLock::new();

/// Reads the policies once, at start.
pub fn load() -> &'static Policies {
    CURRENT.get_or_init(read)
}

/// The policies read at start (none before [`load`]).
pub fn current() -> &'static Policies {
    static NONE: OnceLock<Policies> = OnceLock::new();
    CURRENT
        .get()
        .unwrap_or_else(|| NONE.get_or_init(Policies::default))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_values_are_checked() {
        let p = Policies::from_json(
            r#"{"ServerUrl": "https://plm.example.org/", "AllowOtherServers": 0,
                "CheckForUpdates": 1, "CrashReportPrompt": false,
                "WorkspaceRoot": "/srv/cad", "OrganizationName": "  Example Lab  ",
                "UsageStatistics": 0}"#,
        );
        assert_eq!(p.server_url.as_deref(), Some("https://plm.example.org"));
        assert_eq!(p.allow_other_servers, Some(false));
        assert_eq!(p.check_for_updates, Some(true));
        assert_eq!(p.crash_report_prompt, Some(false));
        assert_eq!(p.workspace_root, Some(PathBuf::from("/srv/cad")));
        assert_eq!(p.organization_name.as_deref(), Some("Example Lab"));
        assert_eq!(p.usage_statistics, Some(false));
        assert!(p.any() && p.server_locked());
    }

    #[test]
    fn wrong_types_and_ranges_count_as_not_set() {
        let p = Policies::from_json(
            r#"{"ServerUrl": "ftp://x", "AllowOtherServers": 2, "CheckForUpdates": "1",
                "CrashReportPrompt": -1, "WorkspaceRoot": "relative/dir",
                "OrganizationName": "two\nlines", "UsageStatistics": 1.5}"#,
        );
        assert_eq!(p, Policies::default());
        assert!(!p.any());
        let long = format!(
            r#"{{"OrganizationName": "{}"}}"#,
            "x".repeat(MAX_ORGANIZATION + 1)
        );
        assert_eq!(Policies::from_json(&long).organization_name, None);
        let fits = format!(
            r#"{{"OrganizationName": "{}"}}"#,
            "ü".repeat(MAX_ORGANIZATION)
        );
        assert!(
            Policies::from_json(&fits).organization_name.is_some(),
            "characters, not bytes"
        );
        // not an object, or not JSON: no policies
        assert_eq!(Policies::from_json("[1]"), Policies::default());
        assert_eq!(
            Policies::from_json("CheckForUpdates=1"),
            Policies::default()
        );
    }

    #[test]
    fn windows_paths_are_absolute_too() {
        let root = |p: &str| {
            Policies::from_json(&serde_json::json!({ "WorkspaceRoot": p }).to_string())
                .workspace_root
        };
        assert!(root(r"D:\CAD").is_some());
        assert!(root(r"\\fileserver\cad").is_some());
        assert!(root("CAD").is_none());
        assert!(root("").is_none());
    }

    #[test]
    fn the_registry_answer_is_read() {
        let out = "\r\nHKEY_LOCAL_MACHINE\\Software\\Policies\\TeamCraft\r\n    ServerUrl    REG_SZ    https://plm.example.org\r\n    CheckForUpdates    REG_DWORD    0x0\r\n    OrganizationName    REG_SZ    Example Lab e.V.\r\n    UsageStatistics    REG_BINARY    01\r\n    CrashReportPrompt    REG_SZ    0\r\n\r\n";
        let pairs = parse_reg_query(out);
        assert_eq!(pairs.len(), 4, "{pairs:?}");
        let p = from_pairs(pairs);
        assert_eq!(p.server_url.as_deref(), Some("https://plm.example.org"));
        assert_eq!(p.check_for_updates, Some(false));
        assert_eq!(p.organization_name.as_deref(), Some("Example Lab e.V."));
        assert_eq!(p.usage_statistics, None, "REG_BINARY is the wrong type");
        assert_eq!(
            p.crash_report_prompt, None,
            "a switch as REG_SZ is the wrong type"
        );
    }

    #[test]
    fn the_computer_wins_value_by_value() {
        let machine = Policies::from_json(r#"{"CheckForUpdates": 0}"#);
        let user = Policies::from_json(r#"{"CheckForUpdates": 1, "OrganizationName": "Lab"}"#);
        let p = machine.or(user);
        assert_eq!(p.check_for_updates, Some(false));
        assert_eq!(p.organization_name.as_deref(), Some("Lab"));
    }

    #[test]
    fn a_policy_wins_over_the_user_and_its_absence_brings_the_choice_back() {
        let none = Policies::default();
        assert!(none.update_check(None) && !none.update_check(Some(false)));
        assert!(none.crash_prompt(None) && !none.crash_prompt(Some(false)));
        let on = Policies::from_json(r#"{"CheckForUpdates": 1, "CrashReportPrompt": 1}"#);
        assert!(on.update_check(Some(false)) && on.crash_prompt(Some(false)));
        let off = Policies::from_json(r#"{"CheckForUpdates": 0, "CrashReportPrompt": 0}"#);
        assert!(!off.update_check(Some(true)) && !off.crash_prompt(None));
        assert_eq!(off.view().fixed, vec!["update_check", "crash_prompt"]);
    }

    #[test]
    fn a_fixed_server_refuses_others_unless_allowed() {
        let p = Policies::from_json(r#"{"ServerUrl": "https://plm.example.org"}"#);
        assert!(p.check_server("https://plm.example.org").is_ok());
        let err = p.check_server("https://other.example.org").unwrap_err();
        assert!(err.contains("Your organization"), "{err}");
        assert_eq!(p.view().fixed, vec!["server"]);
        let open = Policies::from_json(
            r#"{"ServerUrl": "https://plm.example.org", "AllowOtherServers": 1}"#,
        );
        assert!(!open.server_locked());
        assert!(open.check_server("https://other.example.org").is_ok());
        assert!(Policies::default()
            .check_server("https://any.example.org")
            .is_ok());
    }
}
