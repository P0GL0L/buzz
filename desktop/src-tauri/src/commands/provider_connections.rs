use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::managed_agents::resolve_command;

const PROVIDER_RECORD_VERSION: u16 = 2;
const PROVIDER_RECORDS_FILENAME: &str = "connections.json";
const PROVIDER_VERIFY_MARKER: &str = "BUZZ_PROVIDER_RUNTIME_VERIFIED";
const PROVIDER_VERIFY_OUTPUT_LIMIT: usize = 1024 * 1024;
const PROVIDER_VERIFY_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy)]
struct ProviderDefinition {
    provider_id: &'static str,
    runtime_id: &'static str,
    label: &'static str,
    auth_method: &'static str,
    command: &'static str,
    install_url: &'static str,
}

const PROVIDERS: &[ProviderDefinition] = &[
    ProviderDefinition {
        provider_id: "xai",
        runtime_id: "grok",
        label: "Grok Build",
        auth_method: "oauth",
        command: "grok",
        install_url: "https://build.x.ai/docs",
    },
    ProviderDefinition {
        provider_id: "google-personal",
        runtime_id: "antigravity",
        label: "Google Antigravity",
        auth_method: "google-oauth",
        command: "agy",
        install_url: "https://antigravity.google/docs/cli/install",
    },
    ProviderDefinition {
        provider_id: "google-enterprise",
        runtime_id: "gemini",
        label: "Gemini CLI",
        auth_method: "google-oauth-or-cloud",
        command: "gemini",
        install_url: "https://github.com/google-gemini/gemini-cli",
    },
];

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredProviderConnection {
    status: String,
    verification_time: Option<String>,
    failure: Option<ProviderFailure>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderFailure {
    code: String,
    message: String,
    recoverable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectionRecord {
    version: u16,
    provider_id: &'static str,
    runtime_id: &'static str,
    label: &'static str,
    auth_method: &'static str,
    install_state: &'static str,
    authentication_state: &'static str,
    availability: String,
    verification_time: Option<String>,
    expiration_time: Option<String>,
    storage_scope: &'static str,
    failure: Option<ProviderFailure>,
    install_url: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectionLaunch {
    version: u16,
    provider_id: String,
    launched: bool,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDisconnectResult {
    version: u16,
    provider_id: String,
    status: String,
}

#[derive(Debug)]
struct ProviderVerificationSpec {
    program: PathBuf,
    args: Vec<String>,
    env: Vec<(String, String)>,
    current_dir: PathBuf,
}

fn providers_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("providers"))
        .map_err(|error| format!("failed to resolve ASV Buzz provider storage: {error}"))
}

fn provider_home(app: &tauri::AppHandle, runtime_id: &str) -> Result<PathBuf, String> {
    Ok(providers_root(app)?.join(runtime_id).join("home"))
}

fn provider_home_from_root(root: &Path, runtime_id: &str) -> PathBuf {
    root.join(runtime_id).join("home")
}

fn ensure_private_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("failed to create provider storage: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("failed to protect provider storage: {error}"))?;
    }
    Ok(())
}

fn records_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(providers_root(app)?.join(PROVIDER_RECORDS_FILENAME))
}

fn load_records(
    app: &tauri::AppHandle,
) -> Result<BTreeMap<String, StoredProviderConnection>, String> {
    let path = records_path(app)?;
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(BTreeMap::new());
    };
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid provider connection records: {error}"))
}

fn save_records(
    app: &tauri::AppHandle,
    records: &BTreeMap<String, StoredProviderConnection>,
) -> Result<(), String> {
    let root = providers_root(app)?;
    ensure_private_dir(&root)?;
    let path = records_path(app)?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(records)
        .map_err(|error| format!("failed to serialize provider records: {error}"))?;
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("failed to stage provider records: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to protect provider records: {error}"))?;
    }
    std::fs::rename(temporary, path)
        .map_err(|error| format!("failed to publish provider records: {error}"))
}

fn definition(provider_id: &str) -> Result<&'static ProviderDefinition, String> {
    PROVIDERS
        .iter()
        .find(|provider| provider.provider_id == provider_id)
        .ok_or_else(|| format!("unknown provider: {provider_id}"))
}

