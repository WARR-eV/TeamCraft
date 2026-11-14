//! *Try TeamCraft on this computer*: the local test server of the
//! *with local server* downloads. Those installers put the server
//! executable next to the app as `teamcraft-server` (Tauri `externalBin`) and
//! PostgreSQL's programs into the app's resources (`postgresql/bin`); the regular
//! installers have neither, and the button stays hidden.
//!
//! The app starts `teamcraft-server local --port <p> --exit-with-stdin` (see the
//! server's `src/localserver.rs`): a private PostgreSQL cluster and the
//! server, both on 127.0.0.1, data in the per-user data directory. The app
//! keeps the child's standard input open and never writes to it; when the
//! app ends – normally, by a crash or killed – the pipe closes and the
//! server stops itself and PostgreSQL. The output goes to
//! `<config dir>/local-server.log`.

use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use tokio::process::{Child, Command};

/// How long the first start may take (initdb, migrations, a slow disk).
const READY_TIMEOUT: Duration = Duration::from_secs(180);

/// The server child process, while it runs.
pub struct Running {
    pub child: Child,
    pub port: u16,
}

/// `teamcraft-server[.exe]`: the server's `teamcraft` binary under another
/// name – next to the app's own `TeamCraft` executable, `teamcraft` would
/// be the same file name on Windows and macOS (case-insensitive).
fn sidecar_name() -> &'static str {
    if cfg!(windows) {
        "teamcraft-server.exe"
    } else {
        "teamcraft-server"
    }
}

/// The bundled server executable next to the app's own executable, if this
/// is a *with local server* installation.
pub fn sidecar() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    sidecar_in(&dir)
}

fn sidecar_in(dir: &Path) -> Option<PathBuf> {
    let path = dir.join(sidecar_name());
    path.is_file().then_some(path)
}

/// The bundled PostgreSQL programs in the app's resources.
pub fn bundled_pg_bin(resource_dir: &Path) -> Option<PathBuf> {
    let dir = resource_dir.join("postgresql").join("bin");
    dir.is_dir().then_some(dir)
}

/// The address the app uses for the local server.
pub fn url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// The port, if `server_url` is the local server's address [`url`].
pub fn port_of(server_url: &str) -> Option<u16> {
    let port: u16 = server_url.strip_prefix("http://127.0.0.1:")?.parse().ok()?;
    (url(port) == server_url).then_some(port)
}

/// Whether nothing listens on the port on 127.0.0.1.
pub fn port_free(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// A free port on 127.0.0.1, asked from the operating system.
pub fn free_port() -> std::io::Result<u16> {
    Ok(std::net::TcpListener::bind(("127.0.0.1", 0))?
        .local_addr()?
        .port())
}

/// Whether a TeamCraft server answers `/readyz` on the port.
pub async fn ready(client: &reqwest::Client, port: u16) -> bool {
    match client
        .get(format!("{}/readyz", url(port)))
        .timeout(Duration::from_secs(3))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r
            .json::<serde_json::Value>()
            .await
            .is_ok_and(|v| v.get("status").is_some()),
        _ => false,
    }
}

/// The arguments of `teamcraft local` for the app.
pub fn arguments(port: u16, pg_bin: Option<&Path>) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![
        "local".into(),
        "--port".into(),
        port.to_string().into(),
        "--exit-with-stdin".into(),
    ];
    if let Some(dir) = pg_bin {
        args.push("--pg-bin".into());
        args.push(dir.into());
    }
    args
}

/// The last lines of the log, for an error message.
fn log_tail(log: &Path, lines: usize) -> String {
    let Ok(mut file) = std::fs::File::open(log) else {
        return String::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = file.seek(SeekFrom::Start(len.saturating_sub(16 * 1024)));
    let mut text = String::new();
    let _ = file.read_to_string(&mut text);
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

/// Starts the server and waits until it is ready.
pub async fn start(
    client: &reqwest::Client,
    sidecar: &Path,
    pg_bin: Option<&Path>,
    port: u16,
    log: &Path,
) -> Result<Running, String> {
    let out =
        std::fs::File::create(log).map_err(|e| format!("Cannot write {}: {e}", log.display()))?;
    let err = out
        .try_clone()
        .map_err(|e| format!("Cannot write {}: {e}", log.display()))?;
    let mut command = Command::new(sidecar);
    command
        .args(arguments(port, pg_bin))
        // not wherever the app was started from: `teamcraft` reads
        // secrets/.env relative to its working directory
        .current_dir(log.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::piped())
        .stdout(out)
        .stderr(err);
    #[cfg(windows)]
    {
        // no console window for the server (the app has none either)
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("The local server could not be started: {e}"))?;
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        if ready(client, port).await {
            return Ok(Running { child, port });
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "The local server stopped ({status}). Its log ({}) ends with:\n{}",
                log.display(),
                log_tail(log, 12)
            ));
        }
        if Instant::now() > deadline {
            let _ = child.start_kill();
            return Err(format!(
                "The local server did not start within {} s. Its log ({}) ends with:\n{}",
                READY_TIMEOUT.as_secs(),
                log.display(),
                log_tail(log, 12)
            ));
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_local_address_round_trips() {
        assert_eq!(url(18080), "http://127.0.0.1:18080");
        assert_eq!(port_of("http://127.0.0.1:18080"), Some(18080));
        // the address as settings::normalize_server_url stores it
        assert_eq!(
            crate::settings::normalize_server_url("http://127.0.0.1:18080/").as_deref(),
            Ok("http://127.0.0.1:18080")
        );
        for other in [
            "http://localhost:18080",
            "https://127.0.0.1:18080",
            "http://127.0.0.1:18080/plm",
            "http://127.0.0.1:x",
            "http://127.0.0.1",
            "https://plm.example.org",
        ] {
            assert_eq!(port_of(other), None, "{other}");
        }
        assert!(!crate::settings::insecure_http(&url(18080)));
    }

    #[test]
    fn arguments_keep_the_server_tied_to_the_app() {
        let args = arguments(18080, Some(Path::new("/opt/tc/postgresql/bin")));
        let args: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            args,
            [
                "local",
                "--port",
                "18080",
                "--exit-with-stdin",
                "--pg-bin",
                "/opt/tc/postgresql/bin"
            ]
        );
        assert_eq!(arguments(1, None).len(), 4);
    }

    #[test]
    fn the_sidecar_and_postgresql_are_found_only_when_bundled() {
        let dir =
            std::env::temp_dir().join(format!("teamcraft-desktop-local-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(sidecar_in(&dir), None);
        assert_eq!(bundled_pg_bin(&dir), None);
        std::fs::write(dir.join(sidecar_name()), "").unwrap();
        std::fs::create_dir_all(dir.join("postgresql").join("bin")).unwrap();
        assert_eq!(sidecar_in(&dir), Some(dir.join(sidecar_name())));
        assert_eq!(
            bundled_pg_bin(&dir),
            Some(dir.join("postgresql").join("bin"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_log_tail_has_the_last_lines() {
        let path = std::env::temp_dir().join(format!(
            "teamcraft-desktop-local-log-{}",
            std::process::id()
        ));
        let text: String = (1..=30).map(|i| format!("line {i}\n")).collect();
        std::fs::write(&path, text).unwrap();
        assert_eq!(log_tail(&path, 2), "line 29\nline 30");
        assert_eq!(log_tail(Path::new("/does/not/exist"), 2), "");
        let _ = std::fs::remove_file(&path);
    }
}
