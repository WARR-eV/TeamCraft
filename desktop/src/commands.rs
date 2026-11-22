//! Commands the user interface calls through `window.__TAURI__.core.invoke`.
//! Every network access and every file system access of the app is here.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Notify;

use crate::{
    acp, addfiles,
    api::{self, Api, ApiError, Transfer},
    bomimport, cadexport, cadlaunch, folderimport, layout, localchanges, localserver, policy,
    profiles, scbridge, scprops,
    settings::{self, Settings, TokenStorage},
    signin, update, workspace, AppState,
};

type CmdResult<T> = Result<T, ApiError>;

/// A setting a policy fixes (#378) cannot be changed in the app.
const MANAGED: &str = "This setting is set by your organization.";

pub(crate) fn snapshot(state: &State<'_, AppState>) -> Settings {
    state.settings.lock().expect("settings lock").clone()
}

/// The API client for the configured server (and token, if signed in).
fn api_of(state: &State<'_, AppState>) -> CmdResult<Api> {
    let s = snapshot(state);
    let base = s
        .server_url
        .ok_or_else(|| ApiError::local("No TeamCraft server configured."))?;
    Ok(Api {
        client: state.client.clone(),
        base,
        token: s.token,
    })
}

fn signed_in_api(state: &State<'_, AppState>) -> CmdResult<Api> {
    let api = api_of(state)?;
    if api.token.is_none() {
        return Err(ApiError {
            status: 401,
            message: "Please sign in.".into(),
        });
    }
    Ok(api)
}

/// The default working folder (`Documents/TeamCraft`); connections after the
/// first get theirs next to it (src/profiles.rs).
/// The root of new working folders: the policy `WorkspaceRoot` (#378), else
/// `Documents/TeamCraft`.
pub(crate) fn default_workspace(app: &AppHandle) -> PathBuf {
    if let Some(root) = &policy::current().workspace_root {
        return root.clone();
    }
    app.path()
        .document_dir()
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("TeamCraft")
}

pub(crate) fn workspace_root(app: &AppHandle, state: &State<'_, AppState>) -> PathBuf {
    snapshot(state)
        .workspace_dir
        .unwrap_or_else(|| default_workspace(app))
}

fn update_settings(
    state: &State<'_, AppState>,
    change: impl FnOnce(&mut Settings),
) -> CmdResult<Settings> {
    let mut s = state.settings.lock().expect("settings lock");
    change(&mut s);
    s.save(&state.config_dir)
        .map_err(|e| ApiError::local(format!("Settings could not be saved: {e}")))?;
    Ok(s.clone())
}

#[derive(Serialize)]
pub struct Session {
    pub server_url: Option<String>,
    pub signed_in: bool,
    pub skin: String,
    pub workspace_dir: PathBuf,
    pub version: &'static str,
    pub device: String,
    /// Zoom factor of the user interface.
    pub zoom: f64,
    /// Where the token is kept: `file` makes the UI warn (no OS keychain).
    pub token_storage: Option<TokenStorage>,
    /// The daily check for a newer release is switched on.
    pub update_check: bool,
    /// Check-out of an assembly also gets its parts.
    pub checkout_parts: bool,
    /// After a crash, ask at the next start whether to share the report.
    pub crash_prompt: bool,
    /// *Settings → Report problems to* (#381); `None` = GitHub.
    pub report_url: Option<String>,
    /// A *with local server* installation: the connect screen offers *Try
    /// TeamCraft on this computer* (src/localserver.rs).
    pub local_server_available: bool,
    /// The configured server is the local test server: the UI starts it
    /// (`local_server_start`) before it signs in or opens the main window.
    pub local_server_in_use: bool,
    /// Managed deployment policies (#378): the organisation and the settings
    /// fixed by a policy.
    pub policy: policy::PolicyView,
}

#[tauri::command]
pub fn session(app: AppHandle, state: State<'_, AppState>) -> Session {
    let s = snapshot(&state);
    Session {
        token_storage: s.token.as_ref().and(s.token_storage),
        update_check: policy::current().update_check(s.update_check),
        checkout_parts: s.checkout_parts != Some(false),
        report_url: policy::current().report_target(s.report_url.clone()),
        crash_prompt: policy::current().crash_prompt(s.crash_prompt),
        server_url: s.server_url.clone(),
        signed_in: s.token.is_some(),
        skin: s.skin.clone().unwrap_or_else(|| "teamcraft".into()),
        workspace_dir: workspace_root(&app, &state),
        version: env!("CARGO_PKG_VERSION"),
        device: crate::device_name(),
        zoom: s.zoom.unwrap_or(1.0),
        local_server_available: localserver::sidecar().is_some(),
        local_server_in_use: local_server_in_use(&s),
        policy: policy::current().view(),
    }
}

/// Whether the configured server is the app's local test server.
fn local_server_in_use(s: &Settings) -> bool {
    s.local_server_port.is_some()
        && s.server_url.as_deref().and_then(localserver::port_of) == s.local_server_port
}

/// *Try TeamCraft on this computer*: starts the bundled server
/// (`teamcraft local`, src/localserver.rs) and returns its address. The
/// port of the last start is kept, so that the address – and with it the
/// sign-in token – stays the same; if another program took it meanwhile, a
/// new one is chosen and the stored address follows (same server, the token
/// stays valid). A server already answering there (started by another
/// window or `teamcraft local` on the command line) is used as it is.
#[tauri::command]
pub async fn local_server_start(app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    if policy::current().server_locked() {
        return Err(ApiError::local(
            "Your organization set the TeamCraft server – the local test server cannot be used.",
        ));
    }
    {
        let mut running = state.local_server.lock().expect("local server lock");
        if let Some(r) = running.as_mut() {
            if matches!(r.child.try_wait(), Ok(None)) {
                return Ok(localserver::url(r.port));
            }
        }
    }
    let sidecar = localserver::sidecar().ok_or_else(|| {
        ApiError::local(
            "This installation has no local server (download TeamCraft with local server).",
        )
    })?;
    let s = snapshot(&state);
    let probe = reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| ApiError::local(e.to_string()))?;
    let stored = s.local_server_port;
    let port = match stored {
        Some(port) if localserver::ready(&probe, port).await => {
            return Ok(localserver::url(port));
        }
        Some(port) if localserver::port_free(port) => port,
        _ => localserver::free_port().map_err(|e| ApiError::local(e.to_string()))?,
    };
    let pg_bin = app
        .path()
        .resource_dir()
        .ok()
        .and_then(|dir| localserver::bundled_pg_bin(&dir));
    let log = state.config_dir.join("local-server.log");
    std::fs::create_dir_all(&state.config_dir).map_err(|e| ApiError::local(e.to_string()))?;
    let running = localserver::start(&probe, &sidecar, pg_bin.as_deref(), port, &log)
        .await
        .map_err(ApiError::local)?;
    *state.local_server.lock().expect("local server lock") = Some(running);
    let was_in_use = local_server_in_use(&s);
    update_settings(&state, |s| {
        s.local_server_port = Some(port);
        if was_in_use {
            s.server_url = Some(localserver::url(port));
        }
    })?;
    Ok(localserver::url(port))
}

/// Would this address (as typed) reach another computer over plain
/// `http://`? The connect dialog warns while the user types.
#[tauri::command]
pub fn server_url_insecure(url: String) -> bool {
    settings::normalize_server_url(&url).is_ok_and(|base| settings::insecure_http(&base))
}

