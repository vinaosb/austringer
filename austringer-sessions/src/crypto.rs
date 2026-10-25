//! Encryption at rest: XChaCha20-Poly1305 per-session keys under an
//! argon2id-passphrase-wrapped envelope.
//!
//! Each encrypted session owns a random 32-byte data key. The stored form
//! is the key *wrapped* under a KEK derived from the user passphrase
//! (`argon2id`, tunable via [`KdfParams`]); the passphrase itself is
//! never persisted. Event payloads are sealed with the data key, bound to
//! the session id as AAD, so ciphertext moved between sessions fails to
//! open. A wrong passphrase fails key unwrap with a typed
//! [`SessionError::Decrypt`][crate::error::SessionError::Decrypt] — no
//! panic, no partial plaintext.
//!
//! OS-keyring persistence of data keys is caller wiring through the
//! [`KeyStore`] trait (same shape as T10's `SecretStore`: save / load /
//! delete of a secret under a key). The in-memory default keeps keys
//! process-local.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};

use crate::error::SessionError;

/// Argon2id cost parameters (memory in KiB, iterations, lanes).
///
/// Defaults follow the OWASP-baseline 64 MiB / 3 passes; tests use
/// [`KdfParams::test_low`] to stay fast. Every generation of parameters
/// is recorded next to the wrapped key, so old sessions stay openable
/// after the default changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub m_kib: u32,
    /// Time cost (passes).
    pub t: u32,
    /// Parallelism (lanes).
    pub p: u32,
}

impl KdfParams {
    /// Production default: 64 MiB, 3 passes, 1 lane.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            m_kib: 65_536,
            t: 3,
            p: 1,
        }
    }

    /// Deliberately weak, fast parameters — tests only, never defaults.
    /// (32 KiB / 2 passes is the smallest valid Argon2id cost on this
    /// RustCrypto version; production uses [`Self::standard`].)
    #[must_use]
    pub const fn test_low() -> Self {
        Self {
            m_kib: 32,
            t: 2,
            p: 1,
        }
    }
}

/// A per-session 32-byte data key. Debug output is redacted; there is no
/// accessor for the raw bytes outside sealing operations.
#[derive(Clone)]
pub struct SessionKey([u8; 32]);

impl std::fmt::Debug for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionKey([redacted])")
    }
}

/// The stored envelope of a wrapped data key.
#[derive(Debug, Clone)]
pub struct WrappedKey {
    /// Argon2 salt (16 bytes).
    pub salt: Vec<u8>,
    /// KDF cost parameters in force when the key was wrapped.
    pub params: KdfParams,
    /// `nonce(24) || ciphertext` of the data key under the passphrase KEK.
    pub blob: Vec<u8>,
}

/// AAD binding the key-wrap envelope (constant: the envelope lives
/// inside its session row; payload sealing is what binds to ids).
const KEY_WRAP_AAD: &[u8] = b"austringer-session-key-wrap";

fn os_random(len: usize) -> Result<Vec<u8>, SessionError> {
    let mut buf = vec![0u8; len];
    getrandom::fill(&mut buf).map_err(|error| SessionError::Encrypt {
        detail: format!("os entropy unavailable: {error}"),
    })?;
    Ok(buf)
}

fn derive_kek(passphrase: &str, salt: &[u8], params: KdfParams) -> Result<[u8; 32], SessionError> {
    let argon = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(params.m_kib, params.t, params.p, Some(32)).map_err(|error| {
            SessionError::Encrypt {
                detail: format!("bad kdf params: {error}"),
            }
        })?,
    );
    let mut kek = [0u8; 32];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut kek)
        .map_err(|error| SessionError::Encrypt {
            detail: format!("kdf failed: {error}"),
        })?;
    Ok(kek)
}

