use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static PROVIDER_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Register this app instance's provider root before runtime discovery.
pub(crate) fn register_provider_root(root: PathBuf) {
    let _ = PROVIDER_ROOT.set(root);
}
/// Resolve an executable from the isolated provider installation tree.
///
/// The allowlist prevents an arbitrary runtime command from being redirected
/// through app-owned provider storage.
pub(crate) fn provider_command_path(command: &str) -> Option<PathBuf> {
    let runtime = match command {
        "agy" => "antigravity",
        "gemini" => "gemini",
        _ => return None,
    };
    let candidate = PROVIDER_ROOT
        .get()?
        .join(runtime)
        .join("bin")
        .join(format!("{command}{}", std::env::consts::EXE_SUFFIX));
    executable_file(&candidate).then_some(candidate)
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
