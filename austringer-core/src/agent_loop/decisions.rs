//! Policy operations: retry, stop hook (with the loop-enforced block cap),
//! exit-on-error, and the inference seam handler.

use super::config::FailureReason;
use super::dispatch::{OpAction, stage_failure};
use super::inference::{InferenceError, InferenceRequest};
use super::operation::Operation;
use super::pipeline::AgentLoop;
use super::stages::{ErrorFlow, RetryFlow, StageCtx, StopDecision};
use crate::event::Event;

pub(super) async fn retry(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::Retry;
    let Some(error) = agent.last_error.clone() else {
        return OpAction::Proceed;
    };
    let flow = {
        let (stages, mut ctx) = stage_ctx!(agent);
        stages.retry.on_failure(&mut ctx, &error).await
    };
    match flow {
        Ok(RetryFlow::Retry) => {
            agent.last_error = None;
            // Demanding inference invalidates turn-completeness: StopHook
            // runs before Inference and must not consult over a demanded
            // turn (force_inference is consumed at iteration top).
            agent.turn_ended = false;
            agent.force_inference = true;
            OpAction::SkipRest
        }
        Ok(RetryFlow::GiveUp) => OpAction::Proceed,
        Err(error) => stage_failure(operation, error.message),
    }
}

pub(super) async fn stop_hook(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::StopHook;
    // `turn_ended` is NOT reset between run() calls, so it stays true from
    // the previous clean turn; without the last_error guard a FIRST-inference
    // failure on turn >= 2 reached StopHook before ExitOnError and was
    // swallowed as TurnComplete (ledger todo-33 receipt).
    let consulted = agent.turn_ended
        && agent.last_error.is_none()
        && agent.pending_calls.is_empty()
        && agent.steer_queue.is_empty()
        && !agent.steered
        && !agent.force_inference;
    if !consulted {
        return OpAction::Proceed;
    }
    let decision = {
        let (stages, mut ctx) = stage_ctx!(agent);
        stages.stop_hook.on_stop(&mut ctx).await
    };
    match decision {
        Ok(StopDecision::Proceed) => OpAction::TurnComplete,
        Ok(StopDecision::Block(reason)) => {
            if agent.stop_blocks >= agent.config.stop_hook_block_cap {
                let notice = "stop-hook block cap reached; letting exit proceed".to_owned();
                if let Err(error) = agent.record(Event::System { text: notice }) {
                    return stage_failure(operation, error.to_string());
                }
                OpAction::TurnComplete
            } else {
                agent.stop_blocks += 1;
                if let Err(error) = agent.record(Event::System { text: reason }) {
                    return stage_failure(operation, error.to_string());
                }
                agent.turn_ended = false;
                agent.force_inference = true;
                OpAction::SkipRest
            }
        }
        Err(error) => stage_failure(operation, error.message),
    }
}

pub(super) async fn exit_on_error(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::ExitOnError;
    let Some(error) = agent.last_error.clone() else {
        return OpAction::Proceed;
    };
    let flow = {
        let (stages, mut ctx) = stage_ctx!(agent);
        stages.exit_on_error.on_error(&mut ctx, &error).await
    };
    match flow {
        Ok(ErrorFlow::FailSession) => {
            let cause = match &error {
                InferenceError::Failure(message) => message.clone(),
            };
            OpAction::Fail(FailureReason::InferenceExhausted {
                cause,
                retries: agent.config.max_inference_retries,
            })
        }
        Ok(ErrorFlow::EndTurn) => {
            // Handled quietly (e.g. a budget stop already surfaced by the
            // ledger): the error must not haunt the next run.
            agent.last_error = None;
            OpAction::TurnComplete
        }
        Ok(ErrorFlow::FeedModel) => {
            let notice = format!("inference error surfaced to the model: {error}");
            if let Err(error) = agent.record(Event::System { text: notice }) {
                return stage_failure(operation, error.to_string());
            }
            agent.last_error = None;
            agent.turn_ended = false;
            agent.force_inference = true;
            OpAction::SkipRest
        }
        Err(error) => stage_failure(operation, error.message),
    }
}

pub(super) async fn inference(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::Inference;
    let result = {
        let AgentLoop {
            inference,
            events,
            focus,
            token,
            ..
        } = agent;
        let rendered = focus.render();
        let request = InferenceRequest {
            events,
            focus_block: &rendered,
        };
        inference.infer(&request, token).await
    };
    match result {
        Ok(turn) => {
            if let Err(error) = agent.record(Event::Assistant {
                blocks: turn.blocks,
            }) {
                return stage_failure(operation, error.to_string());
            }
            agent.turn += 1;
            agent.turn_ended = turn.tool_calls.is_empty();
            for call in turn.tool_calls {
                let event = Event::ToolCall {
                    call_id: call.call_id.clone(),
                    tool: call.tool.clone(),
                    arguments: call.arguments.clone(),
                };
                if let Err(error) = agent.record(event) {
                    return stage_failure(operation, error.to_string());
                }
                agent.pending_calls.push(call);
            }
            OpAction::Proceed
        }
        Err(error) => {
            agent.failures += 1;
            agent.last_error = Some(error);
            OpAction::SkipRest
        }
    }
}
