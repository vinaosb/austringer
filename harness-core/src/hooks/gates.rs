//! Tool-side hook surfaces (todo 28):
//!
//! - [`HookedApproval`] — the PreToolUse gate wrapping the T7
//!   inspector chain. Gate actions: allow (chain still decides),
//!   reject-and-continue, modify (the MODIFIED call is re-evaluated by
//!   the chain — hooks cannot bypass permissions), synthesize (the call
//!   never executes; the synthesized result is threat-scanned and
//!   budget-capped). A fail-closed hook failure denies the gated call;
//!   fail-open failures are journaled and the chain decides. Every
//!   chain denial fires PermissionDenied.
//! - [`HookedExecutor`] — PostToolUse output replacement/redaction
//!   (sanitized before entering context) and PostToolUseFailure
//!   (observed fail-open; the executor error still propagates).

use async_trait::async_trait;
use serde_json::json;

use super::HookEvent;
use super::config::FailPolicy;
use super::decision::{GateAction, HookOpinion, PostToolAction};
use super::manager::HookOutcome;
use super::shared::SharedHooks;
use crate::agent_loop::PendingCall;
use crate::agent_loop::stages::{
    Decision, StageCtx, StageError, ToolApproval, ToolExecutor, ToolOutcome,
};
use crate::event::Event;
use crate::event::content::ContentBlock;

/// What the PreToolUse gate concluded for one call.
enum GateOutcome {
    /// Deny with a model-facing reason.
    Reject(String),
    /// Produce a synthesized result; the call is dropped.
    Synthesize(String),
    /// Proceed: optionally with a modified call, chain to decide.
    Chain(Option<PendingCall>),
}

/// The T7 chain wrapped by PreToolUse hooks.
pub struct HookedApproval {
    /// Shared hook state.
    pub manager: SharedHooks,
    /// The wrapped approval stage (the inspector chain).
    pub inner: Box<dyn ToolApproval>,
}

#[async_trait]
impl ToolApproval for HookedApproval {
    async fn approve(&mut self, ctx: &mut StageCtx<'_>) -> Result<Vec<Decision>, StageError> {
        let calls = std::mem::take(ctx.pending_calls);
        if calls.is_empty() {
            return Ok(Vec::new());
        }
        let mut survivors = Vec::new();
        let mut decisions = Vec::new();
        for call in calls {
            match self.pre_tool_gate(&call).await {
                GateOutcome::Reject(reason) => {
                    survivors.push(call);
                    decisions.push(Decision::Deny {
                        reason: format!("hook rejected: {reason}"),
                    });
                }
                GateOutcome::Synthesize(text) => {
                    let blocks = {
                        let manager = self.manager.inner().lock().await;
                        manager.sanitize_result(&text)
                    };
                    ctx.append(Event::ToolResult {
                        call_id: call.call_id,
                        blocks,
                    })?;
                }
                GateOutcome::Chain(modified) => {
                    let effective = modified.unwrap_or(call);
                    let decision = self.chain_decide(ctx, &effective).await?;
                    if let Decision::Deny { reason } = &decision {
                        let payload = json!({
                            "event": "permission_denied",
                            "reason": reason,
                            "tool": effective.tool,
                        });
                        let mut manager = self.manager.inner().lock().await;
                        manager.fire(HookEvent::PermissionDenied, payload).await;
                    }
                    survivors.push(effective);
                    decisions.push(decision);
                }
            }
        }
        *ctx.pending_calls = survivors;
        Ok(decisions)
    }
}

