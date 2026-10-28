//! Loop configuration and the typed run-outcome vocabulary.

use std::time::Duration;

use super::operation::Operation;
use super::status::SessionStatus;
use crate::tools::ToolSchema;

/// Typed loop configuration.
#[derive(Clone, Debug)]
pub struct LoopConfig {
    /// Maximum completed inference turns per [`super::pipeline::AgentLoop::run`] call.
    pub max_turns: u32,
    /// How many times a failed inference may be retried per run.
    pub max_inference_retries: u32,
    /// The stop hook may block continuation at most this many times per
    /// run; after the cap the loop must let exit proceed.
    pub stop_hook_block_cap: u32,
    /// Cancel-grace window: in-flight operations get this long after a
    /// cancel to reach a clean boundary before a forced terminal cancel.
    pub cancel_grace: Duration,
    /// The stable system prompt riding every inference request (the
    /// stable-first prefix discipline, research §24.6). Empty = no
    /// system surface is declared.
    pub system: String,
    /// The declared wire tool surface (research §33.1: a provider cannot
    /// emit `tool_use` without declared tools). Empty = no tools ride
    /// the wire. The daemon fills this from the session's pinned tool
    /// registry once per session — never per request.
    pub tools: Vec<ToolSchema>,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            max_turns: 40,
            max_inference_retries: 0,
            stop_hook_block_cap: 1,
            cancel_grace: Duration::from_secs(5),
            system: String::new(),
            tools: Vec::new(),
        }
    }
}

/// Why a run failed.
#[derive(Debug, Clone, PartialEq)]
pub enum FailureReason {
    /// The per-run turn budget was exhausted.
    MaxTurns {
        /// The configured limit that fired.
        limit: u32,
    },
    /// Inference kept failing after the retry budget ran out.
    InferenceExhausted {
        /// The last failure's cause.
        cause: String,
        /// The configured retry budget.
        retries: u32,
    },
    /// A pipeline stage failed (or the log degraded).
    Stage {
        /// The operation that failed.
        operation: Operation,
        /// The failure cause.
        cause: String,
    },
    /// `run()` was called on an already-terminal session.
    Terminal {
        /// The terminal status observed.
        status: SessionStatus,
    },
}

/// The outcome of one [`super::pipeline::AgentLoop::run`] call.
#[derive(Debug, Clone, PartialEq)]
pub enum RunOutcome {
    /// Task-level completion: the explicit completion op ran with all
    /// todos closed and the plan gate clear.
    TaskCompleted,
    /// Turn-level completion: the model ended its turn; the session stays
    /// [`SessionStatus::InProgress`].
    TurnComplete,
    /// The run was cancelled; carries the high-water cancel record.
    Cancelled(super::cancel::CancelRecord),
    /// The run failed; carries the typed reason.
    Failed(FailureReason),
}
