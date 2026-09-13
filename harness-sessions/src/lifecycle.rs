//! Session creation: plaintext and encrypted lifecycles.

use crate::crypto::{KdfParams, SessionKey};
use crate::error::SessionError;
use crate::layout::validate_id;
use crate::meta::SessionMeta;
use crate::store::{SessionStore, constraint, state_bump};

impl SessionStore {
    /// Creates a plaintext session and materializes its directory
    /// contract.
    ///
    /// # Errors
    /// Typed: invalid id, already exists, IO.
    pub fn create(&self, id: &str, title: &str) -> Result<SessionMeta, SessionError> {
        validate_id(id)?;
        self.layout().ensure_session(id)?;
        let now = self.now();
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO sessions (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            rusqlite::params![id, title, now as i64],
        )
        .map_err(|error| constraint(error, id))?;
        state_bump(&conn, "sessions_since_dream", 1)?;
        Ok(fresh_meta(id, title, now, false))
    }

    /// Creates an encrypted session sealed under `passphrase`.
    ///
    /// # Errors
    /// Typed: invalid id, already exists, KDF failure.
    pub fn create_encrypted(
        &self,
        id: &str,
        title: &str,
        passphrase: &str,
        kdf: KdfParams,
    ) -> Result<SessionMeta, SessionError> {
        validate_id(id)?;
        self.layout().ensure_session(id)?;
        let wrapped = SessionKey::generate()?.wrap(passphrase, kdf)?;
        let now = self.now();
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO sessions (id, title, created_at, updated_at, encrypted, kdf_salt, kdf_params, wrapped_key)
             VALUES (?1, ?2, ?3, ?3, 1, ?4, ?5, ?6)",
            rusqlite::params![
                id,
                title,
                now as i64,
                wrapped.salt,
                serde_json::json!({"m_kib": wrapped.params.m_kib, "t": wrapped.params.t, "p": wrapped.params.p}).to_string(),
                wrapped.blob
            ],
        )
        .map_err(|error| constraint(error, id))?;
        state_bump(&conn, "sessions_since_dream", 1)?;
        Ok(fresh_meta(id, title, now, true))
    }
}

fn fresh_meta(id: &str, title: &str, now: u64, encrypted: bool) -> SessionMeta {
    SessionMeta {
        id: id.to_owned(),
        title: title.to_owned(),
        created_at: now,
        updated_at: now,
        last_seq: 0,
        event_count: 0,
        turn_count: 0,
        payload_bytes: 0,
        encrypted,
        forked_from: None,
        pinned: false,
        archived_at: None,
        imported_from: None,
        imported_unknown: 0,
    }
}
