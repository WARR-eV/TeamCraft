//! `teamcraft local`: a test installation on one computer, for trying
//! TeamCraft out without Docker or a database administrator. It runs a
//! private PostgreSQL cluster and the server, both only on `127.0.0.1`, with
//! every file in one data directory:
//!
//! ```text
//! <data dir>/postgres/        the cluster (initdb on the first start)
//! <data dir>/postgres.log     PostgreSQL's log
//! <data dir>/db-password      the generated database password (0600)
//! <data dir>/vault/           the file vault
//! ```
//!
//! The PostgreSQL programs come from the release bundle (`postgresql/bin`
//! next to the executable, as the desktop app's *with local server*
//! installers ship it), from `--pg-bin`, or from an installation on this
//! computer. The cluster gets its own free port and no Unix socket; it uses
//! scram-sha-256 with a random password, so other accounts on the computer
//! cannot connect. The desktop app starts this command for *Try TeamCraft on
//! this computer* (`desktop/src/localserver.rs`). Never for production:
//! one user's laptop, no backups, no TLS.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Output,
};

use anyhow::{bail, Context, Result};
use rand::{rngs::OsRng, RngCore};
use sqlx::{
    postgres::{PgConnectOptions, PgSslMode},
    ConnectOptions, Connection,
};

/// Database role and database of the local installation.
pub const DB_USER: &str = "teamcraft";
pub const DB_NAME: &str = "teamcraft";

/// The name of this binary in the desktop downloads *with local server*
/// (`teamcraft-server[.exe]` next to the app; `desktop/src/localserver.rs`).
pub const SIDECAR_NAME: &str = "teamcraft-server";

/// HTTP port when none is given (the port of every other quick start).
pub const DEFAULT_PORT: u16 = 8080;

/// The directory the release bundles put the PostgreSQL programs in, next to
/// the `teamcraft` executable (`postgresql/bin/initdb`, …).
pub const BUNDLED_PG_DIR: &str = "postgresql";

/// How long `pg_ctl` may take to start or stop the cluster, in seconds.
const PG_CTL_TIMEOUT_SECS: u32 = 120;

/// Line added to `postgresql.conf` after initdb: the settings of each start
/// (port, listen address) are written to this file.
const INCLUDE_LINE: &str = "include_if_exists = 'teamcraft-local.conf'";
const LOCAL_CONF: &str = "teamcraft-local.conf";

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Where the data lives when no `--data-dir` is given: the per-user data
/// directory of the operating system, so that the desktop app and the
/// command line share one installation.
///
/// * Linux: `$XDG_DATA_HOME/teamcraft/local-server` (`~/.local/share/…`)
/// * macOS: `~/Library/Application Support/TeamCraft/local-server`
/// * Windows: `%LOCALAPPDATA%\TeamCraft\local-server`
pub fn default_data_dir() -> Option<PathBuf> {
    data_dir_from(|key| std::env::var_os(key).map(PathBuf::from))
}

fn data_dir_from(var: impl Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    let set = |key: &str| var(key).filter(|p| !p.as_os_str().is_empty());
    if cfg!(windows) {
        return set("LOCALAPPDATA").map(|d| d.join("TeamCraft").join("local-server"));
    }
    let home = set("HOME");
    if cfg!(target_os = "macos") {
        return home.map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("TeamCraft")
                .join("local-server")
        });
    }
    set("XDG_DATA_HOME")
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| h.join(".local").join("share")))
        .map(|d| d.join("teamcraft").join("local-server"))
}

/// Whether `dir` holds the programs `teamcraft local` runs.
pub fn is_pg_bin(dir: &Path) -> bool {
    ["initdb", "pg_ctl", "postgres"]
        .iter()
        .all(|name| dir.join(exe(name)).is_file())
}

