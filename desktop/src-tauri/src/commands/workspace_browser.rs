use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::webview::{DownloadEvent, NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewUrl};

const WEBVIEW_LABEL: &str = "buzz-workspace-browser";
const STATE_EVENT: &str = "workspace-browser-state";
const DOWNLOAD_EVENT: &str = "workspace-browser-download";
const POPUP_EVENT: &str = "workspace-browser-popup-blocked";
const MAX_ACTIONS: usize = 100;
const MAX_HISTORY: usize = 500;
const MAX_EXTRACTED_TEXT: usize = 250_000;
const MAX_WORKSPACE_DOWNLOAD_BYTES: u64 = 50 * 1024 * 1024;
const EVAL_TIMEOUT: Duration = Duration::from_secs(10);

mod artifacts;
mod capture;
use artifacts::completed_download_metadata;
pub(crate) use artifacts::read_workspace_browser_artifact;
use capture::capture_visible_workspace_region;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl BrowserBounds {
    fn validate(&self) -> Result<(), String> {
        if !self.x.is_finite()
            || !self.y.is_finite()
            || !self.width.is_finite()
            || !self.height.is_finite()
            || self.x < 0.0
            || self.y < 0.0
            || self.width < 160.0
            || self.height < 120.0
            || self.width > 10_000.0
            || self.height > 10_000.0
        {
            return Err("invalid workspace browser bounds".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAction {
    version: u16,
    id: String,
    occurred_at: String,
    actor: String,
    action: String,
    target: Option<String>,
    outcome: String,
    correlation_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBrowserState {
    version: u16,
    mode: &'static str,
    current_url: Option<String>,
    title: Option<String>,
    loading: bool,
    active_agent: Option<String>,
    history: Vec<String>,
    history_index: usize,
    actions: Vec<BrowserAction>,
    error: Option<String>,
}

#[derive(Default)]
struct BrowserSession {
    current_url: Option<String>,
    title: Option<String>,
    loading: bool,
    active_agent: Option<String>,
    history: Vec<String>,
    history_index: usize,
    actions: VecDeque<BrowserAction>,
    pending_download: Option<PendingDownload>,
    bounds: Option<BrowserBounds>,
    error: Option<String>,
    loaded_from_disk: bool,
}

struct PendingDownload {
    path: PathBuf,
    filename: String,
}

#[derive(Default)]
pub struct WorkspaceBrowserRuntime {
    session: Mutex<BrowserSession>,
}

#[derive(Serialize, Deserialize)]
struct PersistedHistory {
    version: u16,
    history: Vec<String>,
    history_index: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserPageExtraction {
    version: u16,
    url: String,
    title: String,
    text: String,
    links: Vec<BrowserPageLink>,
    truncated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct BrowserPageLink {
    text: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct BrowserPageExtractionRaw {
    url: String,
    title: String,
    text: String,
    links: Vec<BrowserPageLink>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCaptureResult {
    version: u16,
    completion_state: &'static str,
    reason: Option<String>,
    local_artifact_path: Option<PathBuf>,
    filename: Option<String>,
    mime_type: Option<&'static str>,
    size: Option<u64>,
    sha256: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    actor: String,
    correlation_id: Option<String>,
}

fn browser_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("workspace-browser"))
        .map_err(|error| format!("failed to resolve browser profile: {error}"))
}

fn browser_downloads_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(browser_root(app)?.join("downloads"))
}

fn browser_screenshots_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(browser_root(app)?.join("screenshots"))
}

fn ensure_private_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("failed to create browser profile: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("failed to protect browser profile: {error}"))?;
    }
    Ok(())
}

fn validate_browser_url(raw: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(raw.trim()).map_err(|_| "invalid browser URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("workspace browser allows HTTP and HTTPS only".to_string());
    }
    Ok(url)
}

fn redacted_url(url: &url::Url) -> String {
    let mut safe = url.clone();
    let _ = safe.set_username("");
    let _ = safe.set_password(None);
    safe.set_fragment(None);
    if safe.query().is_some() {
        let redacted: Vec<(String, String)> = safe
            .query_pairs()
            .map(|(key, value)| {
                let lower = key.to_ascii_lowercase();
                let sensitive = ["token", "code", "secret", "key", "auth", "session", "state"]
                    .iter()
                    .any(|needle| lower.contains(needle));
                (
                    key.into_owned(),
                    if sensitive {
                        "[redacted]".to_string()
                    } else {
                        value.into_owned()
                    },
                )
            })
            .collect();
        safe.query_pairs_mut().clear().extend_pairs(redacted);
    }
    safe.to_string()
}

fn history_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(browser_root(app)?.join("history.json"))
}

fn load_history(app: &tauri::AppHandle, session: &mut BrowserSession) {
    if session.loaded_from_disk {
        return;
    }
    session.loaded_from_disk = true;
    let Ok(path) = history_path(app) else {
        return;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let Ok(saved) = serde_json::from_slice::<PersistedHistory>(&bytes) else {
        return;
    };
    session.history = saved.history.into_iter().take(MAX_HISTORY).collect();
    session.history_index = saved
        .history_index
        .min(session.history.len().saturating_sub(1));
}

fn persist_history(app: &tauri::AppHandle, session: &BrowserSession) -> Result<(), String> {
    let root = browser_root(app)?;
    ensure_private_dir(&root)?;
    let payload = PersistedHistory {
        version: 1,
        history: session.history.clone(),
        history_index: session.history_index,
    };
    let bytes = serde_json::to_vec_pretty(&payload)
        .map_err(|error| format!("failed to encode browser history: {error}"))?;
    let path = history_path(app)?;
    atomic_write_file::AtomicWriteFile::options()
        .open(&path)
        .and_then(|mut file| file.write_all(&bytes).and_then(|_| file.commit()))
        .map_err(|error| format!("failed to persist browser history: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to protect browser history: {error}"))?;
    }
    Ok(())
}

fn append_action_log(app: &tauri::AppHandle, action: &BrowserAction) -> Result<(), String> {
    let root = browser_root(app)?;
    ensure_private_dir(&root)?;
    let path = root.join("actions.jsonl");
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("failed to open browser audit log: {error}"))?;
    serde_json::to_writer(&mut file, action)
        .map_err(|error| format!("failed to write browser audit log: {error}"))?;
    file.write_all(b"\n")
        .map_err(|error| format!("failed to finish browser audit log: {error}"))
}

fn snapshot(session: &BrowserSession) -> WorkspaceBrowserState {
    WorkspaceBrowserState {
        version: 1,
        mode: "native",
        current_url: session.current_url.clone(),
        title: session.title.clone(),
        loading: session.loading,
        active_agent: session.active_agent.clone(),
        history: session.history.clone(),
        history_index: session.history_index,
        actions: session.actions.iter().cloned().collect(),
        error: session.error.clone(),
    }
}

fn emit_state(app: &tauri::AppHandle, state: &WorkspaceBrowserRuntime) {
    if let Ok(session) = state.session.lock() {
        let _ = app.emit(STATE_EVENT, snapshot(&session));
    }
}

fn record_action(
    app: &tauri::AppHandle,
    state: &WorkspaceBrowserRuntime,
    actor: Option<&str>,
    action_name: &str,
    target: Option<String>,
    outcome: &str,
    correlation_id: Option<&str>,
) {
    let action = BrowserAction {
        version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        occurred_at: chrono::Utc::now().to_rfc3339(),
        actor: actor.unwrap_or("Charles").trim().chars().take(80).collect(),
        action: action_name.to_string(),
        target,
        outcome: outcome.to_string(),
        correlation_id: correlation_id.map(|value| value.chars().take(128).collect()),
    };
    let _ = append_action_log(app, &action);
    if let Ok(mut session) = state.session.lock() {
        session.active_agent = actor.map(|value| value.trim().chars().take(80).collect());
        session.actions.push_back(action);
        while session.actions.len() > MAX_ACTIONS {
            session.actions.pop_front();
        }
    }
    emit_state(app, state);
}

fn push_history(app: &tauri::AppHandle, state: &WorkspaceBrowserRuntime, raw_url: &url::Url) {
    let safe = redacted_url(raw_url);
    if let Ok(mut session) = state.session.lock() {
        load_history(app, &mut session);
        if session.history.get(session.history_index) != Some(&safe) {
            let keep = session.history_index.saturating_add(1);
            session.history.truncate(keep);
            session.history.push(safe.clone());
            if session.history.len() > MAX_HISTORY {
                let overflow = session.history.len() - MAX_HISTORY;
                session.history.drain(..overflow);
            }
            session.history_index = session.history.len().saturating_sub(1);
        }
        session.current_url = Some(safe);
        let _ = persist_history(app, &session);
    }
}

fn browser_webview(app: &tauri::AppHandle) -> Result<tauri::Webview, String> {
    app.get_webview(WEBVIEW_LABEL)
        .ok_or_else(|| "native workspace browser is not open".to_string())
}

fn set_bounds(webview: &tauri::Webview, bounds: &BrowserBounds) -> Result<(), String> {
    bounds.validate()?;
    webview
        .set_position(LogicalPosition::new(bounds.x, bounds.y))
        .and_then(|_| webview.set_size(LogicalSize::new(bounds.width, bounds.height)))
        .map_err(|error| format!("failed to align native browser: {error}"))
}

fn sanitized_filename(url: &url::Url) -> String {
    let candidate = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|name| !name.is_empty())
        .unwrap_or("download");
    crate::commands::media::sanitize_filename(candidate)
}

fn build_webview(
    app: &tauri::AppHandle,
    initial_url: url::Url,
    bounds: &BrowserBounds,
) -> Result<tauri::Webview, String> {
    let root = browser_root(app)?;
    let profile = root.join("profile");
    let downloads = browser_downloads_root(app)?;
    ensure_private_dir(&profile)?;
    ensure_private_dir(&downloads)?;
    let app_for_navigation = app.clone();
    let app_for_load = app.clone();
    let app_for_title = app.clone();
    let app_for_popup = app.clone();
    let app_for_download = app.clone();
    let data_store = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        format!("{}/workspace-browser", app.config().identifier).as_bytes(),
    )
    .into_bytes();

    let builder = WebviewBuilder::new(WEBVIEW_LABEL, WebviewUrl::External(initial_url))
        .data_directory(profile)
        .data_store_identifier(data_store)
        .zoom_hotkeys_enabled(true)
        .on_navigation(move |url| {
            let allowed = matches!(url.scheme(), "http" | "https");
            if !allowed {
                let _ = app_for_navigation.emit(
                    POPUP_EVENT,
                    serde_json::json!({
                        "version": 1,
                        "url": url.as_str(),
                        "reason": "Only HTTP and HTTPS navigation is allowed"
                    }),
                );
            }
            allowed
        })
        .on_page_load(move |_webview, payload| {
            let state = app_for_load.state::<WorkspaceBrowserRuntime>();
            match payload.event() {
                PageLoadEvent::Started => {
                    if let Ok(mut session) = state.session.lock() {
                        session.loading = true;
                        session.error = None;
                    }
                }
                PageLoadEvent::Finished => {
                    if let Ok(mut session) = state.session.lock() {
                        session.loading = false;
                    }
                    push_history(&app_for_load, &state, payload.url());
                }
            }
            emit_state(&app_for_load, &state);
        })
        .on_document_title_changed(move |_webview, title| {
            let state = app_for_title.state::<WorkspaceBrowserRuntime>();
            if let Ok(mut session) = state.session.lock() {
                session.title = Some(title.chars().take(300).collect());
            }
            emit_state(&app_for_title, &state);
        })
        .on_new_window(move |url, _features| {
            let _ = app_for_popup.emit(
                POPUP_EVENT,
                serde_json::json!({
                    "version": 1,
                    "url": redacted_url(&url),
                    "reason": "Popup blocked; navigate in the workspace or open externally"
                }),
            );
            NewWindowResponse::Deny
        })
        .on_download(move |_webview, event| {
            let state = app_for_download.state::<WorkspaceBrowserRuntime>();
            match event {
                DownloadEvent::Requested { url, destination } => {
                    let original_filename = sanitized_filename(&url);
                    let filename = format!(
                        "{}-{}",
                        chrono::Utc::now().timestamp_millis(),
                        original_filename
                    );
                    let path = downloads.join(filename);
                    *destination = path.clone();
                    if let Ok(mut session) = state.session.lock() {
                        session.pending_download = Some(PendingDownload {
                            path,
                            filename: original_filename,
                        });
                    }
                    true
                }
                DownloadEvent::Finished { url, path, success } => {
                    let pending = state
                        .session
                        .lock()
                        .ok()
                        .and_then(|mut session| session.pending_download.take());
                    let destination =
                        path.or_else(|| pending.as_ref().map(|item| item.path.clone()));
                    let filename = pending
                        .map(|item| item.filename)
                        .unwrap_or_else(|| sanitized_filename(&url));
                    let artifact = destination
                        .as_deref()
                        .filter(|_| success)
                        .and_then(completed_download_metadata);
                    let _ = app_for_download.emit(
                        DOWNLOAD_EVENT,
                        serde_json::json!({
                            "version": 1,
                            "url": redacted_url(&url),
                            "path": destination,
                            "filename": filename,
                            "success": success,
                            "artifact": artifact
                        }),
                    );
                    success
                }
                _ => true,
            }
        });
    let window = app
        .get_window("main")
        .ok_or_else(|| "main Buzz window is unavailable".to_string())?;
    window
        .add_child(
            builder,
            LogicalPosition::new(bounds.x, bounds.y),
            LogicalSize::new(bounds.width, bounds.height),
        )
        .map_err(|error| format!("failed to create native workspace browser: {error}"))
}

