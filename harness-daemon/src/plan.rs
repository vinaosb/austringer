//! Interactive plan mode (todo 16): the opt-in plan-approval flow for
//! MAIN sessions. Entering plan mode (OFF by default —
//! [`crate::protocol::ClientMsg::PlanMode`], wired to the TUI
//! `/mode plan` command) creates a pending plan; every write tool (`edit_lines`,
//! `execute_command` — the core capability matrix's Edit/Exec columns)
//! is then blocked BOTH at the approval stage and re-checked at
//! execution time, and each block broadcasts the pending plan id so a
//! client can approve / edit / reject the artifact through the same
//! [`crate::mediation::PermissionMediator`] machinery as permission
//! and fallback asks (task 1's routing contract).
//!
//! State machine per plan: entry (revision 1, empty artifact — the
//! agent drafts it in the transcript) → `PlanBlocked` per write
//! attempt → one live ask per revision. Approve ends plan mode and
//! autonomy flips back per the matrix; reject keeps the gate closed;
//! an edit replaces the artifact, bumps the revision, and re-asks
//! (the Antigravity plan-as-editable-artifact receipt). Timeout and
//! cancellation NEVER approve (the Antigravity v1.3.2
//! skip-permissions receipt): the plan stays pending and the next
//! write attempt re-fires the ask.
//!
//! The grant-store half of the contract lives in harness-core:
//! entering plan mode suspends every `GrantScope::Session` grant until
//! approval ([`harness_core::autonomy::AutonomyMatrix::honors_grant`]).

use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;

use harness_core::agent_loop::inference::PendingCall;
use harness_core::agent_loop::stages::{
    Decision, StageCtx, StageError, ToolApproval, ToolExecutor, ToolOutcome,
};
use harness_core::autonomy::{Capability, capability_of};
use harness_core::inspectors::permission::extract_target;

use crate::attach::SessionHost;
use crate::protocol::{PlanOutcome, SessionEvent};

/// The pending plan a session's writes gate on.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ActivePlan {
    id: u64,
    artifact: String,
    revision: u64,
}

#[derive(Default)]
struct PlanState {
    next_plan_id: u64,
    active: Option<ActivePlan>,
    ask_open: bool,
}

/// The per-session plan gate. Inactive by default; shares the session
/// host's lifetime and publishes through it.
pub struct PlanGate {
    state: Mutex<PlanState>,
}

impl Default for PlanGate {
    fn default() -> Self {
        Self {
            state: Mutex::new(PlanState::default()),
        }
    }
}

