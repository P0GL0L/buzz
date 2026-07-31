use std::sync::{atomic::AtomicBool, Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;
use tauri::State;

use crate::app_state::AppState;
use crate::huddle::{models, stt, HuddlePhase};
use crate::microphone_lease::{MicrophoneLeaseRuntime, MICROPHONE_OWNER_DICTATION};

const DICTATION_VERSION: u16 = 1;
const MAX_AUDIO_BATCH_BYTES: usize = 100 * 1024;
const MAX_DICTATION_DURATION: Duration = Duration::from_secs(120);
const FLUSH_TIMEOUT: Duration = Duration::from_secs(15);

struct DictationSession {
    pipeline: stt::SttPipeline,
    output_rx: tokio::sync::mpsc::Receiver<stt::SttOutput>,
    started_at: Instant,
    started_at_rfc3339: String,
}

struct DictationRuntimeState {
    phase: &'static str,
    session: Option<DictationSession>,
    message: Option<String>,
}

impl Default for DictationRuntimeState {
    fn default() -> Self {
        Self {
            phase: "idle",
            session: None,
            message: None,
        }
    }
}

/// Isolated local composer-dictation runtime.
///
/// It owns no relay identity and never publishes audio or transcripts. Huddle
/// capture has a separate pipeline and is rejected while dictation is active.
#[derive(Default)]
pub struct DictationRuntime {
    state: Mutex<DictationRuntimeState>,
}

impl DictationRuntime {
    fn lock(&self) -> Result<MutexGuard<'_, DictationRuntimeState>, String> {
        self.state
            .lock()
            .map_err(|_| "dictation state is unavailable".to_string())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictationStatus {
    version: u16,
    phase: &'static str,
    started_at: Option<String>,
    elapsed_ms: u64,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictationResult {
    version: u16,
    completion_state: &'static str,
    transcript: String,
    duration_ms: u64,
    reason: Option<String>,
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn status_from_state(state: &DictationRuntimeState) -> DictationStatus {
    let (started_at, elapsed_ms) = state
        .session
        .as_ref()
        .map(|session| {
            (
                Some(session.started_at_rfc3339.clone()),
                duration_ms(session.started_at.elapsed()),
            )
        })
        .unwrap_or((None, 0));
    DictationStatus {
        version: DICTATION_VERSION,
        phase: state.phase,
        started_at,
        elapsed_ms,
        message: state.message.clone(),
    }
}

/// Start a local English composer-dictation session.
#[tauri::command]
pub async fn start_composer_dictation(
    runtime: State<'_, DictationRuntime>,
    app_state: State<'_, AppState>,
    microphone: State<'_, MicrophoneLeaseRuntime>,
) -> Result<DictationStatus, String> {
    {
        let state = runtime.lock()?;
        if state.session.is_some() || matches!(state.phase, "recording" | "transcribing") {
            return Err("dictation is already active".to_string());
        }
    }
    let mut microphone_claim =
        microphone.claim(MICROPHONE_OWNER_DICTATION, "composer dictation")?;

    {
        let huddle = app_state.huddle()?;
        if huddle.phase != HuddlePhase::Idle {
            return Err(
                "A huddle is using the microphone. Leave the huddle before dictating.".to_string(),
            );
        }
    }

    let model_dir = models::stt_model_dir().ok_or_else(|| {
        "The local English speech model is not ready. Wait for the voice model download to finish."
            .to_string()
    })?;

    let constructed = tokio::task::spawn_blocking(move || {
        stt::SttPipeline::new(model_dir, Arc::new(AtomicBool::new(false)), None, None)
    })
    .await
    .map_err(|error| format!("failed to start local speech recognition: {error}"))??;

    let (pipeline, output_rx) = constructed;
    let mut state = runtime.lock()?;
    if state.session.is_some() {
        pipeline.shutdown();
        return Err("dictation was started by another request".to_string());
    }
    state.phase = "recording";
    state.message = None;
    state.session = Some(DictationSession {
        pipeline,
        output_rx,
        started_at: Instant::now(),
        started_at_rfc3339: Utc::now().to_rfc3339(),
    });
    microphone_claim.retain();
    Ok(status_from_state(&state))
}

/// Push one bounded 48 kHz mono f32 PCM batch into the active dictation.
#[tauri::command]
pub fn push_dictation_pcm(
    request: tauri::ipc::Request<'_>,
    runtime: State<'_, DictationRuntime>,
) -> Result<(), String> {
    let bytes = match request.body() {
        tauri::ipc::InvokeBody::Raw(bytes) => bytes,
        _ => return Err("expected raw binary body".to_string()),
    };
    if bytes.is_empty() || bytes.len() > MAX_AUDIO_BATCH_BYTES {
        return Err(format!(
            "dictation audio batch must be 1..={MAX_AUDIO_BATCH_BYTES} bytes"
        ));
    }

    let state = runtime.lock()?;
    let session = state
        .session
        .as_ref()
        .filter(|_| state.phase == "recording")
        .ok_or_else(|| "dictation is not recording".to_string())?;
    if session.started_at.elapsed() >= MAX_DICTATION_DURATION {
        return Err("dictation reached the two-minute recording limit".to_string());
    }
    session.pipeline.push_audio(bytes.to_vec())
}

/// Finish dictation, flush the final utterance, and return the local transcript.
#[tauri::command]
pub async fn finish_composer_dictation(
    runtime: State<'_, DictationRuntime>,
    microphone: State<'_, MicrophoneLeaseRuntime>,
) -> Result<DictationResult, String> {
    let mut session = {
        let mut state = runtime.lock()?;
        let session = state
            .session
            .take()
            .ok_or_else(|| "dictation is not recording".to_string())?;
        state.phase = "transcribing";
        state.message = None;
        session
    };

    let elapsed = session.started_at.elapsed();
    if let Err(error) = session.pipeline.flush() {
        session.pipeline.shutdown();
        microphone.release(MICROPHONE_OWNER_DICTATION);
        let mut state = runtime.lock()?;
        state.phase = "error";
        state.message = Some(error.clone());
        return Err(error);
    }

    let collect = async {
        let mut segments = Vec::new();
        while let Some(output) = session.output_rx.recv().await {
            match output {
                stt::SttOutput::Transcript(text) => {
                    if !text.trim().is_empty() {
                        segments.push(text.trim().to_string());
                    }
                }
                stt::SttOutput::Flushed => break,
            }
        }
        segments
    };
    let collected = tokio::time::timeout(FLUSH_TIMEOUT, collect).await;
    session.pipeline.shutdown();
    microphone.release(MICROPHONE_OWNER_DICTATION);
    drop(session);

    match collected {
        Ok(segments) => {
            let transcript = segments.join(" ");
            let mut state = runtime.lock()?;
            if transcript.is_empty() {
                let reason = "No speech was detected.".to_string();
                state.phase = "error";
                state.message = Some(reason.clone());
                Ok(DictationResult {
                    version: DICTATION_VERSION,
                    completion_state: "empty",
                    transcript,
                    duration_ms: duration_ms(elapsed),
                    reason: Some(reason),
                })
            } else {
                state.phase = "idle";
                state.message = None;
                Ok(DictationResult {
                    version: DICTATION_VERSION,
                    completion_state: "completed",
                    transcript,
                    duration_ms: duration_ms(elapsed),
                    reason: None,
                })
            }
        }
        Err(_) => {
            let reason = "Local transcription timed out.".to_string();
            let mut state = runtime.lock()?;
            state.phase = "error";
            state.message = Some(reason.clone());
            Ok(DictationResult {
                version: DICTATION_VERSION,
                completion_state: "timeout",
                transcript: String::new(),
                duration_ms: duration_ms(elapsed),
                reason: Some(reason),
            })
        }
    }
}

/// Cancel dictation without transcribing or retaining microphone bytes.
#[tauri::command]
pub fn cancel_composer_dictation(
    runtime: State<'_, DictationRuntime>,
    microphone: State<'_, MicrophoneLeaseRuntime>,
) -> Result<DictationStatus, String> {
    let mut state = runtime.lock()?;
    if let Some(session) = state.session.take() {
        session.pipeline.shutdown();
    }
    microphone.release(MICROPHONE_OWNER_DICTATION);
    state.phase = "cancelled";
    state.message = None;
    Ok(status_from_state(&state))
}

/// Return the current non-secret dictation lifecycle state.
#[tauri::command]
pub fn get_composer_dictation_status(
    runtime: State<'_, DictationRuntime>,
) -> Result<DictationStatus, String> {
    let state = runtime.lock()?;
    Ok(status_from_state(&state))
}

/// Clear a terminal cancelled/error state before the next recording.
#[tauri::command]
pub fn reset_composer_dictation_status(
    runtime: State<'_, DictationRuntime>,
) -> Result<DictationStatus, String> {
    let mut state = runtime.lock()?;
    if state.session.is_some() {
        return Err("cannot reset dictation while it is active".to_string());
    }
    state.phase = "idle";
    state.message = None;
    Ok(status_from_state(&state))
}

#[cfg(test)]
mod tests {
    use super::{duration_ms, status_from_state, DictationRuntimeState};
    use std::time::Duration;

    #[test]
    fn duration_conversion_is_bounded_and_exact_for_normal_sessions() {
        assert_eq!(duration_ms(Duration::from_millis(120_000)), 120_000);
    }

    #[test]
    fn terminal_status_never_claims_an_active_session() {
        let state = DictationRuntimeState {
            phase: "cancelled",
            session: None,
            message: None,
        };
        let status = status_from_state(&state);
        assert_eq!(status.phase, "cancelled");
        assert_eq!(status.elapsed_ms, 0);
        assert!(status.started_at.is_none());
    }
}
