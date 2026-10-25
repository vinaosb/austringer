//! OS-keyring-backed [`SecretStore`] (optional `keyring` feature).
//!
//! OAuth refresh tokens persist in the platform credential store — Windows
//! Credential Manager, macOS Keychain, Linux secret service — under a
//! austringer-owned service name. Off by default: embedders opt into OS
//! persistence; the default remains the process-local in-memory store.

use super::{AuthError, SecretStore, SecretString};

/// Credential entries under one service name; the store `key` becomes the
/// keyring account/user field.
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    /// Uses the default service name `austringer-providers`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: "austringer-providers".to_owned(),
        }
    }

    /// Custom service name (multi-profile setups).
    #[must_use]
    pub fn with_service(service: &str) -> Self {
        Self {
            service: service.to_owned(),
        }
    }

    fn entry(&self, key: &str) -> Result<keyring::Entry, AuthError> {
        keyring::Entry::new(&self.service, key)
            .map_err(|err| AuthError::Storage(format!("keyring entry: {err}")))
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringStore {
    fn save(&self, key: &str, value: &SecretString) -> Result<(), AuthError> {
        self.entry(key)?
            .set_password(value.reveal())
            .map_err(|err| AuthError::Storage(format!("keyring save: {err}")))
    }

    fn load(&self, key: &str) -> Result<Option<SecretString>, AuthError> {
        match self.entry(key)?.get_password() {
            Ok(secret) => Ok(Some(SecretString::new(secret))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(AuthError::Storage(format!("keyring load: {err}"))),
        }
    }

    fn delete(&self, key: &str) -> Result<(), AuthError> {
        match self.entry(key)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(AuthError::Storage(format!("keyring delete: {err}"))),
        }
    }
}
