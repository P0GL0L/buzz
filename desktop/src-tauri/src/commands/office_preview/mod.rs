mod fidelity;
mod model;
mod parser;

use std::path::PathBuf;

use tauri::State;

use self::model::{OfficeFidelity, OfficeFormat, OfficePreview};
use crate::app_state::AppState;
use crate::commands::media_download::{fetch_blob_bytes_with_cap, validate_download_url};
use crate::commands::workspace_browser::read_workspace_browser_artifact;
use crate::relay::relay_api_base_url_with_override;

const MAX_OFFICE_DOWNLOAD_BYTES: u64 = 20 * 1024 * 1024;

fn validate_declared_mime(format: OfficeFormat, mime: Option<&str>) -> Result<(), String> {
    let Some(mime) = mime.map(str::trim).filter(|mime| !mime.is_empty()) else {
        return Ok(());
    };
    let mime = mime.split(';').next().unwrap_or(mime).trim();
    if mime == format.expected_mime()
        || mime == "application/octet-stream"
        || mime == "application/zip"
    {
        Ok(())
    } else {
        Err(format!(
            "format mismatch: filename is {format:?}, but attachment MIME is {mime}"
        ))
    }
}

fn build_preview(
    bytes: Vec<u8>,
    filename: String,
    format: OfficeFormat,
    include_fidelity: bool,
) -> Result<OfficePreview, String> {
    let parsed = parser::parse_office(&bytes, format)?;
    let mut warnings = parsed.warnings;
    let fidelity: Option<OfficeFidelity> = if include_fidelity {
        match fidelity::generate(&bytes, &filename) {
            Ok(preview) => Some(preview),
            Err(error) => {
                warnings.push(error);
                None
            }
        }
    } else {
        None
    };
    Ok(OfficePreview {
        version: 1,
        format: parsed.format,
        document: parsed.document,
        truncated: parsed.truncated,
        warnings,
        fidelity,
    })
}

/// Fetch and safely preview a relay-hosted OOXML attachment.
///
/// The command preserves the relay origin guard, rejects active/encrypted or
/// oversized packages, extracts bounded semantic structure, and optionally
/// adds a sanitized macOS Quick Look representation.
#[tauri::command]
pub async fn preview_office_artifact(
    url: String,
    local_path: Option<PathBuf>,
    filename: String,
    mime: Option<String>,
    include_fidelity: Option<bool>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<OfficePreview, String> {
    let format = OfficeFormat::from_filename(&filename)?;
    validate_declared_mime(format, mime.as_deref())?;
    let bytes = if let Some(path) = local_path {
        read_workspace_browser_artifact(&app, &path, MAX_OFFICE_DOWNLOAD_BYTES)?
    } else {
        let relay_base = relay_api_base_url_with_override(&state);
        validate_download_url(&url, &relay_base)?;
        fetch_blob_bytes_with_cap(&url, &state, MAX_OFFICE_DOWNLOAD_BYTES).await?
    };
    tokio::task::spawn_blocking(move || {
        build_preview(
            bytes,
            filename,
            format,
            include_fidelity.unwrap_or(cfg!(target_os = "macos")),
        )
    })
    .await
    .map_err(|error| format!("Office preview worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::{validate_declared_mime, OfficeFormat};

    #[test]
    fn rejects_declared_office_mime_mismatch() {
        assert!(validate_declared_mime(
            OfficeFormat::Docx,
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        )
        .unwrap_err()
        .contains("format mismatch"));
        assert!(
            validate_declared_mime(OfficeFormat::Docx, Some("application/octet-stream")).is_ok()
        );
    }
}
