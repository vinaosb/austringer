//! Stage-outcome vocabulary: generic flow, per-stage decisions, and the
//! stage error type.

use crate::event::content::ContentBlock;

/// Why a stage failed. The loop attaches the operation when it surfaces the
/// failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("stage error: {message}")]
pub struct StageError {
    /// Human-readable cause.
    pub message: String,
}

impl From<crate::log::AppendError> for StageError {
    fn from(error: crate::log::AppendError) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

/// Generic stage outcome: continue the pipeline, end this iteration early,
/// or fail the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flow {
    /// Proceed to the next operation.
    Continue,
    /// Skip the remaining operations of this iteration.
    EndIteration,
    /// The session is fatally failed (`cause` surfaces in the outcome).
    FailSession(String),
}

/// Outcome of a slash-command dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SlashFlow {
    /// The command was handled; optionally surface a notice.
    Handled {
        /// Optional system notice to record.
        notice: Option<String>,
    },
    /// The queued input is not a command; leave it for Steer.
    #[default]
    NotACommand,
    /// The command is unknown to the handler.
    Unknown,
}

/// Outcome of the max-turns gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnGate {
    /// The budget holds.
    WithinBudget,
    /// The budget is exhausted.
    Exceeded,
}

/// One approval decision for a pending tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Execute the call.
    Allow,
    /// Refuse the call (`reason` reaches the model as tool feedback).
    Deny {
        /// Why the call was refused.
        reason: String,
    },
}

/// The result of one executed tool call.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutcome {
    /// Content blocks for the tool-result event.
    pub blocks: Vec<ContentBlock>,
}

impl ToolOutcome {
    /// A plain-text outcome.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            blocks: vec![ContentBlock::Text { text: text.into() }],
        }
    }
}

/// Outcome of the retry policy for a failed inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryFlow {
    /// Run inference again.
    Retry,
    /// Hand the failure to ExitOnError.
    GiveUp,
}

/// Outcome of the stop hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopDecision {
    /// Let the turn exit.
    Proceed,
    /// Block continuation, forcing another inference round with `reason`
    /// (counted against the loop's block cap).
    Block(String),
}

/// Outcome of the exit-on-error policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorFlow {
    /// Fail the session terminally.
    FailSession,
    /// Surface the error to the model and continue.
    FeedModel,
    /// End the turn without failing the session. For typed budget stops:
    /// the stop is already surfaced through the ledger's signal sink, so
    /// the run ends quietly instead of loudly (signal.rs graceful-stop).
    EndTurn,
}
