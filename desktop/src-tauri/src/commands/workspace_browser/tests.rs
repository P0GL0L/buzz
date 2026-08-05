use super::{redacted_url, validate_browser_url, BrowserBounds};

#[test]
fn browser_urls_are_http_only_and_sensitive_queries_are_redacted() {
    assert!(validate_browser_url("file:///tmp/private").is_err());
    assert!(validate_browser_url("javascript:alert(1)").is_err());
    let url = validate_browser_url(
        "https://user:pass@example.com/path?q=public&access_token=secret#fragment",
    )
    .unwrap();
    let safe = redacted_url(&url);
    assert!(safe.contains("q=public"));
    assert!(safe.contains("access_token=%5Bredacted%5D"));
    assert!(!safe.contains("pass"));
    assert!(!safe.contains("fragment"));
}

#[test]
fn browser_bounds_reject_hidden_or_unbounded_children() {
    assert!(BrowserBounds {
        x: 10.0,
        y: 10.0,
        width: 800.0,
        height: 600.0,
    }
    .validate()
    .is_ok());
    assert!(BrowserBounds {
        x: -1.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    }
    .validate()
    .is_err());
}
