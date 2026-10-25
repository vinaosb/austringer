//! Fork: efficient prefix copy at a sequence point.

use rusqlite::OptionalExtension;

use austringer_core::event::Sequence;

use crate::crypto::{KdfParams, SessionKey, WrappedKey};
use crate::error::SessionError;
use crate::index;
use crate::layout::validate_id;
use crate::meta::SessionMeta;
use crate::store::{SessionStore, constraint, state_bump};

/// Options for [`SessionStore::fork`].
#[derive(Debug, Clone, Default)]
pub struct ForkOptions {
    /// Title for the fork (defaults to the source title).
    pub title: Option<String>,
    /// Passphrase unlocking an encrypted source.
    pub unlock_passphrase: Option<String>,
    /// Passphrase sealing the fork; defaults to the unlock passphrase.
    pub new_passphrase: Option<String>,
    /// KDF parameters for the fork's wrapped key.
    pub kdf: Option<KdfParams>,
}

#[derive(Default)]
struct ForkKeys {
    source: Option<SessionKey>,
    wrapped: Option<WrappedKey>,
    fork: Option<SessionKey>,
}

impl SessionStore {
    /// Forks `source_id` at sequence `at` into `new_id`. Plaintext
    /// sources copy rows in SQL without deserializing (efficient copy:
    /// one SQLite row-copy statement inside a single transaction; a
    /// page-level CoW layer would tax every later read to optimize a
    /// one-shot copy). Encrypted sources decrypt under the unlock
    /// passphrase and re-seal under the fork's own key, because
    /// per-session keys must not be shared.
    ///
    /// # Errors
    /// Typed: not found, fork point beyond log, missing passphrase,
    /// invalid id, already exists.
    pub fn fork(
        &self,
        new_id: &str,
        source_id: &str,
        at: Sequence,
        options: &ForkOptions,
    ) -> Result<SessionMeta, SessionError> {
        validate_id(new_id)?;
        let source = self.load_meta(source_id)?;
        if at.0 > source.last_seq {
            return Err(SessionError::ForkPointBeyondLog {
                at: at.0,
                last: source.last_seq,
            });
        }
        let keys = self.fork_keys(&source, new_id, options)?;
        self.layout().ensure_session(new_id)?;
        let now = self.now();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction().map_err(SessionError::sqlite)?;
        let key_json = keys.wrapped.as_ref().map(|w| {
            serde_json::json!({"m_kib": w.params.m_kib, "t": w.params.t, "p": w.params.p})
                .to_string()
        });
        tx.execute(
            "INSERT INTO sessions (id, title, created_at, updated_at, encrypted, kdf_salt, \
             kdf_params, wrapped_key, forked_from)
             VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                new_id,
                options
                    .title
                    .clone()
                    .unwrap_or_else(|| source.title.clone()),
                now as i64,
                keys.wrapped.is_some() as i64,
                keys.wrapped.as_ref().map(|w| w.salt.clone()),
                key_json,
                keys.wrapped.as_ref().map(|w| w.blob.clone()),
                source_id
            ],
        )
        .map_err(|error| constraint(error, new_id))?;
        if source.encrypted {
            copy_encrypted_prefix(
                &tx,
                &source,
                keys.source.as_ref(),
                keys.fork.as_ref(),
                new_id,
                at.0,
            )?;
        } else {
            tx.execute(
                "INSERT INTO events (session_id, seq, kind, payload)
                 SELECT ?1, seq, kind, payload FROM events WHERE session_id = ?2 AND seq <= ?3",
                rusqlite::params![new_id, source_id, at.0 as i64],
            )
            .map_err(SessionError::sqlite)?;
            tx.execute(
                "INSERT INTO event_fts (session_id, seq, text)
                 SELECT ?1, seq, text FROM event_fts WHERE session_id = ?2 AND seq <= ?3",
                rusqlite::params![new_id, source_id, at.0 as i64],
            )
            .map_err(SessionError::sqlite)?;
        }
        tx.execute(
            "UPDATE sessions SET last_seq = ?2, event_count = ?2, turn_count = 0,
             payload_bytes = (SELECT COALESCE(SUM(LENGTH(payload)), 0) FROM events WHERE session_id = ?1)
             WHERE id = ?1",
            rusqlite::params![new_id, at.0 as i64],
        )
        .map_err(SessionError::sqlite)?;
        index::rebuild(&tx, new_id, at.0)?;
        state_bump(&tx, "sessions_since_dream", 1)?;
        tx.commit().map_err(SessionError::sqlite)?;
        drop(conn);
        self.load_meta(new_id)
    }

    fn fork_keys(
        &self,
        source: &SessionMeta,
        new_id: &str,
        options: &ForkOptions,
    ) -> Result<ForkKeys, SessionError> {
        if !source.encrypted {
            return Ok(ForkKeys::default());
        }
        let passphrase = options.unlock_passphrase.as_deref().ok_or_else(|| {
            SessionError::MissingPassphrase {
                id: source.id.clone(),
            }
        })?;
        let (source_key, source_kdf) = {
            let conn = self.lock_conn()?;
            let row: Option<(Vec<u8>, String, Vec<u8>)> = conn
                .query_row(
                    "SELECT kdf_salt, kdf_params, wrapped_key FROM sessions WHERE id = ?1",
                    [&source.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(SessionError::sqlite)?;
            let (salt, params_raw, blob) = row.ok_or_else(|| SessionError::Corrupt {
                detail: format!(
                    "encrypted session {} is missing its key envelope",
                    source.id
                ),
            })?;
            let wrapped = WrappedKey {
                salt,
                params: crate::keystore::parse_kdf(&params_raw)?,
                blob,
            };
            (
                SessionKey::unwrap(&wrapped, passphrase, &source.id)?,
                wrapped.params,
            )
        };
        let new_passphrase = options.new_passphrase.as_deref().unwrap_or(passphrase);
        let fork = SessionKey::generate()?;
        let wrapped = fork.wrap(new_passphrase, options.kdf.unwrap_or(source_kdf))?;
        let verified = SessionKey::unwrap(&wrapped, new_passphrase, new_id)?;
        Ok(ForkKeys {
            source: Some(source_key),
            wrapped: Some(wrapped),
            fork: Some(verified),
        })
    }
}

fn copy_encrypted_prefix(
    tx: &rusqlite::Transaction<'_>,
    source: &SessionMeta,
    source_key: Option<&SessionKey>,
    fork_key: Option<&SessionKey>,
    new_id: &str,
    at: u64,
) -> Result<(), SessionError> {
    let (Some(source_key), Some(fork_key)) = (source_key, fork_key) else {
        return Err(SessionError::MissingPassphrase {
            id: source.id.clone(),
        });
    };
    let mut stmt = tx
        .prepare("SELECT seq, kind, payload FROM events WHERE session_id = ?1 AND seq <= ?2 ORDER BY seq")
        .map_err(SessionError::sqlite)?;
    let rows = stmt
        .query_map(rusqlite::params![source.id, at as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(SessionError::sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SessionError::sqlite)?;
    for (seq, kind, payload) in rows {
        let plaintext = source_key.open(source.id.as_bytes(), &payload)?;
        let sealed = fork_key.seal(new_id.as_bytes(), &plaintext)?;
        tx.execute(
            "INSERT INTO events (session_id, seq, kind, payload) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![new_id, seq, kind, sealed],
        )
        .map_err(SessionError::sqlite)?;
    }
    Ok(())
}