/// Checks that the address is a TeamCraft server and makes it the active
/// connection: a saved connection to it comes back with its sign-in, a new
/// server gets a connection – and a working folder – of its own; the sign-in
/// of the server left behind is kept for switching back (#323).
#[tauri::command]
pub async fn set_server(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> CmdResult<String> {
    let base = settings::normalize_server_url(&url).map_err(ApiError::local)?;
    policy::current()
        .check_server(&base)
        .map_err(ApiError::local)?;
    let api = Api {
        client: state.client.clone(),
        base: base.clone(),
        token: None,
    };
    let index = api
        .json(Method::GET, "", None)
        .await
        .map_err(|e| ApiError {
            status: e.status,
            message: format!("{base} does not answer like a TeamCraft server: {e}"),
        })?;
    if index["name"] != "TeamCraft REST API" {
        return Err(ApiError::local(format!(
            "{base} is not a TeamCraft server."
        )));
    }
    if snapshot(&state).server_url.as_deref() != Some(base.as_str()) {
        // the agent works with the token of the server left behind
        stop_agent(&state);
    }
    let root = default_workspace(&app);
    change_connection(&state, |s, dir, parking| {
        profiles::connect(s, dir, &settings::OsKeychain, parking, &base, &root)
    })?;
    Ok(base)
}

/// Changes the saved connections under the settings lock and saves them.
fn change_connection(
    state: &State<'_, AppState>,
    change: impl FnOnce(&mut Settings, &std::path::Path, &profiles::OsParking<'_>) -> anyhow::Result<()>,
) -> CmdResult<()> {
    let dir = state.config_dir.clone();
    let parking = profiles::OsParking { dir: &dir };
    let mut s = state.settings.lock().expect("settings lock");
    change(&mut s, &dir, &parking).map_err(|e| ApiError::local(e.to_string()))?;
    s.save(&dir)
        .map_err(|e| ApiError::local(format!("Settings could not be saved: {e}")))
}

/// The saved connections (#322, #323), the active one first.
#[tauri::command]
pub fn connections(state: State<'_, AppState>) -> Vec<profiles::ProfileView> {
    let mut s = state.settings.lock().expect("settings lock");
    if profiles::ensure_active(&mut s).is_some() {
        let _ = s.save(&state.config_dir);
    }
    profiles::list(&s)
}

/// Switches to a saved connection; the UI reloads afterwards.
#[tauri::command]
pub fn switch_connection(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let target = snapshot(&state)
        .profiles
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.server_url.clone());
    if let Some(server) = target {
        policy::current()
            .check_server(&server)
            .map_err(ApiError::local)?;
    }
    stop_agent(&state);
    change_connection(&state, |s, dir, parking| {
        profiles::switch(s, dir, &settings::OsKeychain, parking, &id)
    })
}

/// Another account on the active server: the current sign-in is kept for
/// switching back, the new connection starts signed out with a working folder
/// of its own.
#[tauri::command]
pub fn add_account(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let server = snapshot(&state)
        .server_url
        .ok_or_else(|| ApiError::local("No TeamCraft server configured."))?;
    stop_agent(&state);
    let root = default_workspace(&app);
    change_connection(&state, |s, dir, parking| {
        profiles::add(s, dir, &settings::OsKeychain, parking, &server, &root).map(|_| ())
    })
}

/// Forgets a saved connection that is not active (and its sign-in); its
/// working folder stays on disk.
#[tauri::command]
pub fn remove_connection(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    change_connection(&state, |s, _, parking| profiles::remove(s, parking, &id))
}

/// Notes the signed-in account on the active connection; the settings are
/// written only when it changed (this also runs on every `GET /me`).
fn remember_user(state: &State<'_, AppState>, user: &Value) {
    let (Some(username), display) = (user["username"].as_str(), user["display_name"].as_str())
    else {
        return;
    };
    let mut s = state.settings.lock().expect("settings lock");
    if profiles::record_user(&mut s, username, display.unwrap_or(username)) {
        let _ = s.save(&state.config_dir);
    }
}

/// Whether this app fits the server version (`compat::notice`): a message for
/// the user, or `null`. Needs no sign-in; a server that does not answer is not
/// reported here (other calls show that).
#[tauri::command]
pub async fn server_notice(state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let api = api_of(&state)?;
    let Ok(index) = api.json(Method::GET, "", None).await else {
        return Ok(None);
    };
    Ok(index["version"]
        .as_str()
        .and_then(|server| crate::compat::notice(env!("CARGO_PKG_VERSION"), server)))
}

/// The version the server says it runs (`GET /api/v1`), for problem reports
/// (#381); `null` when it does not answer. Needs no sign-in.
#[tauri::command]
pub async fn server_version(state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let api = api_of(&state)?;
    let Ok(index) = api.json(Method::GET, "", None).await else {
        return Ok(None);
    };
    Ok(index["version"]
        .as_str()
        .filter(|v| {
            v.len() <= 32
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
        })
        .map(str::to_string))
}

fn store_token(state: &State<'_, AppState>, token: String) -> CmdResult<()> {
    let mut s = state.settings.lock().expect("settings lock");
    s.save_token(&state.config_dir, Some(token))
        .map_err(|e| ApiError::local(format!("The sign-in could not be stored: {e}")))
}

/// Sign-in through the browser (single sign-on).
#[tauri::command]
pub async fn sign_in_browser(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Value> {
    let api = api_of(&state)?;
    // A fresh cancel signal per attempt: `notify_one` keeps a permit, so a
    // cancel between two steps (or before the wait started) is not lost, and
    // an old cancel never affects the next attempt.
    let cancel = Arc::new(Notify::new());
    *state.sign_in_cancel.lock().expect("sign-in lock") = Some(cancel.clone());
    let flow = async {
        let attempt = signin::prepare(&api.base, &crate::device_name()).await?;
        app.opener()
            .open_url(attempt.url.clone(), None::<&str>)
            .map_err(|e| ApiError::local(format!("The browser could not be opened: {e}")))?;
        let code = match signin::wait_for_callback(&attempt, &cancel).await? {
            signin::Callback::Code(code) => code,
            signin::Callback::Denied => {
                return Err(ApiError::local("The sign-in was cancelled in the browser."))
            }
        };
        let result = signin::exchange(&api, &code, &attempt.verifier).await?;
        let token = result["token"]
            .as_str()
            .ok_or_else(|| ApiError::local("The server sent no token."))?
            .to_string();
        Ok::<_, ApiError>((token, result["user"].clone()))
    };
    // cancelling drops the flow at whatever step it is in
    let outcome = tokio::select! {
        _ = cancel.notified() => Err(ApiError::local("Sign-in cancelled.")),
        result = flow => result,
    };
    state.sign_in_cancel.lock().expect("sign-in lock").take();
    let (token, user) = outcome?;
    store_token(&state, token)?;
    remember_user(&state, &user);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_focus();
    }
    Ok(user)
}

/// Stops a browser sign-in immediately, whatever step it is in.
#[tauri::command]
pub fn cancel_sign_in(state: State<'_, AppState>) {
    if let Some(cancel) = state.sign_in_cancel.lock().expect("sign-in lock").take() {
        cancel.notify_one();
        // the waiting callback listener holds a second waiter
        cancel.notify_waiters();
    }
}

/// Sign-in with a personal access token from the account page.
#[tauri::command]
pub async fn sign_in_token(state: State<'_, AppState>, token: String) -> CmdResult<Value> {
    let token = token.trim().to_string();
    if !token.starts_with("tc_pat_") {
        return Err(ApiError::local(
            "Personal access tokens start with tc_pat_ – create one on your account page.",
        ));
    }
    let mut api = api_of(&state)?;
    api.token = Some(token.clone());
    let me = api.json(Method::GET, "/me", None).await?;
    store_token(&state, token)?;
    remember_user(&state, &me);
    Ok(me)
}

#[tauri::command]
pub fn sign_out(state: State<'_, AppState>) -> CmdResult<()> {
    // the agent works with the token that is about to go
    stop_agent(&state);
    let mut s = state.settings.lock().expect("settings lock");
    s.save_token(&state.config_dir, None)
        .map_err(|e| ApiError::local(e.to_string()))
}

#[tauri::command]
pub fn set_skin(state: State<'_, AppState>, skin: String) -> CmdResult<()> {
    let skin: String = skin
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(32)
        .collect();
    update_settings(&state, |s| s.skin = Some(skin))?;
    Ok(())
}

#[tauri::command]
pub fn set_workspace_dir(state: State<'_, AppState>, dir: PathBuf) -> CmdResult<()> {
    if !dir.is_absolute() {
        return Err(ApiError::local("Choose an absolute folder."));
    }
    std::fs::create_dir_all(&dir)?;
    update_settings(&state, |s| s.workspace_dir = Some(dir))?;
    Ok(())
}

/// Generic JSON call: `api("GET", "/items?q=x", null)`.
#[tauri::command]
pub async fn api(
    state: State<'_, AppState>,
    method: String,
    path: String,
    body: Option<Value>,
) -> CmdResult<Value> {
    let method = match method.to_ascii_uppercase().as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PATCH" => Method::PATCH,
        "DELETE" => Method::DELETE,
        other => return Err(ApiError::local(format!("Unsupported method {other}."))),
    };
    // The app asks for its account at every start (shell.js): a sign-in kept
    // from an older version gets its account noted on the connection (#356).
    let is_me = method == Method::GET && path == "/me";
    let value = signed_in_api(&state)?
        .json(method, &path, body.as_ref())
        .await?;
    if is_me {
        remember_user(&state, &value);
    }
    Ok(value)
}

/// A question in the assistant sidebar (`POST /assistant`, in the user's
/// conversation). Unlike [`api`] it can be cancelled with
/// [`assistant_cancel`]: the request is dropped, so the server stops the run
/// and stores nothing.
#[tauri::command]
pub async fn assistant_send(state: State<'_, AppState>, body: Value) -> CmdResult<Value> {
    let api = signed_in_api(&state)?;
    let cancel = Arc::new(Notify::new());
    *state.assistant_cancel.lock().expect("assistant lock") = Some(cancel.clone());
    let result = tokio::select! {
        answer = api.json(Method::POST, "/assistant", Some(&body)) => answer,
        _ = cancel.notified() => Err(crate::abortable::cancelled("Cancelled.")),
    };
    state
        .assistant_cancel
        .lock()
        .expect("assistant lock")
        .take();
    result
}

/// Stops the running assistant question, if any.
#[tauri::command]
pub fn assistant_cancel(state: State<'_, AppState>) {
    if let Some(cancel) = state
        .assistant_cancel
        .lock()
        .expect("assistant lock")
        .take()
    {
        cancel.notify_one();
    }
}

