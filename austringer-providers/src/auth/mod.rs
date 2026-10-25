//! Credential sources: static API keys, no-auth (local endpoints), and the
//! OAuth 2.0 device flow with PKCE for subscription-backed logins
//! (Anthropic/OpenAI/Google/xAI device-flow offerings).
//!
//! Adapters never see raw keys: they hold an [`AccessToken`] per request and
//! render it through [`AuthHeaderStyle`]. The billing surface —
//! [`AuthKind::ApiKey`] vs [`AuthKind::Subscription`] — travels on the token
//! and on every adapter (`Provider::auth_kind`), so the cost HUD can
//! distinguish metered API spend from subscription quota later (todo 42)
//! without touching the identity contract.

mod device_flow;
mod http;
mod secret;

#[cfg(feature = "keyring")]
mod keyring;

pub use device_flow::{DeviceAuthorization, DeviceFlowConfig, OAuthDeviceFlow};
#[cfg(feature = "keyring")]
pub use keyring::KeyringStore;
pub use secret::{InMemorySecretStore, SecretStore, SecretString};

use std::time::Instant;

use async_trait::async_trait;
use thiserror::Error;

use crate::transport::TransportFailure;

/// How the credential is billed: metered API spend vs subscription quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthKind {
    /// Pay-per-token API key (cost HUD: dollar accounting).
    ApiKey,
    /// Subscription/OAuth login (cost HUD: quota accounting, not dollars).
    Subscription,
}

/// Where a credential goes on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthHeaderStyle {
    /// `Authorization: Bearer <token>` (OpenAI, xAI, OAuth flows).
    Bearer,
    /// `x-api-key: <key>` (Anthropic API keys).
    XApiKey,
    /// `x-goog-api-key: <key>` (Google AI Studio keys).
    XGoogApiKey,
    /// No header (local OpenAI-compatible endpoints like Ollama).
    None,
}

impl AuthHeaderStyle {
    /// The header name/value pair for a credential, or `None` when the style
    /// attaches nothing.
    #[must_use]
    pub fn header(&self, secret: &SecretString) -> Option<(String, String)> {
        match self {
            Self::Bearer => Some((
                "authorization".to_owned(),
                format!("Bearer {}", secret.reveal()),
            )),
            Self::XApiKey => Some(("x-api-key".to_owned(), secret.reveal().to_owned())),
            Self::XGoogApiKey => Some(("x-goog-api-key".to_owned(), secret.reveal().to_owned())),
            Self::None => None,
        }
    }
}

/// One usable credential: the secret, its wire style, billing kind, and
/// expiry (absent for non-expiring keys).
#[derive(Debug, Clone, PartialEq)]
pub struct AccessToken {
    /// The credential secret.
    pub value: SecretString,
    /// How to attach it to requests.
    pub style: AuthHeaderStyle,
    /// Billing surface for the cost HUD.
    pub kind: AuthKind,
    /// When the token stops being fresh (`None` = does not expire).
    pub expires_at: Option<Instant>,
}

impl AccessToken {
    /// Whether the token is usable at `now` (unexpired).
    #[must_use]
    pub fn is_fresh_at(&self, now: Instant) -> bool {
        self.expires_at.is_none_or(|expiry| now < expiry)
    }
}

/// Supplies credentials on demand. Implementations: [`StaticApiKey`],
/// [`NoCredentials`], [`OAuthDeviceFlow`].
#[async_trait]
pub trait CredentialSource: Send + Sync {
    /// Returns a usable token, refreshing when the implementation supports it.
    async fn credentials(&self) -> Result<AccessToken, AuthError>;

    /// The billing kind this source serves (no await; adapter surface).
    fn kind(&self) -> AuthKind;
}

/// Every credential-path failure, typed.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum AuthError {
    /// Transport failure reaching the auth endpoint.
    #[error("auth transport failure: {0}")]
    Transport(TransportFailure),
    /// The auth endpoint answered a non-OK status.
    #[error("auth http {status}: {error}: {description}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Provider error code (`invalid_grant`, ...).
        error: String,
        /// Provider error description.
        description: String,
    },
    /// Device-flow polling: the user has not approved yet — re-poll later.
    #[error("authorization pending: user has not approved yet")]
    AuthorizationPending,
    /// Device-flow polling: the provider asked to slow down the poll rate.
    #[error("device flow slow_down: back off before re-polling")]
    SlowDown,
    /// The refresh token is gone or revoked — interactive login needed.
    #[error("re-authorization required: {0}")]
    ReauthRequired(String),
    /// No credential available and no refresh path — run a login flow.
    #[error("login required: no cached credential or refresh token")]
    LoginRequired,
    /// The secret store failed.
    #[error("secret store failure: {0}")]
    Storage(String),
    /// The endpoint returned a shape this client cannot parse.
    #[error("unparseable auth response: {0}")]
    InvalidResponse(String),
}

/// A fixed API key: never expires, never refreshes.
#[derive(Debug, Clone)]
pub struct StaticApiKey {
    token: AccessToken,
}

impl StaticApiKey {
    /// An Anthropic-style `x-api-key` credential.
    #[must_use]
    pub fn x_api_key(key: &str) -> Self {
        Self::styled(key, AuthHeaderStyle::XApiKey)
    }

    /// An OpenAI/xAI-style bearer credential.
    #[must_use]
    pub fn bearer(token: &str) -> Self {
        Self::styled(token, AuthHeaderStyle::Bearer)
    }

    /// A Google-style `x-goog-api-key` credential.
    #[must_use]
    pub fn google_api_key(key: &str) -> Self {
        Self::styled(key, AuthHeaderStyle::XGoogApiKey)
    }

    fn styled(secret: &str, style: AuthHeaderStyle) -> Self {
        Self {
            token: AccessToken {
                value: SecretString::new(secret),
                style,
                kind: AuthKind::ApiKey,
                expires_at: None,
            },
        }
    }
}

#[async_trait]
impl CredentialSource for StaticApiKey {
    async fn credentials(&self) -> Result<AccessToken, AuthError> {
        Ok(self.token.clone())
    }

    fn kind(&self) -> AuthKind {
        AuthKind::ApiKey
    }
}

/// The no-credential source for endpoints that need no auth (Ollama, vLLM
/// on localhost).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCredentials;

#[async_trait]
impl CredentialSource for NoCredentials {
    async fn credentials(&self) -> Result<AccessToken, AuthError> {
        Ok(AccessToken {
            value: SecretString::new(String::new()),
            style: AuthHeaderStyle::None,
            kind: AuthKind::ApiKey,
            expires_at: None,
        })
    }

    fn kind(&self) -> AuthKind {
        AuthKind::ApiKey
    }
}
