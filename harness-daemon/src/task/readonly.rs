//! Read-only narrowing for curated agent children (todo 14,
//! ecosystem alignment): `explore` and `librarian` run their tool
//! calls through [`ReadOnlyApproval`], which denies every call
//! outside the agent's allowlist BEFORE the mediated chain — the
//! refusal is structural (no permission ask, no executor), reaches
//! the model as tool-result text through the loop's deny rendering,
//! and is logged on the child stream as a `readonly_refusal`
//! notification. In-allowlist calls and the loop-reserved
//! `task_completed` take the normal approval path unchanged.

use std::sync::Arc;

use async_trait::async_trait;
use harness_core::agent_loop::inference::TASK_COMPLETED_TOOL;
use harness_core::agent_loop::stages::{Decision, StageCtx, StageError, ToolApproval};

use crate::attach::SessionHost;
use crate::protocol::SessionEvent;

/// Notification kind emitted when the read-only allowlist refuses a
/// tool call (the log half of the todo-14 refusal contract).
pub const REFUSAL_KIND: &str = "readonly_refusal";

/// Approval-stage narrowing (todo 14): out-of-allowlist calls are
/// denied before any ask fires; the refusal is logged on the child's
/// stream. The loop renders the deny reason to the model as
/// tool-result text, so the session continues.
pub struct ReadOnlyApproval {
    inner: Box<dyn ToolApproval>,
    agent: String,
    allowed: Vec<String>,
    host: Arc<SessionHost>,
}

impl ReadOnlyApproval {
    /// Wraps the session's normal approval chain (`agent` names the
    /// curated agent in refusal text; `allowed` is its read-only
    /// tool allowlist).
    #[must_use]
    pub fn new(
        inner: Box<dyn ToolApproval>,
        host: Arc<SessionHost>,
        agent: &str,
        allowed: Vec<String>,
    ) -> Self {
        Self {
            inner,
            agent: agent.to_owned(),
            allowed,
            host,
        }
    }
}

#[async_trait]
impl ToolApproval for ReadOnlyApproval {
    async fn approve(&mut self, ctx: &mut StageCtx<'_>) -> Result<Vec<Decision>, StageError> {
        let calls = std::mem::take(ctx.pending_calls);
        let mut decisions = Vec::with_capacity(calls.len());
        for call in &calls {
            let decision = if call.tool != TASK_COMPLETED_TOOL && !self.allowed.contains(&call.tool)
            {
                let reason = format!(
                    "read-only agent `{}` cannot execute `{}` — allowed tools: {}",
                    self.agent,
                    call.tool,
                    self.allowed.join(", ")
                );
                self.host.publish(SessionEvent::Notification {
                    kind: REFUSAL_KIND.to_owned(),
                    message: reason.clone(),
                });
                Decision::Deny { reason }
            } else {
                ctx.pending_calls.clear();
                ctx.pending_calls.push(call.clone());
                let single = self.inner.approve(ctx).await?;
                single.first().cloned().unwrap_or(Decision::Deny {
                    reason: "approval stage returned no decision".into(),
                })
            };
            decisions.push(decision);
        }
        *ctx.pending_calls = calls;
        Ok(decisions)
    }
}
