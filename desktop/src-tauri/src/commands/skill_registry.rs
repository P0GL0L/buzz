use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

const REGISTRY_VIEW_VERSION: u16 = 1;
const MAX_REGISTRY_BYTES: u64 = 12 * 1024 * 1024;
const MAX_SKILLS: usize = 10_000;
const MAX_LIST_VALUES: usize = 64;
const MAX_TEXT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelaySafeSkill {
    abstract_requirements: Vec<String>,
    availability: String,
    capabilities: Vec<String>,
    display_name: String,
    installation_state: String,
    observed_at: String,
    owning_agent: String,
    registry_version: u64,
    routable: bool,
    runtime_class: String,
    skill_id: String,
    #[serde(default, skip_deserializing)]
    expired: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RelaySafeRegistry {
    generated_at: String,
    registry_digest: String,
    registry_revision: String,
    registry_version: u64,
    skills: Vec<RelaySafeSkill>,
    ttl_seconds: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRegistryView {
    version: u16,
    generated_at: String,
    registry_digest: String,
    registry_revision: String,
    registry_version: u64,
    ttl_seconds: u64,
    skills: Vec<RelaySafeSkill>,
}

fn registry_path() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| {
            home.join(".buzz-dev")
                .join("skill-registry")
                .join("relay-safe.json")
        })
        .ok_or_else(|| "the local home directory is unavailable".to_string())
}

fn validate_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn validate_skill_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._:-".contains(&byte)
        })
}

fn validate_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT_BYTES && !value.contains('\0')
}

fn validate_registry(registry: &RelaySafeRegistry) -> Result<(), String> {
    if registry.registry_version == 0
        || registry.ttl_seconds == 0
        || !validate_digest(&registry.registry_digest)
        || !validate_digest(&registry.registry_revision)
        || DateTime::parse_from_rfc3339(&registry.generated_at).is_err()
    {
        return Err("relay-safe registry metadata is invalid".to_string());
    }
    if registry.skills.len() > MAX_SKILLS {
        return Err("relay-safe registry contains too many skill records".to_string());
    }
    for skill in &registry.skills {
        let state_ok = matches!(
            skill.installation_state.as_str(),
            "catalogued" | "installed" | "callable" | "degraded" | "unreachable"
        );
        let availability_ok = matches!(
            skill.availability.as_str(),
            "available" | "degraded" | "unavailable" | "unknown"
        );
        if !state_ok
            || !availability_ok
            || !validate_skill_id(&skill.skill_id)
            || !validate_token(&skill.runtime_class)
            || !validate_text(&skill.display_name)
            || skill.owning_agent.len() != 64
            || !skill
                .owning_agent
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || DateTime::parse_from_rfc3339(&skill.observed_at).is_err()
            || skill.registry_version == 0
            || skill.capabilities.len() > MAX_LIST_VALUES
            || skill.abstract_requirements.len() > MAX_LIST_VALUES
            || skill.capabilities.iter().any(|value| !validate_text(value))
            || skill
                .abstract_requirements
                .iter()
                .any(|value| !validate_text(value))
        {
            return Err(format!(
                "relay-safe registry contains an invalid record for {}",
                skill.skill_id
            ));
        }
    }
    Ok(())
}

fn apply_expiry(registry: &mut RelaySafeRegistry, now: DateTime<Utc>) {
    let ttl = i64::try_from(registry.ttl_seconds)
        .ok()
        .and_then(Duration::try_seconds);
    for skill in &mut registry.skills {
        let observed = DateTime::parse_from_rfc3339(&skill.observed_at)
            .ok()
            .map(|value| value.with_timezone(&Utc));
        let expired = observed
            .zip(ttl)
            .is_none_or(|(observed, ttl)| observed + ttl < now);
        skill.expired = expired;
        if expired {
            skill.availability = "unknown".to_string();
        }
        skill.routable = skill.routable
            && !expired
            && skill.installation_state == "callable"
            && skill.availability == "available";
    }
}

/// Read the local relay-safe skill projection for the desktop Skills surface.
///
/// The canonical registry is intentionally not read or returned: local paths,
/// host details, permissions, and connector configuration stay with the owner.
#[tauri::command]
pub fn list_skill_registry() -> Result<SkillRegistryView, String> {
    let path = registry_path()?;
    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("relay-safe skill registry is unavailable: {error}"))?;
    if metadata.len() > MAX_REGISTRY_BYTES {
        return Err("relay-safe skill registry exceeds the desktop read limit".to_string());
    }
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("failed to read relay-safe skill registry: {error}"))?;
    let mut registry: RelaySafeRegistry = serde_json::from_slice(&bytes)
        .map_err(|error| format!("relay-safe skill registry is invalid: {error}"))?;
    validate_registry(&registry)?;
    apply_expiry(&mut registry, Utc::now());
    Ok(SkillRegistryView {
        version: REGISTRY_VIEW_VERSION,
        generated_at: registry.generated_at,
        registry_digest: registry.registry_digest,
        registry_revision: registry.registry_revision,
        registry_version: registry.registry_version,
        ttl_seconds: registry.ttl_seconds,
        skills: registry.skills,
    })
}

#[cfg(test)]
mod tests {
    use super::{apply_expiry, validate_registry, RelaySafeRegistry, RelaySafeSkill};
    use chrono::{Duration, Utc};

    fn registry(observed_at: String) -> RelaySafeRegistry {
        RelaySafeRegistry {
            generated_at: Utc::now().to_rfc3339(),
            registry_digest:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            registry_revision:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            registry_version: 1,
            ttl_seconds: 60,
            skills: vec![RelaySafeSkill {
                abstract_requirements: vec!["browser access".to_string()],
                availability: "available".to_string(),
                capabilities: vec!["inspect a page".to_string()],
                display_name: "Browser".to_string(),
                installation_state: "callable".to_string(),
                observed_at,
                owning_agent: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_string(),
                registry_version: 1,
                routable: true,
                runtime_class: "codex".to_string(),
                skill_id: "codex:browser".to_string(),
                expired: false,
            }],
        }
    }

    #[test]
    fn valid_relay_projection_passes_without_canonical_fields() {
        assert!(validate_registry(&registry(Utc::now().to_rfc3339())).is_ok());
    }

    #[test]
    fn expired_observation_becomes_unknown_and_not_routable() {
        let mut registry = registry((Utc::now() - Duration::minutes(5)).to_rfc3339());
        apply_expiry(&mut registry, Utc::now());
        assert!(registry.skills[0].expired);
        assert_eq!(registry.skills[0].availability, "unknown");
        assert!(!registry.skills[0].routable);
    }

    #[test]
    fn installed_record_cannot_remain_routable_without_callable_evidence() {
        let mut registry = registry(Utc::now().to_rfc3339());
        registry.skills[0].installation_state = "installed".to_string();
        apply_expiry(&mut registry, Utc::now());
        assert!(!registry.skills[0].routable);
    }
}