impl PlanGate {
    /// Whether plan mode is active (writes gated).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.lock().active.is_some()
    }

    fn lock(&self) -> MutexGuard<'_, PlanState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Enters plan mode (idempotent): a fresh pending plan with
    /// revision 1 and an empty artifact — the agent drafts the plan in
    /// the transcript; clients edit it onto the artifact through
    /// [`PlanDecision::Edit`](crate::protocol::PlanDecision::Edit).
    pub fn enter(&self, host: &Arc<SessionHost>) {
        let plan = {
            let mut state = self.lock();
            if state.active.is_some() {
                return;
            }
            state.next_plan_id = state.next_plan_id.saturating_add(1);
            let id = state.next_plan_id;
            state.active = Some(ActivePlan {
                id,
                artifact: String::new(),
                revision: 1,
            });
            id
        };
        host.publish(SessionEvent::PlanModeEntered { plan });
    }

    /// Exits plan mode without approval (the `/mode off` path): the
    /// pending plan is abandoned and writes un-gate. No-op when off.
    pub fn exit(&self, host: &Arc<SessionHost>) {
        let exited = {
            let mut state = self.lock();
            state.active.take().map(|plan| plan.id)
        };
        if let Some(plan) = exited {
            host.publish(SessionEvent::PlanModeExited { plan });
        }
    }

    /// The plan-gate check every write-tool path consults — at the
    /// approval stage AND again at execution time (in-flight calls
    /// approved before plan mode was entered included: the gate reads
    /// the CURRENT state, never an approval snapshot). Returns the
    /// typed block reason naming the pending plan id, or `None` when
    /// the call may proceed. Each block publishes
    /// [`SessionEvent::PlanBlocked`] and fires the approval ask when
    /// none is live for the pending revision.
    pub fn check_write(&self, host: &Arc<SessionHost>, call: &PendingCall) -> Option<String> {
        if !Self::is_write_tool(&call.tool) {
            return None;
        }
        let (plan, artifact, revision, spawn_ask) = {
            let mut state = self.lock();
            let active = state.active.clone()?;
            let spawn_ask = !state.ask_open;
            if spawn_ask {
                state.ask_open = true;
            }
            (active.id, active.artifact, active.revision, spawn_ask)
        };
        host.publish(SessionEvent::PlanBlocked {
            plan,
            tool: call.tool.clone(),
            target: extract_target(call).map(|target| target.value),
        });
        if spawn_ask {
            spawn_plan_ask(Arc::clone(host), plan, revision, artifact);
        }
        Some(format!(
            "plan {plan} pending approval: write tool '{}' is gated until the plan is \
             approved, edited, or rejected (answer the plan ask)",
            call.tool
        ))
    }

    /// Approval: plan mode ends (autonomy flips back per the matrix)
    /// and writes un-gate. Only the matching plan id applies — a stale
    /// answer to an exited plan approves nothing.
    fn approve(&self, host: &Arc<SessionHost>, plan_id: u64) {
        let approved = {
            let mut state = self.lock();
            let matching = state.active.as_ref().is_some_and(|plan| plan.id == plan_id);
            if matching {
                state.active = None;
                state.ask_open = false;
            }
            matching
        };
        if approved {
            host.publish(SessionEvent::PlanApproved { plan: plan_id });
        }
    }

    /// Rejection: the plan is refused but plan mode STAYS active and
    /// writes stay gated (only approval flips autonomy back).
    fn reject(&self, host: &Arc<SessionHost>, plan_id: u64) {
        let rejected = {
            let mut state = self.lock();
            let matching = state.active.as_ref().is_some_and(|plan| plan.id == plan_id);
            if matching {
                state.ask_open = false;
            }
            matching
        };
        if rejected {
            host.publish(SessionEvent::PlanRejected { plan: plan_id });
        }
    }

    /// Artifact edit: revision bumps, the updated artifact re-gates
    /// (writes stay blocked), and the caller re-asks on the new
    /// revision. Returns the new `(revision, artifact)` for that
    /// re-ask; `None` when the plan no longer matches.
    fn apply_edit(
        &self,
        host: &Arc<SessionHost>,
        plan_id: u64,
        artifact: String,
    ) -> Option<(u64, String)> {
        let updated = {
            let mut state = self.lock();
            let active = state.active.as_mut().filter(|plan| plan.id == plan_id)?;
            active.revision += 1;
            active.artifact = artifact;
            Some((active.revision, active.artifact.clone()))
        };
        let (revision, artifact) = updated?;
        host.publish(SessionEvent::PlanUpdated {
            plan: plan_id,
            revision,
        });
        Some((revision, artifact))
    }

    /// Ask closed without a decision (timeout/cancel): the plan stays
    /// pending; the next write attempt re-fires the ask.
    fn close_ask(&self, plan_id: u64) {
        let mut state = self.lock();
        if state.active.as_ref().is_some_and(|plan| plan.id == plan_id) {
            state.ask_open = false;
        }
    }

    fn is_write_tool(tool: &str) -> bool {
        capability_of(tool)
            .is_some_and(|capability| matches!(capability, Capability::Edit | Capability::Exec))
    }
}

/// Enter or exit plan mode on `host` (the [`crate::protocol::ClientMsg::PlanMode`]
/// handler every transport funnels into).
pub fn set_plan_mode(host: &Arc<SessionHost>, on: bool) {
    if on {
        host.plan_gate().enter(host);
    } else {
        host.plan_gate().exit(host);
    }
}

/// The ask loop: mediate the pending plan (approve / edit / reject
/// under the session's mediation policy), apply the outcome, and on an
/// edit re-ask on the new revision. One loop per plan; it ends on
/// approve/reject/timeout/cancel or when the plan no longer matches
/// (exited and possibly re-entered as a new plan id).
fn spawn_plan_ask(host: Arc<SessionHost>, plan_id: u64, revision: u64, artifact: String) {
    tokio::spawn(async move {
        let mediator = host.mediator();
        let mut revision = revision;
        let mut artifact = artifact;
        loop {
            match mediator.mediate_plan(plan_id, revision, &artifact).await {
                PlanOutcome::Approved { .. } => {
                    host.plan_gate().approve(&host, plan_id);
                    break;
                }
                PlanOutcome::Rejected { .. } => {
                    host.plan_gate().reject(&host, plan_id);
                    break;
                }
                PlanOutcome::Edited {
                    artifact: edited, ..
                } => {
                    let Some((next_revision, next_artifact)) =
                        host.plan_gate().apply_edit(&host, plan_id, edited)
                    else {
                        break;
                    };
                    revision = next_revision;
                    artifact = next_artifact;
                }
                PlanOutcome::Cancelled { .. } | PlanOutcome::Timeout => {
                    host.plan_gate().close_ask(plan_id);
                    break;
                }
            }
        }
    });
}

