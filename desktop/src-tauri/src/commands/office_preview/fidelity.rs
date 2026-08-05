#[cfg(target_os = "macos")]
mod macos {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    use base64::Engine;
    use regex::{Captures, Regex};

    use super::super::model::OfficeFidelity;

    const MAX_HTML_BYTES: usize = 6 * 1024 * 1024;
    const MAX_ASSET_BYTES: u64 = 2 * 1024 * 1024;
    const MAX_INLINE_ASSETS: usize = 32;

    fn preview_html(root: &Path) -> Option<PathBuf> {
        let direct = root.join("Preview.html");
        if direct.is_file() {
            return Some(direct);
        }
        let entries = std::fs::read_dir(root).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = preview_html(&path) {
                    return Some(found);
                }
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("Preview.html"))
            {
                return Some(path);
            }
        }
        None
    }

    fn asset_mime(path: &Path) -> Option<&'static str> {
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("png") => Some("image/png"),
            Some("jpg" | "jpeg") => Some("image/jpeg"),
            Some("gif") => Some("image/gif"),
            Some("webp") => Some("image/webp"),
            _ => None,
        }
    }

    fn inline_src_attributes(html: &str, preview_dir: &Path) -> Result<String, String> {
        let regex = Regex::new(r#"(?is)\ssrc\s*=\s*"([^"]*)""#)
            .map_err(|error| format!("failed to prepare Quick Look sanitizer: {error}"))?;
        let root = preview_dir
            .canonicalize()
            .map_err(|error| format!("failed to validate Quick Look output: {error}"))?;
        let mut inlined = 0_usize;
        Ok(regex
            .replace_all(html, |captures: &Captures<'_>| {
                let source = captures.get(1).map(|value| value.as_str()).unwrap_or("");
                if source.starts_with("data:image/") {
                    return captures[0].to_string();
                }
                if source.contains("://")
                    || source.starts_with("//")
                    || source.starts_with("javascript:")
                    || inlined >= MAX_INLINE_ASSETS
                {
                    return String::new();
                }
                let candidate = preview_dir.join(source);
                let Ok(canonical) = candidate.canonicalize() else {
                    return String::new();
                };
                if !canonical.starts_with(&root) {
                    return String::new();
                }
                let Ok(metadata) = canonical.metadata() else {
                    return String::new();
                };
                let Some(mime) = asset_mime(&canonical) else {
                    return String::new();
                };
                if !metadata.is_file() || metadata.len() > MAX_ASSET_BYTES {
                    return String::new();
                }
                let Ok(bytes) = std::fs::read(canonical) else {
                    return String::new();
                };
                inlined += 1;
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                format!(r#" src="data:{mime};base64,{encoded}""#)
            })
            .into_owned())
    }

    fn sanitize_quick_look_html(html: &str, preview_dir: &Path) -> Result<String, String> {
        let blocked_elements = Regex::new(
            r"(?is)<\s*(script|iframe|object|embed)\b[^>]*>.*?<\s*/\s*(script|iframe|object|embed)\s*>|<\s*(script|iframe|object|embed|link|base)\b[^>]*/?\s*>",
        )
        .map_err(|error| format!("failed to prepare Quick Look sanitizer: {error}"))?;
        let event_attributes =
            Regex::new(r#"(?is)\s+on[a-z0-9_-]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)"#)
                .map_err(|error| format!("failed to prepare Quick Look sanitizer: {error}"))?;
        let href_attributes = Regex::new(r#"(?is)\shref\s*=\s*("[^"]*"|'[^']*')"#)
            .map_err(|error| format!("failed to prepare Quick Look sanitizer: {error}"))?;
        let meta_refresh =
            Regex::new(r#"(?is)<meta\b[^>]*http-equiv\s*=\s*["']?refresh["']?[^>]*>"#)
                .map_err(|error| format!("failed to prepare Quick Look sanitizer: {error}"))?;
        let remote_css = Regex::new(r#"(?is)url\s*\(\s*["']?(https?:|//|file:)[^)]*\)"#)
            .map_err(|error| format!("failed to prepare Quick Look sanitizer: {error}"))?;

        let sanitized = blocked_elements.replace_all(html, "");
        let sanitized = meta_refresh.replace_all(&sanitized, "");
        let sanitized = event_attributes.replace_all(&sanitized, "");
        let sanitized = href_attributes.replace_all(&sanitized, "");
        let sanitized = remote_css.replace_all(&sanitized, "none");
        let sanitized = inline_src_attributes(&sanitized, preview_dir)?;
        let policy = concat!(
            r#"<meta http-equiv="Content-Security-Policy" content=""#,
            "default-src 'none'; img-src data:; style-src 'unsafe-inline'; ",
            r#"font-src data:; media-src 'none'; connect-src 'none'; form-action 'none'">"#
        );
        let output = if let Some(index) = sanitized.to_ascii_lowercase().find("<head>") {
            let insertion = index + "<head>".len();
            format!(
                "{}{}{}",
                &sanitized[..insertion],
                policy,
                &sanitized[insertion..]
            )
        } else {
            format!("{policy}{sanitized}")
        };
        if output.len() > MAX_HTML_BYTES {
            return Err("Quick Look fidelity output exceeded the bounded HTML limit".to_string());
        }
        Ok(output)
    }

    pub(super) fn generate(bytes: &[u8], filename: &str) -> Result<OfficeFidelity, String> {
        let temp = tempfile::tempdir()
            .map_err(|error| format!("failed to create Quick Look workspace: {error}"))?;
        let source_name = Path::new(filename)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("preview.docx");
        let source = temp.path().join(source_name);
        std::fs::write(&source, bytes)
            .map_err(|error| format!("failed to prepare Quick Look preview: {error}"))?;
        let output = temp.path().join("output");
        std::fs::create_dir(&output)
            .map_err(|error| format!("failed to prepare Quick Look output: {error}"))?;
        let status = Command::new("/usr/bin/qlmanage")
            .args(["-p", "-o"])
            .arg(&output)
            .arg(&source)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("Quick Look is unavailable: {error}"))?;
        if !status.success() {
            return Err("Quick Look could not render this Office package".to_string());
        }
        let html_path = preview_html(&output)
            .ok_or_else(|| "Quick Look did not produce an HTML preview".to_string())?;
        let raw = std::fs::read_to_string(&html_path)
            .map_err(|error| format!("failed to read Quick Look preview: {error}"))?;
        let preview_dir = html_path.parent().unwrap_or(&output);
        let html = sanitize_quick_look_html(&raw, preview_dir)?;
        Ok(OfficeFidelity {
            renderer: "macos-quick-look",
            html,
            truncated: false,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::sanitize_quick_look_html;

        #[test]
        fn removes_active_and_remote_quick_look_content() {
            let temp = tempfile::tempdir().unwrap();
            let html = r#"<html><head></head><body onload="steal()"><script>alert(1)</script><a href="https://example.com">x</a><img src="https://example.com/a.png"></body></html>"#;
            let output = sanitize_quick_look_html(html, temp.path()).unwrap();
            assert!(!output.contains("<script"));
            assert!(!output.contains("onload"));
            assert!(!output.contains("https://example.com"));
            assert!(output.contains("Content-Security-Policy"));
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn generate(
    bytes: &[u8],
    filename: &str,
) -> Result<super::model::OfficeFidelity, String> {
    macos::generate(bytes, filename)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn generate(
    _bytes: &[u8],
    _filename: &str,
) -> Result<super::model::OfficeFidelity, String> {
    Err("native Office fidelity is available on macOS only".to_string())
}
