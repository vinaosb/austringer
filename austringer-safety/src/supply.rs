//! Shared supply-chain primitives for external tooling (plan M15,
//! decision 5: promoted from austringer-orchestration's backend install
//! module - the supply-chain home is safety). Zero behavior change:
//! `ReleaseAsset`, `verify_sha256`, and `redact_env` moved verbatim;
//! austringer-orchestration re-exports them so its callers (T40) stay
//! source-compatible.
//!
//! The auto-install gate discipline (Zed ACP child discipline, SS23):
//! release assets verify against a pinned SHA-256 before anything runs,
//! and child environments are redacted - secrets never reach external
//! tool processes.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

/// Patterns stripped from child environments by default.
pub const DEFAULT_REDACT_PATTERNS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GOOGLE_API_KEY",
    "GEMINI_API_KEY",
    "XAI_API_KEY",
    "GROK_API_KEY",
    "DEEPSEEK_API_KEY",
    "HF_TOKEN",
    "GITHUB_TOKEN",
];

/// One pinned release asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    /// Download URL.
    pub url: String,
    /// Pinned SHA-256 (hex).
    pub sha256: String,
    /// Asset version.
    pub version: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Verifies downloaded bytes against a pinned hex digest (case-insensitive;
/// malformed digests fail closed).
pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> bool {
    if expected_hex.len() != 64 || !expected_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    let computed = sha256_hex(bytes);
    computed.eq_ignore_ascii_case(expected_hex)
}

/// Redacts a child environment: keys matching any pattern
/// (case-insensitive substring) are dropped.
pub fn redact_env(env: &BTreeMap<String, String>, deny: &[&str]) -> BTreeMap<String, String> {
    env.iter()
        .filter(|(key, _)| {
            !deny.iter().any(|pattern| {
                key.to_ascii_uppercase()
                    .contains(&pattern.to_ascii_uppercase())
            })
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