/// The directories searched for the PostgreSQL programs, in order:
/// `--pg-bin`; the bundle next to the executable (and in the macOS app's
/// `Resources`); `PATH`; the usual installation directories (Debian/Ubuntu
/// `/usr/lib/postgresql/<n>/bin`, Homebrew, Postgres.app, the Windows
/// installer), newest version first.
pub fn pg_bin_candidates(explicit: Option<&Path>, exe_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = explicit {
        out.push(dir.to_path_buf());
        // only the given directory: a wrong --pg-bin must not fall back silently
        return out;
    }
    if let Some(dir) = exe_dir {
        out.push(dir.join(BUNDLED_PG_DIR).join("bin"));
        // macOS app bundle: Contents/MacOS/teamcraft, Contents/Resources/postgresql
        out.push(
            dir.join("..")
                .join("Resources")
                .join(BUNDLED_PG_DIR)
                .join("bin"),
        );
        // Linux packages and the AppImage: usr/bin/teamcraft, usr/lib/<app>/postgresql
        for app in ["teamcraft", "TeamCraft"] {
            out.push(
                dir.join("..")
                    .join("lib")
                    .join(app)
                    .join(BUNDLED_PG_DIR)
                    .join("bin"),
            );
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        out.extend(std::env::split_paths(&path));
    }
    let versioned = |parent: &Path, prefix: &str, suffix: &[&str]| -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(parent) else {
            return Vec::new();
        };
        let mut found: Vec<(u32, PathBuf)> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let version = name.strip_prefix(prefix)?.parse::<u32>().ok()?;
                let mut path = e.path();
                for part in suffix {
                    path.push(part);
                }
                Some((version, path))
            })
            .collect();
        found.sort_by_key(|(version, _)| std::cmp::Reverse(*version));
        found.into_iter().map(|(_, p)| p).collect()
    };
    if cfg!(windows) {
        for root in ["ProgramFiles", "ProgramW6432"] {
            if let Some(pf) = std::env::var_os(root) {
                out.extend(versioned(
                    &PathBuf::from(pf).join("PostgreSQL"),
                    "",
                    &["bin"],
                ));
            }
        }
    } else {
        out.extend(versioned(Path::new("/usr/lib/postgresql"), "", &["bin"]));
        for brew in ["/opt/homebrew/opt", "/usr/local/opt"] {
            out.extend(versioned(Path::new(brew), "postgresql@", &["bin"]));
        }
        out.push(PathBuf::from(
            "/Applications/Postgres.app/Contents/Versions/latest/bin",
        ));
    }
    out
}

/// The first candidate that holds `initdb`, `pg_ctl` and `postgres`.
pub fn find_pg_bin(explicit: Option<&Path>) -> Result<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let candidates = pg_bin_candidates(explicit, exe_dir.as_deref());
    if let Some(found) = candidates.iter().find(|d| is_pg_bin(d)) {
        return Ok(found.clone());
    }
    match explicit {
        Some(dir) => bail!(
            "{} does not contain initdb, pg_ctl and postgres",
            dir.display()
        ),
        None => bail!(
            "PostgreSQL was not found: no {BUNDLED_PG_DIR}/bin next to teamcraft, none on PATH \
             or in the usual installation directories. Install PostgreSQL (16 or newer), \
             or give its bin directory with --pg-bin"
        ),
    }
}

/// A free TCP port on 127.0.0.1 (asked from the operating system).
pub fn free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .context("cannot find a free port on 127.0.0.1")?;
    Ok(listener.local_addr()?.port())
}

/// 32 random alphanumeric characters (about 190 bits).
fn random_password() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
        .collect()
}

/// Writes a file only the owner can read (0600 on Unix).
fn write_private(path: &Path, content: &str) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("cannot write {}", path.display()))?;
    std::io::Write::write_all(&mut file, content.as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

/// The settings of one start, written to `teamcraft-local.conf` in the
/// cluster: only 127.0.0.1, the given port, no Unix socket (nothing in /tmp,
/// no socket directory that would need root).
pub fn local_conf(port: u16) -> String {
    format!(
        "# written by `teamcraft local` at every start – changes are overwritten\n\
         listen_addresses = '127.0.0.1'\n\
         port = {port}\n\
         unix_socket_directories = ''\n"
    )
}

/// The configuration variables of the local installation. They replace the
/// same variables (and their `_FILE` forms) from the environment; see
/// [`overlay_lookup`].
pub fn local_variables(
    data_dir: &Path,
    http_port: u16,
    db_port: u16,
    password: &str,
) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("TEAMCRAFT_BIND_ADDR", format!("127.0.0.1:{http_port}")),
        (
            "TEAMCRAFT_PUBLIC_URL",
            format!("http://127.0.0.1:{http_port}"),
        ),
        ("TEAMCRAFT_DB_HOST", "127.0.0.1".to_string()),
        ("TEAMCRAFT_DB_PORT", db_port.to_string()),
        ("TEAMCRAFT_DB_USER", DB_USER.to_string()),
        ("TEAMCRAFT_DB_NAME", DB_NAME.to_string()),
        ("TEAMCRAFT_DB_PASSWORD", password.to_string()),
        ("TEAMCRAFT_DB_SSLMODE", "disable".to_string()),
        (
            "TEAMCRAFT_VAULT_DIR",
            data_dir.join("vault").to_string_lossy().to_string(),
        ),
    ])
}

