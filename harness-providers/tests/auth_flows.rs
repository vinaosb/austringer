//! Static-key credential sources, header styles, and secret storage. The
//! scripted OAuth device flow lives in `auth_device_flow.rs`.

use harness_providers::auth::{
    AuthHeaderStyle, AuthKind, CredentialSource, InMemorySecretStore, SecretStore, SecretString,
    StaticApiKey,
};

fn api_key_source() -> StaticApiKey {
    StaticApiKey::x_api_key("sk-test-123")
}

#[tokio::test]
async fn static_api_key_yields_provider_header_and_api_kind() {
    let token = api_key_source()
        .credentials()
        .await
        .expect("static key cannot fail");
    assert_eq!(token.kind, AuthKind::ApiKey);
    assert_eq!(token.style, AuthHeaderStyle::XApiKey);
    assert_eq!(token.value.reveal(), "sk-test-123");
    assert!(token.expires_at.is_none(), "api keys do not expire");
    assert_eq!(api_key_source().kind(), AuthKind::ApiKey);
}

#[tokio::test]
async fn bearer_and_google_styles_format_headers_correctly() {
    let bearer = StaticApiKey::bearer("tok-b")
        .credentials()
        .await
        .expect("static");
    let google = StaticApiKey::google_api_key("g-key")
        .credentials()
        .await
        .expect("static");
    let (name_b, value_b) = bearer
        .style
        .header(&bearer.value)
        .expect("bearer attaches a header");
    assert_eq!(name_b, "authorization");
    assert_eq!(value_b, "Bearer tok-b");
    let (name_g, value_g) = google
        .style
        .header(&google.value)
        .expect("google attaches a header");
    assert_eq!(name_g, "x-goog-api-key");
    assert_eq!(value_g, "g-key");
}

#[tokio::test]
async fn secret_string_redacts_in_debug() {
    let secret = SecretString::new("super-secret-value");
    let rendered = format!("{secret:?}");
    assert!(rendered.contains("[redacted]"), "got: {rendered}");
    assert!(!rendered.contains("super-secret-value"));
}

#[test]
fn in_memory_secret_store_round_trips_and_deletes() {
    let store = InMemorySecretStore::new();
    store
        .save("anthropic/refresh", &SecretString::new("r1"))
        .expect("in-memory save");
    let loaded = store.load("anthropic/refresh").expect("load");
    assert_eq!(loaded.map(|s| s.reveal().to_owned()), Some("r1".to_owned()));
    store.delete("anthropic/refresh").expect("delete");
    assert!(store.load("anthropic/refresh").expect("load").is_none());
}
