use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const ACP_PROTOCOL_VERSION: u64 = 2;
const MAX_FRAME_BYTES: usize = 1024 * 1024;
const MAX_PROMPT_BYTES: usize = 256 * 1024;
const MAX_RESULT_BYTES: usize = 512 * 1024;
const MAX_STDERR_BYTES: usize = 64 * 1024;
const DEFAULT_TIMEOUT_SECONDS: u64 = 20 * 60;

#[derive(Clone)]
struct Session {
    cwd: PathBuf,
    system_prompt: Option<String>,
}

#[derive(Clone)]
struct ActiveTurn {
    session_id: String,
    cancel: CancellationToken,
}

#[derive(Default)]
struct State {
    sessions: Mutex<HashMap<String, Session>>,
    active: Mutex<Option<ActiveTurn>>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("buzz-antigravity-acp: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let state = Arc::new(State::default());
    let (wire_tx, mut wire_rx) = mpsc::channel::<Value>(64);
    let writer = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(message) = wire_rx.recv().await {
            let mut bytes = serde_json::to_vec(&message)
                .map_err(|error| format!("failed to serialize ACP response: {error}"))?;
            bytes.push(b'\n');
            stdout
                .write_all(&bytes)
                .await
                .map_err(|error| format!("failed to write ACP response: {error}"))?;
            stdout
                .flush()
                .await
                .map_err(|error| format!("failed to flush ACP response: {error}"))?;
        }
        Ok::<(), String>(())
    });

    let mut stdin = BufReader::new(tokio::io::stdin());
    while let Some(line) = read_bounded_line(&mut stdin, MAX_FRAME_BYTES)
        .await
        .map_err(|error| format!("failed to read ACP request: {error}"))?
    {
        let message: Value = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(error) => {
                send(
                    &wire_tx,
                    rpc_error(Value::Null, -32700, &format!("invalid JSON: {error}")),
                )
                .await;
                continue;
            }
        };
        dispatch(state.clone(), wire_tx.clone(), message).await;
    }

    if let Some(active) = state.active.lock().await.take() {
        active.cancel.cancel();
    }
    drop(wire_tx);
    writer
        .await
        .map_err(|error| format!("ACP writer task failed: {error}"))?
}

