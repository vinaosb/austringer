//! The typed media error vocabulary.
//!
//! Every failure mode of the media subsystem is one of these variants, and
//! every `Display` message names the next action. Unsupported operations
//! are typed errors with reasons - never fakes, never silent degradation.
//! The vocabulary mirrors the seam discipline of
//! `harness-core/src/tools/provider.rs` (typed ProviderError/DispatchError).

use serde::{Deserialize, Serialize};

use crate::descriptor::Operation;
use crate::format::FormatId;

/// Why a media operation failed. Every variant's `Display` is actionable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum MediaError {
    /// The format is not registered / not in the CORE slate.
    #[error(
        "unsupported media format: probe the bytes first and dispatch through the adapter registry"
    )]
    UnsupportedFormat,
    /// The format is registered but rejects this operation.
    #[error(
        "format {format} does not support {op}: {reason}; request a supported operation or a different output format"
    )]
    UnsupportedOperation {
        /// The format asked about.
        format: FormatId,
        /// The operation that was refused.
        op: Operation,
        /// The honest why, surfaced verbatim from the descriptor.
        reason: String,
    },
    /// An opt-in external tool is not installed.
    #[error("external tool {tool} is unavailable: {install_hint}")]
    ToolUnavailable {
        /// The missing tool's name.
        tool: String,
        /// How to install it (pack name / pinned installer pointer).
        install_hint: String,
    },
    /// A pinned tool installation failed.
    #[error(
        "pinned tool installation failed: re-run the install step and verify the recorded SHA-256 release pin"
    )]
    InstallFailed,
    /// The OS sandbox refused the operation.
    #[error(
        "the OS sandbox denied the operation: check the tool's writable-scope and network profile, then widen the sandbox explicitly if warranted"
    )]
    SandboxDenied,
    /// The operation exceeded its deadline.
    #[error(
        "the media operation exceeded its deadline: narrow the input or raise the caller-provided budget and retry"
    )]
    Timeout,
    /// A generation spec did not satisfy the format's contract.
    #[error(
        "invalid generation spec: fix the params for the target format (see its descriptor) and resubmit"
    )]
    InvalidSpec,
    /// The bytes did not parse as the probed format.
    #[error(
        "the input failed to parse: it may be corrupt or not the probed format; re-probe the bytes and check integrity"
    )]
    ParseFailure,
    /// The document is encrypted; execution of embedded code is banned and
    /// so is opaque decryption of container-level encryption.
    #[error(
        "encrypted documents are not supported: decrypt out-of-band and resubmit the plaintext copy"
    )]
    EncryptedUnsupported,
    /// No deterministic text projection exists for this format.
    #[error(
        "no text projector is registered for this format: use a capability-flagged model or register a projector for it"
    )]
    NoProjector,
    /// Extraction exceeded its wall-clock deadline or total-size cap.
    #[error(
        "extraction exceeded its budget (deadline or size cap): raise the caller-provided ExtractBudgets or split the input into smaller pieces"
    )]
    ExtractTooLarge,
}