/// Content of a file (`kind` = `file` for the current version or `version`)
/// as raw bytes – for the 3D viewer.
#[tauri::command]
pub async fn file_bytes(
    state: State<'_, AppState>,
    kind: String,
    id: String,
) -> CmdResult<tauri::ipc::Response> {
    let id: String = id
        .chars()
        .filter(|c| c.is_ascii_hexdigit() || *c == '-')
        .collect();
    let path = match kind.as_str() {
        "file" => format!("/files/{id}/content"),
        "version" => format!("/file-versions/{id}/content"),
        // 3D preview meshes, converted by the server
        "mesh" => format!("/files/{id}/mesh"),
        "version-mesh" => format!("/file-versions/{id}/mesh"),
        // preview image stored in Inventor / Solid Edge / SolidWorks files
        "thumbnail" => format!("/files/{id}/thumbnail"),
        // logos an administrator replaced (the id is not used)
        "branding-wide" => "/branding/wide".to_string(),
        "branding-square" => "/branding/square".to_string(),
        // screenshots and PDFs attached to a change request
        "request-attachment" => format!("/change-request-attachments/{id}/content"),
        _ => return Err(ApiError::local("Unknown file kind.")),
    };
    let bytes = signed_in_api(&state)?.bytes(&path).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Asks for a target file with the native save dialog (from Rust, so the web
/// view never chooses paths itself). `None` = cancelled.
async fn ask_save_path(
    app: &AppHandle,
    suggested: &str,
    filter: (&str, &[&str]),
) -> CmdResult<Option<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    let app = app.clone();
    let name = workspace::safe_name(suggested);
    let (label, extensions) = (
        filter.0.to_string(),
        filter.1.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
    );
    tokio::task::spawn_blocking(move || {
        let exts: Vec<&str> = extensions.iter().map(String::as_str).collect();
        app.dialog()
            .file()
            .set_file_name(&name)
            .add_filter(&label, &exts)
            .blocking_save_file()
            .and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(|e| ApiError::local(format!("The save dialog failed: {e}")))
}

/// "Pack and Go": downloads the ZIP of a revision and its structure to a
/// place the user chooses. Returns the path, or `null` when cancelled.
#[tauri::command]
pub async fn save_package(
    app: AppHandle,
    state: State<'_, AppState>,
    item: String,
    code: String,
    rule: Option<String>,
) -> CmdResult<Option<PathBuf>> {
    let api = signed_in_api(&state)?;
    let rule = rule.filter(|r| matches!(r.as_str(), "latest" | "released" | "as_released"));
    let suggested = format!(
        "{}_{}{}.zip",
        item,
        code,
        rule.as_deref().map(|r| format!("_{r}")).unwrap_or_default()
    );
    let Some(target) = ask_save_path(&app, &suggested, ("ZIP archive", &["zip"])).await? else {
        return Ok(None);
    };
    let mut path = format!(
        "/items/{}/revisions/{}/package",
        workspace::enc(&item),
        workspace::enc(&code)
    );
    if let Some(r) = rule {
        path.push_str(&format!("?rule={r}"));
    }
    // a fresh cancel switch for this download (`cancel_package` flips it);
    // `package-progress` events follow it: {received, total?}
    let cancel = Arc::new(AtomicBool::new(false));
    *state.package_cancel.lock().expect("package lock") = cancel.clone();
    let events = app.clone();
    let on_bytes = move |received: u64, total: Option<u64>| {
        let _ = events.emit(
            "package-progress",
            json!({ "received": received, "total": total }),
        );
    };
    api.download_to(&path, &target, &cancel, &on_bytes).await?;
    Ok(Some(target))
}

/// Stops the running Pack and Go download (also while the server is still
/// packing); the partial ZIP is removed and nothing is written.
#[tauri::command]
pub fn cancel_package(state: State<'_, AppState>) {
    state
        .package_cancel
        .lock()
        .expect("package lock")
        .store(true, Ordering::Relaxed);
}

/// Saves bytes from the web view (the STL export of the 3D viewer) to a file
/// the user chooses. The raw request body is the content; the header
/// `x-file-name` the suggested name. Returns the path, or `null`.
#[tauri::command]
pub async fn save_stl(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> CmdResult<Option<PathBuf>> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(ApiError::local("Expected the STL data as raw bytes."));
    };
    let name = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        // sent percent-encoded (encodeURIComponent)
        .map(|v| {
            url::form_urlencoded::parse(format!("n={v}").as_bytes())
                .map(|(_, v)| v.into_owned())
                .next()
                .unwrap_or_default()
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "model.stl".into());
    let bytes = bytes.clone();
    let Some(target) = ask_save_path(&app, &name, ("STL mesh", &["stl"])).await? else {
        return Ok(None);
    };
    tokio::fs::write(&target, bytes).await?;
    Ok(Some(target))
}

/// A progress reporter for Gets: `workspace-progress` events to the user
/// interface. Every Get starts with a fresh cancel flag.
fn progress_events(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> (
    Arc<AtomicBool>,
    impl FnMut(&workspace::Progress) + Send + 'static,
) {
    state.get_cancel.store(false, Ordering::Relaxed);
    let app = app.clone();
    (state.get_cancel.clone(), move |p: &workspace::Progress| {
        let _ = app.emit("workspace-progress", p);
    })
}

/// Check out: takes the lock on the server (on this revision only) and copies the files into the
/// working folder; the files of the revision become writable.
#[tauri::command]
pub async fn workspace_checkout(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: String,
    code: String,
) -> CmdResult<workspace::Downloaded> {
    usage(&state, |u| u.count("checkout"));
    let api = signed_in_api(&state)?;
    api.json(
        Method::POST,
        &format!(
            "/items/{}/revisions/{}/checkout",
            workspace::enc(&item),
            workspace::enc(&code)
        ),
        None,
    )
    .await?;
    let (cancel, mut progress) = progress_events(&app, &state);
    let root = workspace_root(&app, &state);
    // only the assembly is locked; its parts come read-only, as in Get
    if snapshot(&state).checkout_parts != Some(false) {
        return workspace::checkout_with_structure(
            &api,
            &root,
            &item,
            &code,
            "latest",
            &cancel,
            &mut progress,
        )
        .await;
    }
    workspace::download_revision(
        &api,
        &root,
        project.as_deref(),
        &item,
        &code,
        &cancel,
        &mut progress,
    )
    .await
}

/// Copies the current files of a revision into the working folder without
/// taking the lock (read-only on disk).
#[tauri::command]
pub async fn workspace_download(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: String,
    code: String,
) -> CmdResult<workspace::Downloaded> {
    let api = signed_in_api(&state)?;
    let (cancel, mut progress) = progress_events(&app, &state);
    workspace::download_revision(
        &api,
        &workspace_root(&app, &state),
        project.as_deref(),
        &item,
        &code,
        &cancel,
        &mut progress,
    )
    .await
}

/// "Get": a revision with its structure (BOM under the revision rule
/// `latest` or `released`) and the CAD files it references – only missing
/// and outdated files are downloaded, with progress events and cancel.
#[tauri::command]
pub async fn workspace_get(
    app: AppHandle,
    state: State<'_, AppState>,
    item: String,
    code: String,
    rule: Option<String>,
) -> CmdResult<workspace::Downloaded> {
    usage(&state, |u| u.count("get_files"));
    let api = signed_in_api(&state)?;
    let (cancel, mut progress) = progress_events(&app, &state);
    workspace::get_structure(
        &api,
        &workspace_root(&app, &state),
        &item,
        &code,
        rule.as_deref().unwrap_or("latest"),
        &cancel,
        &mut progress,
    )
    .await
}

/// Stops the running Get after the current chunk; the files copied so far
/// stay (the next Get continues with the rest).
#[tauri::command]
pub fn workspace_cancel(state: State<'_, AppState>) {
    state.get_cancel.store(true, Ordering::Relaxed);
}

/// The files of a revision in the working folder compared with the server.
#[tauri::command]
pub async fn workspace_status(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: String,
    code: String,
) -> CmdResult<workspace::Status> {
    let api = signed_in_api(&state)?;
    let root = workspace_root(&app, &state);
    let status = workspace::status(&api, &root, project.as_deref(), &item, &code).await?;
    // the SolidCraft plugin shows it (bridge folder, src/scbridge.rs); a
    // failed write only means the plugin shows older state
    if root.is_dir() {
        let value = scbridge::state(scbridge::now_secs(), &item, &code, &status);
        let _ = scbridge::write(&root, &scbridge::state_file(&item, &code), &value);
    }
    Ok(status)
}

/// Check in the chosen files (new versions) and release the lock.
#[tauri::command]
pub async fn workspace_checkin(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: String,
    code: String,
    files: Vec<String>,
    comment: String,
) -> CmdResult<Value> {
    usage(&state, |u| u.count("checkin"));
    let api = signed_in_api(&state)?;
    let root = workspace_root(&app, &state);
    let cancel = new_upload(&state);
    let progress = upload_progress_events(app);
    let transfer = Transfer {
        cancel: &cancel,
        progress: &progress,
    };
    workspace::check_in(
        &api,
        &root,
        project.as_deref(),
        &item,
        &code,
        &files,
        &comment,
        &transfer,
    )
    .await
}

/// Park the chosen files: new versions on the server, the lock stays (issue
/// #173, step 10 – work in progress survives a lost laptop).
#[tauri::command]
pub async fn workspace_park(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: String,
    code: String,
    files: Vec<String>,
    comment: String,
) -> CmdResult<Value> {
    usage(&state, |u| u.count("park"));
    let api = signed_in_api(&state)?;
    let root = workspace_root(&app, &state);
    let cancel = new_upload(&state);
    let progress = upload_progress_events(app);
    let transfer = Transfer {
        cancel: &cancel,
        progress: &progress,
    };
    workspace::park(
        &api,
        &root,
        project.as_deref(),
        &item,
        &code,
        &files,
        &comment,
        &transfer,
    )
    .await
}

/// Which of the given check-outs have local changes in the working folder
/// (before *Check in all* and before signing out – issue #173, step 10).
/// Compares one revision after the other; one that cannot be compared is
/// listed as unreadable, not as clean.
#[tauri::command]
pub async fn workspace_local_changes(
    app: AppHandle,
    state: State<'_, AppState>,
    revisions: Vec<layout::CheckoutRef>,
) -> CmdResult<layout::LocalChanges> {
    let api = signed_in_api(&state)?;
    let root = workspace_root(&app, &state);
    let mut results = Vec::with_capacity(revisions.len());
    for r in revisions {
        let files: Result<Vec<(String, String)>, String> =
            workspace::status(&api, &root, r.project.as_deref(), &r.item, &r.code)
                .await
                .map(|s| s.files.into_iter().map(|f| (f.name, f.state)).collect())
                .map_err(|e| e.to_string());
        results.push((r, files));
    }
    Ok(layout::local_changes(results))
}

/// *Local changes* (#416): the modified, outdated, conflicting, new and
/// missing files of every revision in the working folder – a local pass over
/// the state, then the Workspace status of at most
/// [`localchanges::MAX_REVISIONS`] revisions (`src/localchanges.rs`). Unlike
/// [`workspace_status`] it writes nothing: no SolidCraft bridge state, no
/// read-only flags, no `state.json` (`workspace::status_with(.., false)`).
#[tauri::command]
pub async fn local_changes_scan(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<localchanges::LocalChangesScan> {
    let api = signed_in_api(&state)?;
    let root = workspace_root(&app, &state);
    localchanges::scan(&api, &root).await
}

/// A fresh cancel switch for the upload that starts now (`cancel_upload`
/// flips the switch of the running one).
fn new_upload(state: &State<'_, AppState>) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    *state.upload_cancel.lock().expect("upload lock") = cancel.clone();
    cancel
}

/// Reports the progress of large uploads to the user interface
/// (event `upload-progress`: `{file, sent, total}`; Gets use
/// [`progress_events`]).
fn upload_progress_events(app: AppHandle) -> impl Fn(&str, u64, u64) + Send + Sync {
    move |file: &str, sent: u64, total: u64| {
        let _ = app.emit(
            "upload-progress",
            json!({ "file": file, "sent": sent, "total": total }),
        );
    }
}

/// Stops the running upload before its next chunk; chunks already on the
/// server are discarded and nothing is stored.
#[tauri::command]
pub fn cancel_upload(state: State<'_, AppState>) {
    state
        .upload_cancel
        .lock()
        .expect("upload lock")
        .store(true, Ordering::Relaxed);
}

/// Opens the folder of a revision (or a file of it) with the system's
/// default application; without item and revision the workspace root.
#[tauri::command]
pub async fn workspace_open(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: Option<String>,
    code: Option<String>,
    file: Option<String>,
) -> CmdResult<()> {
    let root = workspace_root(&app, &state);
    let target = match (item, code) {
        (Some(item), Some(code)) => {
            let api = signed_in_api(&state)?;
            match file {
                // Creo: the newest iteration (bracket.prt.7) of bracket.prt.
                Some(name) => {
                    let (dir, found) =
                        workspace::locate(&api, &root, project.as_deref(), &item, &code, &name)
                            .await?;
                    found.unwrap_or(dir)
                }
                None => {
                    let rev =
                        workspace::revision_files(&api, project.as_deref(), &item, &code).await?;
                    workspace::main_dir(&root, project.as_deref(), &rev.files, &rev.item_folder)
                }
            }
        }
        _ => root,
    };
    if !target.exists() {
        std::fs::create_dir_all(&target)?;
    }
    app.opener()
        .open_path(target.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| ApiError::local(format!("Could not open {}: {e}", target.display())))
}

/// Folders of the old layout `<group>/<item>/<revision>/` and what moving
/// them into the working-folder layout would do (nothing is changed).
#[tauri::command]
pub async fn workspace_migration_plan(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<workspace::MigrationPlan> {
    let api = signed_in_api(&state)?;
    workspace::migration_plan(&api, &workspace_root(&app, &state)).await
}

/// Moves the old layout into the working-folder layout (never overwriting
/// anything; every step in `.teamcraft/migration.log`).
#[tauri::command]
pub async fn workspace_migrate(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<workspace::MigrationPlan> {
    let api = signed_in_api(&state)?;
    workspace::migrate(&api, &workspace_root(&app, &state)).await
}

/// Opens a page of the web UI in the default browser (e.g. `/revisions/<id>`).
#[tauri::command]
pub fn open_web(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<()> {
    api::check_path(&path)?;
    let base = api_of(&state)?.base;
    app.opener()
        .open_url(format!("{base}{path}"), None::<&str>)
        .map_err(|e| ApiError::local(format!("The browser could not be opened: {e}")))
}

/// A newer release than this app (the update notice).
#[derive(Serialize)]
pub struct UpdateNotice {
    pub version: String,
    pub current: &'static str,
}

/// Looks for a newer release – on GitHub at most once a day, otherwise from
/// the last answer – unless the user switched the check off. Failures (no
/// network, rate limit) show no notice; the next try is a day later.
#[tauri::command]
pub async fn check_update(state: State<'_, AppState>) -> CmdResult<Option<UpdateNotice>> {
    let s = snapshot(&state);
    if !policy::current().update_check(s.update_check) {
        return Ok(None);
    }
    let mut latest = s.update_latest.clone();
    let now = update::now_secs();
    if update::due(s.update_checked_at, now) {
        // stamped first: a window reload meanwhile does not ask again
        update_settings(&state, |s| s.update_checked_at = Some(now))?;
        if let Ok(found) = update::latest_release().await {
            latest = found.clone();
            update_settings(&state, |s| s.update_latest = found)?;
        }
    }
    let current = env!("CARGO_PKG_VERSION");
    Ok(latest
        .filter(|v| {
            update::notice_due(
                v,
                current,
                s.update_dismissed.as_deref(),
                s.update_remind_until,
                now,
            )
        })
        .map(|version| UpdateNotice { version, current }))
}

/// *Skip this version*: hides the notice of this release version (a newer
/// one shows again).
#[tauri::command]
pub fn dismiss_update(state: State<'_, AppState>, version: String) -> CmdResult<()> {
    let version = update::tag_version(&version)
        .ok_or_else(|| ApiError::local(format!("'{version}' is not a version.")))?;
    update_settings(&state, |s| s.update_dismissed = Some(version))?;
    Ok(())
}

/// *Remind me later*: no notice for three days (#377).
#[tauri::command]
pub fn update_remind_later(state: State<'_, AppState>) -> CmdResult<()> {
    let until = update::now_secs() + update::REMIND_LATER_SECS;
    update_settings(&state, |s| s.update_remind_until = Some(until))?;
    Ok(())
}

/// The answer of *Help → Check for updates*.
#[derive(Serialize)]
pub struct UpdateCheck {
    pub current: &'static str,
    /// The latest release, `None` when none is published yet.
    pub latest: Option<String>,
    pub newer: bool,
}

/// *Help → Check for updates*: asks GitHub now – also when the daily check
/// is switched off – and says whether a newer release is out (#377).
#[tauri::command]
pub async fn check_update_now(state: State<'_, AppState>) -> CmdResult<UpdateCheck> {
    // policy CheckForUpdates = 0 (#378): no checks at all, also on demand
    if policy::current().check_for_updates == Some(false) {
        return Err(ApiError::local(MANAGED));
    }
    let latest = update::latest_release()
        .await
        .map_err(|e| ApiError::local(format!("The release check did not work: {e}")))?;
    let now = update::now_secs();
    update_settings(&state, |s| {
        s.update_checked_at = Some(now);
        s.update_latest = latest.clone();
        // asked on purpose: a running *Remind me later* ends
        s.update_remind_until = None;
    })?;
    let current = env!("CARGO_PKG_VERSION");
    Ok(UpdateCheck {
        newer: latest
            .as_deref()
            .is_some_and(|v| update::is_newer(v, current)),
        latest,
        current,
    })
}

/// What the update window shows for a release: its notes, how this copy was
/// installed and what *Update now* does for it.
#[derive(Serialize)]
pub struct UpdateOffer {
    pub version: String,
    pub current: &'static str,
    pub notes: String,
    pub install: update::InstallKind,
    /// The release file *Update now* downloads; `None` = release page only.
    pub asset: Option<String>,
    /// For package installations: how to update.
    pub package_hint: Option<&'static str>,
}

/// The release of `version` for the update window (#377).
#[tauri::command]
pub async fn update_offer(version: String) -> CmdResult<UpdateOffer> {
    let release = update::release(&version)
        .await
        .map_err(|e| ApiError::local(format!("The release could not be loaded: {e}")))?;
    let install = update::this_install();
    let names: Vec<String> = release.assets.iter().map(|(n, _)| n.clone()).collect();
    let asset = update::pick_asset(
        install,
        std::env::consts::ARCH,
        &release.version,
        localserver::sidecar().is_some(),
        &names,
    )
    .map(str::to_string);
    Ok(UpdateOffer {
        package_hint: (install == update::InstallKind::Package)
            .then(|| update::package_hint(std::env::var_os("FLATPAK_ID").is_some())),
        version: release.version,
        current: env!("CARGO_PKG_VERSION"),
        notes: release.notes,
        install,
        asset,
    })
}

/// The update file downloaded and verified last: the only one
/// [`update_install`] starts – never a path from the web view.
static VERIFIED_UPDATE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// *Update now* / *Download*: downloads this install's file of `version`
/// into the Downloads folder and checks it against the release's
/// `SHA256SUMS.txt`; a mismatch deletes it (#377). Returns the file's path.
#[tauri::command]
pub async fn update_download(app: AppHandle, version: String) -> CmdResult<String> {
    let release = update::release(&version)
        .await
        .map_err(|e| ApiError::local(format!("The release could not be loaded: {e}")))?;
    let names: Vec<String> = release.assets.iter().map(|(n, _)| n.clone()).collect();
    let name = update::pick_asset(
        update::this_install(),
        std::env::consts::ARCH,
        &release.version,
        localserver::sidecar().is_some(),
        &names,
    )
    .ok_or_else(|| ApiError::local("This release has no download for this installation."))?
    .to_string();
    let dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().home_dir())
        .map_err(|e| ApiError::local(e.to_string()))?;
    let file = update::download_verified(&release, &name, &dir)
        .await
        .map_err(ApiError::local)?;
    *VERIFIED_UPDATE.lock().expect("update lock") = Some(file.clone());
    Ok(file.to_string_lossy().into_owned())
}

/// Starts the verified update: a Windows installer runs and the app closes
/// (the installer replaces it); a `.dmg` opens for dragging the app to
/// *Applications*; anything else is shown in its folder.
#[tauri::command]
pub fn update_install(app: AppHandle) -> CmdResult<()> {
    let file = VERIFIED_UPDATE
        .lock()
        .expect("update lock")
        .clone()
        .filter(|f| f.is_file())
        .ok_or_else(|| ApiError::local("Download and verify the update first."))?;
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let installer = name.ends_with(".msi") || name.ends_with("-setup.exe");
    if installer || name.ends_with(".dmg") {
        app.opener()
            .open_path(file.to_string_lossy().to_string(), None::<&str>)
            .map_err(|e| ApiError::local(format!("The update could not be started: {e}")))?;
        if installer {
            app.exit(0);
        }
    } else {
        app.opener()
            .reveal_item_in_dir(&file)
            .map_err(|e| ApiError::local(format!("The folder could not be opened: {e}")))?;
    }
    Ok(())
}

/// *Settings → Workspace*: a check-out of an assembly gets its parts too.
#[tauri::command]
pub fn set_checkout_parts(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    update_settings(&state, |s| s.checkout_parts = Some(enabled))?;
    Ok(())
}

/// Switches the daily release check on or off.
#[tauri::command]
pub fn set_update_check(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    if policy::current().check_for_updates.is_some() {
        return Err(ApiError::local(MANAGED));
    }
    update_settings(&state, |s| s.update_check = Some(enabled))?;
    Ok(())
}

// ------------------------------------------------------------ crash reports

/// The crash reports waiting for the user's decision (src/crash.rs).
#[derive(Serialize)]
pub struct CrashReports {
    /// How many reports (panics, unexpected exits, UI error sessions).
    pub count: usize,
    pub kinds: Vec<String>,
    /// All reports as the plain, redacted text the user can share.
    pub text: String,
    /// Settings: ask at start-up (off: the reports are only kept locally).
    pub ask: bool,
}

/// Crash reports stored by an earlier session, or none.
#[tauri::command]
pub fn crash_reports(state: State<'_, AppState>) -> Option<CrashReports> {
    let dir = crate::crash::store_dir()?;
    let reports = crate::crash::pending(dir);
    if reports.is_empty() {
        return None;
    }
    Some(CrashReports {
        count: reports.len(),
        kinds: reports.iter().map(|r| r.kind.clone()).collect(),
        text: crate::crash::combined(&reports),
        ask: policy::current().crash_prompt(snapshot(&state).crash_prompt),
    })
}

/// Opens a prefilled new issue on GitHub with the reports – the user reviews
/// and submits it there – and removes them here. The address is built from
/// the stored reports, never from text the web view passes in.
#[tauri::command]
pub fn crash_reports_open_issue(app: AppHandle) -> CmdResult<()> {
    let dir =
        crate::crash::store_dir().ok_or_else(|| ApiError::local("No crash report is stored."))?;
    let reports = crate::crash::pending(dir);
    if reports.is_empty() {
        return Err(ApiError::local("No crash report is stored."));
    }
    let url = crate::crash::issue_url(&crate::crash::combined(&reports));
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| ApiError::local(format!("The browser could not be opened: {e}")))?;
    crate::crash::discard(dir);
    Ok(())
}

/// Deletes the stored crash reports without sharing them.
#[tauri::command]
pub fn crash_reports_discard() {
    if let Some(dir) = crate::crash::store_dir() {
        crate::crash::discard(dir);
    }
}

/// Settings → ask after a crash whether to share the report.
#[tauri::command]
pub fn set_crash_prompt(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    if policy::current().crash_report_prompt.is_some() {
        return Err(ApiError::local(MANAGED));
    }
    update_settings(&state, |s| s.crash_prompt = Some(enabled))?;
    Ok(())
}

/// An uncaught error of the user interface (js/crash.js); redacted, deduplicated
/// and capped per session. `true` when it was recorded.
#[tauri::command]
pub fn record_ui_error(message: String, stack: Option<String>, source: Option<String>) -> bool {
    crate::crash::store_dir().is_some_and(|dir| {
        crate::crash::record_ui_error(
            dir,
            &message,
            stack.as_deref().unwrap_or(""),
            source.as_deref().unwrap_or(""),
        )
    })
}

/// Opens the release page in the default browser (a fixed address – the app
/// itself never downloads or installs anything).
#[tauri::command]
pub fn open_release_page(app: AppHandle) -> CmdResult<()> {
    app.opener()
        .open_url(update::RELEASES_PAGE, None::<&str>)
        .map_err(|e| ApiError::local(format!("The browser could not be opened: {e}")))
}

/// Smallest and largest zoom factor of the user interface.
pub const ZOOM_RANGE: (f64, f64) = (0.5, 2.5);

pub fn clamp_zoom(level: f64) -> f64 {
    if level.is_finite() {
        level.clamp(ZOOM_RANGE.0, ZOOM_RANGE.1)
    } else {
        1.0
    }
}

/// Zooms the user interface (Ctrl + mouse wheel, Ctrl + +/-/0) and remembers
/// the factor for the next start.
#[tauri::command]
pub fn set_zoom(app: AppHandle, state: State<'_, AppState>, level: f64) -> CmdResult<f64> {
    let level = clamp_zoom(level);
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_zoom(level)
            .map_err(|e| ApiError::local(format!("Zoom failed: {e}")))?;
    }
    update_settings(&state, |s| s.zoom = Some(level))?;
    Ok(level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_is_clamped() {
        assert_eq!(clamp_zoom(1.2), 1.2);
        assert_eq!(clamp_zoom(0.1), ZOOM_RANGE.0);
        assert_eq!(clamp_zoom(9.0), ZOOM_RANGE.1);
        assert_eq!(clamp_zoom(f64::NAN), 1.0);
    }

    #[test]
    fn only_an_offered_agent_backend_can_be_picked() {
        use acp::AgentBackend::{Company, Local};
        assert!(check_backend(Company, &[Company, Local]).is_ok());
        assert!(check_backend(Local, &[Local]).is_ok());
        let off = check_backend(Local, &[Company]).unwrap_err();
        assert!(off.contains("TEAMCRAFT_LOCAL_AGENTS=off"), "{off}");
        let none = check_backend(Company, &[Local]).unwrap_err();
        assert!(none.contains("no company AI"), "{none}");
        assert!(check_backend(Local, &[]).is_err());
    }

    /// The app starts its local server only while it is the configured one.
    #[test]
    fn the_local_server_is_in_use_only_at_its_own_address() {
        let with = |url: Option<&str>, port: Option<u16>| Settings {
            server_url: url.map(String::from),
            local_server_port: port,
            ..Settings::default()
        };
        assert!(local_server_in_use(&with(
            Some("http://127.0.0.1:18080"),
            Some(18080)
        )));
        assert!(!local_server_in_use(&with(
            Some("http://127.0.0.1:18080"),
            Some(18081)
        )));
        assert!(!local_server_in_use(&with(
            Some("http://127.0.0.1:18080"),
            None
        )));
        assert!(!local_server_in_use(&with(
            Some("https://plm.example.org"),
            Some(18080)
        )));
        assert!(!local_server_in_use(&with(None, Some(18080))));
    }

    #[test]
    fn the_policy_names_backend_and_choices() {
        let mut s = Settings {
            server_url: Some("https://plm.example.org".into()),
            ..Settings::default()
        };
        let me = json!({ "username": "lina", "capabilities": { "assistant": true, "local_agents": true } });
        // both offered, no agent set up, nothing picked: the company AI
        let p = policy_of(&me, &s);
        assert!(p.allowed && p.company);
        assert_eq!(p.backend, "company");
        assert_eq!(p.available, ["company", "local"]);
        s.remember_agent_backend("lina", acp::AgentBackend::Local);
        assert_eq!(policy_of(&me, &s).backend, "local");
        // the administrator turns local agents off: the pick no longer counts
        let off = json!({ "username": "lina", "capabilities": { "assistant": true, "local_agents": false } });
        let p = policy_of(&off, &s);
        assert!(!p.allowed && p.message.is_some());
        assert_eq!(p.backend, "company");
        assert_eq!(p.available, ["company"]);
    }
}

/// Uploads local files into a revision the user has checked out: the files
/// of a new item, or of an item created by the folder import. Creo
/// iterations are uploaded under their logical name (`bracket.prt.7` →
/// `bracket.prt`). `folders` (file name → relative folder, from the folder
/// import) gives every file its place in the project group, so the working
/// folder later mirrors the original folder structure.
///
/// For *New* from files (#359): `refuse_clash` lets the server refuse a file
/// another item of the group has at the same place, `refuse_same_name` one
/// whose name another item uses anywhere in the group – checked atomically
/// while storing (409, nothing stored). With `track`, the uploaded files that
/// sit at their place in the working folder are recorded as the base of the
/// revision (`project` = its group), see `workspace::record_uploaded`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn upload_files(
    app: AppHandle,
    state: State<'_, AppState>,
    item: String,
    code: String,
    paths: Vec<PathBuf>,
    comment: Option<String>,
    folders: Option<HashMap<String, String>>,
    project: Option<String>,
    track: Option<bool>,
    refuse_clash: Option<bool>,
    refuse_same_name: Option<bool>,
) -> CmdResult<Value> {
    let api = signed_in_api(&state)?;
    let files = upload_names(&paths)?;
    if files.is_empty() {
        return Ok(Value::Null);
    }
    let folders: serde_json::Map<String, Value> = folders
        .unwrap_or_default()
        .into_iter()
        .map(|(name, folder)| (name, Value::String(layout::folder_for_upload(&folder))))
        .collect();
    let mut fields = vec![
        ("comment", comment.unwrap_or_default()),
        ("folders", Value::Object(folders).to_string()),
    ];
    if refuse_clash == Some(true) {
        fields.push(("refuse_clash", "true".to_string()));
    }
    if refuse_same_name == Some(true) {
        fields.push(("refuse_same_name", "true".to_string()));
    }
    let cancel = new_upload(&state);
    let progress = upload_progress_events(app.clone());
    let transfer = Transfer {
        cancel: &cancel,
        progress: &progress,
    };
    let result = api
        .send_files(
            &format!(
                "/items/{}/revisions/{}/files",
                workspace::enc(&item),
                workspace::enc(&code)
            ),
            &item,
            &code,
            &files,
            &fields,
            &transfer,
        )
        .await?;
    if track == Some(true) {
        // best effort: the upload itself has succeeded
        let root = workspace_root(&app, &state);
        let _ =
            workspace::record_uploaded(&api, &root, project.as_deref(), &item, &code, &files).await;
    }
    Ok(result)
}

/// The most files *New* takes at once.
const MAX_NEW_ITEM_FILES: usize = 200;

/// Where a file picked for *New* is (#359): in the working folder below a
/// project group folder – then that group and the folder inside it are the
/// item's place, taken from the file instead of typed – or elsewhere.
#[derive(Clone, Debug, Serialize)]
pub struct NewItemPlace {
    pub path: PathBuf,
    /// File name as the server stores it: composed, a Creo iteration under
    /// its logical name.
    pub name: String,
    /// Below `<working folder>/<group>/`.
    pub inside: bool,
    /// The group folder the file is in (its code), when inside.
    pub project: Option<String>,
    /// Folder inside the group (`""` = its top); `""` when outside.
    pub folder: String,
    pub sha256: String,
    pub size: u64,
    /// `ITEM/REV` when the working folder tracks this file already.
    pub tracked: Option<String>,
    /// `ITEM/REV` when the file was added with *Add files…* and is not
    /// checked in yet.
    pub pending: Option<String>,
}

/// For each picked file: where it is, its SHA-256 and whether it belongs to
/// an item already (`state.json`, `added.json`).
#[tauri::command]
pub async fn new_item_places(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<PathBuf>,
) -> CmdResult<Vec<NewItemPlace>> {
    if paths.len() > MAX_NEW_ITEM_FILES {
        return Err(ApiError::local(format!(
            "Choose at most {MAX_NEW_ITEM_FILES} files."
        )));
    }
    let root = workspace_root(&app, &state);
    let known = layout::State::load(&root);
    let added = addfiles::load(&root);
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let meta = tokio::fs::metadata(&path).await?;
        if !meta.is_file() {
            return Err(ApiError::local(format!(
                "{} is not a file.",
                path.display()
            )));
        }
        let disk = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
            ApiError::local(format!("{} has no usable file name.", path.display()))
        })?;
        let (logical, _) = workspace::logical_name(disk);
        let name = layout::nfc(logical);
        let rel = layout::rel_below(&root, &path);
        let (inside, project, folder) = match rel.as_deref() {
            Some(rel) => {
                let parts: Vec<&str> = rel.split('/').collect();
                if parts.len() >= 2 && !parts[0].starts_with('.') {
                    let folder = parts[1..parts.len() - 1]
                        .iter()
                        .copied()
                        .map(layout::nfc)
                        .collect::<Vec<_>>()
                        .join("/");
                    (true, Some(layout::nfc(parts[0])), folder)
                } else {
                    (false, None, String::new())
                }
            }
            None => (false, None, String::new()),
        };
        let key = rel.as_deref().map(layout::key_of);
        let tracked = key
            .as_ref()
            .and_then(|k| known.files.get(k))
            .map(|e| format!("{}/{}", e.item, e.revision));
        let pending = key.as_ref().and_then(|k| {
            added
                .iter()
                .find(|p| layout::key_of(&p.rel) == *k)
                .map(|p| format!("{}/{}", p.item, p.revision))
        });
        out.push(NewItemPlace {
            sha256: workspace::sha256_of(&path).await?,
            size: meta.len(),
            path,
            name,
            inside,
            project,
            folder,
            tracked,
            pending,
        });
    }
    Ok(out)
}

/// Copies files picked from outside the working folder into
/// `<working folder>/<project>/<folder>/` for *New* (#359): the originals are
/// never moved or changed. Nothing is copied when a target exists already –
/// no file in the working folder is ever overwritten. Returns the copies.
#[tauri::command]
pub async fn new_item_copy(
    app: AppHandle,
    state: State<'_, AppState>,
    project: String,
    folder: String,
    paths: Vec<PathBuf>,
) -> CmdResult<Vec<PathBuf>> {
    let group = project.trim();
    if group.is_empty() {
        return Err(ApiError::local("Choose a project group."));
    }
    if paths.len() > MAX_NEW_ITEM_FILES {
        return Err(ApiError::local(format!(
            "Choose at most {MAX_NEW_ITEM_FILES} files."
        )));
    }
    let root = workspace_root(&app, &state);
    let folder = layout::folder_for_upload(&folder);
    // plan everything first: refuse before a single byte is copied
    let mut plan: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(paths.len());
    for src in paths {
        let disk = src.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
            ApiError::local(format!("{} has no usable file name.", src.display()))
        })?;
        let rel = layout::local_rel(Some(group), &folder, &layout::nfc(disk));
        let dst = layout::join_rel(&root, &rel);
        if dst == src {
            plan.push((src, dst));
            continue;
        }
        if tokio::fs::try_exists(&dst).await? {
            return Err(ApiError::local(format!(
                "{rel} exists in the working folder already – nothing was copied. \
                 Choose that file instead, or rename the new one."
            )));
        }
        if plan.iter().any(|(_, d)| {
            layout::key_of(&d.to_string_lossy()) == layout::key_of(&dst.to_string_lossy())
        }) {
            return Err(ApiError::local(format!(
                "Two of the chosen files would both become {rel} – nothing was copied."
            )));
        }
        plan.push((src, dst));
    }
    let mut copied: Vec<PathBuf> = Vec::new();
    for (src, dst) in &plan {
        if src == dst {
            continue;
        }
        let step = async {
            if let Some(parent) = dst.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::copy(src, dst).await?;
            // the new item is checked out to the user: its files are writable
            workspace::set_read_only(dst, false)?;
            Ok::<_, std::io::Error>(())
        };
        if let Err(e) = step.await {
            // leave nothing half done behind
            for done in &copied {
                let _ = tokio::fs::remove_file(done).await;
            }
            let _ = tokio::fs::remove_file(dst).await;
            return Err(ApiError::local(format!(
                "{} could not be copied: {e}",
                src.display()
            )));
        }
        copied.push(dst.clone());
    }
    Ok(plan.into_iter().map(|(_, dst)| dst).collect())
}