fn antigravity_binary(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let bundled = providers_root(app)?
        .join("antigravity")
        .join("bin")
        .join(format!("agy{}", std::env::consts::EXE_SUFFIX));
    if bundled.is_file() {
        Ok(bundled)
    } else {
        resolve_command("agy").ok_or_else(|| "Antigravity CLI is not installed".to_string())
    }
}

fn provider_command(
    app: &tauri::AppHandle,
    provider: &ProviderDefinition,
) -> Result<PathBuf, String> {
    if provider.runtime_id == "antigravity" {
        antigravity_binary(app)
    } else {
        resolve_command(provider.command)
            .ok_or_else(|| format!("{} is not installed", provider.label))
    }
}

fn provider_has_auth_evidence(root: &Path, runtime_id: &str) -> bool {
    let home = provider_home_from_root(root, runtime_id);
    let candidates = match runtime_id {
        "grok" => vec![home.join("auth.json")],
        "antigravity" => vec![home
            .join(".gemini")
            .join("antigravity-cli")
            .join("antigravity-oauth-token")],
        "gemini" => vec![home.join(".gemini").join("oauth_creds.json")],
        _ => Vec::new(),
    };
    candidates.into_iter().any(|path| {
        std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
    })
}

fn record_for(
    app: &tauri::AppHandle,
    provider: &'static ProviderDefinition,
    stored: Option<&StoredProviderConnection>,
) -> ProviderConnectionRecord {
    let installed = provider_command(app, provider).is_ok();
    let provider_root = providers_root(app).ok();
    let has_auth_evidence = provider_root
        .as_deref()
        .is_some_and(|root| provider_has_auth_evidence(root, provider.runtime_id));
    let stored_status = stored
        .map(|record| record.status.as_str())
        .filter(|status| !status.trim().is_empty());
    let availability = if !installed {
        "not_installed".to_string()
    } else if stored_status == Some("disconnected") {
        "disconnected".to_string()
    } else if matches!(stored_status, Some("verified" | "degraded" | "expired")) {
        stored_status.unwrap_or("installed_unverified").to_string()
    } else if has_auth_evidence {
        "authenticated_unverified".to_string()
    } else {
        stored_status.unwrap_or("installed_unverified").to_string()
    };
    ProviderConnectionRecord {
        version: PROVIDER_RECORD_VERSION,
        provider_id: provider.provider_id,
        runtime_id: provider.runtime_id,
        label: provider.label,
        auth_method: provider.auth_method,
        install_state: if installed {
            "installed"
        } else {
            "not_installed"
        },
        authentication_state: if !installed {
            "unavailable"
        } else if stored_status == Some("disconnected") {
            "disconnected"
        } else if has_auth_evidence {
            "authenticated"
        } else {
            "unknown"
        },
        availability,
        verification_time: stored.and_then(|record| record.verification_time.clone()),
        expiration_time: None,
        storage_scope: "buzz-dev-provider-directory-and-provider-owned-keyring",
        failure: stored.and_then(|record| record.failure.clone()),
        install_url: provider.install_url,
    }
}

fn verification_spec(
    app: &tauri::AppHandle,
    provider: &ProviderDefinition,
) -> Result<ProviderVerificationSpec, String> {
    let program = provider_command(app, provider)?;
    let home = provider_home(app, provider.runtime_id)?;
    let current_dir = providers_root(app)?;
    ensure_private_dir(&home)?;
    ensure_private_dir(&current_dir)?;

    let prompt =
        format!("Reply with exactly {PROVIDER_VERIFY_MARKER} and nothing else. Do not use tools.");
    let (args, env) = match provider.runtime_id {
        "grok" => (
            vec![
                "-p".to_string(),
                prompt,
                "--output-format".to_string(),
                "json".to_string(),
                "--permission-mode".to_string(),
                "plan".to_string(),
                "--no-subagents".to_string(),
                "--tools".to_string(),
                String::new(),
            ],
            vec![("GROK_HOME".to_string(), home.display().to_string())],
        ),
        "antigravity" => (
            vec![
                "-p".to_string(),
                prompt,
                "--output-format".to_string(),
                "stream-json".to_string(),
                "--print-timeout".to_string(),
                "90s".to_string(),
                "--sandbox".to_string(),
                "--mode".to_string(),
                "plan".to_string(),
            ],
            vec![
                ("HOME".to_string(), home.display().to_string()),
                (
                    "AGY_CLI_DISABLE_AUTO_UPDATE".to_string(),
                    "true".to_string(),
                ),
            ],
        ),
        "gemini" => (
            vec![
                "-p".to_string(),
                prompt,
                "--output-format".to_string(),
                "json".to_string(),
                "--approval-mode".to_string(),
                "plan".to_string(),
                "--sandbox".to_string(),
            ],
            vec![("GEMINI_CLI_HOME".to_string(), home.display().to_string())],
        ),
        _ => return Err("provider verification is unsupported".to_string()),
    };

    Ok(ProviderVerificationSpec {
        program,
        args,
        env,
        current_dir,
    })
}

