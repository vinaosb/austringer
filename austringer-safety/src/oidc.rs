//! OIDC workload-identity minting (plan todo 26, Cursor pattern): a
//! service reachable only from inside the sandbox mints short-lived
//! signed JWTs so long-lived credentials never enter agent processes.
//!
//! The signer is real crypto (HMAC-SHA256, RFC 4231, verified against
//! the RFC test vectors); the SOCKET transport is a typed seam — the
//! daemon binds `WorkloadIdentityService` to a sandbox-reachable socket
//! in a later todo.

use base64ct::{Base64UrlUnpadded, Encoding};
use hmac::{Hmac, Mac};
use serde_json::{Map, Value};
use sha2::Sha256;

/// JWT compact-serialization HMAC-SHA256 signer.
type HmacSha256 = Hmac<Sha256>;

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let Ok(mut mac) = HmacSha256::new_from_slice(key) else {
        // unreachable for HMAC (every key length is accepted); an empty
        // signature fails every verification, i.e. fails closed
        return Vec::new();
    };
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Subject-template OIDC federation claims (Warp federate pattern):
/// principal / email / teams / environment / agent_name / run_id.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SubjectTemplateClaims {
    /// Stable principal identifier.
    pub principal: String,
    /// Human contact, when the IdP provides one.
    pub email: Option<String>,
    /// Team memberships.
    pub teams: Vec<String>,
    /// Deployment environment name.
    pub environment: String,
    /// Agent display name.
    pub agent_name: String,
    /// Correlating run id.
    pub run_id: String,
}

impl SubjectTemplateClaims {
    /// Render into JWT claim form (plus `iss`/`sub` mapping).
    #[must_use]
    pub fn claims(&self, audience: &str, issued_at: u64, expires_at: u64) -> Map<String, Value> {
        let mut claims = Map::new();
        claims.insert("iss".into(), "austringer-workload-identity".into());
        claims.insert("sub".into(), self.principal.clone().into());
        claims.insert("principal".into(), self.principal.clone().into());
        if let Some(email) = &self.email {
            claims.insert("email".into(), email.clone().into());
        }
        claims.insert(
            "teams".into(),
            Value::Array(self.teams.iter().map(|t| t.clone().into()).collect()),
        );
        claims.insert("environment".into(), self.environment.clone().into());
        claims.insert("agent_name".into(), self.agent_name.clone().into());
        claims.insert("run_id".into(), self.run_id.clone().into());
        claims.insert("aud".into(), audience.into());
        claims.insert("iat".into(), issued_at.into());
        claims.insert("exp".into(), expires_at.into());
        claims
    }
}

/// Signing backend seam.
pub trait OidcSigner: Send + Sync {
    /// The `alg` header value this signer produces.
    fn alg_name(&self) -> &'static str;
    /// Sign the JWT signing input (`header.payload`).
    fn sign(&self, signing_input: &str) -> Vec<u8>;
    /// Verify a signature over the signing input.
    fn verify(&self, signing_input: &str, signature: &[u8]) -> bool;
}

/// Real HMAC-SHA256 signer (keys from OS entropy in production).
#[derive(Clone)]
pub struct HmacSigner {
    key: Vec<u8>,
}

impl std::fmt::Debug for HmacSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // key material is never rendered
        write!(f, "HmacSigner(<redacted {}-byte key>)", self.key.len())
    }
}

impl HmacSigner {
    /// Build with an explicit key (tests, RFC vectors).
    #[must_use]
    pub fn with_key(key: Vec<u8>) -> Self {
        Self { key }
    }

    /// Build with a 32-byte key from OS entropy.
    /// # Errors
    /// [`TokenError::Entropy`] when the OS entropy source fails.
    pub fn from_entropy() -> Result<Self, TokenError> {
        let mut key = vec![0u8; 32];
        getrandom::fill(&mut key).map_err(|_| TokenError::Entropy)?;
        Ok(Self { key })
    }
}

impl OidcSigner for HmacSigner {
    fn alg_name(&self) -> &'static str {
        "HS256"
    }

    fn sign(&self, signing_input: &str) -> Vec<u8> {
        hmac(&self.key, signing_input.as_bytes())
    }

    fn verify(&self, signing_input: &str, signature: &[u8]) -> bool {
        let mut mac = match HmacSha256::new_from_slice(&self.key) {
            Ok(mac) => mac,
            // unreachable for HMAC (every key length is accepted)
            Err(_) => return false,
        };
        mac.update(signing_input.as_bytes());
        mac.verify_slice(signature).is_ok()
    }
}

/// Deterministic mock signer for offline tests.
#[derive(Debug, Clone)]
pub struct MockSigner {
    key: Vec<u8>,
}

