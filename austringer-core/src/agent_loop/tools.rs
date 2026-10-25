//! Tool-side operations: approval, execution (with the reserved
//! task-completion path), and in-flight-call reconstruction on resume.

use super::dispatch::{OpAction, stage_failure};
use super::focus::CompletionBlock;
use super::inference::{PendingCall, TASK_COMPLETED_TOOL};
use super::operation::Operation;
use super::pipeline::AgentLoop;
use super::stages::{Decision, StageCtx};
use super::status::SessionStatus;
use crate::event::content::ContentBlock;
use crate::event::{Event, RecordedEvent};

/// Rebuilds the in-flight tool-call set from the log: tool calls after the
/// last tool result are unanswered (aborted by a cancel/crash) and run
/// again on resume.
pub(super) fn rebuild_pending(events: &[RecordedEvent]) -> Vec<PendingCall> {
    let after = events
        .iter()
        .rposition(|recorded| matches!(recorded.event, Event::ToolResult { .. }))
        .map_or(0, |index| index + 1);
    events[after..]
        .iter()
        .filter_map(|recorded| match &recorded.event {
            Event::ToolCall {
                call_id,
                tool,
                arguments,
            } => Some(PendingCall {
                call_id: call_id.clone(),
                tool: tool.clone(),
                arguments: arguments.clone(),
            }),
            _ => None,
        })
        .collect()
}

pub(super) async fn approval(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::ToolApproval;
    if agent.pending_calls.is_empty() {
        return OpAction::Proceed;
    }
    let decisions = {
        let (stages, mut ctx) = stage_ctx!(agent);
        stages.approval.approve(&mut ctx).await
    };
    let decisions = match decisions {
        Ok(decisions) => decisions,
        Err(error) => return stage_failure(operation, error.message),
    };
    let mut kept = Vec::new();
    let calls = std::mem::take(&mut agent.pending_calls);
    for (call, decision) in calls.into_iter().zip(decisions) {
        match decision {
            Decision::Allow => kept.push(call),
            Decision::Deny { reason } => {
                let text = format!("tool call denied by approval stage: {reason}");
                if let Err(error) = agent.record(Event::ToolResult {
                    call_id: call.call_id,
                    blocks: vec![ContentBlock::Text { text }],
                }) {
                    return stage_failure(operation, error.to_string());
                }
            }
        }
    }
    agent.pending_calls = kept;
    OpAction::Proceed
}

pub(super) async fn execution(agent: &mut AgentLoop) -> OpAction {
    let operation = Operation::ToolExecution;
    // The reserved completion op is loop-enforced, never executed.
    if let Some(index) = agent
        .pending_calls
        .iter()
        .position(|call| call.tool == TASK_COMPLETED_TOOL)
    {
        let call = agent.pending_calls.remove(index);
        return complete_task(agent, call);
    }
    while let Some(call) = agent.pending_calls.first().cloned() {
        if agent.token.is_cancelling() {
            break;
        }
        let outcome = {
            let (stages, mut ctx) = stage_ctx!(agent);
            stages.execution.execute(&mut ctx, call.clone()).await
        };
        match outcome {
            Ok(outcome) => {
                // Never record beyond the high-water mark after a cancel.
                if agent.token.is_cancelling() {
                    break;
                }
                if let Err(error) = agent.record(Event::ToolResult {
                    call_id: call.call_id,
                    blocks: outcome.blocks,
                }) {
                    return stage_failure(operation, error.to_string());
                }
                agent.pending_calls.remove(0);
            }
            Err(error) => return stage_failure(operation, error.message),
        }
    }
    OpAction::Proceed
}

fn complete_task(agent: &mut AgentLoop, call: PendingCall) -> OpAction {
    let operation = Operation::ToolExecution;
    let block = agent.focus.completion_block().or_else(|| {
        agent
            .plan_gate
            .is_hard_blocked()
            .then(|| CompletionBlock::PlanGate {
                rendered: "TASK COMPLETION BLOCKED — plan.md gate is still hard-blocked".to_owned(),
            })
    });
    let Some(block) = block else {
        let result = Event::ToolResult {
            call_id: call.call_id,
            blocks: vec![ContentBlock::Text {
                text: "task completed".to_owned(),
            }],
        };
        if let Err(error) = agent.record(result) {
            return stage_failure(operation, error.to_string());
        }
        agent.last_completion_block = None;
        // Success lands via the driver, after the cancel check.
        return OpAction::Complete;
    };
    let notice = match &block {
        CompletionBlock::OpenTodos {
            open_count,
            rendered,
        } => format!(
            "TASK COMPLETION BLOCKED — {open_count} open todo(s) in the focus chain:\n{rendered}"
        ),
        CompletionBlock::PlanGate { rendered } => rendered.clone(),
    };
    if let Err(error) = agent.record(Event::System { text: notice }) {
        return stage_failure(operation, error.to_string());
    }
    // The typed blocked state is observable while the model is re-prompted:
    // InProgress -> Blocked -> InProgress across the same boundary.
    if let Ok(blocked) = agent.status.transition(SessionStatus::Blocked) {
        agent.status = blocked;
    }
    if let Ok(resumed) = agent.status.transition(SessionStatus::InProgress) {
        agent.status = resumed;
    }
    agent.last_completion_block = Some(block);
    agent.force_inference = true;
    OpAction::SkipRest
}
