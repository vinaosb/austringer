//! Stage fakes for agent-loop (todo 5) tests: cancel-aware stages that park
//! mid-operation until a cancel lands (then return cooperatively, writing
//! nothing), slow/hung compactors for grace-window probes, and an
//! always-blocking stop hook.

#![allow(dead_code)]

use std::time::Duration;

use async_trait::async_trait;
use harness_core::agent_loop::stages::*;
use harness_core::agent_loop::{
    AssistantTurn, Inference, InferenceError, InferenceRequest, PendingCall, StageSet,
};

/// Builds a [`StageSet`] from defaults with a configuration closure.
pub fn stages(configure: impl FnOnce(&mut StageSet)) -> StageSet {
    let mut stages = StageSet::default();
    configure(&mut stages);
    stages
}

/// Parks inside the operation until cancelled; then returns WITHOUT writing
/// any event (the cooperative-abort stage contract).
pub struct CancelAwareEntry;

#[async_trait]
impl EntryHook for CancelAwareEntry {
    async fn on_entry(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        ctx.cancel.cancelled().await;
        Ok(Flow::Continue)
    }
}

/// Cancel-aware slash-command stage.
pub struct CancelAwareSlash;

#[async_trait]
impl SlashCommands for CancelAwareSlash {
    async fn dispatch(
        &mut self,
        ctx: &mut StageCtx<'_>,
        _command: &str,
    ) -> Result<SlashFlow, StageError> {
        ctx.cancel.cancelled().await;
        Ok(SlashFlow::NotACommand)
    }
}

/// Cancel-aware steer stage.
pub struct CancelAwareSteer;

#[async_trait]
impl Steer for CancelAwareSteer {
    async fn steer(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        ctx.cancel.cancelled().await;
        Ok(Flow::Continue)
    }
}

/// Cancel-aware max-turns gate.
pub struct CancelAwareMaxTurns;

#[async_trait]
impl MaxTurnsGate for CancelAwareMaxTurns {
    async fn check(&mut self, ctx: &mut StageCtx<'_>) -> Result<TurnGate, StageError> {
        ctx.cancel.cancelled().await;
        Ok(TurnGate::WithinBudget)
    }
}

/// Cancel-aware compaction stage.
pub struct CancelAwareCompaction;

#[async_trait]
impl Compactor for CancelAwareCompaction {
    async fn compact(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        ctx.cancel.cancelled().await;
        Ok(Flow::Continue)
    }
}

/// Cancel-aware tool-pair compaction stage.
pub struct CancelAwarePairCompaction;

#[async_trait]
impl ToolPairCompactor for CancelAwarePairCompaction {
    async fn fold(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        ctx.cancel.cancelled().await;
        Ok(Flow::Continue)
    }
}

/// Cancel-aware tool-approval stage.
pub struct CancelAwareApproval;

#[async_trait]
impl ToolApproval for CancelAwareApproval {
    async fn approve(&mut self, ctx: &mut StageCtx<'_>) -> Result<Vec<Decision>, StageError> {
        ctx.cancel.cancelled().await;
        Ok(Vec::new())
    }
}

/// Cancel-aware tool-execution stage.
pub struct CancelAwareExecution;

#[async_trait]
impl ToolExecutor for CancelAwareExecution {
    async fn execute(
        &mut self,
        ctx: &mut StageCtx<'_>,
        _call: PendingCall,
    ) -> Result<ToolOutcome, StageError> {
        ctx.cancel.cancelled().await;
        Ok(ToolOutcome::text("(aborted)"))
    }
}

/// Cancel-aware retry stage.
pub struct CancelAwareRetry;

#[async_trait]
impl RetryPolicy for CancelAwareRetry {
    async fn on_failure(
        &mut self,
        ctx: &mut StageCtx<'_>,
        _error: &harness_core::agent_loop::InferenceError,
    ) -> Result<RetryFlow, StageError> {
        ctx.cancel.cancelled().await;
        Ok(RetryFlow::GiveUp)
    }
}

/// Cancel-aware stop hook.
pub struct CancelAwareStopHook;

#[async_trait]
impl StopHook for CancelAwareStopHook {
    async fn on_stop(&mut self, ctx: &mut StageCtx<'_>) -> Result<StopDecision, StageError> {
        ctx.cancel.cancelled().await;
        Ok(StopDecision::Proceed)
    }
}

/// Cancel-aware exit-on-error stage.
pub struct CancelAwareExitOnError;

#[async_trait]
impl ExitOnError for CancelAwareExitOnError {
    async fn on_error(
        &mut self,
        ctx: &mut StageCtx<'_>,
        _error: &harness_core::agent_loop::InferenceError,
    ) -> Result<ErrorFlow, StageError> {
        ctx.cancel.cancelled().await;
        Ok(ErrorFlow::FailSession)
    }
}

/// Sleeps 100 ms (virtual under `start_paused`), then continues.
pub struct SlowCompactor;

#[async_trait]
impl Compactor for SlowCompactor {
    async fn compact(&mut self, _ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(Flow::Continue)
    }
}

/// Never resolves: models a hung stage for grace-window expiry probes.
pub struct HungCompactor;

#[async_trait]
impl Compactor for HungCompactor {
    async fn compact(&mut self, _ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        std::future::pending::<()>().await;
        Ok(Flow::Continue)
    }
}

/// Blocks continuation on every consultation (tests the loop-side cap).
pub struct AlwaysBlockStopHook;

#[async_trait]
impl StopHook for AlwaysBlockStopHook {
    async fn on_stop(&mut self, _ctx: &mut StageCtx<'_>) -> Result<StopDecision, StageError> {
        Ok(StopDecision::Block("keep going".to_owned()))
    }
}

/// Never resolves and never observes the cancel token: models a hung
/// inference call for grace-window expiry probes.
pub struct HungInference;

#[async_trait]
impl Inference for HungInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &harness_core::agent_loop::CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        std::future::pending::<()>().await;
        Ok(AssistantTurn {
            blocks: Vec::new(),
            tool_calls: Vec::new(),
        })
    }
}
