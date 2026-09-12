//! Injectable pipeline stages: one small trait per operation, each with a
//! default no-op implementation, plus the shared stage context.

use std::collections::VecDeque;

use async_trait::async_trait;

use super::cancel::CancelToken;
use super::config::LoopConfig;
pub use super::flow::{
    Decision, ErrorFlow, Flow, RetryFlow, SlashFlow, StageError, StopDecision, ToolOutcome,
    TurnGate,
};
use super::inference::{InferenceError, PendingCall};
use crate::event::content::ContentBlock;
use crate::event::{Event, RecordedEvent, Sequence};
use crate::log::EventLogWriter;

/// The loop state a stage may touch. One concept: the turn's working set.
pub struct StageCtx<'a> {
    /// Cooperative cancellation: stages poll this mid-operation.
    pub cancel: &'a CancelToken,
    /// The loop's configuration.
    pub config: &'a LoopConfig,
    /// The session log writer (append-only).
    pub(super) log: &'a mut EventLogWriter,
    /// The in-memory mirror of recorded events.
    pub(super) mirror: &'a mut Vec<RecordedEvent>,
    /// Queued user input not yet absorbed into the conversation.
    pub steer_queue: &'a mut VecDeque<String>,
    /// Set when this iteration absorbed user input.
    pub steered: &'a mut bool,
    /// Completed inference turns this run.
    pub turn: u32,
    /// Consecutive inference failures this run.
    pub failures: u32,
    /// Tool calls awaiting approval/execution.
    pub pending_calls: &'a mut Vec<PendingCall>,
    /// Whether the last assistant turn ended without tool calls.
    pub turn_ended: bool,
}

impl StageCtx<'_> {
    /// Appends an event to the session log (and the in-memory mirror),
    /// assigning its sequence.
    pub fn append(&mut self, event: Event) -> Result<Sequence, StageError> {
        let seq = self.log.append(event.clone())?;
        self.mirror.push(RecordedEvent { seq, event });
        Ok(seq)
    }
}

/// Per-iteration entry hook. Default: no-op. Todo 28 implements real hooks.
#[async_trait]
pub trait EntryHook: Send {
    /// Runs first in every iteration.
    async fn on_entry(&mut self, _ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        Ok(Flow::Continue)
    }
}

/// Slash-command dispatch. Default: every command is unknown.
#[async_trait]
pub trait SlashCommands: Send {
    /// Handles `command` (already stripped of the leading `/` is NOT
    /// assumed — handlers receive it verbatim).
    async fn dispatch(
        &mut self,
        _ctx: &mut StageCtx<'_>,
        _command: &str,
    ) -> Result<SlashFlow, StageError> {
        Ok(SlashFlow::Unknown)
    }
}

/// Mid-run steering: absorb queued user input. Default: drain the queue,
/// recording each message as a user event.
#[async_trait]
pub trait Steer: Send {
    /// Absorbs queued input.
    async fn steer(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        while let Some(text) = ctx.steer_queue.pop_front() {
            ctx.append(Event::User {
                blocks: vec![ContentBlock::Text { text }],
            })?;
            *ctx.steered = true;
        }
        Ok(Flow::Continue)
    }
}

/// The per-run turn budget. Default: exceed when completed turns reach
/// `config.max_turns`.
#[async_trait]
pub trait MaxTurnsGate: Send {
    /// Checks the budget.
    async fn check(&mut self, ctx: &mut StageCtx<'_>) -> Result<TurnGate, StageError> {
        if ctx.turn >= ctx.config.max_turns {
            Ok(TurnGate::Exceeded)
        } else {
            Ok(TurnGate::WithinBudget)
        }
    }
}

/// History condensation. Default: no-op. Todo 13 implements it.
#[async_trait]
pub trait Compactor: Send {
    /// Condenses history when warranted.
    async fn compact(&mut self, _ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        Ok(Flow::Continue)
    }
}

/// Tool-call/tool-result pair folding. Default: no-op. Todo 12 implements
/// it.
#[async_trait]
pub trait ToolPairCompactor: Send {
    /// Folds old tool pairs.
    async fn fold(&mut self, _ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        Ok(Flow::Continue)
    }
}

/// The inspector chain over pending tool calls. Default: allow everything.
/// Todo 7 replaces the default with permission/repetition inspectors.
#[async_trait]
pub trait ToolApproval: Send {
    /// Decides each pending call, positionally aligned with
    /// `ctx.pending_calls`.
    async fn approve(&mut self, ctx: &mut StageCtx<'_>) -> Result<Vec<Decision>, StageError> {
        Ok(vec![Decision::Allow; ctx.pending_calls.len()])
    }
}

/// Executes one approved tool call. Default: no backend registered. Todo 6
/// wires the real tool registry.
#[async_trait]
pub trait ToolExecutor: Send {
    /// Executes `call`. Implementations poll `ctx.cancel` and return rather
    /// than writing partial state.
    async fn execute(
        &mut self,
        _ctx: &mut StageCtx<'_>,
        call: PendingCall,
    ) -> Result<ToolOutcome, StageError> {
        Ok(ToolOutcome::text(format!(
            "(no tool backend registered; call {} not executed)",
            call.call_id
        )))
    }
}

