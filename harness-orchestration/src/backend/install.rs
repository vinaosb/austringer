//! The auto-install gate for external backends (Zed ACP child
//! discipline, SS23): release assets verify against a pinned SHA-256
//! before anything runs, and the child environment is redacted — secrets
//! never reach backend processes.
//!
//! PROMOTED (plan M15, decision 5): the primitives live in
//! [`harness_safety::supply`] now — the supply-chain home; this module
//! re-exports them unchanged so existing callers (T40 surface) stay
//! source-compatible. Zero behavior change.

pub use harness_safety::supply::{
    DEFAULT_REDACT_PATTERNS, ReleaseAsset, redact_env, verify_sha256,
};