fn drain_bounded<R: Read>(mut reader: R) -> Vec<u8> {
    let mut retained = Vec::new();
    let mut chunk = [0_u8; 8192];
    while let Ok(read) = reader.read(&mut chunk) {
        if read == 0 {
            break;
        }
        let remaining = PROVIDER_VERIFY_OUTPUT_LIMIT.saturating_sub(retained.len());
        retained.extend_from_slice(&chunk[..read.min(remaining)]);
    }
    retained
}

fn classify_verification_failure(output: &[u8], timed_out: bool) -> ProviderFailure {
    if timed_out {
        return ProviderFailure {
            code: "verification_timeout".to_string(),
            message: "The provider runtime did not finish its bounded verification task."
                .to_string(),
            recoverable: true,
        };
    }

    let normalized = String::from_utf8_lossy(output).to_lowercase();
    if normalized.contains("no longer supported")
        || normalized.contains("migrate to the antigravity")
    {
        return ProviderFailure {
            code: "unsupported_account_route".to_string(),
            message: "This signed-in account is not supported by Gemini CLI. Use Antigravity for a personal Google account, or configure an enterprise, Cloud, or API-key Gemini route."
                .to_string(),
            recoverable: true,
        };
    }
    if normalized.contains("eligibility check failed")
        || normalized.contains("account is not eligible")
    {
        return ProviderFailure {
            code: "account_not_eligible".to_string(),
            message: "The signed-in Google account is not currently eligible for Antigravity. Complete Google's account-verification requirement in the provider UI or use another eligible personal account; Buzz will not reopen OAuth automatically."
                .to_string(),
            recoverable: true,
        };
    }
    if normalized.contains("not signed in")
        || normalized.contains("authentication required")
        || normalized.contains("no auth method")
        || normalized.contains("failed to sign in")
        || normalized.contains("login required")
    {
        return ProviderFailure {
            code: "authentication_required".to_string(),
            message: "The provider runtime did not accept the scoped sign-in. Use Sign in again only if the provider credential has actually expired."
                .to_string(),
            recoverable: true,
        };
    }
    if normalized.contains("rate limit") || normalized.contains("too many requests") {
        return ProviderFailure {
            code: "rate_limited".to_string(),
            message: "The provider accepted the connection but rate-limited the verification task."
                .to_string(),
            recoverable: true,
        };
    }
    ProviderFailure {
        code: "verification_failed".to_string(),
        message: "The provider runtime could not complete the bounded verification task."
            .to_string(),
        recoverable: true,
    }
}

fn run_provider_verification(spec: ProviderVerificationSpec) -> Result<(), ProviderFailure> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .envs(spec.env)
        .current_dir(spec.current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::util::configure_no_window(&mut command);

    let mut child = command.spawn().map_err(|_| ProviderFailure {
        code: "runtime_unreachable".to_string(),
        message: "The provider runtime could not be started.".to_string(),
        recoverable: true,
    })?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_reader = stdout.map(|pipe| std::thread::spawn(move || drain_bounded(pipe)));
    let stderr_reader = stderr.map(|pipe| std::thread::spawn(move || drain_bounded(pipe)));

    let deadline = Instant::now() + PROVIDER_VERIFY_TIMEOUT;
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (Some(status), false),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(250));
            }
            Ok(None) => {
                let _ = child.kill();
                let status = child.wait().ok();
                break (status, true);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break (None, false);
            }
        }
    };

    let mut output = stdout_reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    if let Some(mut stderr) = stderr_reader.and_then(|reader| reader.join().ok()) {
        let remaining = PROVIDER_VERIFY_OUTPUT_LIMIT.saturating_sub(output.len());
        output.extend_from_slice(
            &stderr
                .drain(..stderr.len().min(remaining))
                .collect::<Vec<_>>(),
        );
    }

    if !timed_out
        && status.is_some_and(|status| status.success())
        && String::from_utf8_lossy(&output).contains(PROVIDER_VERIFY_MARKER)
    {
        Ok(())
    } else {
        Err(classify_verification_failure(&output, timed_out))
    }
}