impl MockSigner {
    /// Mock signer keyed by `key`.
    #[must_use]
    pub fn new(key: Vec<u8>) -> Self {
        Self { key }
    }
}

impl OidcSigner for MockSigner {
    fn alg_name(&self) -> &'static str {
        "HS256"
    }

    fn sign(&self, signing_input: &str) -> Vec<u8> {
        hmac(&self.key, signing_input.as_bytes())
    }

    fn verify(&self, signing_input: &str, signature: &[u8]) -> bool {
        self.sign(signing_input) == signature
    }
}

/// Token lifecycle errors.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TokenError {
    /// `exp` is in the past relative to the verification clock.
    #[error("token expired")]
    Expired,
    /// Signature mismatch or malformed token.
    #[error("invalid token: {detail}")]
    Invalid {
        /// Why the token was rejected.
        detail: String,
    },
    /// OS entropy unavailable for key generation.
    #[error("entropy source unavailable")]
    Entropy,
}

/// Minted token plus its expiry (unix seconds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenResponse {
    /// The compact JWT.
    pub jwt: String,
    /// Expiry (unix seconds).
    pub expires_at_unix: u64,
}

/// Wire request for the socket seam.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenRequest {
    /// Intended audience (`aud`) claim.
    pub audience: String,
    /// Requested lifetime in seconds (capped by the service).
    pub ttl_secs: u64,
}

/// Cap on minted lifetimes: workload tokens are SHORT-lived by design.
pub const MAX_TTL_SECS: u64 = 3600;

/// The workload-identity service: mints short-lived signed JWTs from the
/// session's subject template. The daemon wires this to a socket that is
/// reachable only inside the sandbox (later todo).
pub struct WorkloadIdentityService<S: OidcSigner> {
    /// Claims template for this run.
    pub claims: SubjectTemplateClaims,
    /// Signer (HMAC in production, mock in tests).
    pub signer: S,
    clock: fn() -> u64,
}

impl<S: OidcSigner> WorkloadIdentityService<S> {
    /// Build the service with an injected clock (deterministic tests).
    #[must_use]
    pub fn new(claims: SubjectTemplateClaims, signer: S, clock: fn() -> u64) -> Self {
        Self {
            claims,
            signer,
            clock,
        }
    }

    /// Mint one short-lived token for `request`.
    ///
    /// # Errors
    /// [`TokenError::Entropy`] never; serialization failures surface as
    /// [`TokenError::Invalid`].
    pub fn mint_for(&self, request: &TokenRequest) -> Result<TokenResponse, TokenError> {
        let now = (self.clock)();
        let ttl = request.ttl_secs.min(MAX_TTL_SECS);
        let expires = now + ttl;
        let header = format!("{{\"alg\":\"{}\",\"typ\":\"JWT\"}}", self.signer.alg_name());
        let claims = self.claims.claims(&request.audience, now, expires);
        let payload = serde_json::to_string(&claims).map_err(|e| TokenError::Invalid {
            detail: e.to_string(),
        })?;
        let signing_input = format!(
            "{}.{}",
            Base64UrlUnpadded::encode_string(header.as_bytes()),
            Base64UrlUnpadded::encode_string(payload.as_bytes())
        );
        let signature = self.signer.sign(&signing_input);
        Ok(TokenResponse {
            jwt: format!(
                "{signing_input}.{}",
                Base64UrlUnpadded::encode_string(&signature)
            ),
            expires_at_unix: expires,
        })
    }
}

/// Verify a minted token (round-trip proof): checks signature, then
/// expiry, then returns the claim set.
///
/// # Errors
/// [`TokenError::Expired`] / [`TokenError::Invalid`].
pub fn verify_jwt(
    token: &str,
    signer: &dyn OidcSigner,
    now: u64,
) -> Result<Map<String, Value>, TokenError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(TokenError::Invalid {
            detail: "not a compact JWT".into(),
        });
    }
    let signature = Base64UrlUnpadded::decode_vec(parts[2]).map_err(|_| TokenError::Invalid {
        detail: "signature encoding".into(),
    })?;
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    if !signer.verify(&signing_input, &signature) {
        return Err(TokenError::Invalid {
            detail: "signature mismatch".into(),
        });
    }
    let payload = Base64UrlUnpadded::decode_vec(parts[1]).map_err(|_| TokenError::Invalid {
        detail: "payload encoding".into(),
    })?;
    let claims: Map<String, Value> =
        serde_json::from_slice(&payload).map_err(|_| TokenError::Invalid {
            detail: "claims not JSON".into(),
        })?;
    let exp = claims
        .get("exp")
        .and_then(Value::as_u64)
        .ok_or_else(|| TokenError::Invalid {
            detail: "missing exp".into(),
        })?;
    if now >= exp {
        return Err(TokenError::Expired);
    }
    Ok(claims)
}