/// Retry policy for failed inference. Default: retry while failures remain
/// within `config.max_inference_retries`.
#[async_trait]
pub trait RetryPolicy: Send {
    /// Decides what to do with the latest failure.
    async fn on_failure(
        &mut self,
        ctx: &mut StageCtx<'_>,
        _error: &InferenceError,
    ) -> Result<RetryFlow, StageError> {
        if ctx.failures <= ctx.config.max_inference_retries {
            Ok(RetryFlow::Retry)
        } else {
            Ok(RetryFlow::GiveUp)
        }
    }
}

/// Stop hook. Default: always let the turn exit. Todo 28 implements real
/// block-until hooks; the loop enforces the block cap regardless.
#[async_trait]
pub trait StopHook: Send {
    /// Decides whether the turn may exit.
    async fn on_stop(&mut self, _ctx: &mut StageCtx<'_>) -> Result<StopDecision, StageError> {
        Ok(StopDecision::Proceed)
    }
}

/// Exit-on-error policy. Default: fail the session.
#[async_trait]
pub trait ExitOnError: Send {
    /// Decides the session's fate for an un-retried failure.
    async fn on_error(
        &mut self,
        _ctx: &mut StageCtx<'_>,
        _error: &InferenceError,
    ) -> Result<ErrorFlow, StageError> {
        Ok(ErrorFlow::FailSession)
    }
}

/// No-op default stages (empty impls ride the trait defaults).
pub mod defaults {
    use super::*;

    /// Default entry hook.
    #[derive(Debug, Default)]
    pub struct NoEntryHook;
    #[async_trait]
    impl EntryHook for NoEntryHook {}

    /// Default slash dispatcher.
    #[derive(Debug, Default)]
    pub struct BuiltinSlash;
    #[async_trait]
    impl SlashCommands for BuiltinSlash {}

    /// Default steerer.
    #[derive(Debug, Default)]
    pub struct QueueSteerer;
    #[async_trait]
    impl Steer for QueueSteerer {}

    /// Default turn limiter.
    #[derive(Debug, Default)]
    pub struct TurnLimiter;
    #[async_trait]
    impl MaxTurnsGate for TurnLimiter {}

    /// Default compactor.
    #[derive(Debug, Default)]
    pub struct NoCompactor;
    #[async_trait]
    impl Compactor for NoCompactor {}

    /// Default pair folder.
    #[derive(Debug, Default)]
    pub struct NoPairCompactor;
    #[async_trait]
    impl ToolPairCompactor for NoPairCompactor {}

    /// Default approver.
    #[derive(Debug, Default)]
    pub struct ApproveAll;
    #[async_trait]
    impl ToolApproval for ApproveAll {}

    /// Default executor.
    #[derive(Debug, Default)]
    pub struct NoToolExecutor;
    #[async_trait]
    impl ToolExecutor for NoToolExecutor {}

    /// Default retry policy.
    #[derive(Debug, Default)]
    pub struct RetryWithBudget;
    #[async_trait]
    impl RetryPolicy for RetryWithBudget {}

    /// Default stop hook.
    #[derive(Debug, Default)]
    pub struct NoStopHook;
    #[async_trait]
    impl StopHook for NoStopHook {}

    /// Default exit-on-error policy.
    #[derive(Debug, Default)]
    pub struct FailFast;
    #[async_trait]
    impl ExitOnError for FailFast {}
}

/// The injectable stage set: one slot per pipeline operation (Inference
/// lives separately on the loop itself).
pub struct StageSet {
    /// EntryHook operation implementation.
    pub entry_hook: Box<dyn EntryHook>,
    /// SlashCommand operation implementation.
    pub slash_commands: Box<dyn SlashCommands>,
    /// Steer operation implementation.
    pub steer: Box<dyn Steer>,
    /// MaxTurns operation implementation.
    pub max_turns: Box<dyn MaxTurnsGate>,
    /// Compaction operation implementation.
    pub compaction: Box<dyn Compactor>,
    /// ToolPairCompaction operation implementation.
    pub tool_pair_compaction: Box<dyn ToolPairCompactor>,
    /// ToolApproval operation implementation.
    pub approval: Box<dyn ToolApproval>,
    /// ToolExecution operation implementation.
    pub execution: Box<dyn ToolExecutor>,
    /// Retry operation implementation.
    pub retry: Box<dyn RetryPolicy>,
    /// StopHook operation implementation.
    pub stop_hook: Box<dyn StopHook>,
    /// ExitOnError operation implementation.
    pub exit_on_error: Box<dyn ExitOnError>,
}

impl Default for StageSet {
    fn default() -> Self {
        Self {
            entry_hook: Box::new(defaults::NoEntryHook),
            slash_commands: Box::new(defaults::BuiltinSlash),
            steer: Box::new(defaults::QueueSteerer),
            max_turns: Box::new(defaults::TurnLimiter),
            compaction: Box::new(defaults::NoCompactor),
            tool_pair_compaction: Box::new(defaults::NoPairCompactor),
            approval: Box::new(defaults::ApproveAll),
            execution: Box::new(defaults::NoToolExecutor),
            retry: Box::new(defaults::RetryWithBudget),
            stop_hook: Box::new(defaults::NoStopHook),
            exit_on_error: Box::new(defaults::FailFast),
        }
    }
}
