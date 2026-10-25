//! Typed registry errors: every failure mode is a variant, never a
//! panic, never a stringly-typed retry.

use crate::registry::governance::Authority;

/// Everything that can go wrong in the pack registry subsystem.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RegistryError {
    /// A filesystem operation failed.
    #[error("registry io: {detail}")]
    Io {
        /// Underlying failure description.
        detail: String,
    },
    /// The registry index file does not exist in the store.
    #[error("registry index missing")]
    IndexMissing,
    /// The registry index exists but is not valid JSON or wrong shape.
    #[error("registry index corrupt: {detail}")]
    IndexCorrupt {
        /// What was wrong with the index.
        detail: String,
    },
    /// The index exceeded the parse budget and was rejected un-parsed.
    #[error("registry index too large: {size} bytes (max {max})")]
    IndexTooLarge {
        /// Actual byte size.
        size: usize,
        /// Configured maximum.
        max: usize,
    },
    /// No entry with that name in the index.
    #[error("registry entry {name:?} not found")]
    EntryNotFound {
        /// The requested name.
        name: String,
    },
    /// No entry with that name+version pair.
    #[error("registry entry {name}@{version} not found")]
    EntryVersionNotFound {
        /// The requested name.
        name: String,
        /// The requested version.
        version: String,
    },
    /// Pack bytes fetched from the store do not hash to the pinned value.
    #[error("hash mismatch for {name:?}: index pinned {expected}, store served {actual}")]
    HashMismatch {
        /// The offending entry name.
        name: String,
        /// blake3 hex pinned in the index.
        expected: String,
        /// blake3 hex of the served bytes.
        actual: String,
    },
    /// The entry is signed but no verifier was supplied (fail closed).
    #[error("entry {name:?} is signed but no signature verifier was supplied")]
    SignatureVerifierMissing {
        /// The offending entry name.
        name: String,
    },
    /// The entry's signature did not verify.
    #[error("signature invalid for {name:?}")]
    SignatureInvalid {
        /// The offending entry name.
        name: String,
    },
    /// A signature was required but the entry is unsigned.
    #[error("signature required but entry {name:?} is unsigned")]
    SignatureMissing {
        /// The offending entry name.
        name: String,
    },
    /// The pack file bytes are not a valid pack document.
    #[error("pack file corrupt: {detail}")]
    PackCorrupt {
        /// What was wrong with the pack.
        detail: String,
    },
    /// The pack file exceeded the size budget.
    #[error("pack too large: {size} bytes (max {max})")]
    PackTooLarge {
        /// Actual byte size.
        size: usize,
        /// Configured maximum.
        max: usize,
    },
    /// The store does not have a pack file for the requested hash.
    #[error("pack file {hash:?} missing from store")]
    PackFileMissing {
        /// The requested content hash.
        hash: String,
    },
    /// A name failed validation (registry text is untrusted input).
    #[error("invalid name {name:?}")]
    InvalidName {
        /// The rejected name.
        name: String,
    },
    /// A version string failed validation.
    #[error("invalid version {version:?}")]
    InvalidVersion {
        /// The rejected version.
        version: String,
    },
    /// A content pin is not a 64-char hex hash.
    #[error("invalid pin {pin:?}")]
    InvalidPin {
        /// The rejected pin.
        pin: String,
    },
    /// A file path inside a pack failed validation.
    #[error("invalid pack path {path:?}")]
    InvalidPackPath {
        /// The rejected path.
        path: String,
    },
    /// A declared secret does not resolve through the secret source.
    #[error("missing secret: {name}")]
    MissingSecret {
        /// The missing secret NAME (never a value).
        name: String,
    },
    /// Review was required but no valid acceptance was supplied.
    #[error("entry {name:?} is unreviewed")]
    Unreviewed {
        /// The offending entry name.
        name: String,
    },
    /// Governance denied the plugin.
    #[error("governance denied {plugin:?} at {by:?} via forbidden pattern {pattern:?}")]
    GovernanceDeny {
        /// The denied plugin name.
        plugin: String,
        /// The authority whose rule fired.
        by: Authority,
        /// The matching forbidden glob.
        pattern: String,
    },
    /// A forbid-'*' rule locks down all installs.
    #[error("registry lockdown at {by:?}: forbid-'*' cannot be weakened by lower authority")]
    Lockdown {
        /// The authority that declared the lockdown.
        by: Authority,
    },
    /// The bundle forbids a plugin that is installed or bundled.
    #[error("plugin {plugin:?} conflicts with bundle forbidden pattern {pattern:?}")]
    ForbiddenPluginConflict {
        /// The conflicting plugin.
        plugin: String,
        /// The matching forbidden glob.
        pattern: String,
    },
    /// A required plugin is neither installed nor in the store.
    #[error("required plugin {plugin:?} missing")]
    MissingRequiredPlugin {
        /// The missing plugin name.
        plugin: String,
    },
    /// A pinned plugin hash does not match the index or ledger.
    #[error("pin mismatch for {plugin:?}: pinned {pinned}, found {actual}")]
    PinMismatch {
        /// The offending plugin.
        plugin: String,
        /// The pinned hash.
        pinned: String,
        /// The hash actually found.
        actual: String,
    },
    /// A bundle tried to nest another bundle.
    #[error("nested bundle {name:?}: bundles ship plugins, not bundles")]
    NestedBundle {
        /// The offending nested bundle name.
        name: String,
    },
    /// The registry ledger (registry-lock.json) is corrupt.
    #[error("registry ledger corrupt: {detail}")]
    LedgerCorrupt {
        /// What was wrong with the ledger.
        detail: String,
    },
    /// A non-UTF-8 file was found while building a pack.
    #[error("pack source file {path:?} is not valid UTF-8 text")]
    NonUtf8PackFile {
        /// The offending path.
        path: std::path::PathBuf,
    },
}

impl RegistryError {
    /// Wraps any std::io error with context.
    pub fn io(context: &str, err: &std::io::Error) -> Self {
        Self::Io {
            detail: format!("{context}: {err}"),
        }
    }
}
