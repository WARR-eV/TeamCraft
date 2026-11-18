//! Saved connections (#322, #323): several TeamCraft servers, and several
//! accounts on one server, side by side in one app.
//!
//! A [`Profile`] is a server address, the account signed in there and its own
//! working folder. Exactly one is active: its address, folder and token are the
//! ones in [`Settings`] (`server_url`, `workspace_dir`, the token in the
//! keychain entry `api-token`), so the rest of the app knows nothing about
//! profiles. Switching *parks* the active token under its profile – the
//! keychain entry `api-token@<id>`, or without a keychain the private file
//! `token-<id>` – and brings the other profile's token back.
//!
//! **Files of different servers never mix.** Item numbers, project codes and
//! revisions of two servers can be the same, so every connection after the
//! first gets a working folder of its own next to the default one
//! (`…/TeamCraft-plm.example.org`, `…/TeamCraft-plm.example.org-2` for a
//! second account there) – a sibling, never a folder inside another
//! connection's working folder. The first connection keeps the folder it
//! had, so nothing moves for existing installations.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::settings::{self, Keychain, Settings};

/// Most saved connections.
pub const MAX_PROFILES: usize = 20;

/// A saved connection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Random id (16 hex characters), names the parked token.
    pub id: String,
    /// Base URL as `settings::normalize_server_url` stores it.
    pub server_url: String,
    /// The account last signed in through this connection (unknown until the
    /// first sign-in).
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    /// Working folder; `None` = the app's default folder (first connection).
    #[serde(default)]
    pub workspace_dir: Option<PathBuf>,
}

/// Where the tokens of inactive connections are parked.
pub trait TokenParking {
    fn park(&self, id: &str, token: &str) -> anyhow::Result<()>;
    fn take(&self, id: &str) -> anyhow::Result<Option<String>>;
    fn forget(&self, id: &str) -> anyhow::Result<()>;
}

/// The OS keychain (`api-token@<id>`), or the private file `token-<id>` in the
/// settings folder where there is none – like the active token.
pub struct OsParking<'a> {
    pub dir: &'a Path,
}

impl OsParking<'_> {
    fn entry(id: &str) -> settings::OsKeychainEntry {
        settings::OsKeychainEntry(format!("api-token@{id}"))
    }
    fn file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("token-{id}"))
    }
}

impl TokenParking for OsParking<'_> {
    fn park(&self, id: &str, token: &str) -> anyhow::Result<()> {
        let entry = Self::entry(id);
        let stored =
            entry.set(token).is_ok() && entry.get().ok().flatten().as_deref() == Some(token);
        if stored {
            let _ = fs::remove_file(self.file(id));
            Ok(())
        } else {
            settings::write_private_file(self.dir, &format!("token-{id}"), token)
        }
    }

    fn take(&self, id: &str) -> anyhow::Result<Option<String>> {
        if let Ok(token) = fs::read_to_string(self.file(id)) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Ok(Some(token));
            }
        }
        Ok(Self::entry(id)
            .get()
            .ok()
            .flatten()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty()))
    }

    fn forget(&self, id: &str) -> anyhow::Result<()> {
        if let Err(e) = fs::remove_file(self.file(id)) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(e.into());
            }
        }
        let entry = Self::entry(id);
        if let Err(e) = entry.delete() {
            if matches!(entry.get(), Ok(Some(_))) {
                anyhow::bail!("the parked token could not be removed from the keychain: {e}");
            }
        }
        Ok(())
    }
}

fn new_id() -> String {
    hex::encode(rand::random::<[u8; 8]>())
}

/// What the settings dialog lists.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ProfileView {
    pub id: String,
    pub server_url: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    /// `None` = the default folder.
    pub workspace_dir: Option<PathBuf>,
    pub active: bool,
}

/// The saved connections, the active one first.
pub fn list(s: &Settings) -> Vec<ProfileView> {
    let mut out: Vec<ProfileView> = s
        .profiles
        .iter()
        .map(|p| {
            let active = s.active_profile.as_deref() == Some(p.id.as_str());
            ProfileView {
                id: p.id.clone(),
                server_url: if active {
                    s.server_url.clone().unwrap_or_else(|| p.server_url.clone())
                } else {
                    p.server_url.clone()
                },
                username: p.username.clone(),
                display_name: p.display_name.clone(),
                workspace_dir: if active {
                    s.workspace_dir.clone()
                } else {
                    p.workspace_dir.clone()
                },
                active,
            }
        })
        .collect();
    out.sort_by_key(|p| !p.active);
    out
}