/// Variables that would point the server at another database; hidden from
/// the configuration of the local installation.
const MASKED: &[&str] = &["DATABASE_URL", "TEAMCRAFT_DATABASE_URL"];

/// A lookup for [`crate::config::Config::from_lookup`]: the local variables
/// win, their `_FILE` forms and the database URLs are hidden, everything
/// else (an assistant key, OIDC, …) comes from `env`.
pub fn overlay_lookup<F>(
    local: BTreeMap<&'static str, String>,
    env: F,
) -> impl Fn(&str) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    move |key: &str| {
        if let Some(value) = local.get(key) {
            return Some(value.clone());
        }
        let base = key.strip_suffix("_FILE").unwrap_or(key);
        if local.contains_key(base) || MASKED.contains(&base) {
            return None;
        }
        env(key)
    }
}

/// A command for a PostgreSQL program. On Windows without a console window:
/// the desktop app starts `teamcraft local` without one, and every program
/// started from there would otherwise open its own.
fn pg_command(bin: &Path, name: &str) -> tokio::process::Command {
    #[allow(unused_mut)]
    let mut command = tokio::process::Command::new(bin.join(exe(name)));
    command.stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

fn output_text(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(stdout.trim());
    }
    text
}

async fn run(mut command: tokio::process::Command, what: &str) -> Result<Output> {
    let output = command
        .output()
        .await
        .with_context(|| format!("cannot run {what}"))?;
    if !output.status.success() {
        let text = output_text(&output);
        if text.contains("cannot be run as root") {
            bail!(
                "PostgreSQL refuses to run as root: start `teamcraft local` as a normal user\n{text}"
            );
        }
        bail!("{what} failed ({}):\n{text}", output.status);
    }
    Ok(output)
}

/// The running PostgreSQL cluster of a local installation.
pub struct Cluster {
    bin: PathBuf,
    pgdata: PathBuf,
    /// The port of this start.
    pub port: u16,
    /// The database password (from `<data dir>/db-password`).
    pub password: String,
}

impl Cluster {
    /// Creates the cluster on the first start, then starts it on a free port
    /// and creates the database if it is missing. A cluster left running by
    /// a previous start that did not stop it (killed, power loss) is stopped
    /// first.
    pub async fn start(bin: &Path, data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)
            .with_context(|| format!("cannot create {}", data_dir.display()))?;
        let pgdata = data_dir.join("postgres");
        let password_file = data_dir.join("db-password");

        if !pgdata.join("PG_VERSION").is_file() {
            if pgdata.exists() && std::fs::read_dir(&pgdata)?.next().is_some() {
                bail!(
                    "{} exists but is not a PostgreSQL cluster; move it away or choose another --data-dir",
                    pgdata.display()
                );
            }
            tracing::info!(dir = %pgdata.display(), "creating the PostgreSQL cluster (first start)");
            let password = random_password();
            write_private(&password_file, &password)?;
            // initdb reads the password from a file; it must not end up on
            // the command line, where other users could see it
            let pwfile = data_dir.join("initdb-password.tmp");
            write_private(&pwfile, &format!("{password}\n"))?;
            let mut initdb = pg_command(bin, "initdb");
            initdb
                .arg("--pgdata")
                .arg(&pgdata)
                .args(["--username", DB_USER])
                .args(["--auth", "scram-sha-256"])
                .arg(format!("--pwfile={}", pwfile.display()))
                .args(["--encoding", "UTF8", "--no-locale"]);
            let result = run(initdb, "initdb").await;
            let _ = std::fs::remove_file(&pwfile);
            if let Err(e) = result {
                // a half-created cluster would block the next attempt
                let _ = std::fs::remove_dir_all(&pgdata);
                return Err(e);
            }
            let conf = pgdata.join("postgresql.conf");
            let mut text = std::fs::read_to_string(&conf)
                .with_context(|| format!("cannot read {}", conf.display()))?;
            text.push_str(&format!("\n# added by `teamcraft local`\n{INCLUDE_LINE}\n"));
            std::fs::write(&conf, text)
                .with_context(|| format!("cannot write {}", conf.display()))?;
        }
        let password = std::fs::read_to_string(&password_file)
            .with_context(|| format!("cannot read {}", password_file.display()))?
            .trim()
            .to_string();
        if password.is_empty() {
            bail!("{} is empty", password_file.display());
        }

