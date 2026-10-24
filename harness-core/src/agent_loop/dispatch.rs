//! Operation dispatch: routes each pipeline operation to its handler and
//! defines the shared stage-context split-borrow macro.

use super::config::{FailureReason, RunOutcome};
use super::operation::Operation;
use super::pipeline::AgentLoop;
use super::stages::{Flow, SlashFlow, StageCtx, StageError, TurnGate};
use crate::event::Event;

/// What one completed operation tells the iteration loop.
///
/// Terminal outcomes are PENDING until the cancel check in the pipeline
/// driver applies them: a handler never mutates the session status inside
/// its raced future, so a mid-stage cancel always finds a transitionable
/// state. [`OpAction::Stopped`] alone is pre-applied (cancel paths).
pub(super) enum OpAction {
    /// Continue with the next operation.
    Proceed,
    /// Skip the remaining operations of this iteration.
    SkipRest,
    /// Terminal failure, applied after the cancel check.
    Fail(FailureReason),
    /// Task-level completion, applied after the cancel check.
    Complete,
    /// Turn-level completion (no status change).
    TurnComplete,
    /// Already-applied terminal outcome (cancels).
    Stopped(RunOutcome),
}

/// Splits the loop into (stages, stage-context) disjoint borrows. The
/// context must be dropped before the loop is whole again.
macro_rules! stage_ctx {
    ($agent:ident) => {{
        let AgentLoop {
            stages,
            token,
            config,
            log,
            events,
            steer_queue,
            block_queue,
            steered,
            pending_calls,
            turn,
            failures,
            turn_ended,
            ..
        } = $agent;
        let ctx = StageCtx {
            cancel: token,
            config,
            log,
            mirror: events,
            steer_queue,
            block_queue,
            steered,
            turn: *turn,
            failures: *failures,
            pending_calls,
            turn_ended: *turn_ended,
        };
        (stages, ctx)
    }};
}

/// Runs the handler for one operation.
pub(super) async fn dispatch(agent: &mut AgentLoop, operation: Operation) -> OpAction {
    match operation {
        Operation::EntryHook => entry_hook(agent).await,
        Operation::SlashCommand => slash_command(agent).await,
        Operation::Steer => steer(agent).await,
        Operation::MaxTurns => max_turns(agent).await,
        Operation::Compaction => compaction(agent).await,
        Operation::ToolPairCompaction => pair_compaction(agent).await,
        Operation::ToolApproval => super::tools::approval(agent).await,
        Operation::ToolExecution => super::tools::execution(agent).await,
        Operation::Retry => super::decisions::retry(agent).await,
        Operation::StopHook => super::decisions::stop_hook(agent).await,
        Operation::ExitOnError => super::decisions::exit_on_error(agent).await,
        Operation::Inference => super::decisions::inference(agent).await,
    }
}

/// Maps a generic stage result into a pending iteration action.
pub(super) fn apply_flow(operation: Operation, result: Result<Flow, StageError>) -> OpAction {
    match result {
        Ok(Flow::Continue) => OpAction::Proceed,
        Ok(Flow::EndIteration) => OpAction::SkipRest,
        Ok(Flow::FailSession(cause)) | Err(StageError { message: cause }) => {
            OpAction::Fail(FailureReason::Stage { operation, cause })
        }
    }
}

/// A pending stage/IO failure attributed to an operation.
pub(super) fn stage_failure(operation: Operation, cause: String) -> OpAction {
    OpAction::Fail(FailureReason::Stage { operation, cause })
}

async fn entry_hook(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::EntryHook;
    let flow = {
        let (stages, mut ctx) = stage_ctx!(agent);
        stages.entry_hook.on_entry(&mut ctx).await
    };
    apply_flow(operation, flow)
}

async fn slash_command(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::SlashCommand;
    let is_command = agent
        .steer_queue
        .front()
        .is_some_and(|text| text.starts_with('/'));
    if !is_command {
        return OpAction::Proceed;
    }
    let command = match agent.steer_queue.pop_front() {
        Some(command) => command,
        None => return OpAction::Proceed,
    };
    let flow = {
        let (stages, mut ctx) = stage_ctx!(agent);
        stages.slash_commands.dispatch(&mut ctx, &command).await
    };
    match flow {
        Ok(SlashFlow::Handled { notice }) => {
            if let Some(text) = notice
                && let Err(error) = agent.record(Event::System { text })
            {
                return stage_failure(operation, error.to_string());
            }
            OpAction::SkipRest
        }
        Ok(SlashFlow::NotACommand) => {
            agent.steer_queue.push_front(command);
            OpAction::Proceed
        }
        Ok(SlashFlow::Unknown) => {
            let notice = format!("unknown command: {command}");
            if let Err(error) = agent.record(Event::System { text: notice }) {
                return stage_failure(operation, error.to_string());
            }
            OpAction::SkipRest
        }
        Err(error) => stage_failure(operation, error.message),
    }
}

async fn steer(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::Steer;
    let flow = {
        let (stages, mut ctx) = stage_ctx!(agent);
        stages.steer.steer(&mut ctx).await
    };
    apply_flow(operation, flow)
}

async fn max_turns(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::MaxTurns;
    let gate = {
        let (stages, mut ctx) = stage_ctx!(agent);
        stages.max_turns.check(&mut ctx).await
    };
    match gate {
        Ok(TurnGate::WithinBudget) => OpAction::Proceed,
        Ok(TurnGate::Exceeded) => OpAction::Fail(FailureReason::MaxTurns {
            limit: agent.config.max_turns,
        }),
        Err(error) => stage_failure(operation, error.message),
    }
}

async fn compaction(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::Compaction;
    let flow = {
        let (stages, mut ctx) = stage_ctx!(agent);
        stages.compaction.compact(&mut ctx).await
    };
    apply_flow(operation, flow)
}

async fn pair_compaction(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::ToolPairCompaction;
    let flow = {
        let (stages, mut ctx) = stage_ctx!(agent);
        stages.tool_pair_compaction.fold(&mut ctx).await
    };
    apply_flow(operation, flow)
}
