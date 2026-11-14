//! `teamcraft local` end to end: the binary creates a private PostgreSQL
//! cluster in a temporary data directory, serves the first-start page on
//! 127.0.0.1, refuses a second instance on the same data, shuts down with
//! PostgreSQL when its standard input closes (as when the desktop app ends)
//! and reuses the cluster on the next start.
//!
//! Needs PostgreSQL's server programs (initdb, pg_ctl, postgres) on this
//! computer – found like `teamcraft local` finds them, e.g. in
//! `/usr/lib/postgresql/<n>/bin` on the GitHub Linux runner. Without them,
//! or as root (PostgreSQL refuses to run as root), the test is skipped.

use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use teamcraft::localserver;

const BIN: &str = env!("CARGO_BIN_EXE_teamcraft");

struct Instance {
    child: Child,
    log: PathBuf,
    port: u16,
}

fn start(data: &Path, pg_bin: &Path, log: PathBuf) -> Instance {
    let port = localserver::free_port().unwrap();
    let out = std::fs::File::create(&log).unwrap();
    let child = Command::new(BIN)
        .arg("local")
        .arg("--data-dir")
        .arg(data)
        .arg("--port")
        .arg(port.to_string())
        .arg("--pg-bin")
        .arg(pg_bin)
        .arg("--exit-with-stdin")
        // no configuration of the developer's machine
        .env_remove("TEAMCRAFT_ENV_FILE")
        .current_dir(data.parent().unwrap())
        .stdin(Stdio::piped())
        .stdout(out.try_clone().unwrap())
        .stderr(out)
        .spawn()
        .unwrap();
    Instance { child, log, port }
}

fn log_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

async fn get(client: &reqwest::Client, port: u16, path: &str) -> Option<(u16, String)> {
    let response = client
        .get(format!("http://127.0.0.1:{port}{path}"))
        .send()
        .await
        .ok()?;
    let status = response.status().as_u16();
    Some((status, response.text().await.ok()?))
}

/// Waits until `/readyz` answers 200. `None` when the instance refused to
/// run as root (the test is then skipped).
async fn wait_ready(client: &reqwest::Client, instance: &mut Instance) -> Option<()> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some((200, _)) = get(client, instance.port, "/readyz").await {
            return Some(());
        }
        if let Some(status) = instance.child.try_wait().unwrap() {
            let log = log_text(&instance.log);
            if log.contains("cannot be run as root") || log.contains("refuses to run as root") {
                return None;
            }
            panic!("teamcraft local exited ({status}) before it was ready:\n{log}");
        }
        assert!(
            Instant::now() < deadline,
            "teamcraft local was not ready within 120 s:\n{}",
            log_text(&instance.log)
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Closes standard input and waits for a clean exit.
fn stop(mut instance: Instance) -> String {
    drop(instance.child.stdin.take());
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = instance.child.try_wait().unwrap() {
            let log = log_text(&instance.log);
            assert!(status.success(), "exit {status}:\n{log}");
            return log;
        }
        if Instant::now() > deadline {
            let _ = instance.child.kill();
            panic!(
                "no exit within 60 s after stdin closed:\n{}",
                log_text(&instance.log)
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[tokio::test]
async fn local_server_runs_stops_and_restarts() {
    let Ok(pg_bin) = localserver::find_pg_bin(None) else {
        eprintln!("skipped: no PostgreSQL server programs (initdb, pg_ctl, postgres) found");
        return;
    };
    let tmp = std::env::temp_dir().join(format!("teamcraft-local-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).unwrap();
    let data = tmp.join("local-server");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // first start: the cluster is created, the first-start page is offered
    let mut first = start(&data, &pg_bin, tmp.join("first.log"));
    if wait_ready(&client, &mut first).await.is_none() {
        eprintln!("skipped: PostgreSQL refuses to run as root");
        return;
    }
    let (status, login) = get(&client, first.port, "/login").await.unwrap();
    assert_eq!(status, 200);
    assert!(login.contains("Create the administrator"), "{login}");
    assert!(data.join("postgres").join("PG_VERSION").is_file());
    assert!(data.join("vault").is_dir());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(data.join("db-password"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "the database password is private");
    }

    // a second instance on the same data refuses instead of stopping the first
    let second = Command::new(BIN)
        .arg("local")
        .arg("--data-dir")
        .arg(&data)
        .arg("--port")
        .arg(localserver::free_port().unwrap().to_string())
        .arg("--pg-bin")
        .arg(&pg_bin)
        .current_dir(&tmp)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!second.status.success());
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("already runs"), "{stderr}");
    assert_eq!(
        get(&client, first.port, "/readyz").await.map(|r| r.0),
        Some(200),
        "the first instance still runs"
    );

    // stdin closes (the desktop app ended): server and PostgreSQL stop
    let log = stop(first);
    assert!(log.contains("standard input closed"), "{log}");
    assert!(log.contains("PostgreSQL stopped"), "{log}");
    assert!(
        !data.join("postgres").join("postmaster.pid").exists(),
        "PostgreSQL is still running:\n{log}"
    );
    assert!(!data.join("local-server.port").exists());

    // the next start reuses the cluster
    let mut again = start(&data, &pg_bin, tmp.join("again.log"));
    wait_ready(&client, &mut again).await.unwrap();
    let log = stop(again);
    assert!(!log.contains("creating the PostgreSQL cluster"), "{log}");
    let _ = std::fs::remove_dir_all(&tmp);
}
