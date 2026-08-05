/// Validate a configured relay URL without collapsing its host authority.
///
/// Runtime keys intentionally canonicalize equivalent loopback spellings for
/// process bookkeeping. Relay communities, however, are selected from the
/// configured authority, so network operations must preserve `localhost`
/// versus `127.0.0.1`.
pub(crate) fn configured_relay_connection_url(raw: &str) -> Result<String, String> {
    buzz_core_pkg::relay::normalize_relay_url(raw).map_err(|error| error.to_string())?;
    let mut url = url::Url::parse(raw.trim()).map_err(|error| error.to_string())?;
    if url.path() == "/" {
        url.set_path("");
    }
    Ok(url.to_string().trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::configured_relay_connection_url;
    use crate::managed_agents::ManagedAgentRuntimeKey;

    #[test]
    fn configured_connection_preserves_loopback_tenant_authority() {
        assert_eq!(
            configured_relay_connection_url("ws://localhost:3000/").unwrap(),
            "ws://localhost:3000"
        );
        assert_eq!(
            configured_relay_connection_url("ws://127.0.0.1:3000/").unwrap(),
            "ws://127.0.0.1:3000"
        );

        let localhost =
            ManagedAgentRuntimeKey::new("aa".repeat(32), "ws://localhost:3000").unwrap();
        let loopback = ManagedAgentRuntimeKey::new("aa".repeat(32), "ws://127.0.0.1:3000").unwrap();
        assert_eq!(localhost, loopback);
    }
}
