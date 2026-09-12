//! Secrets: a redacted-by-default string and the storage trait for
//! refresh-token persistence.

use std::collections::BTreeMap;
use std::sync::Mutex;

use super::AuthError;

/// A credential string that never leaks through `Debug`/`Display`.
/// `reveal` is the only read path — reserved for header construction.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    /// Wraps a secret.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The raw secret (header construction only; never log this).
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretString([redacted])")
    }
}

/// Persistence for long-lived credentials (OAuth refresh tokens). Production
/// wiring persists to the OS keyring (feature `keyring`); the in-memory
/// impl keeps secrets process-local for tests and ephemeral sessions.
pub trait SecretStore: Send + Sync {
    /// Persists `value` under `key` (upsert).
    fn save(&self, key: &str, value: &SecretString) -> Result<(), AuthError>;

    /// Loads the secret under `key`, if present.
    fn load(&self, key: &str) -> Result<Option<SecretString>, AuthError>;

    /// Removes the secret under `key` (logout / rotation).
    fn delete(&self, key: &str) -> Result<(), AuthError>;
}

/// Process-local [`SecretStore`]: a plain map behind a mutex. Secrets die
/// with the process — the safe default when OS persistence is not opted into.
#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    entries: Mutex<BTreeMap<String, SecretString>>,
}

impl InMemorySecretStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for InMemorySecretStore {
    fn save(&self, key: &str, value: &SecretString) -> Result<(), AuthError> {
        self.entries
            .lock()
            .map_err(|_| AuthError::Storage("store mutex poisoned".to_owned()))?
            .insert(key.to_owned(), value.clone());
        Ok(())
    }

    fn load(&self, key: &str) -> Result<Option<SecretString>, AuthError> {
        let guard = self
            .entries
            .lock()
            .map_err(|_| AuthError::Storage("store mutex poisoned".to_owned()))?;
        Ok(guard.get(key).cloned())
    }

    fn delete(&self, key: &str) -> Result<(), AuthError> {
        self.entries
            .lock()
            .map_err(|_| AuthError::Storage("store mutex poisoned".to_owned()))?
            .remove(key);
        Ok(())
    }
}
