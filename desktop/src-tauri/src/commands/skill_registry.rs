use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use nostr::JsonUtil;
use serde::{Deserialize, Serialize};

const REGISTRY_VIEW_VERSION: u16 = 1;
const MAX_REGISTRY_BYTES: u64 = 12 * 1024 * 1024;
const MAX_SKILLS: usize = 10_000;
const MAX_LIST_VALUES: usize = 64;
const MAX_TEXT_BYTES: usize = 16 * 1024;
const MAX_TASK_BYTES: usize = 64 * 1024;

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRequestView {
    action: String,
    content: String,
    correlation_id: String,
    expected_owner: String,
    skill_id: String,
    version: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillResultProofs {
    runtime_discovered: bool,
    dependencies_probed: bool,
    bounded_verification: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillResultPayload {
    completed_at: String,
    completion_state: String,
    correlation_id: String,
    evidence_summary: String,
    execution_host_class: String,
    failure_reason: Option<String>,
    proofs: SkillResultProofs,
    request_event_id: String,
    signer: String,
    skill_hash: String,
    skill_id: String,
    skill_version: String,
    version: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillResultReview {
    completed_at: String,
    completion_state: String,
    correlation_id: String,
    evidence_summary: String,
    event_id: String,
    execution_host_class: String,
    failure_reason: Option<String>,
    proofs_complete: bool,
    signer: String,
    skill_hash: String,
    skill_id: String,
    skill_version: String,
    signature_valid: bool,
    version: u16,
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

fn has_sensitive_reference(value: &str) -> bool {
    value.split_whitespace().any(|token| {
        token.starts_with('/')
            || token.starts_with("~/")
            || token.contains("://")
            || token.to_ascii_lowercase().contains("token=")
            || token.to_ascii_lowercase().contains("secret=")
    })
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

/// Build a bounded relay-safe skill request for the desktop to sign and send.
#[tauri::command]
pub fn build_skill_request(
    action: String,
    skill_id: String,
    expected_owner: Option<String>,
    task: String,
    required_evidence: Vec<String>,
    correlation_id: Option<String>,
) -> Result<SkillRequestView, String> {
    if task.trim().is_empty() || task.len() > MAX_TASK_BYTES || task.contains('\0') {
        return Err("skill request task is empty or exceeds the 64 KiB limit".to_string());
    }
    if required_evidence.len() > 16
        || required_evidence
            .iter()
            .any(|value| !validate_text(value) || has_sensitive_reference(value))
    {
        return Err("skill request evidence labels are invalid or sensitive".to_string());
    }
    let registry = list_skill_registry()?;
    let mut candidates: Vec<&RelaySafeSkill> = registry
        .skills
        .iter()
        .filter(|skill| skill.skill_id == skill_id)
        .filter(|skill| {
            expected_owner
                .as_ref()
                .is_none_or(|owner| skill.owning_agent == *owner)
        })
        .collect();
    candidates.sort_by(|left, right| right.observed_at.cmp(&left.observed_at));
    let selected = match action.as_str() {
        "route" => candidates.into_iter().find(|skill| skill.routable),
        "verify" => candidates.into_iter().find(|skill| !skill.expired),
        "owner-detail" => candidates.into_iter().next(),
        _ => return Err("unsupported skill action".to_string()),
    }
    .ok_or_else(|| format!("no eligible owner observation for skill '{skill_id}'"))?;
    let correlation_id = match correlation_id {
        Some(value) => uuid::Uuid::parse_str(&value)
            .map_err(|error| format!("invalid correlation id: {error}"))?
            .to_string(),
        None => uuid::Uuid::new_v4().to_string(),
    };
    let payload = serde_json::json!({
        "version": 1,
        "action": action,
        "correlationId": correlation_id,
        "skillId": selected.skill_id,
        "expectedOwner": selected.owning_agent,
        "registryDigest": registry.registry_digest,
        "registryRevision": registry.registry_revision,
        "skillObservedAt": selected.observed_at,
        "task": task,
        "requiredEvidence": required_evidence,
    });
    let prefix = match action.as_str() {
        "route" => "skill-route/v1",
        "verify" => "skill-verify/v1",
        "owner-detail" => "skill-owner-detail/v1",
        _ => unreachable!(),
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("failed to serialize skill request: {error}"))?;
    Ok(SkillRequestView {
        action,
        content: format!("{prefix}\n```json\n{json}\n```"),
        correlation_id,
        expected_owner: selected.owning_agent.clone(),
        skill_id,
        version: 1,
    })
}

/// Verify and decode a signed skill-result/v1 event without exposing host data.
#[tauri::command]
pub fn review_signed_skill_result(
    event_json: String,
    expected_correlation_id: Option<String>,
) -> Result<SkillResultReview, String> {
    if event_json.len() > MAX_TASK_BYTES {
        return Err("signed skill result exceeds the 64 KiB review limit".to_string());
    }
    let event = nostr::Event::from_json(event_json)
        .map_err(|error| format!("invalid signed event: {error}"))?;
    event
        .verify()
        .map_err(|error| format!("skill result signature is invalid: {error}"))?;
    let body = event
        .content
        .strip_prefix("skill-result/v1")
        .ok_or_else(|| "event is not a skill-result/v1 reply".to_string())?
        .trim();
    let body = body
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(body);
    let result: SkillResultPayload = serde_json::from_str(body)
        .map_err(|error| format!("invalid skill-result/v1 payload: {error}"))?;
    if result.version != 1
        || result.signer != event.pubkey.to_hex()
        || !validate_skill_id(&result.skill_id)
        || !validate_token(&result.execution_host_class)
        || !validate_digest(&result.skill_hash)
        || !validate_text(&result.evidence_summary)
        || has_sensitive_reference(&result.evidence_summary)
        || result
            .failure_reason
            .as_ref()
            .is_some_and(|value| !validate_text(value) || has_sensitive_reference(value))
        || result.request_event_id.len() != 64
        || !result
            .request_event_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || DateTime::parse_from_rfc3339(&result.completed_at).is_err()
        || uuid::Uuid::parse_str(&result.correlation_id).is_err()
    {
        return Err("skill result contains invalid or sensitive fields".to_string());
    }
    if expected_correlation_id
        .as_ref()
        .is_some_and(|expected| expected != &result.correlation_id)
    {
        return Err("skill result correlation does not match the requested task".to_string());
    }
    let completion_state_ok = matches!(
        result.completion_state.as_str(),
        "completed" | "offline" | "denied" | "expired" | "degraded" | "unreachable"
    );
    if !completion_state_ok {
        return Err("skill result completion state is invalid".to_string());
    }
    let proofs_complete = result.proofs.runtime_discovered
        && result.proofs.dependencies_probed
        && result.proofs.bounded_verification;
    if result.completion_state == "completed"
        && (!proofs_complete || result.failure_reason.is_some())
    {
        return Err("completed skill result is missing required proofs".to_string());
    }
    if result.completion_state != "completed" && result.failure_reason.is_none() {
        return Err("failed skill result is missing a structured reason".to_string());
    }
    Ok(SkillResultReview {
        completed_at: result.completed_at,
        completion_state: result.completion_state,
        correlation_id: result.correlation_id,
        evidence_summary: result.evidence_summary,
        event_id: event.id.to_hex(),
        execution_host_class: result.execution_host_class,
        failure_reason: result.failure_reason,
        proofs_complete,
        signer: result.signer,
        skill_hash: result.skill_hash,
        skill_id: result.skill_id,
        skill_version: result.skill_version,
        signature_valid: true,
        version: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        apply_expiry, review_signed_skill_result, validate_registry, RelaySafeRegistry,
        RelaySafeSkill,
    };
    use chrono::{Duration, Utc};
    use nostr::{EventBuilder, JsonUtil, Keys, Kind};

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

    #[test]
    fn signed_result_review_rejects_tampering_and_accepts_complete_proofs() {
        let keys = Keys::generate();
        let signer = keys.public_key().to_hex();
        let payload = serde_json::json!({
            "version": 1,
            "completedAt": Utc::now().to_rfc3339(),
            "completionState": "completed",
            "correlationId": "98f62eef-6b94-48e0-b2f6-d07aa798ef47",
            "evidenceSummary": "Bounded verification completed",
            "executionHostClass": "codex",
            "failureReason": null,
            "proofs": {
                "runtimeDiscovered": true,
                "dependenciesProbed": true,
                "boundedVerification": true
            },
            "requestEventId": "e".repeat(64),
            "signer": signer,
            "skillHash": format!("sha256:{}", "a".repeat(64)),
            "skillId": "codex:review",
            "skillVersion": "1",
        });
        let content = format!(
            "skill-result/v1\n```json\n{}\n```",
            serde_json::to_string_pretty(&payload).unwrap()
        );
        let event = EventBuilder::new(Kind::TextNote, content)
            .sign_with_keys(&keys)
            .unwrap();
        let reviewed = review_signed_skill_result(
            event.as_json(),
            Some("98f62eef-6b94-48e0-b2f6-d07aa798ef47".into()),
        )
        .unwrap();
        assert!(reviewed.signature_valid);
        assert!(reviewed.proofs_complete);
        assert_eq!(reviewed.signer, keys.public_key().to_hex());

        let tampered = event.as_json().replace(
            "Bounded verification completed",
            "Bounded verification forged",
        );
        assert!(review_signed_skill_result(tampered, None).is_err());
    }
}
