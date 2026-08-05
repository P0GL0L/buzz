/// Service name for the desktop OS keyring. Debug builds default to a distinct
/// service, while standalone worktree launches may request a scoped dev service.
///
/// The compiled value is required for packaged debug applications: launching
/// `ASV Buzz.app` from Finder does not inherit the shell environment that built
/// it. A valid runtime override still wins for `tauri dev` and test instances.
fn dev_keyring_service(runtime: Option<String>, compiled: Option<&str>) -> String {
    runtime
        .into_iter()
        .chain(compiled.map(str::to_owned))
        .find(|service| service.starts_with("buzz-desktop-dev."))
        // The canonical development checkout owns the `.main` scope. This
        // fallback is intentionally scoped rather than the historical
        // `buzz-desktop-dev` service so a debug bundle rebuilt and launched
        // directly from Finder sees the same identity as `just
        // desktop-standalone`, even when it no longer inherits the build
        // shell's environment.
        .unwrap_or_else(|| "buzz-desktop-dev.main".to_string())
}

pub(crate) fn keyring_service() -> &'static str {
    if cfg!(debug_assertions) {
        static DEV_SERVICE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        DEV_SERVICE
            .get_or_init(|| {
                dev_keyring_service(
                    std::env::var("BUZZ_DEV_KEYRING_SERVICE").ok(),
                    option_env!("BUZZ_DEV_KEYRING_SERVICE"),
                )
            })
            .as_str()
    } else {
        "buzz-desktop"
    }
}

pub(super) fn migration_marker_name(service: &str, default_name: &str) -> String {
    if service == "buzz-desktop" || service == "buzz-desktop-dev" {
        default_name.to_string()
    } else {
        format!("identity.{service}.migrated")
    }
}

#[cfg(test)]
mod tests {
    use super::{dev_keyring_service, migration_marker_name};

    #[test]
    fn standalone_scope_must_remain_under_dev_service() {
        assert_eq!(
            dev_keyring_service(Some("buzz-desktop-dev.example".to_string()), None),
            "buzz-desktop-dev.example"
        );
        assert_eq!(
            dev_keyring_service(Some("buzz-desktop".to_string()), None),
            "buzz-desktop-dev.main"
        );
    }

    #[test]
    fn packaged_debug_app_uses_compiled_scope_without_runtime_environment() {
        assert_eq!(
            dev_keyring_service(None, Some("buzz-desktop-dev.main")),
            "buzz-desktop-dev.main"
        );
    }

    #[test]
    fn directly_built_debug_app_uses_canonical_main_scope() {
        assert_eq!(dev_keyring_service(None, None), "buzz-desktop-dev.main");
    }

    #[test]
    fn valid_runtime_scope_wins_over_compiled_scope() {
        assert_eq!(
            dev_keyring_service(
                Some("buzz-desktop-dev.test".to_string()),
                Some("buzz-desktop-dev.main")
            ),
            "buzz-desktop-dev.test"
        );
    }

    #[test]
    fn invalid_runtime_scope_falls_back_to_compiled_scope() {
        assert_eq!(
            dev_keyring_service(
                Some("buzz-desktop".to_string()),
                Some("buzz-desktop-dev.main")
            ),
            "buzz-desktop-dev.main"
        );
    }

    #[test]
    fn standalone_scope_uses_its_own_migration_marker() {
        assert_eq!(
            migration_marker_name("buzz-desktop", "identity.migrated"),
            "identity.migrated"
        );
        assert_eq!(
            migration_marker_name("buzz-desktop-dev", "identity.migrated"),
            "identity.migrated"
        );
        assert_eq!(
            migration_marker_name("buzz-desktop-dev.example", "identity.migrated"),
            "identity.buzz-desktop-dev.example.migrated"
        );
    }
}