/// Run a bounded, no-tools provider task and persist only its non-secret result.
///
/// This never opens OAuth, reads credential contents, or returns provider
/// stdout. It is the explicit reconciliation step after provider-owned login.
#[tauri::command]
pub async fn verify_provider_connection(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<ProviderConnectionRecord, String> {
    let provider = definition(&provider_id)?;
    let spec = verification_spec(&app, provider)?;
    let result = tokio::task::spawn_blocking(move || run_provider_verification(spec))
        .await
        .map_err(|error| format!("provider verification task failed: {error}"))?;

    let mut records = load_records(&app)?;
    let stored = match result {
        Ok(()) => StoredProviderConnection {
            status: "verified".to_string(),
            verification_time: Some(chrono::Utc::now().to_rfc3339()),
            failure: None,
        },
        Err(failure) => StoredProviderConnection {
            status: "degraded".to_string(),
            verification_time: None,
            failure: Some(failure),
        },
    };
    records.insert(provider.provider_id.to_string(), stored);
    save_records(&app, &records)?;
    Ok(record_for(
        &app,
        provider,
        records.get(provider.provider_id),
    ))
}

/// Return non-secret provider connection state. This never reads OAuth tokens
/// or returns filesystem paths, account addresses, or connector configuration.
#[tauri::command]
pub fn list_provider_connections(
    app: tauri::AppHandle,
) -> Result<Vec<ProviderConnectionRecord>, String> {
    let records = load_records(&app)?;
    Ok(PROVIDERS
        .iter()
        .map(|provider| record_for(&app, provider, records.get(provider.provider_id)))
        .collect())
}

fn login_argv(
    app: &tauri::AppHandle,
    provider: &ProviderDefinition,
) -> Result<Vec<String>, String> {
    let binary = provider_command(app, provider)?;
    let home = provider_home(app, provider.runtime_id)?;
    ensure_private_dir(&home)?;
    let mut argv = vec!["env".to_string()];
    match provider.runtime_id {
        "grok" => {
            argv.push(format!("GROK_HOME={}", home.display()));
            argv.push(binary.display().to_string());
            argv.extend(["login".to_string(), "--oauth".to_string()]);
        }
        "antigravity" => {
            argv.push(format!("HOME={}", home.display()));
            argv.push("AGY_CLI_DISABLE_AUTO_UPDATE=true".to_string());
            argv.push(binary.display().to_string());
        }
        "gemini" => {
            argv.push(format!("GEMINI_CLI_HOME={}", home.display()));
            argv.push(binary.display().to_string());
        }
        _ => return Err("provider login is unsupported".to_string()),
    }
    Ok(argv)
}

/// Launch the official provider-owned interactive login flow in a visible
/// terminal. Only non-secret storage paths are passed in argv; OAuth tokens
/// remain in the provider's own keyring and never return to Buzz.
#[tauri::command]
pub fn connect_provider_connection(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<ProviderConnectionLaunch, String> {
    let provider = definition(&provider_id)?;
    let argv = login_argv(&app, provider)?;
    crate::commands::agent_auth::launch_visible_terminal(&argv)?;
    let mut records = load_records(&app)?;
    records.insert(
        provider.provider_id.to_string(),
        StoredProviderConnection {
            status: "pending_consent".to_string(),
            verification_time: None,
            failure: None,
        },
    );
    save_records(&app, &records)?;
    Ok(ProviderConnectionLaunch {
        version: PROVIDER_RECORD_VERSION,
        provider_id,
        launched: true,
        status: "pending_consent".to_string(),
    })
}

/// Disable Buzz routing for a provider without reading or deleting the
/// provider-owned OAuth credential. Reconnecting always re-enters the
/// provider's official login flow.
#[tauri::command]
pub fn disconnect_provider_connection(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<ProviderDisconnectResult, String> {
    let provider = definition(&provider_id)?;
    let mut records = load_records(&app)?;
    records.insert(
        provider.provider_id.to_string(),
        StoredProviderConnection {
            status: "disconnected".to_string(),
            verification_time: None,
            failure: None,
        },
    );
    save_records(&app, &records)?;
    Ok(ProviderDisconnectResult {
        version: PROVIDER_RECORD_VERSION,
        provider_id,
        status: "disconnected".to_string(),
    })
}

pub(crate) fn apply_runtime_env(
    app: &tauri::AppHandle,
    runtime_command: &str,
    command: &mut std::process::Command,
) -> Result<(), String> {
    let records = load_records(app)?;
    let provider_id = match runtime_command {
        "grok" => Some("xai"),
        "gemini" => Some("google-enterprise"),
        "buzz-antigravity-acp" => Some("google-personal"),
        _ => None,
    };
    if provider_id
        .and_then(|id| records.get(id))
        .is_some_and(|record| record.status == "disconnected")
    {
        return Err(format!(
            "{runtime_command} is disconnected from ASV Buzz; reconnect it in Settings"
        ));
    }
    match runtime_command {
        "grok" => {
            let home = provider_home(app, "grok")?;
            ensure_private_dir(&home)?;
            command.env("GROK_HOME", home);
        }
        "gemini" => {
            let home = provider_home(app, "gemini")?;
            ensure_private_dir(&home)?;
            command.env("GEMINI_CLI_HOME", home);
        }
        "buzz-antigravity-acp" => {
            let home = provider_home(app, "antigravity")?;
            ensure_private_dir(&home)?;
            command.env("HOME", home);
            command.env("AGY_CLI_DISABLE_AUTO_UPDATE", "true");
            command.env("BUZZ_ANTIGRAVITY_CLI", antigravity_binary(app)?);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        classify_verification_failure, definition, provider_has_auth_evidence,
        ProviderConnectionRecord, PROVIDERS,
    };

    #[test]
    fn provider_catalog_has_unique_non_secret_routes() {
        let mut ids = std::collections::BTreeSet::new();
        for provider in PROVIDERS {
            assert!(ids.insert(provider.provider_id));
            assert!(!provider.auth_method.contains("token"));
            assert!(provider.install_url.starts_with("https://"));
        }
        assert_eq!(definition("xai").unwrap().runtime_id, "grok");
        assert!(definition("unknown").is_err());
    }

    #[test]
    fn relay_safe_status_shape_has_no_credentials_or_local_paths() {
        let record = ProviderConnectionRecord {
            version: 2,
            provider_id: "xai",
            runtime_id: "grok",
            label: "Grok Build",
            auth_method: "oauth",
            install_state: "installed",
            authentication_state: "authenticated",
            availability: "authenticated_unverified".to_string(),
            verification_time: None,
            expiration_time: None,
            storage_scope: "provider-owned-keyring",
            failure: None,
            install_url: "https://build.x.ai/docs",
        };
        let value = serde_json::to_value(record).unwrap();
        let object = value.as_object().unwrap();
        for forbidden in [
            "token",
            "secret",
            "credential",
            "path",
            "account",
            "address",
            "host",
        ] {
            assert!(
                object
                    .keys()
                    .all(|key| !key.to_lowercase().contains(forbidden)),
                "provider status exposed forbidden field class: {forbidden}"
            );
        }
    }

    #[test]
    fn authentication_evidence_is_presence_only_and_provider_scoped() {
        let root = tempfile::tempdir().unwrap();
        let grok_home = root.path().join("grok").join("home");
        std::fs::create_dir_all(&grok_home).unwrap();
        assert!(!provider_has_auth_evidence(root.path(), "grok"));
        std::fs::write(grok_home.join("auth.json"), b"{}").unwrap();
        assert!(provider_has_auth_evidence(root.path(), "grok"));
        assert!(!provider_has_auth_evidence(root.path(), "gemini"));
    }

    #[test]
    fn provider_failures_are_sanitized_and_actionable() {
        let unsupported = classify_verification_failure(
            b"Failed to sign in. This client is no longer supported; migrate to Antigravity.",
            false,
        );
        assert_eq!(unsupported.code, "unsupported_account_route");
        assert!(!unsupported.message.contains("Failed to sign in"));

        let ineligible = classify_verification_failure(
            b"Eligibility check failed: account is not eligible",
            false,
        );
        assert_eq!(ineligible.code, "account_not_eligible");

        let auth = classify_verification_failure(b"Not signed in", false);
        assert_eq!(auth.code, "authentication_required");

        let timeout = classify_verification_failure(b"", true);
        assert_eq!(timeout.code, "verification_timeout");
    }
}