/// Approval-stage wrapper: pending write calls are denied with the
/// typed plan reason BEFORE any permission ask fires; everything else
/// delegates to the wrapped approval (the reserved `task_completed`
/// carve-out lives inside it).
pub struct PlanAwareApproval {
    inner: Box<dyn ToolApproval>,
    host: Arc<SessionHost>,
}

impl PlanAwareApproval {
    /// Wraps `inner` with the plan gate of `host`.
    #[must_use]
    pub fn new(inner: Box<dyn ToolApproval>, host: Arc<SessionHost>) -> Self {
        Self { inner, host }
    }
}

#[async_trait]
impl ToolApproval for PlanAwareApproval {
    async fn approve(&mut self, ctx: &mut StageCtx<'_>) -> Result<Vec<Decision>, StageError> {
        let calls = std::mem::take(ctx.pending_calls);
        let mut decisions = Vec::with_capacity(calls.len());
        for call in &calls {
            let decision = match self.host.plan_gate().check_write(&self.host, call) {
                Some(reason) => Decision::Deny { reason },
                None => {
                    ctx.pending_calls.clear();
                    ctx.pending_calls.push(call.clone());
                    let single = self.inner.approve(ctx).await?;
                    single.first().cloned().unwrap_or(Decision::Deny {
                        reason: "approval stage returned no decision".into(),
                    })
                }
            };
            decisions.push(decision);
        }
        *ctx.pending_calls = calls;
        Ok(decisions)
    }
}

/// Execution-stage wrapper: the plan gate is RE-CHECKED at execution
/// time on the CURRENT state — a write approved before plan mode was
/// entered (in-flight) blocks here, never reaching the real executor.
pub struct PlanGatedExecutor {
    inner: Box<dyn ToolExecutor>,
    host: Arc<SessionHost>,
}

impl PlanGatedExecutor {
    /// Wraps `inner` with the plan gate of `host`.
    #[must_use]
    pub fn new(inner: Box<dyn ToolExecutor>, host: Arc<SessionHost>) -> Self {
        Self { inner, host }
    }
}