impl SessionKey {
    /// Generates a fresh random data key.
    ///
    /// # Errors
    /// Typed [`SessionError::Encrypt`][crate::error::SessionError] when
    /// OS entropy is unavailable.
    pub fn generate() -> Result<Self, SessionError> {
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).map_err(|error| SessionError::Encrypt {
            detail: format!("os entropy unavailable: {error}"),
        })?;
        Ok(Self(key))
    }

    /// Wraps this key under `passphrase` using the given KDF parameters.
    ///
    /// # Errors
    /// Typed encryption failure.
    pub fn wrap(&self, passphrase: &str, params: KdfParams) -> Result<WrappedKey, SessionError> {
        self.wrap_for_test(passphrase, params, os_random(16)?)
    }

    /// Unwraps a data key. Any wrong passphrase is a clean
    /// [`SessionError::Decrypt`][crate::error::SessionError::Decrypt].
    ///
    /// # Errors
    /// Typed decrypt failure; never panics, never returns a wrong key.
    pub fn unwrap(
        wrapped: &WrappedKey,
        passphrase: &str,
        session_id: &str,
    ) -> Result<Self, SessionError> {
        let kek = derive_kek(passphrase, &wrapped.salt, wrapped.params)?;
        let aead = XChaCha20Poly1305::new((&kek).into());
        let decrypt_error = || SessionError::Decrypt {
            id: session_id.to_owned(),
            detail: "wrong passphrase or tampered wrapped key".to_owned(),
        };
        let nonce_bytes: [u8; 24] = wrapped
            .blob
            .get(..24)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(decrypt_error)?;
        let ciphertext = wrapped.blob.get(24..).ok_or_else(decrypt_error)?;
        let plaintext = aead
            .decrypt(
                &XNonce::from(nonce_bytes),
                chacha20poly1305::aead::Payload {
                    msg: ciphertext,
                    aad: KEY_WRAP_AAD,
                },
            )
            .map_err(|_| decrypt_error())?;
        let key: [u8; 32] = plaintext
            .as_slice()
            .try_into()
            .map_err(|_| decrypt_error())?;
        Ok(Self(key))
    }

    /// Seals `plaintext` bound to `aad` (the session id). Returns
    /// `nonce(24) || ciphertext`.
    pub(crate) fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, SessionError> {
        let nonce_bytes: [u8; 24] =
            os_random(24)?
                .as_slice()
                .try_into()
                .map_err(|_| SessionError::Encrypt {
                    detail: "nonce length".to_owned(),
                })?;
        let aead = XChaCha20Poly1305::new((&self.0).into());
        let ciphertext = aead
            .encrypt(
                &XNonce::from(nonce_bytes),
                chacha20poly1305::aead::Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|error| SessionError::Encrypt {
                detail: error.to_string(),
            })?;
        let mut blob = nonce_bytes.to_vec();
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    /// Opens a `nonce || ciphertext` blob produced by [`Self::seal`].
    ///
    /// # Errors
    /// Clean [`SessionError::Decrypt`][crate::error::SessionError::Decrypt]
    /// on wrong key, tampering, or AAD mismatch.
    pub(crate) fn open(&self, aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, SessionError> {
        let decrypt_error = |detail: &str| SessionError::Decrypt {
            id: String::from_utf8_lossy(aad).to_string(),
            detail: detail.to_owned(),
        };
        let nonce_bytes: [u8; 24] = blob
            .get(..24)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| decrypt_error("ciphertext shorter than a nonce"))?;
        let ciphertext = blob
            .get(24..)
            .ok_or_else(|| decrypt_error("no ciphertext"))?;
        let aead = XChaCha20Poly1305::new((&self.0).into());
        aead.decrypt(
            &XNonce::from(nonce_bytes),
            chacha20poly1305::aead::Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| decrypt_error("wrong key or tampered ciphertext"))
    }
}

impl SessionKey {
    /// Wraps with a caller-supplied salt (test seam pinning KDF
    /// inputs); production callers use [`Self::wrap`].
    pub(crate) fn wrap_for_test(
        &self,
        passphrase: &str,
        params: KdfParams,
        salt: Vec<u8>,
    ) -> Result<WrappedKey, SessionError> {
        let kek = derive_kek(passphrase, &salt, params)?;
        let nonce_bytes: [u8; 24] =
            os_random(24)?
                .as_slice()
                .try_into()
                .map_err(|_| SessionError::Encrypt {
                    detail: "nonce length".to_owned(),
                })?;
        let aead = XChaCha20Poly1305::new((&kek).into());
        let ciphertext = aead
            .encrypt(
                &XNonce::from(nonce_bytes),
                chacha20poly1305::aead::Payload {
                    msg: &self.0,
                    aad: KEY_WRAP_AAD,
                },
            )
            .map_err(|error| SessionError::Encrypt {
                detail: error.to_string(),
            })?;
        let mut blob = nonce_bytes.to_vec();
        blob.extend_from_slice(&ciphertext);
        Ok(WrappedKey { salt, params, blob })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_passphrase_is_a_clean_decrypt_error() {
        let key = SessionKey::generate().unwrap();
        let wrapped = key
            .wrap_for_test("correct horse", KdfParams::test_low(), vec![1; 16])
            .unwrap();
        let err = SessionKey::unwrap(&wrapped, "battery staple", "s1").unwrap_err();
        assert!(matches!(err, SessionError::Decrypt { .. }));
        let back = SessionKey::unwrap(&wrapped, "correct horse", "s1").unwrap();
        let sealed = back.seal(b"s1", b"payload").unwrap();
        assert_eq!(back.open(b"s1", &sealed).unwrap(), b"payload");
    }

    #[test]
    fn ciphertext_is_bound_to_session_aad() {
        let key = SessionKey::generate().unwrap();
        let sealed = key.seal(b"session-a", b"secret").unwrap();
        assert_ne!(&sealed[24..], b"secret");
        assert!(key.open(b"session-b", &sealed).is_err());
    }
}