/// Makes sure the configured server is a saved connection and active (the
/// settings of an older app version, or the first server): it keeps the
/// working folder it has. Returns the active id, `None` without a server.
pub fn ensure_active(s: &mut Settings) -> Option<String> {
    let server = s.server_url.clone()?;
    if let Some(id) = s.active_profile.clone() {
        if let Some(p) = s.profiles.iter_mut().find(|p| p.id == id) {
            // follows a moved local server port and a changed folder
            p.server_url = server;
            p.workspace_dir = s.workspace_dir.clone();
            return Some(id);
        }
    }
    let id = new_id();
    s.profiles.push(Profile {
        id: id.clone(),
        server_url: server,
        username: None,
        display_name: None,
        workspace_dir: s.workspace_dir.clone(),
    });
    s.active_profile = Some(id.clone());
    Some(id)
}

/// Notes the account of the active connection after a sign-in.
pub fn record_user(s: &mut Settings, username: &str, display_name: &str) {
    let Some(id) = ensure_active(s) else {
        return;
    };
    if let Some(p) = s.profiles.iter_mut().find(|p| p.id == id) {
        p.username = Some(username.to_string()).filter(|u| !u.is_empty());
        p.display_name = Some(display_name.to_string()).filter(|d| !d.is_empty());
    }
}

/// Parks the active token (or forgets a parked one when signed out).
fn park_active(s: &mut Settings, parking: &dyn TokenParking) -> anyhow::Result<()> {
    if let Some(id) = ensure_active(s) {
        match &s.token {
            Some(token) => parking.park(&id, token)?,
            None => parking.forget(&id)?,
        }
    }
    Ok(())
}

/// Switches to the saved connection `id`: the active token is parked under its
/// connection, the other one's token comes back (or none: sign in again).
pub fn switch(
    s: &mut Settings,
    dir: &Path,
    keychain: &dyn Keychain,
    parking: &dyn TokenParking,
    id: &str,
) -> anyhow::Result<()> {
    let target = s
        .profiles
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("This connection is no longer saved."))?;
    if s.active_profile.as_deref() == Some(id) && s.server_url.is_some() {
        return Ok(());
    }
    park_active(s, parking)?;
    let token = parking.take(id)?;
    s.save_token_with(dir, keychain, token)?;
    parking.forget(id)?;
    s.server_url = Some(target.server_url);
    s.workspace_dir = target.workspace_dir;
    s.active_profile = Some(target.id);
    Ok(())
}

/// A file-name-safe form of the server address: host and port.
fn slug(server_url: &str) -> String {
    let (host, port) = url::Url::parse(server_url)
        .ok()
        .map(|u| (u.host_str().unwrap_or("server").to_string(), u.port()))
        .unwrap_or_else(|| ("server".into(), None));
    let mut name: String = host
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if let Some(port) = port {
        name.push_str(&format!("-{port}"));
    }
    name.trim_matches(|c| c == '.' || c == '-')
        .chars()
        .take(64)
        .collect()
}

/// The working folder of a new connection: next to `default_root`, named
/// after the server, numbered when taken (by a saved connection or an
/// existing folder).
fn new_workspace(s: &Settings, server_url: &str, default_root: &Path) -> PathBuf {
    let base = default_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "TeamCraft".into());
    let parent = default_root.parent().unwrap_or(default_root);
    let taken = |p: &Path| {
        p == default_root
            || s.profiles
                .iter()
                .any(|q| q.workspace_dir.as_deref() == Some(p))
            || s.workspace_dir.as_deref() == Some(p)
            || p.exists()
    };
    let first = parent.join(format!("{base}-{}", slug(server_url)));
    if !taken(&first) {
        return first;
    }
    (2..)
        .map(|n| parent.join(format!("{base}-{}-{n}", slug(server_url))))
        .find(|p| !taken(p))
        .expect("an unused folder name")
}

/// Adds a connection to `server_url` (a new server, or another account on a
/// known one), parks the active token and makes the new one active, signed
/// out, with a working folder of its own.
pub fn add(
    s: &mut Settings,
    dir: &Path,
    keychain: &dyn Keychain,
    parking: &dyn TokenParking,
    server_url: &str,
    default_root: &Path,
) -> anyhow::Result<String> {
    if s.profiles.len() >= MAX_PROFILES {
        anyhow::bail!("At most {MAX_PROFILES} connections can be saved – remove one first.");
    }
    if s.server_url.is_none() {
        // the very first server: no folder of its own needed
        s.server_url = Some(server_url.to_string());
        return Ok(ensure_active(s).expect("a server"));
    }
    park_active(s, parking)?;
    let workspace = new_workspace(s, server_url, default_root);
    let id = new_id();
    s.profiles.push(Profile {
        id: id.clone(),
        server_url: server_url.to_string(),
        username: None,
        display_name: None,
        workspace_dir: Some(workspace.clone()),
    });
    s.save_token_with(dir, keychain, None)?;
    s.server_url = Some(server_url.to_string());
    s.workspace_dir = Some(workspace);
    s.active_profile = Some(id.clone());
    Ok(id)
}