#[async_trait]
impl ToolExecutor for PlanGatedExecutor {
    async fn execute(
        &mut self,
        ctx: &mut StageCtx<'_>,
        call: PendingCall,
    ) -> Result<ToolOutcome, StageError> {
        if let Some(reason) = self.host.plan_gate().check_write(&self.host, &call) {
            return Ok(ToolOutcome::text(reason));
        }
        self.inner.execute(ctx, call).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use crate::mediation::MediationPolicy;
    use crate::protocol::ServerMsg;

    fn host() -> Arc<SessionHost> {
        let (host, _cmd_rx) = SessionHost::new(
            "s-plan-unit",
            64,
            16,
            8,
            MediationPolicy::FirstResponder,
            Duration::from_millis(200),
            None,
        );
        host
    }

    fn edit_call(tool: &str) -> PendingCall {
        PendingCall {
            call_id: "c1".into(),
            tool: tool.to_owned(),
            arguments: serde_json::json!({"path": "src/lib.rs"}),
        }
    }

    fn blocked_events(host: &Arc<SessionHost>) -> Vec<u64> {
        host.snapshot()
            .iter()
            .filter_map(|frame| match &frame.event {
                SessionEvent::PlanBlocked { plan, .. } => Some(*plan),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn gate_is_off_by_default_and_write_tools_are_named() {
        let host = host();
        assert!(!host.plan_gate().is_active(), "plan mode is OFF by default");
        assert!(
            host.plan_gate()
                .check_write(&host, &edit_call("edit_lines"))
                .is_none()
        );
        assert!(
            host.plan_gate()
                .check_write(&host, &edit_call("execute_command"))
                .is_none()
        );
        assert!(
            host.plan_gate()
                .check_write(&host, &edit_call("view_window"))
                .is_none()
        );
        assert!(host.snapshot().is_empty());
        assert!(PlanGate::is_write_tool("edit_lines"));
        assert!(PlanGate::is_write_tool("execute_command"));
        assert!(!PlanGate::is_write_tool("view_window"));
        assert!(!PlanGate::is_write_tool("task_completed"));
    }

    #[tokio::test(start_paused = true)]
    async fn blocked_write_carries_plan_id_and_reason() {
        let host = host();
        set_plan_mode(&host, true);
        let reason = host
            .plan_gate()
            .check_write(&host, &edit_call("edit_lines"))
            .expect("gated");
        assert!(
            reason.contains("plan 1"),
            "reason names the plan id: {reason}"
        );
        assert_eq!(blocked_events(&host), vec![1]);
        let read = host
            .plan_gate()
            .check_write(&host, &edit_call("view_window"));
        assert!(read.is_none(), "reads stay open in plan mode");
        // The fired ask times out (paused clock) and the plan stays
        // pending — the ask loop keeps running until advanced past the
        // deadline; close via the gate state assertions in the e2e.
    }

    #[tokio::test(start_paused = true)]
    async fn enter_is_idempotent_and_exit_abandons() {
        let host = host();
        set_plan_mode(&host, true);
        set_plan_mode(&host, true);
        let reason = host
            .plan_gate()
            .check_write(&host, &edit_call("edit_lines"))
            .expect("gated");
        assert!(
            reason.contains("plan 1"),
            "re-entering keeps plan 1: {reason}"
        );
        set_plan_mode(&host, false);
        assert!(!host.plan_gate().is_active());
        assert!(
            host.plan_gate()
                .check_write(&host, &edit_call("edit_lines"))
                .is_none()
        );
        let kinds: Vec<&str> = host
            .snapshot()
            .iter()
            .filter_map(|frame| match &frame.event {
                SessionEvent::PlanModeEntered { .. } => Some("entered"),
                SessionEvent::PlanModeExited { .. } => Some("exited"),
                _ => None,
            })
            .collect();
        assert_eq!(kinds, vec!["entered", "exited"]);
    }

    #[tokio::test(start_paused = true)]
    async fn approve_ends_the_mode_and_reject_stays() {
        let host = host();
        set_plan_mode(&host, true);
        let _ = host
            .plan_gate()
            .check_write(&host, &edit_call("edit_lines"));
        host.plan_gate().approve(&host, 1);
        assert!(
            !host.plan_gate().is_active(),
            "approval flips autonomy back"
        );
        assert!(
            host.plan_gate()
                .check_write(&host, &edit_call("edit_lines"))
                .is_none()
        );

        set_plan_mode(&host, true); // plan 2
        let _ = host
            .plan_gate()
            .check_write(&host, &edit_call("edit_lines"));
        host.plan_gate().reject(&host, 2);
        assert!(host.plan_gate().is_active(), "rejection keeps plan mode on");
        let reason = host
            .plan_gate()
            .check_write(&host, &edit_call("edit_lines"))
            .expect("still gated after rejection");
        assert!(reason.contains("plan 2"));
        host.plan_gate().approve(&host, 1);
        assert!(
            host.plan_gate().is_active(),
            "a stale approval of an old plan id approves nothing"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn edit_bumps_revision_and_the_stale_edit_is_ignored() {
        let host = host();
        set_plan_mode(&host, true);
        let _ = host
            .plan_gate()
            .check_write(&host, &edit_call("edit_lines"));
        let (revision, artifact) = host
            .plan_gate()
            .apply_edit(&host, 1, "# Revised plan".to_owned())
            .expect("edit applies");
        assert_eq!((revision, artifact.as_str()), (2, "# Revised plan"));
        let reason = host
            .plan_gate()
            .check_write(&host, &edit_call("edit_lines"))
            .expect("the updated artifact re-gates");
        assert!(reason.contains("plan 1"));

        set_plan_mode(&host, false);
        set_plan_mode(&host, true); // plan 2 now active
        assert!(
            host.plan_gate()
                .apply_edit(&host, 1, "stale".to_owned())
                .is_none(),
            "an edit for an exited plan applies nothing"
        );
        let revisions: Vec<u64> = host
            .snapshot()
            .iter()
            .filter_map(|frame| match &frame.event {
                SessionEvent::PlanUpdated { revision, .. } => Some(*revision),
                _ => None,
            })
            .collect();
        assert_eq!(revisions, vec![2]);
    }

    #[tokio::test(start_paused = true)]
    async fn timeout_keeps_the_plan_pending_and_refires() {
        let host = host();
        let mut control = host.mediator().subscribe();
        set_plan_mode(&host, true);
        let first = host
            .plan_gate()
            .check_write(&host, &edit_call("edit_lines"))
            .expect("gated");
        assert!(first.contains("plan 1"));
        // Let the ask task start (register its deadline + broadcast)
        // BEFORE advancing past it — a paused clock fires only
        // already-registered timers.
        tokio::time::sleep(Duration::from_millis(1)).await;
        tokio::time::advance(Duration::from_millis(250)).await;
        tokio::time::sleep(Duration::from_millis(1)).await;
        let second = host
            .plan_gate()
            .check_write(&host, &edit_call("edit_lines"))
            .expect("still gated after timeout");
        assert!(
            second.contains("plan 1"),
            "timeout never approves; the ask re-fires"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
        let mut plan_asks = 0;
        while let Ok(msg) = control.try_recv() {
            if matches!(msg, ServerMsg::PlanAsk { .. }) {
                plan_asks += 1;
            }
        }
        assert!(
            plan_asks >= 2,
            "the first ask timed out and the second block re-fired one"
        );
    }
}