async fn dispatch(state: Arc<State>, wire: mpsc::Sender<Value>, message: Value) {
    if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        send(
            &wire,
            rpc_error(
                message.get("id").cloned().unwrap_or(Value::Null),
                -32600,
                "jsonrpc must be 2.0",
            ),
        )
        .await;
        return;
    }

    let id = message.get("id").cloned();
    let method = message.get("method").and_then(Value::as_str);
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    match (method, id.clone()) {
        (Some("initialize"), Some(id)) => {
            send(
                &wire,
                rpc_ok(
                    id,
                    json!({
                        "protocolVersion": ACP_PROTOCOL_VERSION,
                        "agentCapabilities": {
                            "loadSession": false,
                            "promptCapabilities": {
                                "image": false,
                                "audio": false,
                                "embeddedContext": false
                            }
                        },
                        "agentInfo": {
                            "name": "buzz-antigravity-acp",
                            "title": "Google Antigravity",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                ),
            )
            .await;
        }
        (Some("session/new"), Some(id)) => new_session(&state, &wire, id, params).await,
        (Some("session/prompt"), Some(id)) => {
            start_prompt(state, wire, id, params).await;
        }
        (Some("session/cancel"), _) => cancel_session(&state, &wire, id, params).await,
        (Some(_), Some(id)) => {
            send(&wire, rpc_error(id, -32601, "method not found")).await;
        }
        (Some(_), None) => {}
        (None, Some(_)) => {}
        (None, None) => {
            send(
                &wire,
                rpc_error(Value::Null, -32600, "request is missing method"),
            )
            .await;
        }
    }
}

async fn new_session(state: &Arc<State>, wire: &mpsc::Sender<Value>, id: Value, params: Value) {
    let Some(cwd) = params.get("cwd").and_then(Value::as_str) else {
        send(wire, rpc_error(id, -32602, "session/new requires cwd")).await;
        return;
    };
    let path = PathBuf::from(cwd);
    if !path.is_absolute() || !path.is_dir() {
        send(
            wire,
            rpc_error(
                id,
                -32602,
                "session/new cwd must be an existing absolute directory",
            ),
        )
        .await;
        return;
    }
    let system_prompt = params
        .get("systemPrompt")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if system_prompt
        .as_ref()
        .is_some_and(|prompt| prompt.len() > MAX_PROMPT_BYTES)
    {
        send(
            wire,
            rpc_error(id, -32602, "session/new system prompt is too large"),
        )
        .await;
        return;
    }
    let session_id = Uuid::new_v4().to_string();
    state.sessions.lock().await.insert(
        session_id.clone(),
        Session {
            cwd: path,
            system_prompt,
        },
    );
    send(wire, rpc_ok(id, json!({ "sessionId": session_id }))).await;
}

async fn start_prompt(state: Arc<State>, wire: mpsc::Sender<Value>, id: Value, params: Value) {
    let Some(session_id) = params
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        send(
            &wire,
            rpc_error(id, -32602, "session/prompt requires sessionId"),
        )
        .await;
        return;
    };
    let prompt = match extract_prompt(&params) {
        Ok(prompt) => prompt,
        Err(error) => {
            send(&wire, rpc_error(id, -32602, &error)).await;
            return;
        }
    };
    let Some(session) = state.sessions.lock().await.get(&session_id).cloned() else {
        send(&wire, rpc_error(id, -32602, "unknown sessionId")).await;
        return;
    };
    let bounded_prompt = match compose_prompt(session.system_prompt.as_deref(), &prompt) {
        Ok(prompt) => prompt,
        Err(error) => {
            send(&wire, rpc_error(id, -32602, &error)).await;
            return;
        }
    };
    let cancel = CancellationToken::new();
    {
        let mut active = state.active.lock().await;
        if active.is_some() {
            send(
                &wire,
                rpc_error(
                    id,
                    -32001,
                    "Antigravity adapter permits only one active task",
                ),
            )
            .await;
            return;
        }
        *active = Some(ActiveTurn {
            session_id: session_id.clone(),
            cancel: cancel.clone(),
        });
    }

    tokio::spawn(async move {
        let outcome = run_antigravity(&session.cwd, &bounded_prompt, cancel).await;
        match outcome {
            Ok(PromptOutcome::Completed(text)) => {
                send(&wire, agent_message(&session_id, &text)).await;
                send(&wire, rpc_ok(id, json!({ "stopReason": "end_turn" }))).await;
            }
            Ok(PromptOutcome::Cancelled) => {
                send(&wire, rpc_ok(id, json!({ "stopReason": "cancelled" }))).await;
            }
            Err(error) => {
                send(&wire, rpc_error(id, -32000, &error)).await;
            }
        }
        let mut active = state.active.lock().await;
        if active
            .as_ref()
            .is_some_and(|turn| turn.session_id == session_id)
        {
            *active = None;
        }
    });
}

async fn cancel_session(
    state: &Arc<State>,
    wire: &mpsc::Sender<Value>,
    id: Option<Value>,
    params: Value,
) {
    let requested = params.get("sessionId").and_then(Value::as_str);
    let active = state.active.lock().await.clone();
    if let Some(active) = active.filter(|turn| requested == Some(turn.session_id.as_str())) {
        active.cancel.cancel();
    }
    if let Some(id) = id {
        send(wire, rpc_ok(id, json!({}))).await;
    }
}

enum PromptOutcome {
    Completed(String),
    Cancelled,
}

async fn run_antigravity(
    cwd: &Path,
    prompt: &str,
    cancel: CancellationToken,
) -> Result<PromptOutcome, String> {
    let binary = std::env::var_os("BUZZ_ANTIGRAVITY_CLI")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("agy"));
    let timeout_seconds = std::env::var("BUZZ_ANTIGRAVITY_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_TIMEOUT_SECONDS);
    let mut command = Command::new(binary);
    command.args(antigravity_args(cwd, prompt));
    let mut child = command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("failed to start Antigravity CLI: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Antigravity CLI stdout was unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Antigravity CLI stderr was unavailable".to_string())?;
    let stderr_task = tokio::spawn(read_bounded_stream(stderr, MAX_STDERR_BYTES));
    let deadline = tokio::time::sleep(Duration::from_secs(timeout_seconds));
    tokio::pin!(deadline);
    let mut reader = BufReader::new(stdout);
    let mut result_text: Option<String> = None;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Ok(PromptOutcome::Cancelled);
            }
            _ = &mut deadline => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(format!("Antigravity task exceeded {timeout_seconds} seconds"));
            }
            line = read_bounded_line(&mut reader, MAX_FRAME_BYTES) => {
                let line = line.map_err(|error| format!("invalid Antigravity stream: {error}"))?;
                let Some(line) = line else { break };
                let event: Value = serde_json::from_str(&line)
                    .map_err(|error| format!("Antigravity emitted invalid stream JSON: {error}"))?;
                if let Some(text) = result_text_from_event(&event) {
                    if text.len() > MAX_RESULT_BYTES {
                        return Err("Antigravity result exceeded the bounded output limit".to_string());
                    }
                    result_text = Some(text);
                }
                if event.get("type").and_then(Value::as_str) == Some("error") {
                    return Err(event_error_message(&event));
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|error| format!("failed to wait for Antigravity CLI: {error}"))?;
    let stderr = stderr_task
        .await
        .map_err(|error| format!("failed to collect Antigravity stderr: {error}"))?
        .map_err(|error| format!("failed to read Antigravity stderr: {error}"))?;
    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr);
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!("Antigravity CLI exited with {status}")
        } else {
            format!("Antigravity CLI exited with {status}: {detail}")
        });
    }
    let text = result_text
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| "Antigravity stream ended without a terminal result".to_string())?;
    Ok(PromptOutcome::Completed(text))
}