impl HookedApproval {
    async fn pre_tool_gate(&mut self, call: &PendingCall) -> GateOutcome {
        let payload = json!({
            "arguments": call.arguments,
            "call_id": call.call_id,
            "event": "pre_tool_use",
            "tool": call.tool,
        });
        let fired = {
            let mut manager = self.manager.inner().lock().await;
            manager
                .fire_matching(HookEvent::PreToolUse, Some(&call.tool), payload)
                .await
        };
        let mut reject = None;
        let mut fail_closed: Option<String> = None;
        let mut modified = None;
        let mut synth = None;
        for firing in &fired {
            match &firing.outcome {
                HookOutcome::Succeeded(HookOpinion::Gate(GateAction::Reject { reason }))
                    if reject.is_none() =>
                {
                    reject = Some(reason.clone());
                }
                HookOutcome::Succeeded(HookOpinion::Gate(GateAction::Modify {
                    tool,
                    arguments,
                })) if modified.is_none() => {
                    modified = Some(PendingCall {
                        call_id: call.call_id.clone(),
                        tool: tool.clone(),
                        arguments: arguments.clone(),
                    });
                }
                HookOutcome::Succeeded(HookOpinion::Gate(GateAction::Synthesize { text }))
                    if synth.is_none() =>
                {
                    synth = Some(text.clone());
                }
                HookOutcome::Failed(error) if firing.fail_policy == FailPolicy::FailClosed => {
                    fail_closed = Some(error.to_string());
                }
                HookOutcome::Failed(_)
                | HookOutcome::Succeeded(_)
                | HookOutcome::InertPendingReview
                | HookOutcome::InertModified => {}
            }
        }
        if let Some(reason) = reject {
            return GateOutcome::Reject(reason);
        }
        if let Some(error) = fail_closed {
            return GateOutcome::Reject(format!("pre-tool-use hook failed closed: {error}"));
        }
        if let Some(modified) = modified {
            return GateOutcome::Chain(Some(modified));
        }
        if let Some(text) = synth {
            return GateOutcome::Synthesize(text);
        }
        GateOutcome::Chain(None)
    }

    /// Re-evaluates one call through the wrapped chain by temporarily
    /// narrowing the stage context to that single call — the structural
    /// guarantee that modified calls pass back through permissions.
    async fn chain_decide(
        &mut self,
        ctx: &mut StageCtx<'_>,
        call: &PendingCall,
    ) -> Result<Decision, StageError> {
        let saved = std::mem::take(ctx.pending_calls);
        ctx.pending_calls.push(call.clone());
        let decisions = self.inner.approve(ctx).await;
        *ctx.pending_calls = saved;
        decisions?.into_iter().next().ok_or_else(|| StageError {
            message: "approval stage returned no decision for a submitted call".to_owned(),
        })
    }
}

/// The tool executor wrapped by PostToolUse hooks.
pub struct HookedExecutor {
    /// Shared hook state.
    pub manager: SharedHooks,
    /// The wrapped execution stage.
    pub inner: Box<dyn ToolExecutor>,
}

#[async_trait]
impl ToolExecutor for HookedExecutor {
    async fn execute(
        &mut self,
        ctx: &mut StageCtx<'_>,
        call: PendingCall,
    ) -> Result<ToolOutcome, StageError> {
        let result = self.inner.execute(ctx, call.clone()).await;
        match result {
            Ok(outcome) => {
                let result_text = outcome
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        ContentBlock::Image(_) => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let payload = {
                    let manager = self.manager.inner().lock().await;
                    json!({
                        "call_id": call.call_id,
                        "event": "post_tool_use",
                        "result": manager.cap_result_text(&result_text),
                        "tool": call.tool,
                    })
                };
                let fired = {
                    let mut manager = self.manager.inner().lock().await;
                    manager
                        .fire_matching(HookEvent::PostToolUse, Some(&call.tool), payload)
                        .await
                };
                let replacement = fired.iter().find_map(|firing| match &firing.outcome {
                    HookOutcome::Succeeded(HookOpinion::PostTool(PostToolAction::Replace {
                        text,
                    })) => Some(text.clone()),
                    _ => None,
                });
                if let Some(text) = replacement {
                    let blocks = {
                        let manager = self.manager.inner().lock().await;
                        manager.sanitize_result(&text)
                    };
                    return Ok(ToolOutcome { blocks });
                }
                Ok(outcome)
            }
            Err(error) => {
                let payload = json!({
                    "call_id": call.call_id,
                    "error": error.message,
                    "event": "post_tool_use_failure",
                    "tool": call.tool,
                });
                let mut manager = self.manager.inner().lock().await;
                manager
                    .fire_matching(HookEvent::PostToolUseFailure, Some(&call.tool), payload)
                    .await;
                Err(error)
            }
        }
    }
}