/// Largest attachment of a change request (the server enforces the same).
const MAX_REQUEST_ATTACHMENT: u64 = 10 * 1024 * 1024;

/// Attaches a screenshot, mark-up or PDF (chosen in the native file dialog)
/// to a change request. The server accepts images and PDF only, decided by
/// the content.
#[tauri::command]
pub async fn upload_request_attachment(
    state: State<'_, AppState>,
    number: String,
    path: PathBuf,
) -> CmdResult<Value> {
    let api = signed_in_api(&state)?;
    let size = tokio::fs::metadata(&path).await?.len();
    if size > MAX_REQUEST_ATTACHMENT {
        return Err(ApiError::local(format!(
            "{} is larger than 10 MiB.",
            path.display()
        )));
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| ApiError::local(format!("{} has no usable file name.", path.display())))?
        .to_string();
    api.multipart(
        &format!("/change-requests/{}/attachments", workspace::enc(&number)),
        &[(name, path.clone())],
        &[],
    )
    .await
}

/// *Import BOM…*: asks for the BOM table or STEP assembly with the native
/// open dialog (from Rust, so the web view never chooses paths itself).
/// `None` = cancelled.
#[tauri::command]
pub async fn pick_bom_file(app: AppHandle) -> CmdResult<Option<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("BOM to import")
            .add_filter(
                "BOM tables, STEP assemblies and SolidCraft designs",
                bomimport::EXTENSIONS,
            )
            .blocking_pick_file()
            .and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(|e| ApiError::local(format!("The file dialog failed: {e}")))
}

