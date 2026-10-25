//! OAuth 2.0 device authorization grant (RFC 8628) with PKCE (RFC 7636),
//! driven entirely through the injected [`Transport`] — the UI shows the
//! user code, polls, and the tests script every endpoint response. Refresh
//! tokens live in the [`SecretStore`], never in configuration.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::http::{auth_post, field_str, field_u64};
use super::{
    AccessToken, AuthError, AuthHeaderStyle, AuthKind, CredentialSource, SecretStore, SecretString,
};
use crate::retry::{JitterSource as _, SeededJitter};
use crate::transport::Transport;

/// Endpoints and client identity for one provider's device flow. Client ids
/// are configuration-owned (provider-registered applications), never baked
/// into the austringer.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceFlowConfig {
    /// Device authorization endpoint (RFC 8628 §3.1).
    pub device_endpoint: String,
    /// Token endpoint (device grant + refresh grant).
    pub token_endpoint: String,
    /// The registered OAuth client id.
    pub client_id: String,
    /// Requested scopes.
    pub scopes: Vec<String>,
    /// Send a PKCE S256 challenge with the device request (Google-style
    /// public-client hardening; providers without PKCE set `false`).
    pub pkce: bool,
    /// Seed for verifier generation (deterministic in tests, seeded in
    /// production config).
    pub verifier_seed: u64,
}

/// The pending device authorization shown to the user.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceAuthorization {
    /// The device verification code (opaque to the client).
    pub device_code: String,
    /// The code the user types at the verification page.
    pub user_code: String,
    /// Where the user goes to approve.
    pub verification_uri: String,
    /// When the authorization lapses.
    pub expires_at: Instant,
    /// Recommended poll interval.
    pub interval: Duration,
    /// The PKCE verifier for this authorization (empty when PKCE is off).
    pub(crate) verifier: SecretString,
}

/// One PKCE pair.
struct Pkce {
    verifier: String,
    challenge: String,
}

impl Pkce {
    fn generate(jitter: &mut SeededJitter) -> Self {
        const ALPHABET: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        let mut verifier = String::with_capacity(64);
        for _ in 0..64 {
            let idx = (jitter.next_jitter() * ALPHABET.len() as f64) as usize;
            // next_jitter is [0,1): the clamp guards the (impossible) 1.0.
            let byte = ALPHABET[idx.min(ALPHABET.len() - 1)];
            verifier.push(byte as char);
        }
        let digest = Sha256::digest(verifier.as_bytes());
        use base64ct::{Base64UrlUnpadded, Encoding as _};
        Self {
            challenge: Base64UrlUnpadded::encode_string(&digest),
            verifier,
        }
    }
}

/// The device-flow credential source: cache → refresh → login-required.
pub struct OAuthDeviceFlow {
    transport: std::sync::Arc<dyn Transport>,
    config: DeviceFlowConfig,
    store: std::sync::Arc<dyn SecretStore>,
    state: Mutex<FlowState>,
}

#[derive(Default)]
struct FlowState {
    cached: Option<AccessToken>,
}

const REFRESH_KEY: &str = "oauth/refresh";

