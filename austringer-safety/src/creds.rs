//! Credential hygiene (plan todo 26, Amp pattern): per-command short-lived
//! tokens via credential-helper indirection. No long-lived provider
//! tokens in the agent environment; nothing persisted to disk — this
//! helper is memory-only by construction.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// A short-lived credential. `Debug` is redacted so tokens never leak
/// into logs.
#[derive(Clone)]
pub struct ShortLivedToken {
    /// The secret value (never rendered).
    pub value: String,
    /// Expiry, unix seconds.
    pub expires_at_unix: u64,
}

impl fmt::Debug for ShortLivedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ShortLivedToken(<redacted>, expires {})",
            self.expires_at_unix
        )
    }
}

impl ShortLivedToken {
    /// True while `now` is before expiry.
    #[must_use]
    pub fn is_valid(&self, now: u64) -> bool {
        now < self.expires_at_unix
    }
}

/// Credential-helper errors.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CredentialError {
    /// The minted credential has expired — mint a fresh one.
    #[error("credential expired")]
    Expired,
    /// No credential was ever minted for this command — fail closed.
    #[error("no credential minted for this command")]
    NoCredential,
}

/// The credential-helper seam the exec layer consumes: commands ask for
/// their per-invocation token instead of reading a long-lived env var.
pub trait CredentialHelper {
    /// Fetch the current short-lived credential for one command.
    ///
    /// # Errors
    /// [`CredentialError::Expired`] / [`CredentialError::NoCredential`].
    fn credential_for(&self, command: &str) -> Result<ShortLivedToken, CredentialError>;
}

/// In-memory helper: mints per-command tokens with explicit TTLs and
/// never writes anything down. Deterministic clock injection for tests.
#[derive(Default)]
pub struct InMemoryCredentialHelper {
    vault: std::collections::HashMap<String, ShortLivedToken>,
    clock: Option<fn() -> u64>,
}

impl InMemoryCredentialHelper {
    /// An empty helper on the real clock.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty helper on an injected clock (tests).
    #[must_use]
    pub fn with_clock(clock: fn() -> u64) -> Self {
        Self {
            vault: std::collections::HashMap::new(),
            clock: Some(clock),
        }
    }

    /// Mint a token binding `command` to `ttl_secs`.
    #[must_use]
    pub fn mint_for_command(&mut self, command: &str, ttl_secs: u64) -> Option<ShortLivedToken> {
        let value = random_token_value().ok()?;
        let now = self.clock.map_or_else(unix_now, |c| c());
        let token = ShortLivedToken {
            value: value.clone(),
            expires_at_unix: now + ttl_secs,
        };
        self.vault.insert(command.to_string(), token.clone());
        Some(token)
    }

    fn now(&self) -> u64 {
        self.clock.map_or_else(unix_now, |c| c())
    }
}

impl CredentialHelper for InMemoryCredentialHelper {
    fn credential_for(&self, command: &str) -> Result<ShortLivedToken, CredentialError> {
        let token = self
            .vault
            .get(command)
            .ok_or(CredentialError::NoCredential)?;
        if !token.is_valid(self.now()) {
            return Err(CredentialError::Expired);
        }
        Ok(ShortLivedToken {
            value: token.value.clone(),
            expires_at_unix: token.expires_at_unix,
        })
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn random_token_value() -> Result<String, CredentialError> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|_| CredentialError::NoCredential)?;
    Ok(hex(&bytes))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}