/// *Import BOM…*: sends the chosen file to the server's BOM import – with
/// `dry_run` for the preview (the plan), then for real.
#[tauri::command]
pub async fn import_bom(
    state: State<'_, AppState>,
    path: PathBuf,
    request: bomimport::Request,
    dry_run: bool,
) -> CmdResult<Value> {
    let api = signed_in_api(&state)?;
    bomimport::send(&api, &path, &request, dry_run).await
}

/// `(name on the server, local path)` for every path.
fn upload_names(paths: &[PathBuf]) -> CmdResult<Vec<(String, PathBuf)>> {
    paths
        .iter()
        .map(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
                ApiError::local(format!("{} has no usable file name.", path.display()))
            })?;
            let (logical, _) = workspace::logical_name(name);
            Ok((logical.to_string(), path.clone()))
        })
        .collect()
}

#[cfg(test)]
mod upload_tests {
    use super::*;

    #[test]
    fn upload_names_drop_creo_iterations() {
        let names = upload_names(&[
            PathBuf::from("/cad/frame.asm.12"),
            PathBuf::from("/cad/Bracket.ipt"),
        ])
        .unwrap();
        assert_eq!(names[0].0, "frame.asm");
        assert_eq!(names[1].0, "Bracket.ipt");
    }
}

/// The CAD programs on this computer that can export STEP for the 3D
/// preview (Windows: Inventor, Solid Edge; elsewhere none).
#[tauri::command]
pub async fn cad_programs() -> Vec<cadexport::CadProgram> {
    tokio::task::spawn_blocking(cadexport::available)
        .await
        .unwrap_or_default()
}

