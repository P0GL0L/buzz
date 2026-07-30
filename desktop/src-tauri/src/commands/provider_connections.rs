use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::managed_agents::resolve_command;

const PROVIDER_RECORD_VERSION: u16 = 1;
const PROVIDER_RECORDS_FILENAME: &str = "connections.json";

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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

fn providers_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("providers"))
        .map_err(|error| format!("failed to resolve Buzz Dev provider storage: {error}"))
}

fn provider_home(app: &tauri::AppHandle, runtime_id: &str) -> Result<PathBuf, String> {
    Ok(providers_root(app)?.join(runtime_id).join("home"))
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

fn record_for(
    app: &tauri::AppHandle,
    provider: &'static ProviderDefinition,
    stored: Option<&StoredProviderConnection>,
) -> ProviderConnectionRecord {
    let installed = provider_command(app, provider).is_ok();
    let availability = if !installed {
        "not_installed".to_string()
    } else {
        stored
            .map(|record| record.status.clone())
            .filter(|status| !status.trim().is_empty())
            .unwrap_or_else(|| "installed_unverified".to_string())
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
        availability,
        verification_time: stored.and_then(|record| record.verification_time.clone()),
        expiration_time: None,
        storage_scope: "buzz-dev-provider-directory-and-provider-owned-keyring",
        failure: stored.and_then(|record| record.failure.clone()),
        install_url: provider.install_url,
    }
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
        _ => None,
    };
    if provider_id
        .and_then(|id| records.get(id))
        .is_some_and(|record| record.status == "disconnected")
    {
        return Err(format!(
            "{runtime_command} is disconnected from Buzz Dev; reconnect it in Settings"
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
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{definition, ProviderConnectionRecord, PROVIDERS};

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
            version: 1,
            provider_id: "xai",
            runtime_id: "grok",
            label: "Grok Build",
            auth_method: "oauth",
            install_state: "installed",
            availability: "pending_consent".to_string(),
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
}