        let mut cluster = Self {
            bin: bin.to_path_buf(),
            pgdata,
            port: 0,
            password,
        };
        if cluster.running().await {
            tracing::info!("stopping the PostgreSQL cluster left running by an earlier start");
            cluster.stop_async().await?;
        }
        cluster.port = free_port()?;
        std::fs::write(cluster.pgdata.join(LOCAL_CONF), local_conf(cluster.port))
            .context("cannot write the cluster settings")?;
        // pg_ctl's own output goes to a file, never to a pipe: on Windows
        // pg_ctl starts postgres through cmd.exe with handle inheritance, so
        // the postmaster – which keeps running – would hold the write end of
        // a pipe, and reading the pipe to its end would wait forever.
        let pg_ctl_log = data_dir.join("pg_ctl.log");
        let out = std::fs::File::create(&pg_ctl_log)
            .with_context(|| format!("cannot write {}", pg_ctl_log.display()))?;
        let err = out.try_clone()?;
        let mut start = pg_command(bin, "pg_ctl");
        start
            .arg("start")
            .arg("--pgdata")
            .arg(&cluster.pgdata)
            .arg("--log")
            .arg(data_dir.join("postgres.log"))
            .args(["--wait", "--silent"])
            .arg(format!("--timeout={PG_CTL_TIMEOUT_SECS}"))
            .stdout(out)
            .stderr(err);
        let status = start.status().await.context("cannot run pg_ctl start")?;
        if !status.success() {
            let said = std::fs::read_to_string(&pg_ctl_log).unwrap_or_default();
            bail!(
                "PostgreSQL did not start ({status}); see {}\n{}",
                data_dir.join("postgres.log").display(),
                said.trim()
            );
        }
        tracing::info!(port = cluster.port, "PostgreSQL is running on 127.0.0.1");
        if let Err(e) = cluster.ensure_database().await {
            cluster.stop();
            return Err(e);
        }
        Ok(cluster)
    }

    fn options(&self, database: &str) -> PgConnectOptions {
        PgConnectOptions::new()
            .host("127.0.0.1")
            .port(self.port)
            .username(DB_USER)
            .password(&self.password)
            .database(database)
            .ssl_mode(PgSslMode::Disable)
            .application_name("teamcraft local")
    }

    /// Creates the database on the first start.
    async fn ensure_database(&self) -> Result<()> {
        let mut conn = self
            .options("postgres")
            .connect()
            .await
            .context("cannot connect to the local PostgreSQL cluster")?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
                .bind(DB_NAME)
                .fetch_one(&mut conn)
                .await?;
        if !exists {
            // the name is a constant: no quoting needed
            sqlx::query(&format!("CREATE DATABASE {DB_NAME}"))
                .execute(&mut conn)
                .await
                .context("cannot create the database")?;
        }
        conn.close().await.ok();
        Ok(())
    }

    /// Whether a postmaster runs on this data directory (`pg_ctl status`).
    async fn running(&self) -> bool {
        let mut status = pg_command(&self.bin, "pg_ctl");
        status.arg("status").arg("--pgdata").arg(&self.pgdata);
        matches!(status.output().await, Ok(o) if o.status.success())
    }

    async fn stop_async(&self) -> Result<()> {
        let mut stop = pg_command(&self.bin, "pg_ctl");
        stop.args(stop_args(&self.pgdata));
        run(stop, "pg_ctl stop").await.map(|_| ())
    }

    /// Stops the cluster (fast shutdown: open connections are closed). Used
    /// after the server stopped; errors are logged, not returned.
    pub fn stop(&self) {
        let mut stop = std::process::Command::new(self.bin.join(exe("pg_ctl")));
        stop.args(stop_args(&self.pgdata))
            .stdin(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            stop.creation_flags(0x0800_0000);
        }
        match stop.output() {
            Ok(o) if o.status.success() => tracing::info!("PostgreSQL stopped"),
            Ok(o) => tracing::warn!(error = %output_text(&o), "pg_ctl stop failed"),
            Err(e) => tracing::warn!(error = %e, "cannot run pg_ctl stop"),
        }
    }
}