/// Exports a workspace file of a revision to STEP with the local Inventor or
/// Solid Edge; the STEP file lands next to it (and is checked in with it).
/// Returns the name of the STEP file.
#[tauri::command]
pub async fn cad_export_step(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: String,
    code: String,
    file: String,
) -> CmdResult<String> {
    if file.contains(['/', '\\']) || file == ".." || file.is_empty() {
        return Err(ApiError::local(format!("'{file}' is not a file name.")));
    }
    let api = signed_in_api(&state)?;
    let (_, found) = workspace::locate(
        &api,
        &workspace_root(&app, &state),
        project.as_deref(),
        &item,
        &code,
        &file,
    )
    .await?;
    let Some(input) = found.filter(|p| p.is_file()) else {
        return Err(ApiError::local(format!(
            "{file} is not in the working folder – check out or use “Get files” first."
        )));
    };
    let program = cadexport::program_for(&input);
    let output = tokio::task::spawn_blocking(move || cadexport::export_step(&input))
        .await
        .map_err(|e| ApiError::local(format!("The export stopped: {e}")))?
        .map_err(ApiError::local)?;
    // usage statistics (#383): which program exported, never the file
    let name = match program {
        Some(cadexport::CadProgram::Inventor) => "inventor",
        Some(cadexport::CadProgram::SolidEdge) => "solid-edge",
        Some(cadexport::CadProgram::SolidCraft) => "solidcraft",
        None => "",
    };
    usage(&state, |u| u.count_export(name));
    Ok(output
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string())
}

/// *Skip the export* in the check-in dialog (#342): stops the running STEP
/// export; the check-in goes on without it.
#[tauri::command]
pub fn cad_export_cancel() {
    cadexport::cancel();
}

/// Writes the item number, revision and name (and TeamCraft's ids) into the
/// properties of a SolidCraft design of my check-out, with SolidCraft's
/// command-line tool (#369). Refused for a design that is read-only or open
/// in SolidCraft.
#[tauri::command]
pub async fn solidcraft_write_item_data(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: String,
    code: String,
    file: String,
    data: scprops::ItemData,
) -> CmdResult<()> {
    if file.contains(['/', '\\']) || file == ".." || file.is_empty() {
        return Err(ApiError::local(format!("'{file}' is not a file name.")));
    }
    let api = signed_in_api(&state)?;
    let (_, found) = workspace::locate(
        &api,
        &workspace_root(&app, &state),
        project.as_deref(),
        &item,
        &code,
        &file,
    )
    .await?;
    let Some(design) = found.filter(|p| p.is_file()) else {
        return Err(ApiError::local(format!(
            "{file} is not in the working folder – check out or use “Get files” first."
        )));
    };
    let server = api.base.clone();
    tokio::task::spawn_blocking(move || scprops::write(&design, &data, &server))
        .await
        .map_err(|e| ApiError::local(format!("SolidCraft stopped: {e}")))?
        .map_err(ApiError::local)
}

/// The CAD programs found on this computer (for *Check out and open*).
#[tauri::command]
pub async fn cad_installed() -> Vec<cadlaunch::Installed> {
    tokio::task::spawn_blocking(|| cadlaunch::installed().to_vec())
        .await
        .unwrap_or_default()
}

/// Which program would open a file (shown next to *Check out and open*).
#[tauri::command]
pub async fn cad_open_plan(wish: cadlaunch::Wish) -> cadlaunch::OpenPlan {
    tokio::task::spawn_blocking(move || cadlaunch::plan(cadlaunch::installed(), &wish))
        .await
        .unwrap_or_else(|_| cadlaunch::plan(&[], &cadlaunch::Wish::default()))
}

/// Opens a workspace file of a revision with the CAD program chosen for it
/// (the project group's default, else the newest installed program that
/// fits the file, else the operating system's default application).
/// An Inventor that already runs with the working folder's project opens it
/// as a new window (#350). Returns how it was opened.
#[tauri::command]
pub async fn workspace_open_cad(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: String,
    code: String,
    file: String,
    wish: cadlaunch::Wish,
) -> CmdResult<cadlaunch::OpenPlan> {
    usage(&state, |u| u.count("open_cad"));
    let api = signed_in_api(&state)?;
    let root = workspace_root(&app, &state);
    let (_, found) =
        workspace::locate(&api, &root, project.as_deref(), &item, &code, &file).await?;
    let Some(path) = found.filter(|p| p.is_file()) else {
        return Err(ApiError::local(format!(
            "{file} is not in the working folder – check out or use “Get files” first."
        )));
    };
    let mut plan =
        tokio::task::spawn_blocking(move || cadlaunch::plan(cadlaunch::installed(), &wish))
            .await
            .map_err(|e| ApiError::local(format!("The program could not be chosen: {e}")))?;
    if let Some(program) = plan.program.clone() {
        // an Inventor that already runs with this working folder's project
        // opens the file as a new window instead of a second instance (#350)
        let (asked, file_path) = (program.clone(), path.clone());
        let reuse = tokio::task::spawn_blocking(move || {
            cadlaunch::open_in_running(&asked, &file_path, &root)
        })
        .await
        .unwrap_or(cadlaunch::Reuse::NewInstance(String::new()));
        match reuse {
            cadlaunch::Reuse::Opened => {
                plan.reused = true;
                return Ok(plan);
            }
            cadlaunch::Reuse::NewInstance(note) => plan.note = note,
        }
        match cadlaunch::launch(&program, &path) {
            Ok(()) => return Ok(plan),
            Err(e) => {
                // the system's program as a last resort
                plan.reason = format!("{e} – opened with the default application instead.");
                plan.label = "Default app".into();
                plan.program = None;
                plan.fallback = true;
            }
        }
    }
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| ApiError::local(format!("Could not open {}: {e}", path.display())))?;
    Ok(plan)
}

/// Scans a CAD folder for the import wizard (nothing is uploaded).
#[tauri::command]
pub async fn scan_folder(path: PathBuf) -> CmdResult<folderimport::Scan> {
    tokio::task::spawn_blocking(move || folderimport::scan(&path))
        .await
        .map_err(|e| ApiError::local(format!("The scan stopped: {e}")))?
        .map_err(ApiError::local)
}

// ---------------------------------------------------------------- ACP agents

/// The agent session the desktop runs (at most one).
pub struct RunningAgent {
    /// Number of the session; its events carry it.
    pub session: u64,
    /// Name of the agent in the settings.
    pub name: String,
    pub agent: Arc<acp::Session>,
}

/// Events of an agent session go to the window as `agent` events.
struct AgentSink {
    app: AppHandle,
    session: u64,
}

impl acp::Sink for AgentSink {
    fn emit(&self, event: acp::Event) {
        let _ = self.app.emit(
            "agent",
            acp::Envelope {
                session: self.session,
                event,
            },
        );
    }
}

/// An agent as the settings dialog edits it: the arguments as one line.
#[derive(Serialize, Deserialize)]
pub struct AgentEntry {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: String,
}

#[derive(Serialize)]
pub struct AgentProfiles {
    pub agents: Vec<AgentEntry>,
    pub selected: Option<String>,
    /// Ask before read-only TeamCraft tools too (default: allowed once
    /// without a prompt).
    pub ask_read_only: bool,
}

