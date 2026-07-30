//! `buzz skills` — provenance-aware local inventory and signed routing.
//!
//! The canonical registry remains on the host that collected it. A separate,
//! deliberately smaller relay-safe projection is the only representation that
//! may be published to a Buzz channel.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::client::BuzzClient;
use crate::error::CliError;
use crate::validate::{read_or_stdin, validate_hex64, MAX_CONTENT_BYTES};
use crate::SkillsCmd;

#[cfg(test)]
const DEFAULT_TTL_SECONDS: u64 = 86_400;
const REGISTRY_DIR: &str = "skill-registry";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum InstallationState {
    Catalogued,
    Installed,
    Callable,
    Degraded,
    Unreachable,
}

impl InstallationState {
    fn parse(value: &str) -> Result<Self, CliError> {
        match value {
            "catalogued" => Ok(Self::Catalogued),
            "installed" => Ok(Self::Installed),
            "callable" => Ok(Self::Callable),
            "degraded" => Ok(Self::Degraded),
            "unreachable" => Ok(Self::Unreachable),
            _ => Err(CliError::Usage(format!(
                "invalid source state '{value}' (expected catalogued, installed, callable, degraded, or unreachable)"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Availability {
    Available,
    Degraded,
    Unavailable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SharingPolicy {
    Private,
    TeamIndexed,
    Portable,
}

impl SharingPolicy {
    fn parse(value: &str) -> Result<Self, CliError> {
        match value {
            "private" => Ok(Self::Private),
            "team-indexed" => Ok(Self::TeamIndexed),
            "portable" => Ok(Self::Portable),
            _ => Err(CliError::Usage(format!(
                "invalid sharing policy '{value}' (expected private, team-indexed, or portable)"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VerificationEvidence {
    method: String,
    status: String,
    observed_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillObservation {
    availability: Availability,
    entrypoint: String,
    installation_state: InstallationState,
    invocation_owner: String,
    observed_at: String,
    runtime_identity: String,
    source_host: String,
    verification: VerificationEvidence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanonicalSkill {
    abstract_requirements: Vec<String>,
    capabilities: Vec<String>,
    connectors: Vec<String>,
    content_hash: String,
    display_name: String,
    observations: Vec<SkillObservation>,
    permissions: Vec<String>,
    sharing_policy: SharingPolicy,
    skill_id: String,
    source_package: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanonicalRegistry {
    generated_at: String,
    registry_revision: String,
    registry_version: u64,
    skills: Vec<CanonicalSkill>,
    ttl_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RelaySafeSkill {
    abstract_requirements: Vec<String>,
    availability: Availability,
    capabilities: Vec<String>,
    display_name: String,
    installation_state: InstallationState,
    observed_at: String,
    owning_agent: String,
    registry_version: u64,
    routable: bool,
    runtime_class: String,
    skill_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RelaySafeRegistry {
    generated_at: String,
    registry_digest: String,
    registry_revision: String,
    registry_version: u64,
    skills: Vec<RelaySafeSkill>,
    ttl_seconds: u64,
}

#[derive(Debug)]
struct SourceSpec {
    state: InstallationState,
    sharing_policy: SharingPolicy,
    package: String,
    path: PathBuf,
}

impl SourceSpec {
    /// Parse `STATE:SHARING:PACKAGE:PATH`.
    fn parse(value: &str) -> Result<Self, CliError> {
        let parts: Vec<&str> = value.splitn(4, ':').collect();
        if parts.len() != 4 {
            return Err(CliError::Usage(format!(
                "invalid --source '{value}'; expected STATE:SHARING:PACKAGE:PATH"
            )));
        }
        let package = normalize_identifier(parts[2]);
        if package.is_empty() {
            return Err(CliError::Usage(
                "source package must contain an identifier character".into(),
            ));
        }
        let path = PathBuf::from(parts[3]);
        if !path.is_dir() {
            return Err(CliError::Usage(format!(
                "skill source is not a directory: {}",
                path.display()
            )));
        }
        Ok(Self {
            state: InstallationState::parse(parts[0])?,
            sharing_policy: SharingPolicy::parse(parts[1])?,
            package,
            path,
        })
    }
}

#[derive(Default)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    capabilities: Vec<String>,
    connectors: Vec<String>,
    permissions: Vec<String>,
    requirements: Vec<String>,
}

fn parse_inline_list(value: &str) -> Vec<String> {
    let trimmed = value.trim().trim_start_matches('[').trim_end_matches(']');
    trimmed
        .split(',')
        .map(|part| part.trim().trim_matches(['"', '\'']).to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn parse_frontmatter(content: &str) -> SkillFrontmatter {
    let mut result = SkillFrontmatter::default();
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return result;
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches(['"', '\'']);
        match key.trim() {
            "name" => result.name = (!value.is_empty()).then(|| value.to_string()),
            "description" => {
                result.description = (!value.is_empty()).then(|| value.to_string());
            }
            "capabilities" => result.capabilities = parse_inline_list(value),
            "connectors" => result.connectors = parse_inline_list(value),
            "permissions" => result.permissions = parse_inline_list(value),
            "requires" | "requirements" => result.requirements = parse_inline_list(value),
            _ => {}
        }
    }
    result
}

fn normalize_identifier(value: &str) -> String {
    let mut out = String::new();
    let mut last_separator = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        let accepted = ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ':' | '-');
        if accepted {
            out.push(ch);
            last_separator = false;
        } else if !last_separator && !out.is_empty() {
            out.push('-');
            last_separator = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn sanitize_summary(value: &str) -> String {
    let mut output = Vec::new();
    for token in value.split_whitespace() {
        let sanitized =
            if token.starts_with('/') || token.starts_with("~/") || token.contains("://") {
                "[redacted-reference]"
            } else {
                token
            };
        output.push(sanitized);
        if output.join(" ").chars().count() >= 240 {
            break;
        }
    }
    output.join(" ").chars().take(240).collect()
}

fn collect_skill_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), CliError> {
    let entries = fs::read_dir(root)
        .map_err(|e| CliError::Other(format!("failed to read {}: {e}", root.display())))?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            CliError::Other(format!(
                "failed to read an entry in {}: {e}",
                root.display()
            ))
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| CliError::Other(format!("failed to inspect {}: {e}", path.display())))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_skill_files(&path, out)?;
        } else if file_type.is_file() && entry.file_name() == "SKILL.md" {
            out.push(path);
        }
    }
    Ok(())
}

fn availability_for_state(state: InstallationState) -> Availability {
    match state {
        InstallationState::Callable => Availability::Available,
        InstallationState::Degraded => Availability::Degraded,
        InstallationState::Unreachable => Availability::Unavailable,
        InstallationState::Catalogued | InstallationState::Installed => Availability::Unknown,
    }
}

fn restrictive_sharing_policy(left: SharingPolicy, right: SharingPolicy) -> SharingPolicy {
    match (left, right) {
        (SharingPolicy::Private, _) | (_, SharingPolicy::Private) => SharingPolicy::Private,
        (SharingPolicy::TeamIndexed, _) | (_, SharingPolicy::TeamIndexed) => {
            SharingPolicy::TeamIndexed
        }
        (SharingPolicy::Portable, SharingPolicy::Portable) => SharingPolicy::Portable,
    }
}

fn abstract_requirements(frontmatter: &SkillFrontmatter) -> Vec<String> {
    let mut values = Vec::new();
    if !frontmatter.connectors.is_empty() {
        values.push("connector-access".to_string());
    }
    if !frontmatter.permissions.is_empty() {
        values.push("host-permission".to_string());
    }
    if !frontmatter.requirements.is_empty() {
        values.push("runtime-dependency".to_string());
    }
    values
}

fn registry_default_path(filename: &str) -> Result<PathBuf, CliError> {
    let home = dirs::home_dir()
        .ok_or_else(|| CliError::Other("could not resolve the user home directory".into()))?;
    Ok(home.join(".buzz-dev").join(REGISTRY_DIR).join(filename))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    let parent = path
        .parent()
        .ok_or_else(|| CliError::Usage(format!("output path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)
        .map_err(|e| CliError::Other(format!("failed to create {}: {e}", parent.display())))?;
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| CliError::Other(format!("failed to serialize registry: {e}")))?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("registry"),
        std::process::id()
    ));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .map_err(|e| CliError::Other(format!("failed to create {}: {e}", temporary.display())))?;
    file.write_all(&bytes)
        .map_err(|e| CliError::Other(format!("failed to write {}: {e}", temporary.display())))?;
    file.sync_all()
        .map_err(|e| CliError::Other(format!("failed to sync {}: {e}", temporary.display())))?;
    fs::rename(&temporary, path)
        .map_err(|e| CliError::Other(format!("failed to replace {}: {e}", path.display())))
}

fn canonical_digest(registry: &CanonicalRegistry) -> Result<String, CliError> {
    let bytes = serde_json::to_vec(registry)
        .map_err(|e| CliError::Other(format!("failed to serialize registry digest input: {e}")))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn effective_availability(
    observation: &SkillObservation,
    now: DateTime<Utc>,
    ttl_seconds: u64,
) -> Availability {
    let Ok(observed_at) = DateTime::parse_from_rfc3339(&observation.observed_at) else {
        return Availability::Unknown;
    };
    let age = now.signed_duration_since(observed_at.with_timezone(&Utc));
    if age.num_seconds() < 0 || age.num_seconds() as u64 > ttl_seconds {
        Availability::Unknown
    } else {
        observation.availability
    }
}

fn relay_safe_projection(registry: &CanonicalRegistry, digest: String) -> RelaySafeRegistry {
    let now = Utc::now();
    let mut skills = Vec::new();
    for skill in &registry.skills {
        if skill.sharing_policy == SharingPolicy::Private {
            continue;
        }
        for observation in &skill.observations {
            let availability = effective_availability(observation, now, registry.ttl_seconds);
            skills.push(RelaySafeSkill {
                abstract_requirements: skill.abstract_requirements.clone(),
                availability,
                capabilities: skill.capabilities.clone(),
                display_name: skill.display_name.clone(),
                installation_state: observation.installation_state,
                observed_at: observation.observed_at.clone(),
                owning_agent: observation.invocation_owner.clone(),
                registry_version: registry.registry_version,
                routable: observation.installation_state == InstallationState::Callable
                    && availability == Availability::Available,
                runtime_class: observation.runtime_identity.clone(),
                skill_id: skill.skill_id.clone(),
            });
        }
    }
    skills.sort_by(|a, b| {
        (&a.skill_id, &a.owning_agent, &a.observed_at).cmp(&(
            &b.skill_id,
            &b.owning_agent,
            &b.observed_at,
        ))
    });
    RelaySafeRegistry {
        generated_at: registry.generated_at.clone(),
        registry_digest: digest,
        registry_revision: registry.registry_revision.clone(),
        registry_version: registry.registry_version,
        skills,
        ttl_seconds: registry.ttl_seconds,
    }
}

fn scan_registry(
    source_values: &[String],
    source_host: &str,
    runtime_identity: &str,
    invocation_owner: &str,
    registry_version: u64,
    ttl_seconds: u64,
) -> Result<(CanonicalRegistry, RelaySafeRegistry), CliError> {
    if source_values.is_empty() {
        return Err(CliError::Usage(
            "at least one --source STATE:SHARING:PACKAGE:PATH is required".into(),
        ));
    }
    if source_host.trim().is_empty()
        || runtime_identity.trim().is_empty()
        || invocation_owner.trim().is_empty()
    {
        return Err(CliError::Usage(
            "--host, --runtime, and --owner must be non-empty".into(),
        ));
    }
    validate_hex64(invocation_owner)?;
    if ttl_seconds == 0 {
        return Err(CliError::Usage(
            "--ttl-seconds must be greater than zero".into(),
        ));
    }
    let observed_at = Utc::now().to_rfc3339();
    let mut records: BTreeMap<(String, String), CanonicalSkill> = BTreeMap::new();
    for source_value in source_values {
        let source = SourceSpec::parse(source_value)?;
        let mut files = Vec::new();
        collect_skill_files(&source.path, &mut files)?;
        files.sort();
        for path in files {
            let content = fs::read_to_string(&path)
                .map_err(|e| CliError::Other(format!("failed to read {}: {e}", path.display())))?;
            let frontmatter = parse_frontmatter(&content);
            let fallback_name = path
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .unwrap_or("skill");
            let display_name = frontmatter
                .name
                .clone()
                .unwrap_or_else(|| fallback_name.to_string());
            let local_name = normalize_identifier(&display_name);
            if local_name.is_empty() {
                continue;
            }
            let skill_id = format!("{}:{local_name}", source.package);
            let content_hash =
                format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())));
            let key = (skill_id.clone(), content_hash.clone());
            let capabilities = if frontmatter.capabilities.is_empty() {
                frontmatter
                    .description
                    .as_deref()
                    .map(sanitize_summary)
                    .filter(|summary| !summary.is_empty())
                    .into_iter()
                    .collect()
            } else {
                frontmatter
                    .capabilities
                    .iter()
                    .map(|value| sanitize_summary(value))
                    .collect()
            };
            let observation = SkillObservation {
                availability: availability_for_state(source.state),
                entrypoint: path.to_string_lossy().into_owned(),
                installation_state: source.state,
                invocation_owner: invocation_owner.to_string(),
                observed_at: observed_at.clone(),
                runtime_identity: runtime_identity.to_string(),
                source_host: source_host.to_string(),
                verification: VerificationEvidence {
                    method: "host-root-observation".into(),
                    status: match source.state {
                        InstallationState::Callable => "callable-root-observed",
                        InstallationState::Installed => "installed-not-invoked",
                        InstallationState::Catalogued => "catalogue-only",
                        InstallationState::Degraded => "degraded",
                        InstallationState::Unreachable => "host-unreachable",
                    }
                    .into(),
                    observed_at: observed_at.clone(),
                    evidence: None,
                },
            };
            records
                .entry(key)
                .and_modify(|record| {
                    record.sharing_policy =
                        restrictive_sharing_policy(record.sharing_policy, source.sharing_policy);
                    record.observations.push(observation.clone());
                })
                .or_insert_with(|| CanonicalSkill {
                    abstract_requirements: abstract_requirements(&frontmatter),
                    capabilities,
                    connectors: frontmatter.connectors.clone(),
                    content_hash,
                    display_name,
                    observations: vec![observation],
                    permissions: frontmatter.permissions.clone(),
                    sharing_policy: source.sharing_policy,
                    skill_id,
                    source_package: source.package.clone(),
                });
        }
    }
    let mut registry = CanonicalRegistry {
        generated_at: observed_at,
        registry_revision: String::new(),
        registry_version,
        skills: records.into_values().collect(),
        ttl_seconds,
    };
    let pre_revision_digest = canonical_digest(&registry)?;
    registry.registry_revision = pre_revision_digest.clone();
    let digest = canonical_digest(&registry)?;
    let relay = relay_safe_projection(&registry, digest);
    Ok((registry, relay))
}

fn load_canonical(path: &Path) -> Result<CanonicalRegistry, CliError> {
    let bytes = fs::read(path)
        .map_err(|e| CliError::Other(format!("failed to read {}: {e}", path.display())))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| CliError::Usage(format!("invalid canonical registry: {e}")))
}

fn load_relay_safe(path: &Path) -> Result<RelaySafeRegistry, CliError> {
    let bytes = fs::read(path)
        .map_err(|e| CliError::Other(format!("failed to read {}: {e}", path.display())))?;
    let mut registry: RelaySafeRegistry = serde_json::from_slice(&bytes)
        .map_err(|e| CliError::Usage(format!("invalid relay-safe registry: {e}")))?;
    let now = Utc::now();
    for record in &mut registry.skills {
        let stale = DateTime::parse_from_rfc3339(&record.observed_at)
            .map(|observed| {
                let age = now.signed_duration_since(observed.with_timezone(&Utc));
                age.num_seconds() < 0 || age.num_seconds() as u64 > registry.ttl_seconds
            })
            .unwrap_or(true);
        if stale {
            record.availability = Availability::Unknown;
            record.routable = false;
        }
    }
    Ok(registry)
}

fn validate_relay_redaction(value: &serde_json::Value) -> Result<(), CliError> {
    const FORBIDDEN: &[&str] = &[
        "entrypoint",
        "sourceHost",
        "permissions",
        "connectors",
        "contentHash",
        "verification",
        "evidence",
    ];
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if FORBIDDEN.contains(&key.as_str()) {
                    return Err(CliError::Usage(format!(
                        "relay-safe registry contains forbidden field '{key}'"
                    )));
                }
                validate_relay_redaction(child)?;
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                validate_relay_redaction(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

struct ScanParams<'a> {
    sources: &'a [String],
    host: &'a str,
    runtime: &'a str,
    owner: &'a str,
    registry_version: u64,
    ttl_seconds: u64,
    out: Option<&'a Path>,
    relay_out: Option<&'a Path>,
}

fn cmd_scan(params: ScanParams<'_>) -> Result<(), CliError> {
    let canonical_path = params
        .out
        .map(Path::to_path_buf)
        .map(Ok)
        .unwrap_or_else(|| registry_default_path("canonical.json"))?;
    let relay_path = params
        .relay_out
        .map(Path::to_path_buf)
        .map(Ok)
        .unwrap_or_else(|| registry_default_path("relay-safe.json"))?;
    let (canonical, relay) = scan_registry(
        params.sources,
        params.host,
        params.runtime,
        params.owner,
        params.registry_version,
        params.ttl_seconds,
    )?;
    write_json(&canonical_path, &canonical)?;
    write_json(&relay_path, &relay)?;
    println!(
        "{}",
        serde_json::json!({
            "canonical": canonical_path,
            "relaySafe": relay_path,
            "registryRevision": canonical.registry_revision,
            "skills": canonical.skills.len(),
            "relayRecords": relay.skills.len(),
            "routable": relay.skills.iter().filter(|skill| skill.routable).count(),
        })
    );
    Ok(())
}

fn cmd_validate(path: &Path, relay_safe: bool) -> Result<(), CliError> {
    if relay_safe {
        let bytes = fs::read(path)
            .map_err(|e| CliError::Other(format!("failed to read {}: {e}", path.display())))?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| CliError::Usage(format!("invalid JSON: {e}")))?;
        validate_relay_redaction(&value)?;
        let registry: RelaySafeRegistry = serde_json::from_value(value)
            .map_err(|e| CliError::Usage(format!("invalid relay-safe registry: {e}")))?;
        println!(
            "{}",
            serde_json::json!({"valid": true, "kind": "relay-safe", "skills": registry.skills.len()})
        );
    } else {
        let registry = load_canonical(path)?;
        let digest = canonical_digest(&registry)?;
        println!(
            "{}",
            serde_json::json!({"valid": true, "kind": "canonical", "skills": registry.skills.len(), "digest": digest})
        );
    }
    Ok(())
}

fn cmd_list(path: &Path, query: Option<&str>, routable_only: bool) -> Result<(), CliError> {
    let registry = load_relay_safe(path)?;
    let query = query.map(str::to_ascii_lowercase);
    let mut records: Vec<&RelaySafeSkill> = registry
        .skills
        .iter()
        .filter(|record| !routable_only || record.routable)
        .filter(|record| {
            query.as_ref().is_none_or(|needle| {
                record.skill_id.to_ascii_lowercase().contains(needle)
                    || record.display_name.to_ascii_lowercase().contains(needle)
                    || record
                        .capabilities
                        .iter()
                        .any(|capability| capability.to_ascii_lowercase().contains(needle))
            })
        })
        .collect();
    records.sort_by_key(|record| (&record.skill_id, &record.owning_agent));
    println!(
        "{}",
        serde_json::to_string_pretty(&records)
            .map_err(|e| CliError::Other(format!("failed to serialize skill list: {e}")))?
    );
    Ok(())
}

fn cmd_show(path: &Path, skill_id: &str) -> Result<(), CliError> {
    let registry = load_relay_safe(path)?;
    let records: Vec<&RelaySafeSkill> = registry
        .skills
        .iter()
        .filter(|record| record.skill_id == skill_id)
        .collect();
    if records.is_empty() {
        return Err(CliError::NotFound(format!("skill not found: {skill_id}")));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&records)
            .map_err(|e| CliError::Other(format!("failed to serialize skill: {e}")))?
    );
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RouteRequest {
    correlation_id: String,
    expected_owner: String,
    registry_digest: String,
    registry_revision: String,
    required_evidence: Vec<String>,
    skill_id: String,
    skill_observed_at: String,
    task: String,
    version: u8,
}

fn select_route<'a>(
    registry: &'a RelaySafeRegistry,
    skill_id: &str,
    expected_owner: Option<&str>,
) -> Result<&'a RelaySafeSkill, CliError> {
    let mut candidates: Vec<&RelaySafeSkill> = registry
        .skills
        .iter()
        .filter(|record| record.skill_id == skill_id && record.routable)
        .filter(|record| expected_owner.is_none_or(|owner| record.owning_agent == owner))
        .collect();
    candidates.sort_by(|a, b| b.observed_at.cmp(&a.observed_at));
    candidates.into_iter().next().ok_or_else(|| {
        CliError::NotFound(format!(
            "no current routable observation for skill '{skill_id}'{}",
            expected_owner
                .map(|owner| format!(" owned by {owner}"))
                .unwrap_or_default()
        ))
    })
}

fn build_route_request(
    registry: &RelaySafeRegistry,
    skill_id: &str,
    expected_owner: Option<&str>,
    task: &str,
    evidence: &[String],
    correlation_id: Option<&str>,
) -> Result<(RouteRequest, String), CliError> {
    let selected = select_route(registry, skill_id, expected_owner)?;
    validate_hex64(&selected.owning_agent)?;
    let correlation_id = match correlation_id {
        Some(value) => Uuid::parse_str(value)
            .map_err(|e| CliError::Usage(format!("invalid correlation id: {e}")))?
            .to_string(),
        None => Uuid::new_v4().to_string(),
    };
    Ok((
        RouteRequest {
            correlation_id,
            expected_owner: selected.owning_agent.clone(),
            registry_digest: registry.registry_digest.clone(),
            registry_revision: registry.registry_revision.clone(),
            required_evidence: evidence.to_vec(),
            skill_id: selected.skill_id.clone(),
            skill_observed_at: selected.observed_at.clone(),
            task: task.to_string(),
            version: 1,
        },
        selected.owning_agent.clone(),
    ))
}

struct RouteParams<'a> {
    path: &'a Path,
    skill_id: &'a str,
    expected_owner: Option<&'a str>,
    channel: &'a str,
    task: &'a str,
    evidence: &'a [String],
    correlation_id: Option<&'a str>,
    dry_run: bool,
}

async fn cmd_route(client: &BuzzClient, params: RouteParams<'_>) -> Result<(), CliError> {
    let registry = load_relay_safe(params.path)?;
    let task = read_or_stdin(params.task)?;
    let (request, owner) = build_route_request(
        &registry,
        params.skill_id,
        params.expected_owner,
        &task,
        params.evidence,
        params.correlation_id,
    )?;
    let request_json = serde_json::to_string_pretty(&request)
        .map_err(|e| CliError::Other(format!("failed to serialize route request: {e}")))?;
    if params.dry_run {
        println!("{request_json}");
        return Ok(());
    }
    let content = format!("skill-route/v1\n```json\n{request_json}\n```");
    crate::commands::messages::cmd_send_message(
        client,
        crate::commands::messages::SendMessageParams {
            channel_id: params.channel.to_string(),
            content,
            kind: None,
            reply_to: None,
            broadcast: false,
            files: vec![],
            mentions: vec![owner],
        },
    )
    .await
}

fn publication_chunks(registry: &RelaySafeRegistry) -> Result<Vec<String>, CliError> {
    let header = format!(
        "skill-registry/v1\nrevision: {}\ndigest: {}\n```json\n",
        registry.registry_revision, registry.registry_digest
    );
    let footer = "```\n";
    let mut chunks = Vec::new();
    let mut current = header.clone();
    for record in &registry.skills {
        let line = serde_json::to_string(record)
            .map_err(|e| CliError::Other(format!("failed to serialize relay record: {e}")))?;
        if current.len() + line.len() + 1 + footer.len() > MAX_CONTENT_BYTES {
            if current == header {
                return Err(CliError::Usage(format!(
                    "relay-safe skill record exceeds the message size limit: {}",
                    record.skill_id
                )));
            }
            current.push_str(footer);
            chunks.push(current);
            current = header.clone();
        }
        current.push_str(&line);
        current.push('\n');
    }
    if current != header || chunks.is_empty() {
        current.push_str(footer);
        chunks.push(current);
    }
    Ok(chunks)
}

async fn cmd_publish(
    client: &BuzzClient,
    path: &Path,
    channel: &str,
    dry_run: bool,
) -> Result<(), CliError> {
    let bytes = fs::read(path)
        .map_err(|e| CliError::Other(format!("failed to read {}: {e}", path.display())))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| CliError::Usage(format!("invalid JSON: {e}")))?;
    validate_relay_redaction(&value)?;
    let registry: RelaySafeRegistry = serde_json::from_value(value)
        .map_err(|e| CliError::Usage(format!("invalid relay-safe registry: {e}")))?;
    let chunks = publication_chunks(&registry)?;
    if dry_run {
        println!(
            "{}",
            serde_json::json!({
                "channel": channel,
                "chunks": chunks.len(),
                "records": registry.skills.len(),
                "registryDigest": registry.registry_digest,
            })
        );
        return Ok(());
    }
    for content in chunks {
        crate::commands::messages::cmd_send_message(
            client,
            crate::commands::messages::SendMessageParams {
                channel_id: channel.to_string(),
                content,
                kind: None,
                reply_to: None,
                broadcast: false,
                files: vec![],
                mentions: vec![],
            },
        )
        .await?;
    }
    Ok(())
}

pub(crate) fn dispatch_local(command: &SkillsCmd) -> Result<(), CliError> {
    match command {
        SkillsCmd::Scan {
            source,
            host,
            runtime,
            owner,
            registry_version,
            ttl_seconds,
            out,
            relay_out,
        } => cmd_scan(ScanParams {
            sources: source,
            host,
            runtime,
            owner,
            registry_version: *registry_version,
            ttl_seconds: *ttl_seconds,
            out: out.as_deref(),
            relay_out: relay_out.as_deref(),
        }),
        SkillsCmd::Validate {
            registry,
            relay_safe,
        } => cmd_validate(registry, *relay_safe),
        SkillsCmd::List {
            registry,
            query,
            routable,
        } => cmd_list(registry, query.as_deref(), *routable),
        SkillsCmd::Show { registry, skill_id } => cmd_show(registry, skill_id),
        SkillsCmd::Route { .. } | SkillsCmd::Publish { .. } => Err(CliError::Other(
            "relay skill command reached local dispatcher".into(),
        )),
    }
}

pub(crate) async fn dispatch_relay(
    command: SkillsCmd,
    client: &BuzzClient,
) -> Result<(), CliError> {
    match command {
        SkillsCmd::Route {
            registry,
            skill_id,
            owner,
            channel,
            task,
            evidence,
            correlation_id,
            dry_run,
        } => {
            cmd_route(
                client,
                RouteParams {
                    path: &registry,
                    skill_id: &skill_id,
                    expected_owner: owner.as_deref(),
                    channel: &channel,
                    task: &task,
                    evidence: &evidence,
                    correlation_id: correlation_id.as_deref(),
                    dry_run,
                },
            )
            .await
        }
        SkillsCmd::Publish {
            registry,
            channel,
            dry_run,
        } => cmd_publish(client, &registry, &channel, dry_run).await,
        other => dispatch_local(&other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_skill(root: &Path, name: &str, description: &str) -> PathBuf {
        let directory = root.join(name);
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("SKILL.md");
        fs::write(
            &path,
            format!(
                "---\nname: {name}\ndescription: {description}\nconnectors: [github]\n---\n# Body\n"
            ),
        )
        .unwrap();
        path
    }

    #[test]
    fn scan_deduplicates_content_and_preserves_observations() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        fixture_skill(first.path(), "review", "Review a change");
        fixture_skill(second.path(), "review", "Review a change");
        let sources = vec![
            format!("callable:team-indexed:codex:{}", first.path().display()),
            format!("installed:team-indexed:codex:{}", second.path().display()),
        ];
        let (canonical, relay) =
            scan_registry(&sources, "host-a", "codex", &"a".repeat(64), 1, 60).unwrap();
        assert_eq!(canonical.skills.len(), 1);
        assert_eq!(canonical.skills[0].observations.len(), 2);
        assert_eq!(relay.skills.len(), 2);
        assert_eq!(
            relay.skills.iter().filter(|record| record.routable).count(),
            1
        );
    }

    #[test]
    fn relay_projection_omits_private_and_sensitive_fields() {
        let root = tempfile::tempdir().unwrap();
        fixture_skill(root.path(), "private-one", "Private");
        let sources = vec![format!("callable:private:codex:{}", root.path().display())];
        let (_, relay) =
            scan_registry(&sources, "secret-host", "codex", &"b".repeat(64), 1, 60).unwrap();
        assert!(relay.skills.is_empty());
        let value = serde_json::to_value(&relay).unwrap();
        validate_relay_redaction(&value).unwrap();
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(!encoded.contains("secret-host"));
        assert!(!encoded.contains("entrypoint"));
        assert!(!encoded.contains("connectors"));
    }

    #[test]
    fn stale_observations_become_unknown_and_not_routable() {
        let observation = SkillObservation {
            availability: Availability::Available,
            entrypoint: "/private/skill".into(),
            installation_state: InstallationState::Callable,
            invocation_owner: "c".repeat(64),
            observed_at: "2020-01-01T00:00:00Z".into(),
            runtime_identity: "codex".into(),
            source_host: "host".into(),
            verification: VerificationEvidence {
                method: "test".into(),
                status: "ok".into(),
                observed_at: "2020-01-01T00:00:00Z".into(),
                evidence: None,
            },
        };
        assert_eq!(
            effective_availability(&observation, Utc::now(), DEFAULT_TTL_SECONDS),
            Availability::Unknown
        );
    }

    #[test]
    fn route_requires_a_current_callable_owner() {
        let registry = RelaySafeRegistry {
            generated_at: Utc::now().to_rfc3339(),
            registry_digest: "sha256:test".into(),
            registry_revision: "revision".into(),
            registry_version: 1,
            ttl_seconds: 60,
            skills: vec![RelaySafeSkill {
                abstract_requirements: vec![],
                availability: Availability::Unknown,
                capabilities: vec!["Review".into()],
                display_name: "review".into(),
                installation_state: InstallationState::Installed,
                observed_at: Utc::now().to_rfc3339(),
                owning_agent: "d".repeat(64),
                registry_version: 1,
                routable: false,
                runtime_class: "codex".into(),
                skill_id: "codex:review".into(),
            }],
        };
        assert!(select_route(&registry, "codex:review", None).is_err());
    }

    #[test]
    fn publication_fences_registry_records_to_disable_mention_parsing() {
        let registry = RelaySafeRegistry {
            generated_at: Utc::now().to_rfc3339(),
            registry_digest: "sha256:test".into(),
            registry_revision: "revision".into(),
            registry_version: 1,
            ttl_seconds: 60,
            skills: vec![RelaySafeSkill {
                abstract_requirements: vec![],
                availability: Availability::Unknown,
                capabilities: vec!["Document the literal @mention syntax".into()],
                display_name: "review".into(),
                installation_state: InstallationState::Installed,
                observed_at: Utc::now().to_rfc3339(),
                owning_agent: "d".repeat(64),
                registry_version: 1,
                routable: false,
                runtime_class: "codex".into(),
                skill_id: "codex:review".into(),
            }],
        };

        let chunks = publication_chunks(&registry).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].contains("```json\n"));
        assert!(chunks[0].contains("@mention"));
        assert!(chunks[0].ends_with("```\n"));
    }

    #[test]
    fn source_spec_preserves_absolute_path() {
        let root = tempfile::tempdir().unwrap();
        let value = format!("callable:team-indexed:codex:{}", root.path().display());
        let parsed = SourceSpec::parse(&value).unwrap();
        assert_eq!(parsed.path, root.path());
        assert_eq!(parsed.state, InstallationState::Callable);
        assert_eq!(parsed.sharing_policy, SharingPolicy::TeamIndexed);
    }
}