/// The connect screen chose `server_url`: nothing changes for the active
/// server; a saved connection to it becomes active (with its token, if it has
/// one); otherwise a new connection is added.
pub fn connect(
    s: &mut Settings,
    dir: &Path,
    keychain: &dyn Keychain,
    parking: &dyn TokenParking,
    server_url: &str,
    default_root: &Path,
) -> anyhow::Result<()> {
    if s.server_url.as_deref() == Some(server_url) {
        ensure_active(s);
        return Ok(());
    }
    let saved = s
        .profiles
        .iter()
        .find(|p| p.server_url == server_url && s.active_profile.as_deref() != Some(p.id.as_str()))
        .map(|p| p.id.clone());
    match saved {
        Some(id) => switch(s, dir, keychain, parking, &id),
        None => add(s, dir, keychain, parking, server_url, default_root).map(|_| ()),
    }
}

/// Removes a saved connection that is not active, with its parked token.
/// Its working folder stays on disk.
pub fn remove(s: &mut Settings, parking: &dyn TokenParking, id: &str) -> anyhow::Result<()> {
    if s.active_profile.as_deref() == Some(id) {
        anyhow::bail!("Switch to another connection before removing this one.");
    }
    parking.forget(id)?;
    s.profiles.retain(|p| p.id != id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap};

    /// The active token's keychain, in memory.
    #[derive(Default)]
    struct Active(RefCell<Option<String>>);
    impl Keychain for Active {
        fn get(&self) -> Result<Option<String>, String> {
            Ok(self.0.borrow().clone())
        }
        fn set(&self, token: &str) -> Result<(), String> {
            *self.0.borrow_mut() = Some(token.into());
            Ok(())
        }
        fn delete(&self) -> Result<(), String> {
            *self.0.borrow_mut() = None;
            Ok(())
        }
    }

    #[derive(Default)]
    struct Parking(RefCell<HashMap<String, String>>);
    impl TokenParking for Parking {
        fn park(&self, id: &str, token: &str) -> anyhow::Result<()> {
            self.0.borrow_mut().insert(id.into(), token.into());
            Ok(())
        }
        fn take(&self, id: &str) -> anyhow::Result<Option<String>> {
            Ok(self.0.borrow().get(id).cloned())
        }
        fn forget(&self, id: &str) -> anyhow::Result<()> {
            self.0.borrow_mut().remove(id);
            Ok(())
        }
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tc-profiles-{}", new_id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const A: &str = "https://plm.example.org";
    const B: &str = "https://plm.other.example";

    #[test]
    fn an_older_installation_becomes_the_first_connection_unchanged() {
        let mut s = Settings {
            server_url: Some(A.into()),
            workspace_dir: Some(PathBuf::from("/home/u/CAD")),
            ..Settings::default()
        };
        let id = ensure_active(&mut s).unwrap();
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(
            s.profiles[0].workspace_dir.as_deref(),
            Some(Path::new("/home/u/CAD"))
        );
        assert_eq!(ensure_active(&mut s), Some(id), "idempotent");
        assert_eq!(s.profiles.len(), 1);
        // no server: nothing to save
        assert_eq!(ensure_active(&mut Settings::default()), None);
    }

    #[test]
    fn switching_servers_keeps_each_sign_in_and_separate_folders() {
        let dir = temp_dir();
        let (keychain, parking) = (Active::default(), Parking::default());
        let root = dir.join("Documents").join("TeamCraft");
        let mut s = Settings::default();
        // the first server keeps the default folder
        connect(&mut s, &dir, &keychain, &parking, A, &root).unwrap();
        assert_eq!(s.workspace_dir, None);
        s.save_token_with(&dir, &keychain, Some("tc_pat_a".into()))
            .unwrap();
        record_user(&mut s, "erika", "Erika");
        let first = s.active_profile.clone().unwrap();

        // another server: a folder of its own next to the default, signed out
        connect(&mut s, &dir, &keychain, &parking, B, &root).unwrap();
        let second = s.active_profile.clone().unwrap();
        assert_ne!(first, second);
        assert_eq!(s.server_url.as_deref(), Some(B));
        assert_eq!(s.token, None);
        assert_eq!(keychain.get().unwrap(), None);
        let ws = s.workspace_dir.clone().unwrap();
        assert_eq!(
            ws,
            dir.join("Documents").join("TeamCraft-plm.other.example")
        );
        assert!(
            !ws.starts_with(&root),
            "never inside another working folder"
        );
        assert_eq!(parking.take(&first).unwrap().as_deref(), Some("tc_pat_a"));
        s.save_token_with(&dir, &keychain, Some("tc_pat_b".into()))
            .unwrap();

        // back to the first: its token returns, the second is parked
        connect(&mut s, &dir, &keychain, &parking, A, &root).unwrap();
        assert_eq!(s.active_profile.as_deref(), Some(first.as_str()));
        assert_eq!(s.token.as_deref(), Some("tc_pat_a"));
        assert_eq!(keychain.get().unwrap().as_deref(), Some("tc_pat_a"));
        assert_eq!(s.workspace_dir, None);
        assert_eq!(parking.take(&first).unwrap(), None, "no copy left parked");
        assert_eq!(parking.take(&second).unwrap().as_deref(), Some("tc_pat_b"));

        // the list: active first, with the account
        let listed = list(&s);
        assert_eq!(listed.len(), 2);
        assert!(listed[0].active && listed[0].username.as_deref() == Some("erika"));
        assert_eq!(listed[1].workspace_dir.as_deref(), Some(ws.as_path()));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_second_account_on_the_same_server_gets_its_own_folder() {
        let dir = temp_dir();
        let (keychain, parking) = (Active::default(), Parking::default());
        let root = dir.join("TeamCraft");
        let mut s = Settings::default();
        connect(&mut s, &dir, &keychain, &parking, A, &root).unwrap();
        s.save_token_with(&dir, &keychain, Some("tc_pat_erika".into()))
            .unwrap();
        let erika = s.active_profile.clone().unwrap();
        let max = add(&mut s, &dir, &keychain, &parking, A, &root).unwrap();
        assert_eq!(s.token, None, "the new account signs in");
        assert_eq!(s.server_url.as_deref(), Some(A));
        assert_eq!(
            s.workspace_dir.clone().unwrap(),
            dir.join("TeamCraft-plm.example.org")
        );
        add(&mut s, &dir, &keychain, &parking, A, &root).unwrap();
        assert_eq!(
            s.workspace_dir.clone().unwrap(),
            dir.join("TeamCraft-plm.example.org-2")
        );
        // switching between the accounts of one server
        switch(&mut s, &dir, &keychain, &parking, &erika).unwrap();
        assert_eq!(s.token.as_deref(), Some("tc_pat_erika"));
        // removing: not the active one; the parked token goes with it
        assert!(remove(&mut s, &parking, &erika).is_err());
        parking.park(&max, "tc_pat_max").unwrap();
        remove(&mut s, &parking, &max).unwrap();
        assert_eq!(parking.take(&max).unwrap(), None);
        assert_eq!(s.profiles.len(), 2);
        assert!(
            switch(&mut s, &dir, &keychain, &parking, &max).is_err(),
            "gone"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn folder_names_follow_the_server() {
        assert_eq!(slug("https://plm.example.org"), "plm.example.org");
        assert_eq!(slug("http://127.0.0.1:18080"), "127.0.0.1-18080");
        assert_eq!(slug("https://[::1]:8443/teamcraft"), "1--8443");
        assert_eq!(slug("not a url"), "server");
    }

    #[test]
    fn the_number_of_connections_is_bounded() {
        let dir = temp_dir();
        let (keychain, parking) = (Active::default(), Parking::default());
        let root = dir.join("TeamCraft");
        let mut s = Settings::default();
        for i in 0..MAX_PROFILES {
            add(
                &mut s,
                &dir,
                &keychain,
                &parking,
                &format!("https://s{i}.example"),
                &root,
            )
            .unwrap();
        }
        assert!(add(
            &mut s,
            &dir,
            &keychain,
            &parking,
            "https://one-more.example",
            &root
        )
        .is_err());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn os_parking_falls_back_to_a_private_file() {
        // without a working keychain for this entry (tests never touch the OS
        // keychain: an id nobody uses, and the file wins on take)
        let dir = temp_dir();
        let parking = OsParking { dir: &dir };
        settings::write_private_file(&dir, "token-test0000", "tc_pat_file").unwrap();
        assert_eq!(
            parking.take("test0000").unwrap().as_deref(),
            Some("tc_pat_file")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.join("token-test0000"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        fs::remove_dir_all(dir).ok();
    }
}