fn antigravity_args(cwd: &Path, prompt: &str) -> Vec<std::ffi::OsString> {
    vec![
        "-p".into(),
        prompt.into(),
        "--cwd".into(),
        cwd.as_os_str().to_owned(),
        "--output-format".into(),
        "stream-json".into(),
    ]
}

fn extract_prompt(params: &Value) -> Result<String, String> {
    let blocks = params
        .get("prompt")
        .and_then(Value::as_array)
        .ok_or_else(|| "session/prompt requires prompt content blocks".to_string())?;
    let mut text = String::new();
    for block in blocks {
        if block.get("type").and_then(Value::as_str) != Some("text") {
            return Err("Antigravity adapter accepts text prompt blocks only".to_string());
        }
        let value = block
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| "text prompt block is missing text".to_string())?;
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(value);
        if text.len() > MAX_PROMPT_BYTES {
            return Err("session/prompt content is too large".to_string());
        }
    }
    if text.trim().is_empty() {
        return Err("session/prompt content is empty".to_string());
    }
    Ok(text)
}

fn compose_prompt(system_prompt: Option<&str>, prompt: &str) -> Result<String, String> {
    let value = match system_prompt.filter(|value| !value.trim().is_empty()) {
        Some(system) => {
            format!("<buzz_system_context>\n{system}\n</buzz_system_context>\n\n{prompt}")
        }
        None => prompt.to_string(),
    };
    if value.len() > MAX_PROMPT_BYTES {
        Err("combined Antigravity task context is too large".to_string())
    } else {
        Ok(value)
    }
}

