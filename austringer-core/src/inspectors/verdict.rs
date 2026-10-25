//! Inspector vocabulary: the per-call verdict and the inspector trait every
//! guard in this module implements.

use async_trait::async_trait;

use crate::agent_loop::PendingCall;
use crate::agent_loop::stages::StageError;

/// One inspector's judgment over one pending tool call.
///
/// Precedence (enforced by [`super::chain::InspectorChain`]): a
/// [`Verdict::Deny`] or [`Verdict::Intervene`] always blocks execution and
/// short-circuits the remaining inspectors; an *authoritative* `Allow`
/// (permission rule or stored grant) suppresses `Ask` verdicts raised by
/// later policy inspectors; everything else passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Pass. `authoritative` marks an explicit policy allow (rule match or
    /// stored user grant), which short-circuits later ask-generators where
    /// the policy says so.
    Allow {
        /// Whether this allow is backed by an explicit policy decision.
        authoritative: bool,
    },
    /// The call needs user approval before it may execute. `reason` is
    /// model-safe (already redacted); `notice_on_approval` is appended to
    /// the transcript when the ask is granted.
    Ask {
        /// Redacted, model-safe explanation of what needs approval.
        reason: String,
        /// Optional LLM-visible notice to record when the ask is granted.
        notice_on_approval: Option<String>,
    },
    /// Blocks execution; `reason` becomes the typed model-feedback payload.
    Deny {
        /// Why the call is refused (model-safe, redacted where secret).
        reason: String,
    },
    /// Behavioral circuit-breaker block (e.g. repetition): execution is
    /// refused with feedback suggesting variation. Not suppressible by an
    /// authoritative allow.
    Intervene {
        /// Feedback suggesting how to vary the call.
        reason: String,
    },
}

/// One guard in the per-tool-call inspector chain. Implementations run
/// sequentially before execution; see [`super::chain`] for ordering and
/// precedence.
#[async_trait]
pub trait CallInspector: Send {
    /// The inspector's name, used in feedback attribution.
    fn name(&self) -> &'static str;

    /// Judges one pending tool call.
    async fn inspect(&mut self, call: &PendingCall) -> Result<Verdict, StageError>;
}
