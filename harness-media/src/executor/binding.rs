//! Adapter bindings: the per-operation dispatch table one format
//! registers. Each op carries exactly the handler type it can run, so a
//! request-kind/binding-kind mismatch is unrepresentable by construction
//! (the ops struct fields are typed, not a stringly map).

use super::types::AdapterTier;
use crate::descriptor::{FormatDescriptor, Operation};
use crate::diff::RoundTripVerdict;
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};
use crate::format::FormatId;
use crate::generate::GenerationSpec;
use crate::probe::ProbeResult;
use serde_json::Value;

/// Probe handler: typed identification, never a panic, never a guess.
pub type ProbeHandler = fn(&[u8], Option<&str>) -> ProbeResult;

/// Extract handler: bytes + caller budgets, typed failures.
pub type ExtractHandler = fn(&[u8], ExtractBudgets) -> Result<Extracted, MediaError>;

/// Generate handler: spec to deterministic bytes, typed failures.
pub type GenerateHandler = fn(&GenerationSpec) -> Result<Vec<u8>, MediaError>;

/// Verify handler: independent expected tree + produced bytes, typed
/// failures (never a silent pass).
pub type VerifyHandler = fn(&Value, &[u8]) -> Result<RoundTripVerdict, MediaError>;

/// One operation's dispatch state: a bound pure handler, a decided
/// subprocess lane, or a typed-unsupported refusal with the honest
/// reason.
///
/// `Subprocess` bindings route through the M15
/// [`ToolExecutor`](crate::tool::ToolExecutor): dispatching one answers
/// the typed
/// [`MediaError::ToolUnavailable`](crate::error::MediaError::ToolUnavailable)
/// with the manifest install hint while the tool is genuinely not
/// enabled/installed, and reports the pending per-format adapter
/// honestly once it is - support is DECIDED, execution is never faked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpBinding<H> {
    /// A pure-Rust handler is registered under `engine`.
    Pure {
        /// The named engine for the producer disclosure.
        engine: String,
        /// The handler.
        handler: H,
    },
    /// The op runs via a sandboxed external tool (M15 executor).
    Subprocess {
        /// The external tool this lane spawns.
        tool: String,
        /// How to install it (pack name / pinned installer pointer).
        install_hint: String,
    },
    /// The op is refused; the reason is surfaced verbatim in errors and
    /// the M27 honesty matrix.
    Unsupported {
        /// The honest why, never empty.
        reason: String,
    },
}

impl<H> OpBinding<H> {
    /// The typed refusal this binding produces when dispatched without a
    /// bound handler. (A `Pure` binding has no refusal path in dispatch -
    /// the request-kind match arms make that arm unreachable - but the
    /// mapping stays total rather than panicking.)
    pub(super) fn refusal(&self, format: FormatId, op: Operation) -> MediaError {
        match self {
            Self::Subprocess { tool, install_hint } => MediaError::ToolUnavailable {
                tool: tool.clone(),
                install_hint: install_hint.clone(),
            },
            Self::Unsupported { reason } => MediaError::UnsupportedOperation {
                format,
                op,
                reason: reason.clone(),
            },
            Self::Pure { .. } => MediaError::UnsupportedOperation {
                format,
                op,
                reason: "a pure handler is bound for this op; the request kind cannot reach it"
                    .into(),
            },
        }
    }
}

/// The four operation bindings of one format. A struct (not a map): all
/// four ops are present by construction and each carries its own handler
/// type - "registered without probe + (extract | typed-unsupported)" is
/// unrepresentable at the type level, and the builder re-checks probe
/// purity on top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterOps {
    /// Identification (bound for every registered format - M2 sniffing).
    pub probe: OpBinding<ProbeHandler>,
    /// Structural extraction.
    pub extract: OpBinding<ExtractHandler>,
    /// Spec-driven generation.
    pub generate: OpBinding<GenerateHandler>,
    /// Round-trip verification.
    pub verify: OpBinding<VerifyHandler>,
}

/// One registered format: its static contract (the M2 descriptor table -
/// the executor attaches to it, never replaces it), its tier, and its
/// dispatch bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterEntry {
    /// The static per-format contract (mime, extensions, op claims).
    pub descriptor: FormatDescriptor,
    /// The format's execution tier.
    pub tier: AdapterTier,
    /// The per-op dispatch table.
    pub ops: AdapterOps,
}