fn result_text_from_event(event: &Value) -> Option<String> {
    if event.get("type").and_then(Value::as_str) != Some("result") {
        return None;
    }
    for candidate in [
        event.get("response"),
        event.get("result"),
        event.get("output"),
        event.get("text"),
        event.pointer("/result/response"),
        event.pointer("/result/output"),
        event.pointer("/result/text"),
    ] {
        if let Some(text) = candidate.and_then(Value::as_str) {
            return Some(text.to_string());
        }
    }
    None
}

fn event_error_message(event: &Value) -> String {
    for candidate in [
        event.get("message"),
        event.get("error"),
        event.pointer("/error/message"),
    ] {
        if let Some(message) = candidate.and_then(Value::as_str) {
            return format!("Antigravity task failed: {message}");
        }
    }
    "Antigravity task failed".to_string()
}

fn rpc_ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

fn agent_message(session_id: &str, text: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": session_id,
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": { "type": "text", "text": text }
            }
        }
    })
}

async fn send(wire: &mpsc::Sender<Value>, message: Value) {
    let _ = wire.send(message).await;
}

async fn read_bounded_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> std::io::Result<Option<String>> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unterminated JSON line",
            ));
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if bytes.len().saturating_add(take) > max {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("line exceeds {max} bytes"),
            ));
        }
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take);
        if bytes.ends_with(b"\n") {
            bytes.pop();
            if bytes.ends_with(b"\r") {
                bytes.pop();
            }
            return String::from_utf8(bytes).map(Some).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "line is not UTF-8")
            });
        }
    }
}

async fn read_bounded_stream<R: AsyncRead + Unpin>(
    mut reader: R,
    max: usize,
) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Ok(bytes);
        }
        let remaining = max.saturating_sub(bytes.len());
        if remaining == 0 {
            continue;
        }
        bytes.extend_from_slice(&chunk[..read.min(remaining)]);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        antigravity_args, compose_prompt, extract_prompt, result_text_from_event, MAX_PROMPT_BYTES,
    };
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn extracts_text_blocks_without_accepting_other_content() {
        let params = json!({
            "prompt": [
                { "type": "text", "text": "first" },
                { "type": "text", "text": "second" }
            ]
        });
        assert_eq!(extract_prompt(&params).unwrap(), "first\nsecond");
        assert!(extract_prompt(&json!({
            "prompt": [{ "type": "image", "data": "ignored" }]
        }))
        .is_err());
    }

    #[test]
    fn combined_context_is_explicit_and_bounded() {
        let prompt = compose_prompt(Some("owner-only rules"), "do work").unwrap();
        assert!(prompt.contains("<buzz_system_context>"));
        assert!(prompt.ends_with("do work"));
        assert!(compose_prompt(Some(&"x".repeat(MAX_PROMPT_BYTES)), "y").is_err());
    }

    #[test]
    fn only_terminal_result_events_become_agent_output() {
        assert_eq!(
            result_text_from_event(&json!({
                "type": "result",
                "response": "complete"
            })),
            Some("complete".to_string())
        );
        assert_eq!(
            result_text_from_event(&json!({
                "type": "result",
                "result": { "text": "nested" }
            })),
            Some("nested".to_string())
        );
        assert!(result_text_from_event(&json!({
            "type": "step_update",
            "text": "not terminal"
        }))
        .is_none());
    }

    #[test]
    fn invocation_is_one_shot_stream_json_without_resume() {
        let args = antigravity_args(Path::new("/tmp/work"), "bounded task");
        let args = args
            .iter()
            .map(|value| value.to_string_lossy())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "-p",
                "bounded task",
                "--cwd",
                "/tmp/work",
                "--output-format",
                "stream-json"
            ]
        );
        assert!(!args.iter().any(|value| value.contains("continue")));
        assert!(!args.iter().any(|value| value.contains("token")));
    }
}
