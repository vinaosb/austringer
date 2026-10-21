//! The agent loop (plan todo 5): a named-operation pipeline, cancellable
//! mid-operation with high-water marks, anchored on the focus chain.
//!
//! (Named `agent_loop`, not `loop`: `loop` is a Rust keyword.)
//!
//! # Pipeline
//!
//! The loop never runs as an unstructured while-turn. Every iteration
//! dispatches [`Operation::ALL`] in order:
//!
//! ```text
//! EntryHook -> SlashCommand -> Steer -> MaxTurns -> Compaction
//!   -> ToolPairCompaction -> ToolApproval -> ToolExecution -> Retry
//!   -> StopHook (block-capped) -> ExitOnError -> Inference
//! ```
//!
//! Stages later todos implement (Compaction = 13, tool inspectors = 7,
//! hooks = 28) exist today as injectable traits with default no-op
//! implementations — the shape is fixed from day one.
//!
//! # Cancellation
//!
//! A cancel lands between/at operations: the loop races each operation
//! against the cancel token plus a grace window ([`LoopConfig`]).
//! Cooperative stages return without writing; a grace-window expiry drops
//! the operation mid-flight. Either way exactly one System cancel mark is
//! appended after the high-water sequence — the last committed event of
//! completed work — so resuming never sees poisoned state (crush lesson:
//! never poison later prompts). A cancel while inference streams abandons
//! the stream and marks cancelled-at-seq.
//!
//! # Completion
//!
//! Two levels: turn-level (the model ends its turn; status stays
//! [`SessionStatus::InProgress`]) vs task-level (the explicit
//! [`TASK_COMPLETED_TOOL`] op with every focus-chain todo closed and the
//! plan gate clear). While todos are open (or the gate is hard-blocked)
//! completion returns a typed [`CompletionBlock`] that re-prompts the model
//! with the open chain.

#[macro_use]
mod dispatch;

mod config;
mod decisions;
mod flow;
mod tools;

pub mod cancel;
pub mod focus;
pub mod inference;
pub mod operation;
pub mod pipeline;
pub mod stages;
pub mod status;

pub use cancel::{CancelHandle, CancelRecord, CancelToken};
pub use config::{FailureReason, LoopConfig, RunOutcome};
pub use focus::{CompletionBlock, FocusChain, FocusTodo, PlanGate, TodoId};
pub use inference::{
    AssistantTurn, Inference, InferenceError, InferenceRequest, PendingCall, TurnUsage,
    TASK_COMPLETED_TOOL,
};
pub use operation::Operation;
pub use pipeline::AgentLoop;
pub use stages::StageSet;
pub use status::{IllegalTransition, SessionStatus};
