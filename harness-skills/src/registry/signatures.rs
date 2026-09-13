//! Pack signatures (todo 32). The reference scheme is KEYED-BLAKE3
//! (blake3 MAC mode): it adds ZERO dependencies (blake3 is already a
//! crate dependency) and gives content authenticity between the registry
//! builder — who holds the [`SecretKey`] — and every consumer that
//! verifies with the same key, which matches the self-hosted registry
//! model. The [`SignatureVerifier`] trait is the asymmetric upgrade seam:
//! the daemon (or a registry operator) can swap in an ed25519 verifier
//! without touching the install pipeline.

/// A 32-byte signing/verification key. Symmetric by design (see module
/// docs); keys are sourced by the embedder (config, secret store) — this
/// type never generates or persists keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretKey([u8; 32]);

impl SecretKey {
    /// Wraps 32 raw bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The raw key bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Keyed-blake3 MAC of `bytes` as hex — the signature recorded in the
/// registry index.
pub fn sign_pack(bytes: &[u8], key: &SecretKey) -> String {
    let mut hasher = blake3::Hasher::new_keyed(key.as_bytes());
    hasher.update(bytes);
    hasher.finalize().to_hex().to_string()
}

/// Verifies pack signatures. Object-safe so install options can carry
/// any implementation (keyed-blake3 today, ed25519 via the daemon later).
pub trait SignatureVerifier: Send + Sync {
    /// Whether `signature_hex` authenticates `bytes`.
    fn verify(&self, bytes: &[u8], signature_hex: &str) -> bool;
}

/// The reference verifier: keyed-blake3, constant-time comparison.
#[derive(Debug, Clone)]
pub struct Blake3Verifier {
    key: SecretKey,
}

impl Blake3Verifier {
    /// Verifies with the same key the registry was signed with.
    pub fn new(key: &SecretKey) -> Self {
        Self { key: key.clone() }
    }
}

impl SignatureVerifier for Blake3Verifier {
    fn verify(&self, bytes: &[u8], signature_hex: &str) -> bool {
        let expected = sign_pack(bytes, &self.key);
        constant_time_eq(expected.as_bytes(), signature_hex.as_bytes())
    }
}

/// Byte-length-independent XOR accumulation: every byte always
/// contributes, so the comparison does not short-circuit on the first
/// differing character.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
