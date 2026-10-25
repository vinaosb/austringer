//! Device-flow credential tests: fully scripted OAuth 2.0 endpoints via the
//! in-memory transport (zero live OAuth), PKCE shape, refresh lifecycle.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use austringer_providers::auth::{
    AccessToken, AuthError, AuthHeaderStyle, AuthKind, CredentialSource, DeviceFlowConfig,
    InMemorySecretStore, OAuthDeviceFlow, SecretStore, SecretString,
};
use austringer_providers::transport::{
    Body, Transport, TransportFailure, TransportRequest, TransportResponse,
};
use serde_json::{Value, json};
/// Scripted transport for OAuth endpoints: pops queued JSON replies per URL.
struct OauthTransport {
    replies: std::sync::Mutex<std::collections::VecDeque<(String, Result<Value, u16>)>>,
    requests: std::sync::Mutex<Vec<TransportRequest>>,
}

impl OauthTransport {
    fn new(replies: Vec<(&str, Result<Value, u16>)>) -> Self {
        Self {
            replies: std::sync::Mutex::new(
                replies
                    .into_iter()
                    .map(|(url, r)| (url.to_owned(), r))
                    .collect(),
            ),
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn request_count(&self) -> usize {
        self.requests.lock().map(|r| r.len()).unwrap_or(0)
    }

    fn bodies(&self) -> Vec<String> {
        self.requests
            .lock()
            .map(|reqs| {
                reqs.iter()
                    .map(|r| String::from_utf8_lossy(&r.body).to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[async_trait]
impl Transport for OauthTransport {
    async fn send(&self, request: TransportRequest) -> Result<TransportResponse, TransportFailure> {
        self.requests
            .lock()
            .map(|mut reqs| reqs.push(request.clone()))
            .ok();
        let next = self.replies.lock().ok().and_then(|mut q| q.pop_front());
        match next {
            Some((url, Ok(body))) if request.url.contains(&url) => Ok(TransportResponse::ok(
                Body::Full(serde_json::to_vec(&body).unwrap_or_default()),
            )),
            Some((_, Err(status))) => Ok(TransportResponse::with_status(
                status,
                Body::Full(
                    serde_json::to_vec(&json!({ "error": "invalid_grant" })).unwrap_or_default(),
                ),
            )),
            _ => Err(TransportFailure::Connect("script exhausted".to_owned())),
        }
    }
}

fn flow_config() -> DeviceFlowConfig {
    DeviceFlowConfig {
        device_endpoint: "https://auth.test/device".to_owned(),
        token_endpoint: "https://auth.test/token".to_owned(),
        client_id: "client-abc".to_owned(),
        scopes: vec!["model.completion".to_owned()],
        pkce: true,
        verifier_seed: 7,
    }
}

fn device_reply() -> Value {
    json!({
        "device_code": "dev-1",
        "user_code": "WDJB-MJHT",
        "verification_uri": "https://auth.test/activate",
        "expires_in": 1800,
        "interval": 1
    })
}

fn token_reply(access: &str, refresh: Option<&str>) -> Value {
    json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": 3600,
        "refresh_token": refresh,
    })
}

fn expired_token(access: &str) -> AccessToken {
    AccessToken {
        value: SecretString::new(access),
        style: AuthHeaderStyle::Bearer,
        kind: AuthKind::Subscription,
        expires_at: Some(Instant::now() - Duration::from_secs(1)),
    }
}

#[tokio::test]
async fn device_flow_end_to_end_then_cached_credentials_avoid_network() {
    let transport = Arc::new(OauthTransport::new(vec![
        ("device", Ok(device_reply())),
        ("token", Ok(json!({ "error": "authorization_pending" }))),
        ("token", Ok(token_reply("acc-1", Some("ref-1")))),
        ("token", Ok(token_reply("acc-2-refreshed", None))),
    ]));
    let store = Arc::new(InMemorySecretStore::new());
    let flow = OAuthDeviceFlow::new(transport.clone(), flow_config(), store.clone());

    let auth = flow
        .begin_device_authorization()
        .await
        .expect("device authorization");
    assert_eq!(auth.user_code, "WDJB-MJHT");
    assert_eq!(auth.verification_uri, "https://auth.test/activate");
    // PKCE: challenge present on the device request; verifier joins the token poll.
    let device_body = &transport.bodies()[0];
    assert!(
        device_body.contains("code_challenge="),
        "PKCE on device auth"
    );
    assert!(device_body.contains("scope=model.completion"));

    // First poll: provider says pending (the UI re-polls).
    let pending = flow.poll_for_token(&auth).await.unwrap_err();
    assert!(matches!(pending, AuthError::AuthorizationPending));
    // Second poll succeeds: bearer token + refresh token persisted.
    let token = flow.poll_for_token(&auth).await.expect("token granted");
    assert_eq!(token.value.reveal(), "acc-1");
    assert_eq!(token.kind, AuthKind::Subscription);
    assert_eq!(
        store
            .load("oauth/refresh")
            .expect("store")
            .map(|s| s.reveal().to_owned()),
        Some("ref-1".to_owned())
    );
    let (_, value) = token.style.header(&token.value).expect("bearer header");
    assert_eq!(value, "Bearer acc-1");

    // Fresh cache: credentials() must not touch the transport again.
    let before = transport.request_count();
    let cached = flow.credentials().await.expect("cached credentials");
    assert_eq!(cached.value.reveal(), "acc-1");
    assert_eq!(
        transport.request_count(),
        before,
        "no network on fresh cache"
    );
}

#[tokio::test]
async fn expired_cache_refreshes_via_stored_refresh_token() {
    let transport = Arc::new(OauthTransport::new(vec![(
        "token",
        Ok(token_reply("acc-2-refreshed", None)),
    )]));
    let store = Arc::new(InMemorySecretStore::new());
    store
        .save("oauth/refresh", &SecretString::new("ref-1"))
        .expect("seed refresh token");
    let flow = OAuthDeviceFlow::new(transport.clone(), flow_config(), store);
    flow.cache_token(expired_token("acc-old"));

    let token = flow.credentials().await.expect("refreshed");
    assert_eq!(token.value.reveal(), "acc-2-refreshed");
    let body = &transport.bodies()[0];
    assert!(body.contains("grant_type=refresh_token"), "body: {body}");
    assert!(body.contains("refresh_token=ref-1"));
}

#[tokio::test]
async fn invalid_grant_on_refresh_demands_relogin() {
    let transport = Arc::new(OauthTransport::new(vec![("token", Err(400u16))]));
    let store = Arc::new(InMemorySecretStore::new());
    store
        .save("oauth/refresh", &SecretString::new("revoked"))
        .expect("seed");
    let flow = OAuthDeviceFlow::new(transport, flow_config(), store);
    flow.cache_token(expired_token("acc-old"));
    let err = flow.credentials().await.unwrap_err();
    assert!(
        matches!(err, AuthError::ReauthRequired { .. }),
        "got: {err:?}"
    );
}

#[tokio::test]
async fn no_cached_token_and_no_refresh_token_requires_login() {
    let transport = Arc::new(OauthTransport::new(Vec::new()));
    let flow = OAuthDeviceFlow::new(
        transport,
        flow_config(),
        Arc::new(InMemorySecretStore::new()),
    );
    let err = flow.credentials().await.unwrap_err();
    assert!(matches!(err, AuthError::LoginRequired), "got: {err:?}");
}

#[tokio::test]
async fn pkce_verifier_and_challenge_are_spec_shaped() {
    let transport = Arc::new(OauthTransport::new(vec![
        ("device", Ok(device_reply())),
        ("token", Ok(token_reply("acc-1", None))),
    ]));
    let flow = OAuthDeviceFlow::new(
        transport.clone(),
        flow_config(),
        Arc::new(InMemorySecretStore::new()),
    );
    let auth = flow.begin_device_authorization().await.expect("device");
    let _ = flow.poll_for_token(&auth).await;

    let device_body = transport.bodies()[0].clone();
    let token_body = transport.bodies()[1].clone();
    let challenge = device_body
        .split("code_challenge=")
        .nth(1)
        .and_then(|rest| rest.split('&').next())
        .unwrap_or_default()
        .to_owned();
    let verifier = token_body
        .split("code_verifier=")
        .nth(1)
        .and_then(|rest| rest.split('&').next())
        .unwrap_or_default()
        .to_owned();

    assert!(
        (43..=128).contains(&verifier.len()),
        "RFC 7636 verifier length 43..=128, got {}",
        verifier.len()
    );
    assert!(
        verifier.bytes().all(|b| b.is_ascii_alphanumeric()
            || b == b'-'
            || b == b'.'
            || b == b'_'
            || b == b'~'),
        "verifier must be unreserved chars only"
    );
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(verifier.as_bytes());
    let expected = {
        use base64ct::{Base64UrlUnpadded, Encoding};
        Base64UrlUnpadded::encode_string(&digest)
    };
    assert_eq!(challenge, expected, "S256: base64url(sha256(verifier))");
}