fn stop_args(pgdata: &Path) -> Vec<&OsStr> {
    vec![
        OsStr::new("stop"),
        OsStr::new("--pgdata"),
        pgdata.as_os_str(),
        OsStr::new("--mode=fast"),
        OsStr::new("--wait"),
        OsStr::new("--silent"),
    ]
}

/// `<data dir>/local-server.port`: the HTTP port of the running instance,
/// written once PostgreSQL runs and removed when it stopped.
const PORT_FILE: &str = "local-server.port";

/// Records the HTTP port of this instance in the data directory.
pub fn write_port_file(data_dir: &Path, http_port: u16) -> Result<()> {
    std::fs::write(data_dir.join(PORT_FILE), format!("{http_port}\n"))
        .context("cannot write the port file")
}

/// Removes the port file again (after the cluster stopped).
pub fn remove_port_file(data_dir: &Path) {
    let _ = std::fs::remove_file(data_dir.join(PORT_FILE));
}

/// The port of another `teamcraft local` that still serves this data
/// directory: its port file names a port whose `/readyz` answers like
/// TeamCraft. Starting a second one would stop its PostgreSQL (it looks
/// left running), so the caller refuses instead. A port file left behind by
/// a killed instance does not count.
pub async fn running_instance(data_dir: &Path) -> Option<u16> {
    let port: u16 = std::fs::read_to_string(data_dir.join(PORT_FILE))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .ok()?;
    let body: serde_json::Value = client
        .get(format!("http://127.0.0.1:{port}/readyz"))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    body.get("status").is_some().then_some(port)
}