impl OAuthDeviceFlow {
    /// Builds the flow over `transport` with refresh persistence in `store`.
    #[must_use]
    pub fn new(
        transport: std::sync::Arc<dyn Transport>,
        config: DeviceFlowConfig,
        store: std::sync::Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            transport,
            config,
            store,
            state: Mutex::new(FlowState::default()),
        }
    }

    /// Seeds the token cache (session restore, test setup). A fresh cached
    /// token short-circuits [`CredentialSource::credentials`].
    pub fn cache_token(&self, token: AccessToken) {
        if let Ok(mut state) = self.state.lock() {
            state.cached = Some(token);
        }
    }

    /// Starts the device authorization: returns what the user must see
    /// (code + URI). With PKCE configured, the S256 challenge rides on this
    /// request and the verifier joins the token poll.
    pub async fn begin_device_authorization(&self) -> Result<DeviceAuthorization, AuthError> {
        let mut jitter = SeededJitter::new(self.config.verifier_seed);
        let pkce = self.config.pkce.then(|| Pkce::generate(&mut jitter));
        let mut form = vec![
            ("client_id", self.config.client_id.clone()),
            (
                "scope",
                self.config
                    .scopes
                    .iter()
                    .cloned()
                    .reduce(|acc, scope| format!("{acc} {scope}"))
                    .unwrap_or_default(),
            ),
        ];
        if let Some(pair) = &pkce {
            form.push(("code_challenge", pair.challenge.clone()));
            form.push(("code_challenge_method", "S256".to_owned()));
        }
        let body = auth_post(&*self.transport, &self.config.device_endpoint, &form).await?;
        let expires_in = field_u64(&body, "expires_in").unwrap_or(1800);
        Ok(DeviceAuthorization {
            device_code: field_str(&body, "device_code")
                .ok_or_else(|| AuthError::InvalidResponse("device_code missing".to_owned()))?
                .to_owned(),
            user_code: field_str(&body, "user_code")
                .ok_or_else(|| AuthError::InvalidResponse("user_code missing".to_owned()))?
                .to_owned(),
            verification_uri: field_str(&body, "verification_uri")
                .ok_or_else(|| AuthError::InvalidResponse("verification_uri missing".to_owned()))?
                .to_owned(),
            expires_at: Instant::now() + Duration::from_secs(expires_in),
            interval: Duration::from_secs(field_u64(&body, "interval").unwrap_or(5)),
            verifier: pkce.map_or_else(
                || SecretString::new(String::new()),
                |pair| SecretString::new(pair.verifier),
            ),
        })
    }

    /// Polls the token endpoint once. `authorization_pending`/`slow_down`
    /// return as typed errors for the caller's poll loop; success caches the
    /// access token, persists the refresh token, and returns the token.
    pub async fn poll_for_token(
        &self,
        auth: &DeviceAuthorization,
    ) -> Result<AccessToken, AuthError> {
        let mut form = vec![
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code".to_owned(),
            ),
            ("device_code", auth.device_code.clone()),
            ("client_id", self.config.client_id.clone()),
        ];
        if !auth.verifier.reveal().is_empty() {
            form.push(("code_verifier", auth.verifier.reveal().to_owned()));
        }
        let body = auth_post(&*self.transport, &self.config.token_endpoint, &form).await?;
        self.adopt_token(body).await
    }

    /// Refreshes via the stored refresh token (rotation-aware: a new
    /// refresh_token in the response replaces the stored one).
    pub async fn refresh(&self) -> Result<AccessToken, AuthError> {
        let Some(stored) = self.store.load(REFRESH_KEY)? else {
            return Err(AuthError::LoginRequired);
        };
        let form = vec![
            ("grant_type", "refresh_token".to_owned()),
            ("refresh_token", stored.reveal().to_owned()),
            ("client_id", self.config.client_id.clone()),
        ];
        let body = auth_post(&*self.transport, &self.config.token_endpoint, &form).await?;
        self.adopt_token(body).await
    }

    async fn adopt_token(&self, body: Value) -> Result<AccessToken, AuthError> {
        let access = field_str(&body, "access_token")
            .ok_or_else(|| AuthError::InvalidResponse("access_token missing".to_owned()))?;
        if let Some(rotation) = field_str(&body, "refresh_token") {
            self.store.save(REFRESH_KEY, &SecretString::new(rotation))?;
        }
        let expires_in = field_u64(&body, "expires_in").unwrap_or(3600);
        let token = AccessToken {
            value: SecretString::new(access),
            style: AuthHeaderStyle::Bearer,
            kind: AuthKind::Subscription,
            expires_at: Some(Instant::now() + Duration::from_secs(expires_in)),
        };
        if let Ok(mut state) = self.state.lock() {
            state.cached = Some(token.clone());
        }
        Ok(token)
    }
}

#[async_trait]
impl CredentialSource for OAuthDeviceFlow {
    async fn credentials(&self) -> Result<AccessToken, AuthError> {
        if let Ok(state) = self.state.lock()
            && let Some(cached) = &state.cached
            && cached.is_fresh_at(Instant::now())
        {
            return Ok(cached.clone());
        }
        match self.store.load(REFRESH_KEY)? {
            Some(_) => self.refresh().await,
            None => Err(AuthError::LoginRequired),
        }
    }

    fn kind(&self) -> AuthKind {
        AuthKind::Subscription
    }
}
