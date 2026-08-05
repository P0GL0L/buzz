use std::process::Command;

use sha2::{Digest, Sha256};
use tauri::Manager;

use super::{
    browser_screenshots_root, ensure_private_dir, BrowserCaptureResult, WorkspaceBrowserRuntime,
};

const MAX_BROWSER_CAPTURE_BYTES: u64 = 50 * 1024 * 1024;

impl BrowserCaptureResult {
    fn failure(
        completion_state: &'static str,
        reason: impl Into<String>,
        actor: String,
        correlation_id: Option<String>,
    ) -> Self {
        Self {
            version: 1,
            completion_state,
            reason: Some(reason.into()),
            local_artifact_path: None,
            filename: None,
            mime_type: None,
            size: None,
            sha256: None,
            width: None,
            height: None,
            actor,
            correlation_id,
        }
    }
}

#[cfg(target_os = "macos")]
pub(super) fn capture_visible_workspace_region(
    app: &tauri::AppHandle,
    runtime: &WorkspaceBrowserRuntime,
    actor: &str,
    correlation_id: Option<String>,
) -> BrowserCaptureResult {
    let fail = |state: &'static str, reason: &str| {
        BrowserCaptureResult::failure(state, reason, actor.to_string(), correlation_id.clone())
    };
    let Some(window) = app.get_window("main") else {
        return fail("inactive-window", "The ASV Buzz window is unavailable.");
    };
    if !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(true) {
        return fail(
            "inactive-window",
            "Bring the ASV Buzz window onscreen before capturing the browser.",
        );
    }
    if !window.is_focused().unwrap_or(false) {
        return fail(
            "inactive-window",
            "Focus ASV Buzz before capture so another window cannot be mistaken for browser content.",
        );
    }
    let bounds = runtime
        .session
        .lock()
        .ok()
        .and_then(|session| session.bounds.clone());
    let Some(bounds) = bounds else {
        return fail(
            "invalid-bounds",
            "The browser workspace has not reported capture bounds.",
        );
    };
    if let Err(error) = bounds.validate() {
        return fail("invalid-bounds", &error);
    }

    let scale = match window.scale_factor() {
        Ok(value) if value.is_finite() && value > 0.0 => value,
        _ => return fail("invalid-bounds", "The window scale factor is unavailable."),
    };
    let outer_position = match window.outer_position() {
        Ok(value) => value.to_logical::<f64>(scale),
        Err(error) => {
            return fail(
                "invalid-bounds",
                &format!("Window position unavailable: {error}"),
            )
        }
    };
    let outer_size = match window.outer_size() {
        Ok(value) => value.to_logical::<f64>(scale),
        Err(error) => {
            return fail(
                "invalid-bounds",
                &format!("Window size unavailable: {error}"),
            )
        }
    };
    let inner_size = match window.inner_size() {
        Ok(value) => value.to_logical::<f64>(scale),
        Err(error) => {
            return fail(
                "invalid-bounds",
                &format!("Content size unavailable: {error}"),
            )
        }
    };
    let horizontal_inset = ((outer_size.width - inner_size.width) / 2.0).max(0.0);
    let top_inset = (outer_size.height - inner_size.height - horizontal_inset).max(0.0);
    let x = (outer_position.x + horizontal_inset + bounds.x).round() as i64;
    let y = (outer_position.y + top_inset + bounds.y).round() as i64;
    let width = bounds.width.round() as i64;
    let height = bounds.height.round() as i64;
    if x < 0 || y < 0 || width < 160 || height < 120 {
        return fail(
            "invalid-bounds",
            "The visible browser region is outside the available screen coordinates.",
        );
    }

    let screenshots = match browser_screenshots_root(app) {
        Ok(path) => path,
        Err(error) => return fail("capture-failed", &error),
    };
    if let Err(error) = ensure_private_dir(&screenshots) {
        return fail("capture-failed", &error);
    }
    let filename = format!(
        "browser-{}-{}.png",
        chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
        uuid::Uuid::new_v4()
    );
    let path = screenshots.join(&filename);
    let region = format!("{x},{y},{width},{height}");
    let output = match Command::new("/usr/sbin/screencapture")
        .args(["-x", "-R", &region])
        .arg(&path)
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return fail(
                "capture-failed",
                &format!("The macOS capture service could not start: {error}"),
            );
        }
    };
    if !output.status.success() {
        let _ = std::fs::remove_file(&path);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let reason = if stderr.to_ascii_lowercase().contains("permission") {
            "macOS Screen Recording permission is required for visible browser capture.".to_string()
        } else if stderr.is_empty() {
            "macOS declined the visible browser capture. Check Screen Recording permission and window visibility.".to_string()
        } else {
            format!("macOS browser capture failed: {stderr}")
        };
        return fail("permission-or-capture-failure", &reason);
    }

    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata)
            if metadata.file_type().is_file()
                && metadata.len() > 0
                && metadata.len() <= MAX_BROWSER_CAPTURE_BYTES =>
        {
            metadata
        }
        Ok(metadata) if metadata.len() > MAX_BROWSER_CAPTURE_BYTES => {
            let _ = std::fs::remove_file(&path);
            return fail(
                "oversized",
                "The browser capture exceeded the 50 MiB limit.",
            );
        }
        _ => {
            let _ = std::fs::remove_file(&path);
            return fail(
                "capture-failed",
                "macOS did not produce a valid browser capture.",
            );
        }
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => bytes,
        _ => {
            let _ = std::fs::remove_file(&path);
            return fail("capture-failed", "The browser capture was not a valid PNG.");
        }
    };
    let image = match image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) {
        Ok(image) => image,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return fail(
                "capture-failed",
                &format!("The browser capture could not be decoded: {error}"),
            );
        }
    };

    BrowserCaptureResult {
        version: 1,
        completion_state: "completed",
        reason: None,
        local_artifact_path: Some(path),
        filename: Some(filename),
        mime_type: Some("image/png"),
        size: Some(metadata.len()),
        sha256: Some(hex::encode(Sha256::digest(&bytes))),
        width: Some(image.width()),
        height: Some(image.height()),
        actor: actor.to_string(),
        correlation_id,
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn capture_visible_workspace_region(
    _app: &tauri::AppHandle,
    _runtime: &WorkspaceBrowserRuntime,
    actor: &str,
    correlation_id: Option<String>,
) -> BrowserCaptureResult {
    BrowserCaptureResult::failure(
        "unsupported",
        "Visible-region browser capture is available in the macOS ASV Buzz build.",
        actor.to_string(),
        correlation_id,
    )
}
