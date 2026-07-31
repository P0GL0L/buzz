use std::path::Path;

use sha2::{Digest, Sha256};

use super::{
    browser_downloads_root, browser_screenshots_root, ensure_private_dir,
    MAX_WORKSPACE_DOWNLOAD_BYTES,
};

pub(super) fn completed_download_metadata(path: &Path) -> Option<serde_json::Value> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_WORKSPACE_DOWNLOAD_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let mime = infer::get(&bytes)
        .map(|kind| kind.mime_type().to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    Some(serde_json::json!({
        "artifactId": uuid::Uuid::new_v4().to_string(),
        "artifactVersion": 1,
        "artifactSource": "browser-download",
        "mime": mime,
        "size": metadata.len(),
        "sha256": hex::encode(Sha256::digest(&bytes))
    }))
}

pub(crate) fn read_workspace_browser_artifact(
    app: &tauri::AppHandle,
    requested_path: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    let downloads = browser_downloads_root(app)?;
    let screenshots = browser_screenshots_root(app)?;
    ensure_private_dir(&downloads)?;
    ensure_private_dir(&screenshots)?;
    let downloads = downloads
        .canonicalize()
        .map_err(|error| format!("failed to resolve browser downloads: {error}"))?;
    let screenshots = screenshots
        .canonicalize()
        .map_err(|error| format!("failed to resolve browser screenshots: {error}"))?;
    let path = requested_path
        .canonicalize()
        .map_err(|error| format!("browser artifact is unavailable: {error}"))?;
    if !path.starts_with(&downloads) && !path.starts_with(&screenshots) {
        return Err("browser artifact is outside the ASV Buzz workspace".to_string());
    }
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|error| format!("failed to inspect browser artifact: {error}"))?;
    if !metadata.file_type().is_file() {
        return Err("browser artifact is not a regular file".to_string());
    }
    if metadata.len() > max_bytes {
        return Err(format!(
            "browser artifact exceeds the {} MiB preview limit",
            max_bytes / (1024 * 1024)
        ));
    }
    std::fs::read(path).map_err(|error| format!("failed to read browser artifact: {error}"))
}