/// Arguments as one line again (quoted where they contain spaces).
fn join_args(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.is_empty() || a.chars().any(char::is_whitespace) {
                format!("\"{a}\"")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn profiles_of(s: &Settings) -> AgentProfiles {
    AgentProfiles {
        agents: s
            .agents
            .iter()
            .map(|a| AgentEntry {
                name: a.name.clone(),
                command: a.command.clone(),
                args: join_args(&a.args),
            })
            .collect(),
        selected: s.agent.clone(),
        ask_read_only: s.agent_ask_read_only,
    }
}

/// The configured agents (Settings → Agents).
#[tauri::command]
pub fn agent_profiles(state: State<'_, AppState>) -> AgentProfiles {
    profiles_of(&snapshot(&state))
}

/// Most agents the settings hold.
const MAX_AGENTS: usize = 20;

fn parse_profiles(agents: Vec<AgentEntry>) -> Result<Vec<acp::AgentProfile>, String> {
    if agents.len() > MAX_AGENTS {
        return Err(format!("At most {MAX_AGENTS} agents."));
    }
    let mut profiles: Vec<acp::AgentProfile> = Vec::new();
    for a in agents {
        let profile = acp::AgentProfile {
            name: a.name.trim().to_string(),
            command: a.command.trim().to_string(),
            args: acp::split_args(&a.args),
        };
        profile.validate()?;
        if profiles.iter().any(|p| p.name == profile.name) {
            return Err(format!("Two agents are called “{}”.", profile.name));
        }
        profiles.push(profile);
    }
    Ok(profiles)
}

#[tauri::command]
pub fn set_agent_profiles(
    state: State<'_, AppState>,
    agents: Vec<AgentEntry>,
    ask_read_only: Option<bool>,
) -> CmdResult<AgentProfiles> {
    let profiles = parse_profiles(agents).map_err(ApiError::local)?;
    // asking more takes effect at once, even if saving fails
    if ask_read_only == Some(true) {
        set_running_ask_read_only(&state, true);
    }
    let s = update_settings(&state, |s| {
        if let Some(ask) = ask_read_only {
            s.agent_ask_read_only = ask;
        }
        if !s
            .agent
            .as_ref()
            .is_some_and(|name| profiles.iter().any(|p| &p.name == name))
        {
            s.agent = profiles.first().map(|p| p.name.clone());
        }
        s.agents = profiles;
    })?;
    // a running agent follows the saved setting
    set_running_ask_read_only(&state, s.agent_ask_read_only);
    Ok(profiles_of(&s))
}

fn set_running_ask_read_only(state: &State<'_, AppState>, ask: bool) {
    if let Some(running) = state.agent.lock().expect("agent lock").as_ref() {
        running.agent.set_ask_read_only(ask);
    }
}

/// The read-only tools of TeamCraft's MCP server by its own `tools/list`
/// (`annotations.readOnlyHint`), asked with the signed-in user's token – the
/// list the agent session about to start uses. `None` when the server does
/// not answer with a list (old server, refusal, network error): then every
/// tool call asks, as before.
async fn read_only_tools(api: &Api) -> Option<crate::autoallow::ReadOnlyTools> {
    let list = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
    let answer = api.json(Method::POST, "/mcp", Some(&list)).await.ok()?;
    crate::autoallow::ReadOnlyTools::from_tools_list(&answer)
}

#[derive(Serialize)]
pub struct AgentState {
    pub session: u64,
    pub name: String,
    pub info: acp::AgentInfo,
    /// A prompt turn is running.
    pub busy: bool,
    /// The models the agent offers, if it offers a choice.
    pub models: Option<acp::Models>,
}

fn describe(running: &RunningAgent) -> AgentState {
    AgentState {
        session: running.session,
        name: running.name.clone(),
        info: running.agent.info.clone(),
        busy: running.agent.busy(),
        models: running.agent.models(),
    }
}

fn current_agent(state: &State<'_, AppState>) -> CmdResult<Arc<acp::Session>> {
    state
        .agent
        .lock()
        .expect("agent lock")
        .as_ref()
        .map(|r| r.agent.clone())
        .ok_or_else(|| ApiError::local("No agent is running – start one first."))
}

fn stop_agent(state: &State<'_, AppState>) {
    let running = state.agent.lock().expect("agent lock").take();
    if let Some(running) = running {
        running.agent.stop();
    }
}

/// The running agent session, if any (after a reload of the window).
#[tauri::command]
pub fn agent_status(state: State<'_, AppState>) -> Option<AgentState> {
    state
        .agent
        .lock()
        .expect("agent lock")
        .as_ref()
        .map(describe)
}

/// What the Agent view may use: the server's policy and the user's pick.
#[derive(Serialize)]
pub struct AgentPolicy {
    /// The server lets this user run local agents.
    pub allowed: bool,
    /// The server runs its assistant – the company AI.
    pub company: bool,
    /// The Agent view's backend: `company`, `local` or `off` (no AI).
    pub backend: &'static str,
    /// The backends the user may switch between (company first).
    pub available: Vec<&'static str>,
    /// Why local agents are not allowed, naming the administrator setting.
    pub message: Option<String>,
}

fn policy_of(me: &Value, s: &Settings) -> AgentPolicy {
    let gate = acp::local_agents_gate(me);
    let user = me["username"].as_str().unwrap_or_default();
    let (backend, available) =
        acp::choose_backend(me, !s.agents.is_empty(), s.stored_agent_backend(user));
    AgentPolicy {
        allowed: gate.is_ok(),
        company: acp::company_ai(me),
        backend: backend.as_str(),
        available: available.iter().map(|b| b.as_str()).collect(),
        message: gate.err(),
    }
}

/// May the user pick `wanted`? Only a backend the policy offers.
fn check_backend(wanted: acp::AgentBackend, available: &[acp::AgentBackend]) -> Result<(), String> {
    if available.contains(&wanted) {
        return Ok(());
    }
    Err(match wanted {
        acp::AgentBackend::Local => acp::LOCAL_AGENTS_OFF.to_string(),
        _ => "This TeamCraft server has no company AI configured.".to_string(),
    })
}

/// Asks the server what the Agent view may use – local agents
/// (`TEAMCRAFT_LOCAL_AGENTS`) and the company AI (the server's assistant) –
/// and ends a running agent when local agents are off. The user interface
/// calls it after sign-in, when the Agent view opens and every minute; with
/// local agents off the Agent view runs on the company AI, or shows a notice
/// without one. `agent_start`, `agent_prompt` and an allowing
/// `agent_permission` check again.
#[tauri::command]
pub async fn agent_policy(state: State<'_, AppState>) -> CmdResult<AgentPolicy> {
    let api = signed_in_api(&state)?;
    let me = api.json(Method::GET, "/me", None).await?;
    if acp::local_agents_gate(&me).is_err() {
        stop_agent(&state);
    }
    Ok(policy_of(&me, &snapshot(&state)))
}

/// Asks the server again whether local agents are allowed, before a running
/// agent gets a message or a permission: an administrator who turned them
/// off while the app was open ends the agent here, not only at the next
/// sign-in (the user interface also asks every minute, agent_policy).
async fn ensure_local_agents(state: &State<'_, AppState>) -> CmdResult<()> {
    let api = signed_in_api(state)?;
    let me = api.json(Method::GET, "/me", None).await?;
    acp::local_agents_gate(&me).map_err(|message| {
        stop_agent(state);
        ApiError::local(message)
    })
}

/// The user switches the Agent view between the company AI and the local
/// agent; remembered per server and user. The server is asked again, so a
/// backend the policy does not offer (any more) is refused.
#[tauri::command]
pub async fn set_agent_backend(
    state: State<'_, AppState>,
    backend: String,
) -> CmdResult<AgentPolicy> {
    let wanted = acp::AgentBackend::parse(&backend)
        .ok_or_else(|| ApiError::local(format!("Unknown agent backend “{backend}”.")))?;
    let api = signed_in_api(&state)?;
    let me = api.json(Method::GET, "/me", None).await?;
    let has_agents = !snapshot(&state).agents.is_empty();
    let (_, available) = acp::choose_backend(&me, has_agents, None);
    check_backend(wanted, &available).map_err(ApiError::local)?;
    let user = me["username"].as_str().unwrap_or_default().to_string();
    let s = update_settings(&state, |s| s.remember_agent_backend(&user, wanted))?;
    Ok(policy_of(&me, &s))
}

/// Starts a configured agent in the workspace folder and opens a session
/// with TeamCraft's MCP server (the desktop's token, so the user's rights).
#[tauri::command]
pub async fn agent_start(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<AgentState> {
    let api = signed_in_api(&state)?;
    // the server's administrator may have turned local agents off: asked
    // fresh at every start, so a changed setting needs no restart of the app
    let me = api.json(Method::GET, "/me", None).await?;
    acp::local_agents_gate(&me).map_err(ApiError::local)?;
    let profile = snapshot(&state)
        .agents
        .into_iter()
        .find(|a| a.name == name)
        .ok_or_else(|| ApiError::local(format!("There is no agent “{name}” in the settings.")))?;
    stop_agent(&state);
    let root = workspace_root(&app, &state);
    std::fs::create_dir_all(&root)?;
    let session = state.agent_seq.fetch_add(1, Ordering::SeqCst) + 1;
    let sink = Arc::new(AgentSink {
        app: app.clone(),
        session,
    });
    let ask_read_only = snapshot(&state).agent_ask_read_only;
    // fetched per session: it holds one server and one user's token, and a
    // new sign-in ends the agent (also fetched while the setting asks, so
    // turning it off applies to the running agent)
    let read_only_tools = if api.token.is_some() {
        read_only_tools(&api).await
    } else {
        None
    };
    let opts = acp::Options {
        read_only_tools,
        ask_read_only,
        cwd: root,
        mcp: api.token.clone().map(|token| acp::McpServer {
            url: format!("{}/api/v1/mcp", api.base),
            token,
        }),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        stderr: acp::Tail::default(),
    };
    let agent = Arc::new(
        acp::spawn(&profile, sink, opts)
            .await
            .map_err(ApiError::local)?,
    );
    if state.agent_seq.load(Ordering::SeqCst) != session {
        agent.stop();
        return Err(ApiError::local("Another agent was started meanwhile."));
    }
    // the model picked last for this agent, if it still offers it
    let remembered = snapshot(&state).agent_models.get(&name).cloned();
    if let (Some(model), Some(models)) = (remembered, agent.models()) {
        let offered = models.options.iter().any(|o| o.id == model);
        if offered && models.current.as_deref() != Some(model.as_str()) {
            // a failure keeps the agent's default; the picker shows which
            let _ = agent.set_model(&model).await;
        }
    }
    let _ = update_settings(&state, |s| s.agent = Some(name.clone()));
    let running = RunningAgent {
        session,
        name,
        agent,
    };
    let described = describe(&running);
    let old = state.agent.lock().expect("agent lock").replace(running);
    if let Some(old) = old {
        old.agent.stop();
    }
    Ok(described)
}

/// Sends a message (with what the user is looking at) and waits for the end
/// of the turn; the answer streams as `agent` events. Returns the stop reason.
#[tauri::command]
pub async fn agent_prompt(
    state: State<'_, AppState>,
    text: String,
    context: Option<acp::PromptContext>,
) -> CmdResult<String> {
    let agent = current_agent(&state)?;
    ensure_local_agents(&state).await?;
    agent
        .prompt(&text, &context.unwrap_or_default())
        .await
        .map_err(ApiError::local)
}

/// The user's answer to a permission request: allow once or reject. The
/// request is named by its session and key – keys restart with every
/// session, so a card of an ended session must not answer a new request.
#[tauri::command]
pub async fn agent_permission(
    state: State<'_, AppState>,
    session: u64,
    key: u64,
    allow: bool,
) -> CmdResult<()> {
    let agent = state
        .agent
        .lock()
        .expect("agent lock")
        .as_ref()
        .filter(|r| r.session == session)
        .map(|r| r.agent.clone())
        .ok_or_else(|| ApiError::local("This request is no longer open."))?;
    if allow {
        ensure_local_agents(&state).await?;
    }
    agent
        .answer_permission(key, allow)
        .await
        .map_err(ApiError::local)
}

/// Switches the agent to one of the models it offers and remembers the pick
/// for its next start. Named by session, like permission answers.
#[tauri::command]
pub async fn agent_set_model(
    state: State<'_, AppState>,
    session: u64,
    model: String,
) -> CmdResult<acp::Models> {
    let (name, agent) = state
        .agent
        .lock()
        .expect("agent lock")
        .as_ref()
        .filter(|r| r.session == session)
        .map(|r| (r.name.clone(), r.agent.clone()))
        .ok_or_else(|| ApiError::local("This agent is no longer running."))?;
    let models = agent.set_model(&model).await.map_err(ApiError::local)?;
    let _ = update_settings(&state, |s| {
        s.agent_models.insert(name, model);
    });
    Ok(models)
}

/// Stops the running turn of the agent.
#[tauri::command]
pub async fn agent_cancel(state: State<'_, AppState>) -> CmdResult<()> {
    let agent = current_agent(&state)?;
    agent.cancel().await.map_err(ApiError::local)
}

/// Ends the agent process.
#[tauri::command]
pub fn agent_stop(state: State<'_, AppState>) {
    stop_agent(&state);
}

#[cfg(test)]
mod agent_tests {
    use super::*;

    #[test]
    fn agent_entries_round_trip() {
        let profiles = parse_profiles(vec![
            AgentEntry {
                name: " Claude Code ".into(),
                command: "npx".into(),
                args: "-y @zed-industries/claude-code-acp".into(),
            },
            AgentEntry {
                name: "Gemini".into(),
                command: "gemini".into(),
                args: "--experimental-acp --x \"a b\"".into(),
            },
        ])
        .unwrap();
        assert_eq!(profiles[0].name, "Claude Code");
        assert_eq!(
            profiles[0].args,
            vec!["-y", "@zed-industries/claude-code-acp"]
        );
        assert_eq!(
            join_args(&profiles[1].args),
            "--experimental-acp --x \"a b\""
        );
        let twice = vec![
            AgentEntry {
                name: "A".into(),
                command: "a".into(),
                args: String::new(),
            },
            AgentEntry {
                name: "A".into(),
                command: "b".into(),
                args: String::new(),
            },
        ];
        assert!(parse_profiles(twice).is_err());
    }
}

/// *Add files…*: asks for the files to add with the native open dialog
/// (several at once). Empty = cancelled.
#[tauri::command]
pub async fn pick_files(app: AppHandle) -> CmdResult<Vec<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Files to add")
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| p.into_path().ok())
            .collect()
    })
    .await
    .map_err(|e| ApiError::local(format!("The file dialog failed: {e}")))
}