/// Resolves when standard input reaches its end (or fails). The desktop app
/// starts `teamcraft local --exit-with-stdin` with a pipe it never writes
/// to: when the app ends – also when it crashes or is killed – the pipe
/// closes and the server and PostgreSQL shut down cleanly. A plain thread,
/// not a blocking tokio task: the runtime would wait for it at shutdown.
pub async fn stdin_closed() {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 256];
        let mut stdin = std::io::stdin();
        loop {
            match std::io::Read::read(&mut stdin, &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        let _ = tx.send(());
    });
    let _ = rx.await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_per_os() {
        let vars = |pairs: &'static [(&'static str, &'static str)]| {
            move |key: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| PathBuf::from(v))
            }
        };
        if cfg!(windows) {
            assert_eq!(
                data_dir_from(vars(&[("LOCALAPPDATA", r"C:\Users\a\AppData\Local")])),
                Some(PathBuf::from(
                    r"C:\Users\a\AppData\Local\TeamCraft\local-server"
                ))
            );
            assert_eq!(data_dir_from(vars(&[])), None);
        } else if cfg!(target_os = "macos") {
            assert_eq!(
                data_dir_from(vars(&[("HOME", "/Users/a")])),
                Some(PathBuf::from(
                    "/Users/a/Library/Application Support/TeamCraft/local-server"
                ))
            );
        } else {
            assert_eq!(
                data_dir_from(vars(&[("HOME", "/home/a")])),
                Some(PathBuf::from("/home/a/.local/share/teamcraft/local-server"))
            );
            assert_eq!(
                data_dir_from(vars(&[("HOME", "/home/a"), ("XDG_DATA_HOME", "/data")])),
                Some(PathBuf::from("/data/teamcraft/local-server"))
            );
            // a relative XDG_DATA_HOME is invalid by the specification
            assert_eq!(
                data_dir_from(vars(&[("HOME", "/home/a"), ("XDG_DATA_HOME", "rel")])),
                Some(PathBuf::from("/home/a/.local/share/teamcraft/local-server"))
            );
            assert_eq!(data_dir_from(vars(&[("HOME", "")])), None);
        }
    }

    #[test]
    fn explicit_pg_bin_is_the_only_candidate() {
        let dir = Path::new("/somewhere/pg/bin");
        assert_eq!(
            pg_bin_candidates(Some(dir), Some(Path::new("/opt/tc"))),
            vec![dir.to_path_buf()]
        );
    }

    #[test]
    fn bundle_next_to_the_executable_comes_first() {
        let exe_dir = Path::new("/opt/tc");
        let candidates = pg_bin_candidates(None, Some(exe_dir));
        assert_eq!(candidates[0], exe_dir.join("postgresql").join("bin"));
        assert_eq!(
            candidates[1],
            exe_dir
                .join("..")
                .join("Resources")
                .join("postgresql")
                .join("bin")
        );
    }

    /// A new empty directory below the system's temporary directory.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("teamcraft-local-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn pg_bin_needs_all_three_programs() {
        let dir = scratch("bin");
        assert!(!is_pg_bin(&dir));
        for name in ["initdb", "pg_ctl"] {
            std::fs::write(dir.join(exe(name)), "").unwrap();
        }
        assert!(!is_pg_bin(&dir));
        std::fs::write(dir.join(exe("postgres")), "").unwrap();
        assert!(is_pg_bin(&dir));
        assert_eq!(find_pg_bin(Some(&dir)).unwrap(), dir);
        let empty = scratch("empty");
        let err = find_pg_bin(Some(&empty)).unwrap_err().to_string();
        assert!(err.contains("does not contain initdb"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&empty);
    }

    #[test]
    fn cluster_settings_listen_on_loopback_only() {
        let conf = local_conf(54321);
        assert!(conf.contains("listen_addresses = '127.0.0.1'\n"));
        assert!(conf.contains("port = 54321\n"));
        assert!(conf.contains("unix_socket_directories = ''\n"));
    }

    #[test]
    fn passwords_are_random_and_alphanumeric() {
        let (a, b) = (random_password(), random_password());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[cfg(unix)]
    #[test]
    fn private_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("private");
        let path = dir.join("secret");
        write_private(&path, "x").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_variables_win_over_the_environment() {
        let password = random_password();
        let local = local_variables(Path::new("/data"), 8099, 54000, &password);
        let env = |key: &str| -> Option<String> {
            match key {
                "TEAMCRAFT_DATABASE_URL" => Some("postgres://elsewhere/db".into()),
                "DATABASE_URL_FILE" => Some("/run/secrets/url".into()),
                "TEAMCRAFT_DB_PASSWORD_FILE" => Some("/run/secrets/pw".into()),
                "TEAMCRAFT_BIND_ADDR" => Some("0.0.0.0:80".into()),
                "TEAMCRAFT_INSTANCE_NAME" => Some("Kept".into()),
                _ => None,
            }
        };
        let lookup = overlay_lookup(local, env);
        assert_eq!(
            lookup("TEAMCRAFT_BIND_ADDR").as_deref(),
            Some("127.0.0.1:8099")
        );
        assert_eq!(
            lookup("TEAMCRAFT_PUBLIC_URL").as_deref(),
            Some("http://127.0.0.1:8099")
        );
        assert_eq!(lookup("TEAMCRAFT_DB_PORT").as_deref(), Some("54000"));
        assert_eq!(
            lookup("TEAMCRAFT_DB_PASSWORD").as_deref(),
            Some(password.as_str())
        );
        assert_eq!(lookup("TEAMCRAFT_DB_PASSWORD_FILE"), None);
        assert_eq!(lookup("TEAMCRAFT_DATABASE_URL"), None);
        assert_eq!(lookup("DATABASE_URL_FILE"), None);
        assert_eq!(lookup("TEAMCRAFT_INSTANCE_NAME").as_deref(), Some("Kept"));
        let vault = PathBuf::from(lookup("TEAMCRAFT_VAULT_DIR").unwrap());
        assert_eq!(vault, Path::new("/data").join("vault"));
    }

    /// The overlay gives a valid server configuration on loopback, which the
    /// first-start setup page needs (`web::auth_pages::setup_open`).
    #[test]
    fn local_configuration_is_valid_and_loopback() {
        let local = local_variables(Path::new("/data"), 8099, 54000, &random_password());
        let config = crate::config::Config::from_lookup(overlay_lookup(local, |_| None)).unwrap();
        assert!(config.bind_addr.ip().is_loopback());
        assert_eq!(config.bind_addr.port(), 8099);
        assert_eq!(config.public_url, "http://127.0.0.1:8099");
        assert_eq!(config.vault_dir, Path::new("/data").join("vault"));
    }
}