async fn eval_json<T: serde::de::DeserializeOwned>(
    webview: &tauri::Webview,
    script: String,
) -> Result<T, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = Mutex::new(Some(sender));
    webview
        .eval_with_callback(script, move |value| {
            if let Ok(mut sender) = sender.lock() {
                if let Some(sender) = sender.take() {
                    let _ = sender.send(value);
                }
            }
        })
        .map_err(|error| format!("browser action failed: {error}"))?;
    let raw = tokio::time::timeout(EVAL_TIMEOUT, receiver)
        .await
        .map_err(|_| "browser action timed out".to_string())?
        .map_err(|_| "browser action result was cancelled".to_string())?;
    serde_json::from_str(&raw).map_err(|error| format!("invalid browser action result: {error}"))
}

#[tauri::command]
pub fn open_workspace_browser(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    url: String,
    bounds: BrowserBounds,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> Result<WorkspaceBrowserState, String> {
    let parsed = validate_browser_url(&url)?;
    bounds.validate()?;
    if let Some(webview) = app.get_webview(WEBVIEW_LABEL) {
        set_bounds(&webview, &bounds)?;
        webview.show().map_err(|error| error.to_string())?;
        if webview.url().ok().as_ref() != Some(&parsed) {
            webview
                .navigate(parsed.clone())
                .map_err(|error| error.to_string())?;
        }
    } else {
        build_webview(&app, parsed.clone(), &bounds)?;
    }
    if let Ok(mut session) = runtime.session.lock() {
        session.bounds = Some(bounds);
    }
    push_history(&app, &runtime, &parsed);
    record_action(
        &app,
        &runtime,
        actor.as_deref(),
        "open",
        Some(redacted_url(&parsed)),
        "accepted",
        correlation_id.as_deref(),
    );
    runtime
        .session
        .lock()
        .map(|session| snapshot(&session))
        .map_err(|_| "browser state is unavailable".to_string())
}

#[tauri::command]
pub fn update_workspace_browser_bounds(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    bounds: BrowserBounds,
) -> Result<(), String> {
    set_bounds(&browser_webview(&app)?, &bounds)?;
    if let Ok(mut session) = runtime.session.lock() {
        session.bounds = Some(bounds);
    }
    Ok(())
}

#[tauri::command]
pub fn navigate_workspace_browser(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    url: String,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> Result<(), String> {
    let parsed = validate_browser_url(&url)?;
    browser_webview(&app)?
        .navigate(parsed.clone())
        .map_err(|error| format!("navigation failed: {error}"))?;
    record_action(
        &app,
        &runtime,
        actor.as_deref(),
        "navigate",
        Some(redacted_url(&parsed)),
        "accepted",
        correlation_id.as_deref(),
    );
    Ok(())
}

fn eval_navigation(
    app: &tauri::AppHandle,
    runtime: &WorkspaceBrowserRuntime,
    actor: Option<&str>,
    action: &str,
    script: &str,
    correlation_id: Option<&str>,
) -> Result<(), String> {
    browser_webview(app)?
        .eval(script)
        .map_err(|error| format!("{action} failed: {error}"))?;
    record_action(
        app,
        runtime,
        actor,
        action,
        None,
        "accepted",
        correlation_id,
    );
    Ok(())
}

#[tauri::command]
pub fn workspace_browser_back(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> Result<(), String> {
    eval_navigation(
        &app,
        &runtime,
        actor.as_deref(),
        "back",
        "history.back()",
        correlation_id.as_deref(),
    )
}

#[tauri::command]
pub fn workspace_browser_forward(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> Result<(), String> {
    eval_navigation(
        &app,
        &runtime,
        actor.as_deref(),
        "forward",
        "history.forward()",
        correlation_id.as_deref(),
    )
}

#[tauri::command]
pub fn reload_workspace_browser(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> Result<(), String> {
    browser_webview(&app)?
        .reload()
        .map_err(|error| format!("reload failed: {error}"))?;
    record_action(
        &app,
        &runtime,
        actor.as_deref(),
        "reload",
        None,
        "accepted",
        correlation_id.as_deref(),
    );
    Ok(())
}

#[tauri::command]
pub fn stop_workspace_browser(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> Result<(), String> {
    eval_navigation(
        &app,
        &runtime,
        actor.as_deref(),
        "stop",
        "window.stop()",
        correlation_id.as_deref(),
    )?;
    if let Ok(mut session) = runtime.session.lock() {
        session.loading = false;
    }
    Ok(())
}

#[tauri::command]
pub fn close_workspace_browser(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(webview) = app.get_webview(WEBVIEW_LABEL) {
        webview
            .close()
            .map_err(|error| format!("failed to close native browser: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn clear_workspace_browser_session(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
) -> Result<(), String> {
    eval_navigation(
        &app,
        &runtime,
        Some("Charles"),
        "clear-session",
        "sessionStorage.clear()",
        None,
    )
}

#[tauri::command]
pub fn clear_workspace_browser_profile(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
) -> Result<(), String> {
    browser_webview(&app)?
        .clear_all_browsing_data()
        .map_err(|error| format!("failed to clear browser profile: {error}"))?;
    if let Ok(mut session) = runtime.session.lock() {
        session.history.clear();
        session.history_index = 0;
        session.current_url = None;
        session.title = None;
        let _ = persist_history(&app, &session);
    }
    record_action(
        &app,
        &runtime,
        Some("Charles"),
        "clear-profile",
        None,
        "completed",
        None,
    );
    Ok(())
}

#[tauri::command]
pub fn get_workspace_browser_state(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
) -> Result<WorkspaceBrowserState, String> {
    let current = app
        .get_webview(WEBVIEW_LABEL)
        .and_then(|webview| webview.url().ok())
        .filter(|url| matches!(url.scheme(), "http" | "https"));
    if let Some(url) = current {
        push_history(&app, &runtime, &url);
    }
    runtime
        .session
        .lock()
        .map(|session| snapshot(&session))
        .map_err(|_| "browser state is unavailable".to_string())
}

#[tauri::command]
pub async fn extract_workspace_browser_page(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> Result<BrowserPageExtraction, String> {
    let script = format!(
        r#"(() => {{
          const links = Array.from(document.querySelectorAll('a[href]')).slice(0, 200)
            .map((a) => ({{ text: (a.textContent || '').trim().slice(0, 500), url: a.href }}));
          return {{ url: location.href, title: document.title, text: (document.body?.innerText || '').slice(0, {}), links }};
        }})()"#,
        MAX_EXTRACTED_TEXT + 1
    );
    let raw: BrowserPageExtractionRaw = eval_json(&browser_webview(&app)?, script).await?;
    let mut text = raw.text;
    let truncated = text.len() > MAX_EXTRACTED_TEXT;
    if truncated {
        let mut boundary = MAX_EXTRACTED_TEXT;
        while boundary > 0 && !text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        text.truncate(boundary);
    }
    let result = BrowserPageExtraction {
        version: 1,
        url: validate_browser_url(&raw.url)
            .map(|url| redacted_url(&url))
            .unwrap_or_default(),
        title: raw.title.chars().take(500).collect(),
        text,
        links: raw.links.into_iter().take(200).collect(),
        truncated,
    };
    record_action(
        &app,
        &runtime,
        actor.as_deref(),
        "extract",
        Some(result.url.clone()),
        "completed",
        correlation_id.as_deref(),
    );
    Ok(result)
}

async fn bounded_selector_action(
    app: &tauri::AppHandle,
    runtime: &WorkspaceBrowserRuntime,
    actor: Option<&str>,
    action: &str,
    selector: &str,
    script: String,
    correlation_id: Option<&str>,
) -> Result<(), String> {
    if selector.is_empty() || selector.len() > 2_000 {
        return Err("browser selector is empty or too long".to_string());
    }
    let completed: bool = eval_json(&browser_webview(app)?, script).await?;
    let outcome = if completed { "completed" } else { "not-found" };
    record_action(
        app,
        runtime,
        actor,
        action,
        Some(selector.chars().take(300).collect()),
        outcome,
        correlation_id,
    );
    if completed {
        Ok(())
    } else {
        Err("browser target was not found".to_string())
    }
}

#[tauri::command]
pub async fn click_workspace_browser(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    selector: String,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> Result<(), String> {
    let encoded =
        serde_json::to_string(&selector).map_err(|error| format!("invalid selector: {error}"))?;
    bounded_selector_action(
        &app,
        &runtime,
        actor.as_deref(),
        "click",
        &selector,
        format!(
            "(() => {{ const element = document.querySelector({encoded}); if (!element) return false; element.click(); return true; }})()"
        ),
        correlation_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn type_workspace_browser(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    selector: String,
    text: String,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> Result<(), String> {
    if text.len() > 100_000 {
        return Err("browser input is too large".to_string());
    }
    let encoded_selector =
        serde_json::to_string(&selector).map_err(|error| format!("invalid selector: {error}"))?;
    let encoded_text =
        serde_json::to_string(&text).map_err(|error| format!("invalid browser input: {error}"))?;
    bounded_selector_action(
        &app,
        &runtime,
        actor.as_deref(),
        "type",
        &selector,
        format!(
            "(() => {{ const element = document.querySelector({encoded_selector}); if (!(element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element.isContentEditable)) return false; if (element.isContentEditable) element.textContent = {encoded_text}; else element.value = {encoded_text}; element.dispatchEvent(new InputEvent('input', {{ bubbles: true, inputType: 'insertText' }})); element.dispatchEvent(new Event('change', {{ bubbles: true }})); return true; }})()"
        ),
        correlation_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub fn scroll_workspace_browser(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    x: f64,
    y: f64,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> Result<(), String> {
    if !x.is_finite() || !y.is_finite() || x.abs() > 100_000.0 || y.abs() > 100_000.0 {
        return Err("invalid browser scroll distance".to_string());
    }
    eval_navigation(
        &app,
        &runtime,
        actor.as_deref(),
        "scroll",
        &format!("window.scrollBy({x}, {y})"),
        correlation_id.as_deref(),
    )
}

#[tauri::command]
pub fn capture_workspace_browser(
    app: tauri::AppHandle,
    runtime: State<'_, WorkspaceBrowserRuntime>,
    actor: Option<String>,
    correlation_id: Option<String>,
) -> BrowserCaptureResult {
    let actor = actor
        .as_deref()
        .unwrap_or("Charles")
        .trim()
        .chars()
        .take(80)
        .collect::<String>();
    let correlation_id = correlation_id
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.chars().take(128).collect::<String>());
    let result = capture_visible_workspace_region(&app, &runtime, &actor, correlation_id.clone());
    record_action(
        &app,
        &runtime,
        Some(&actor),
        "screenshot",
        result
            .local_artifact_path
            .as_ref()
            .map(|path| path.display().to_string()),
        result.completion_state,
        correlation_id.as_deref(),
    );
    result
}

/// Return bytes for a browser download or screenshot after proving that the
/// path is a regular file inside this ASV Buzz app's isolated browser roots.
#[tauri::command]
pub fn fetch_workspace_browser_download(
    app: tauri::AppHandle,
    path: PathBuf,
) -> Result<tauri::ipc::Response, String> {
    read_workspace_browser_artifact(&app, &path, MAX_WORKSPACE_DOWNLOAD_BYTES)
        .map(tauri::ipc::Response::new)
}

#[cfg(test)]
#[path = "workspace_browser/tests.rs"]
mod tests;