/// *Add files…*: what copying `paths` into the working folder would do
/// (which names become new versions or replace a local file).
#[tauri::command]
pub async fn workspace_add_plan(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: String,
    code: String,
    paths: Vec<PathBuf>,
    subfolder: String,
) -> CmdResult<Vec<addfiles::PlanItem>> {
    let api = signed_in_api(&state)?;
    let rev = workspace::revision_files(&api, project.as_deref(), &item, &code).await?;
    addfiles::plan(
        &workspace_root(&app, &state),
        project.as_deref(),
        &rev.files,
        &rev.item_folder,
        &paths,
        &subfolder,
    )
}

/// *Add files…*: copies the files into the working folder of a revision
/// the user has checked out; the originals stay where they are. Names in
/// `confirmed` may replace existing content. *Check in…* lists the copies as
/// new files.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn workspace_add_files(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
    item: String,
    code: String,
    paths: Vec<PathBuf>,
    subfolder: String,
    confirmed: Vec<String>,
) -> CmdResult<Value> {
    let api = signed_in_api(&state)?;
    let root = workspace_root(&app, &state);
    let rev = workspace::revision_files(&api, project.as_deref(), &item, &code).await?;
    if !rev.locked_by_me {
        return Err(ApiError::local(format!(
            "Check out {item}/{code} first – files can only be added under your check-out."
        )));
    }
    let plan = addfiles::plan(
        &root,
        project.as_deref(),
        &rev.files,
        &rev.item_folder,
        &paths,
        &subfolder,
    )?;
    let (copied, skipped) = addfiles::copy_in(&root, &plan, &confirmed)?;
    let done: Vec<addfiles::PlanItem> = plan
        .into_iter()
        .filter(|p| copied.contains(&p.name))
        .collect();
    addfiles::record(&root, &item, &code, &done)
        .map_err(|e| ApiError::local(format!("Cannot remember the added files: {e}")))?;
    Ok(json!({ "copied": copied, "skipped": skipped }))
}

// ------------------------------------------------------- usage counts (#383)

/// Counts for the own server's usage statistics (src/usage.rs): rolls the
/// day, applies `change` and saves; a failure to save is ignored – counting
/// never gets in the way.
pub(crate) fn usage(state: &State<'_, AppState>, change: impl FnOnce(&mut crate::usage::Usage)) {
    let mut s = state.settings.lock().expect("settings lock");
    // desktop policy UsageStatistics = 0 (#378): nothing is counted, kept or sent
    if policy::current().usage_statistics == Some(false) {
        if s.usage != crate::usage::Usage::default() {
            s.usage = crate::usage::Usage::default();
            let _ = s.save(&state.config_dir);
        }
        return;
    }
    let today = crate::usage::today_utc();
    s.usage.roll(&today, crate::usage::package(), None);
    change(&mut s.usage);
    let _ = s.save(&state.config_dir);
}

/// Counts one of the fixed actions the UI performs (unknown names are
/// ignored).
#[tauri::command]
pub fn usage_count(state: State<'_, AppState>, action: String) {
    usage(&state, |u| u.count(&action));
}

/// One more minute with the window in front (the UI asks once a minute).
#[tauri::command]
pub fn usage_tick(state: State<'_, AppState>) {
    usage(&state, |u| u.tick());
}

/// Sends a finished day's report to the server the app is signed in to
/// (`POST /api/v1/usage`); kept for *Show what is sent*. A failure is
/// silent – the report is replaced by the next day's.
#[tauri::command]
pub async fn usage_flush(state: State<'_, AppState>) -> CmdResult<()> {
    usage(&state, |_| {});
    if policy::current().usage_statistics == Some(false) {
        return Ok(());
    }
    let Some(pending) = snapshot(&state).usage.pending.clone() else {
        return Ok(());
    };
    let api = signed_in_api(&state)?;
    if api
        .json(Method::POST, "/usage", Some(&pending))
        .await
        .is_ok()
    {
        let mut s = state.settings.lock().expect("settings lock");
        s.usage.pending = None;
        s.usage.last_sent = Some(pending);
        let _ = s.save(&state.config_dir);
    }
    Ok(())
}

/// *Settings → Privacy → Show what is sent*: the report being counted
/// today, a finished one waiting to be sent, and the one sent last.
#[tauri::command]
pub fn usage_show(state: State<'_, AppState>) -> Value {
    usage(&state, |_| {});
    let s = snapshot(&state);
    json!({
        "today": crate::usage::report(&s.usage, crate::usage::package(), None),
        "pending": s.usage.pending,
        "last_sent": s.usage.last_sent,
    })
}
// ------------------------------------------------------ report a problem (#381)

/// The prefilled, redacted report of a failure for the review dialog.
#[tauri::command]
pub fn problem_report(problem: crate::report::Problem) -> Value {
    let home = crate::report::home();
    json!({
        "title": crate::report::title(&problem, home.as_deref()),
        "body": crate::report::body(&problem, home.as_deref()),
        "target": "github",
    })
}

/// *Open issue form*: the reviewed text in a new issue on GitHub, or in the
/// tracker set under *Settings → Report problems to*. Returns whether the
/// text had to be cut in the address (the UI then copies the full text).
#[tauri::command]
pub fn problem_report_open(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
    body: String,
) -> CmdResult<bool> {
    let base = policy::current()
        .report_target(snapshot(&state).report_url)
        .unwrap_or_else(|| crate::crash::ISSUE_URL.to_string());
    let (url, cut) = crate::report::form_url(&base, &title, &body).map_err(ApiError::local)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| ApiError::local(format!("The browser could not be opened: {e}")))?;
    Ok(cut)
}

/// *Save report…*: the reviewed text into a file the user picks.
#[tauri::command]
pub async fn problem_report_save(app: AppHandle, text: String) -> CmdResult<Option<PathBuf>> {
    let Some(path) = ask_save_path(
        &app,
        "teamcraft-problem-report.md",
        ("Markdown", &["md", "txt"]),
    )
    .await?
    else {
        return Ok(None);
    };
    std::fs::write(&path, text)
        .map_err(|e| ApiError::local(format!("The report could not be saved: {e}")))?;
    Ok(Some(path))
}

/// *Settings → Report problems to*: an http(s) address of the operator's
/// tracker (`title` and `body` are added as query parameters); empty = the
/// TeamCraft issue form on GitHub.
#[tauri::command]
pub fn set_report_url(state: State<'_, AppState>, url: String) -> CmdResult<()> {
    if policy::current().report_url.is_some() {
        return Err(ApiError::local(MANAGED));
    }
    let url = url.trim();
    let value = if url.is_empty() {
        None
    } else {
        crate::report::form_url(url, "", "").map_err(ApiError::local)?;
        Some(url.to_string())
    };
    update_settings(&state, |s| s.report_url = value)?;
    Ok(())
}

/// The bridge to the SolidCraft plugin (src/scbridge.rs), for as long as the
/// app runs: every 5 s the plugin's requests are picked up and handed to the
/// user interface (event `solidcraft-request`); every 30 s the heartbeat says
/// that the app runs and whether it reaches the server. Only for a working
/// folder that exists – the bridge never creates one.
pub(crate) async fn solidcraft_bridge(app: AppHandle) {
    let mut tick: u64 = 0;
    loop {
        let (root, api) = {
            let state = app.state::<AppState>();
            (workspace_root(&app, &state), signed_in_api(&state).ok())
        };
        if root.is_dir() {
            let beat = if tick % 6 == 0 {
                let reachable = match &api {
                    Some(api) => api.json(Method::GET, "/me", None).await.is_ok(),
                    None => false,
                };
                Some(scbridge::heartbeat(scbridge::now_secs(), reachable))
            } else {
                None
            };
            let requests = tokio::task::spawn_blocking(move || {
                if let Some(beat) = beat {
                    let _ = scbridge::write(&root, "desktop.json", &beat);
                }
                scbridge::take_requests(&root, scbridge::now_secs())
            })
            .await
            .unwrap_or_default();
            for request in requests {
                let _ = app.emit("solidcraft-request", &request);
            }
        }
        tick = tick.wrapping_add(1);
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}
