use nostr::JsonUtil;
use serde::{Deserialize, Serialize};

const MAX_EVENT_BYTES: usize = 64 * 1024;
const MAX_TARGET_BYTES: usize = 4 * 1024;
const MAX_TEXT_BYTES: usize = 16 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BrowserActionPayload {
    version: u16,
    action: String,
    actor: String,
    task_owner: String,
    thread_id: String,
    correlation_id: String,
    target: Option<String>,
    text: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedBrowserAction {
    version: u16,
    event_id: String,
    signer: String,
    signature_valid: bool,
    action: String,
    task_owner: String,
    thread_id: String,
    correlation_id: String,
    target: Option<String>,
    text: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BrowserActionResultPayload {
    version: u16,
    request_event_id: String,
    signer: String,
    action: String,
    thread_id: String,
    correlation_id: String,
    completion_state: String,
    evidence_summary: String,
    structured_failure: Option<String>,
    artifact_id: Option<String>,
    artifact_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedBrowserActionResult {
    version: u16,
    event_id: String,
    request_event_id: String,
    signer: String,
    signature_valid: bool,
    action: String,
    thread_id: String,
    correlation_id: String,
    completion_state: String,
    evidence_summary: String,
    structured_failure: Option<String>,
    artifact_id: Option<String>,
    artifact_sha256: Option<String>,
}

fn event_payload<'a>(content: &'a str, schema: &str) -> Result<&'a str, String> {
    let body = content
        .strip_prefix(schema)
        .ok_or_else(|| format!("event is not a {schema} record"))?
        .trim();
    Ok(body
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(body))
}

fn parse_signed_event(event_json: String, schema: &str) -> Result<nostr::Event, String> {
    if event_json.len() > MAX_EVENT_BYTES {
        return Err(format!("{schema} exceeds the 64 KiB review limit"));
    }
    let event = nostr::Event::from_json(event_json)
        .map_err(|error| format!("invalid signed event: {error}"))?;
    event
        .verify()
        .map_err(|error| format!("{schema} signature is invalid: {error}"))?;
    Ok(event)
}

fn valid_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_action(action: &str) -> bool {
    matches!(
        action,
        "navigate"
            | "reload"
            | "back"
            | "forward"
            | "click"
            | "type"
            | "scroll"
            | "extract"
            | "download"
            | "stop"
            | "capture"
            | "clear-session"
            | "clear-profile"
    )
}

fn validate_request_shape(payload: &BrowserActionPayload) -> Result<(), String> {
    if payload.version != 1
        || !valid_hex(&payload.actor, 64)
        || !valid_hex(&payload.task_owner, 64)
        || payload.thread_id.trim().is_empty()
        || payload.thread_id.len() > 128
        || uuid::Uuid::parse_str(&payload.correlation_id).is_err()
        || !valid_action(&payload.action)
        || payload
            .target
            .as_ref()
            .is_some_and(|value| value.len() > MAX_TARGET_BYTES || value.contains('\0'))
        || payload
            .text
            .as_ref()
            .is_some_and(|value| value.len() > MAX_TEXT_BYTES || value.contains('\0'))
        || payload
            .x
            .is_some_and(|value| !value.is_finite() || value.abs() > 100_000.0)
        || payload
            .y
            .is_some_and(|value| !value.is_finite() || value.abs() > 100_000.0)
    {
        return Err("browser action contains invalid or unbounded fields".to_string());
    }
    match payload.action.as_str() {
        "navigate" => {
            let target = payload
                .target
                .as_deref()
                .ok_or_else(|| "navigate action requires a target URL".to_string())?;
            let url = url::Url::parse(target)
                .map_err(|_| "navigate action target is not a valid URL".to_string())?;
            if !matches!(url.scheme(), "http" | "https") {
                return Err("navigate action permits HTTP and HTTPS only".to_string());
            }
        }
        "click" | "type" => {
            if payload.target.as_deref().is_none_or(str::is_empty) {
                return Err(format!("{} action requires a selector", payload.action));
            }
            if payload.action == "type" && payload.text.is_none() {
                return Err("type action requires bounded text".to_string());
            }
        }
        "scroll" if payload.x.is_none() && payload.y.is_none() => {
            return Err("scroll action requires an x or y distance".to_string());
        }
        _ => {}
    }
    Ok(())
}

/// Verify a browser-action/v1 request at the dispatch boundary.
///
/// The caller supplies the task owner, originating thread, and correlation
/// already established by membership, mention, and single-turn dispatch.
#[tauri::command]
pub fn review_signed_browser_action(
    event_json: String,
    expected_task_owner: String,
    expected_thread_id: String,
    expected_correlation_id: String,
) -> Result<ReviewedBrowserAction, String> {
    let event = parse_signed_event(event_json, "browser-action/v1")?;
    let payload: BrowserActionPayload =
        serde_json::from_str(event_payload(&event.content, "browser-action/v1")?)
            .map_err(|error| format!("invalid browser-action/v1 payload: {error}"))?;
    validate_request_shape(&payload)?;
    if payload.actor != event.pubkey.to_hex() {
        return Err("browser action actor does not match its signer".to_string());
    }
    if payload.task_owner != expected_task_owner
        || payload.thread_id != expected_thread_id
        || payload.correlation_id != expected_correlation_id
    {
        return Err(
            "browser action is outside the active task, thread, or correlation".to_string(),
        );
    }
    Ok(ReviewedBrowserAction {
        version: 1,
        event_id: event.id.to_hex(),
        signer: payload.actor,
        signature_valid: true,
        action: payload.action,
        task_owner: payload.task_owner,
        thread_id: payload.thread_id,
        correlation_id: payload.correlation_id,
        target: payload.target,
        text: payload.text,
        x: payload.x,
        y: payload.y,
    })
}

/// Verify a signed browser-action-result/v1 reply and its correlation.
#[tauri::command]
pub fn review_signed_browser_action_result(
    event_json: String,
    expected_request_event_id: String,
    expected_correlation_id: String,
) -> Result<ReviewedBrowserActionResult, String> {
    let event = parse_signed_event(event_json, "browser-action-result/v1")?;
    let payload: BrowserActionResultPayload =
        serde_json::from_str(event_payload(&event.content, "browser-action-result/v1")?)
            .map_err(|error| format!("invalid browser-action-result/v1 payload: {error}"))?;
    let completion_valid = matches!(
        payload.completion_state.as_str(),
        "completed"
            | "denied"
            | "offline"
            | "inactive-window"
            | "permission"
            | "occluded"
            | "interrupted"
            | "unsupported"
            | "failed"
    );
    if payload.version != 1
        || payload.signer != event.pubkey.to_hex()
        || !valid_hex(&payload.signer, 64)
        || !valid_hex(&payload.request_event_id, 64)
        || !valid_action(&payload.action)
        || payload.thread_id.trim().is_empty()
        || payload.thread_id.len() > 128
        || uuid::Uuid::parse_str(&payload.correlation_id).is_err()
        || payload.evidence_summary.trim().is_empty()
        || payload.evidence_summary.len() > 8 * 1024
        || payload.evidence_summary.contains('\0')
        || !completion_valid
        || payload
            .artifact_sha256
            .as_ref()
            .is_some_and(|value| !value.starts_with("sha256:") || !valid_hex(&value[7..], 64))
        || payload
            .artifact_id
            .as_ref()
            .is_some_and(|value| uuid::Uuid::parse_str(value).is_err())
    {
        return Err("browser action result contains invalid or unbounded fields".to_string());
    }
    if payload.request_event_id != expected_request_event_id
        || payload.correlation_id != expected_correlation_id
    {
        return Err("browser action result does not match the requested action".to_string());
    }
    if payload.completion_state == "completed" && payload.structured_failure.is_some() {
        return Err("completed browser action result includes a failure".to_string());
    }
    if payload.completion_state != "completed" && payload.structured_failure.is_none() {
        return Err("failed browser action result lacks a structured reason".to_string());
    }
    Ok(ReviewedBrowserActionResult {
        version: 1,
        event_id: event.id.to_hex(),
        request_event_id: payload.request_event_id,
        signer: payload.signer,
        signature_valid: true,
        action: payload.action,
        thread_id: payload.thread_id,
        correlation_id: payload.correlation_id,
        completion_state: payload.completion_state,
        evidence_summary: payload.evidence_summary,
        structured_failure: payload.structured_failure,
        artifact_id: payload.artifact_id,
        artifact_sha256: payload.artifact_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::{review_signed_browser_action, review_signed_browser_action_result};
    use nostr::{EventBuilder, JsonUtil, Keys, Kind};

    fn signed(schema: &str, payload: serde_json::Value, keys: &Keys) -> nostr::Event {
        EventBuilder::new(
            Kind::TextNote,
            format!("{schema}\n```json\n{}\n```", payload),
        )
        .sign_with_keys(keys)
        .unwrap()
    }

    #[test]
    fn action_requires_valid_signature_actor_and_task_correlation() {
        let keys = Keys::generate();
        let owner = "a".repeat(64);
        let correlation = "98f62eef-6b94-48e0-b2f6-d07aa798ef47";
        let event = signed(
            "browser-action/v1",
            serde_json::json!({
                "version": 1,
                "action": "navigate",
                "actor": keys.public_key().to_hex(),
                "taskOwner": owner,
                "threadId": "thread-1",
                "correlationId": correlation,
                "target": "https://example.com",
                "text": null,
                "x": null,
                "y": null
            }),
            &keys,
        );
        assert!(review_signed_browser_action(
            event.as_json(),
            owner.clone(),
            "thread-1".into(),
            correlation.into()
        )
        .is_ok());
        let tampered = event.as_json().replace("example.com", "example.org");
        assert!(review_signed_browser_action(
            tampered,
            owner,
            "thread-1".into(),
            correlation.into()
        )
        .is_err());
    }

    #[test]
    fn result_requires_signature_correlation_and_structured_failure() {
        let keys = Keys::generate();
        let request = "e".repeat(64);
        let correlation = "98f62eef-6b94-48e0-b2f6-d07aa798ef47";
        let event = signed(
            "browser-action-result/v1",
            serde_json::json!({
                "version": 1,
                "requestEventId": request,
                "signer": keys.public_key().to_hex(),
                "action": "capture",
                "threadId": "thread-1",
                "correlationId": correlation,
                "completionState": "permission",
                "evidenceSummary": "macOS denied visible-region capture",
                "structuredFailure": "screen-recording-permission",
                "artifactId": null,
                "artifactSha256": null
            }),
            &keys,
        );
        let reviewed =
            review_signed_browser_action_result(event.as_json(), request, correlation.into())
                .unwrap();
        assert!(reviewed.signature_valid);
        assert_eq!(reviewed.completion_state, "permission");
    }
}
