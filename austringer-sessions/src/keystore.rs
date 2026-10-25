//! Wrapped-key envelopes and optional key persistence (the T10
//! `SecretStore` shape — an OS keyring implementation is drop-in caller
//! wiring; this crate never depends on a keyring provider).

use std::collections::BTreeMap;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension};

use crate::crypto::{KdfParams, SessionKey, WrappedKey};
use crate::error::SessionError;

/// Reads a session's key envelope from its `sessions` row and unwraps
/// the data key. Wrong passphrase → typed decrypt error.
pub(crate) fn unlock_session_key(
    conn: &Connection,
    session_id: &str,
    passphrase: &str,
) -> Result<SessionKey, SessionError> {
    let row = conn
        .query_row(
            "SELECT kdf_salt, kdf_params, wrapped_key FROM sessions WHERE id = ?1",
            [session_id],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(SessionError::sqlite)?
        .ok_or_else(|| SessionError::Corrupt {
            detail: format!("encrypted session {session_id} is missing its key envelope"),
        })?;
    let (salt, params_raw, blob) = row;
    let wrapped = WrappedKey {
        salt,
        params: parse_kdf(&params_raw)?,
        blob,
    };
    SessionKey::unwrap(&wrapped, passphrase, session_id)
}

pub(crate) fn parse_kdf(params_raw: &str) -> Result<KdfParams, SessionError> {
    let params: serde_json::Value =
        serde_json::from_str(params_raw).map_err(|e| SessionError::Corrupt {
            detail: format!("kdf params malformed: {e}"),
        })?;
    let cost = |key: &str| {
        params
            .pointer(&format!("/params/{key}"))
            .or_else(|| params.pointer(&format!("/{key}")))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default() as u32
    };
    Ok(KdfParams {
        m_kib: cost("m_kib"),
        t: cost("t"),
        p: cost("p"),
    })
}

impl WrappedKey {
    /// Serializes the envelope for the `sessions` row (sorted-key JSON).
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "salt": self.salt,
            "params": {"m_kib": self.params.m_kib, "t": self.params.t, "p": self.params.p},
            "blob": self.blob,
        })
        .to_string()
    }

    /// Parses a stored envelope.
    ///
    /// # Errors
    /// Typed transfer error on malformed JSON.
    pub fn from_json(raw: &str) -> Result<Self, SessionError> {
        let value: serde_json::Value =
            serde_json::from_str(raw).map_err(|error| SessionError::Transfer {
                detail: format!("wrapped key malformed: {error}"),
            })?;
        let field = |name: &str| -> Result<Vec<u8>, SessionError> {
            value
                .get(name)
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|b| b.as_u64().and_then(|b| u8::try_from(b).ok()))
                        .collect()
                })
                .filter(|v: &Vec<u8>| !v.is_empty())
                .ok_or_else(|| SessionError::Transfer {
                    detail: format!("wrapped key missing {name}"),
                })
        };
        let m_kib = value
            .pointer("/params/m_kib")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0);
        let t = value
            .pointer("/params/t")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0);
        let p = value
            .pointer("/params/p")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0);
        if m_kib == 0 || t == 0 || p == 0 {
            return Err(SessionError::Transfer {
                detail: "wrapped key missing kdf params".to_owned(),
            });
        }
        Ok(Self {
            salt: field("salt")?,
            params: KdfParams { m_kib, t, p },
            blob: field("blob")?,
        })
    }
}

/// Persistence for session data keys — the T10 `SecretStore` shape
/// (save / load / delete under a key) so an OS-keyring implementation
/// is drop-in caller wiring.
pub trait KeyStore: Send + Sync {
    /// Persists the wrapped-key envelope for `session_id` (upsert).
    ///
    /// # Errors
    /// Implementation-defined typed failure.
    fn save(&self, session_id: &str, wrapped: &WrappedKey) -> Result<(), SessionError>;

    /// Loads the envelope for `session_id`, if persisted.
    ///
    /// # Errors
    /// Implementation-defined typed failure.
    fn load(&self, session_id: &str) -> Result<Option<WrappedKey>, SessionError>;

    /// Removes the entry for `session_id`.
    ///
    /// # Errors
    /// Implementation-defined typed failure.
    fn delete(&self, session_id: &str) -> Result<(), SessionError>;
}

/// Process-local [`KeyStore`]: envelopes live only for this process.
#[derive(Debug, Default)]
pub struct InMemoryKeyStore {
    entries: Mutex<BTreeMap<String, WrappedKey>>,
}

impl InMemoryKeyStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl KeyStore for InMemoryKeyStore {
    fn save(&self, session_id: &str, wrapped: &WrappedKey) -> Result<(), SessionError> {
        self.entries
            .lock()
            .map_err(|_| SessionError::Encrypt {
                detail: "key store mutex poisoned".to_owned(),
            })?
            .insert(session_id.to_owned(), wrapped.clone());
        Ok(())
    }

    fn load(&self, session_id: &str) -> Result<Option<WrappedKey>, SessionError> {
        Ok(self
            .entries
            .lock()
            .map_err(|_| SessionError::Encrypt {
                detail: "key store mutex poisoned".to_owned(),
            })?
            .get(session_id)
            .cloned())
    }

    fn delete(&self, session_id: &str) -> Result<(), SessionError> {
        self.entries
            .lock()
            .map_err(|_| SessionError::Encrypt {
                detail: "key store mutex poisoned".to_owned(),
            })?
            .remove(session_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SessionKey;

    #[test]
    fn wrapped_key_json_round_trips() {
        let key = SessionKey::generate().unwrap();
        let wrapped = key
            .wrap_for_test("pw", KdfParams::test_low(), vec![7; 16])
            .unwrap();
        let parsed = WrappedKey::from_json(&wrapped.to_json()).unwrap();
        assert_eq!(parsed.params, wrapped.params);
        assert!(SessionKey::unwrap(&parsed, "pw", "s").is_ok());
    }

    #[test]
    fn in_memory_store_upserts_and_deletes() {
        let key = SessionKey::generate().unwrap();
        let wrapped = key
            .wrap_for_test("pw", KdfParams::test_low(), vec![3; 16])
            .unwrap();
        let store = InMemoryKeyStore::new();
        assert!(store.load("s1").unwrap().is_none());
        store.save("s1", &wrapped).unwrap();
        assert!(store.load("s1").unwrap().is_some());
        store.delete("s1").unwrap();
        assert!(store.load("s1").unwrap().is_none());
    }
}
