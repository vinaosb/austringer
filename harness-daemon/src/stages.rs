//! Engine-stage adapters: the T7 inspector chain with the daemon's
//! mediated ask policy, and the split-brain tool executor that routes
//! approved calls to a remote worker through the durable journal.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use harness_core::agent_loop::inference::{PendingCall, TASK_COMPLETED_TOOL};
use harness_core::agent_loop::stages::{
    Decision, StageCtx, StageError, ToolApproval, ToolExecutor, ToolOutcome,
};
use harness_core::inspectors::chain::InspectorChain;
use harness_core::inspectors::store::AskPolicy;

use crate::journal::{Journal, JournalStep, activity_id};
use crate::mediation::MediatedAskPolicy;
use crate::protocol::SessionEvent;
use crate::worker::WorkerRouter;

/// The T7 chain over default (empty) rules — everything asks — with the
/// daemon's mediated policy as the resolution seam.
pub fn mediated_chain(policy: Box<dyn AskPolicy + Send>) -> InspectorChain {
    InspectorChain::new(Default::default()).with_ask_policy(policy)
}

/// Convenience: the chain wired to a shared mediator.
pub fn chain_over_mediator(mediator: Arc<crate::mediation::PermissionMediator>) -> InspectorChain {
    mediated_chain(Box::new(MediatedAskPolicy::new(mediator)))
}

/// Approval wrapper that lets the loop-reserved `task_completed` call
/// through without an ask (it is a harness-internal completion signal,
/// never a real tool) while every real call goes through the chain.
pub struct ReservedAwareApproval {
    inner: InspectorChain,
}

impl ReservedAwareApproval {
    /// Wrap a chain.
    #[must_use]
    pub fn new(inner: InspectorChain) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl ToolApproval for ReservedAwareApproval {
    async fn approve(&mut self, ctx: &mut StageCtx<'_>) -> Result<Vec<Decision>, StageError> {
        let calls = std::mem::take(ctx.pending_calls);
        let mut decisions = Vec::with_capacity(calls.len());
        for call in &calls {
            let decision = if call.tool == TASK_COMPLETED_TOOL {
                Decision::Allow
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

/// Split-brain tool execution: dispatch approved calls to a remote
/// worker (outbound-only worker connection), journal each activity
/// (durable, content-addressed), and surface diffs on the event stream.
/// Worker failures become tool-result text — the SESSION continues
/// (split-brain preserve).
pub struct WorkerToolExecutor {
    router: Arc<WorkerRouter>,
    journal: Option<Arc<Mutex<Journal>>>,
    event_sink: tokio::sync::mpsc::Sender<SessionEvent>,
}

impl WorkerToolExecutor {
    /// Executor over a worker router, optionally journaling activities,
    /// forwarding daemon-level events (diffs) to `event_sink`.
    pub fn new(
        router: Arc<WorkerRouter>,
        journal: Option<Arc<Mutex<Journal>>>,
        event_sink: tokio::sync::mpsc::Sender<SessionEvent>,
    ) -> Self {
        Self {
            router,
            journal,
            event_sink,
        }
    }

    fn journal_start(&self, name: &str, input: &serde_json::Value) {
        if let Some(journal) = &self.journal {
            let mut guard = match journal.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let id = activity_id(name, input);
            let _ = guard.record(JournalStep::ActivityStarted {
                id: id.0,
                name: name.to_owned(),
                input: input.clone(),
            });
        }
    }

    fn journal_complete(&self, name: &str, input: &serde_json::Value, output: &serde_json::Value) {
        if let Some(journal) = &self.journal {
            let mut guard = match journal.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let id = activity_id(name, input);
            let _ = guard.record(JournalStep::ActivityCompleted {
                id: id.0,
                output: output.clone(),
                retried: false,
            });
        }
    }

    fn forward_diff(&self, output: &serde_json::Value) {
        if let Some(diff) = output.get("diff") {
            let (path, before_hash, after_hash, hunks) = (
                diff.get("path")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(""),
                diff.get("before_hash")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(""),
                diff.get("after_hash")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(""),
                diff.get("hunks")
                    .and_then(serde_json::Value::as_array)
                    .map(|hs| {
                        hs.iter()
                            .filter_map(|h| h.as_str().map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            );
            let _ = self.event_sink.try_send(SessionEvent::Diff {
                path: path.to_owned(),
                before_hash: before_hash.to_owned(),
                after_hash: after_hash.to_owned(),
                hunks,
            });
        }
    }

    /// Dispatch one call and render its result text (the same path the
    /// [`ToolExecutor`] impl takes, without needing a loop context).
    pub async fn execute_call(&self, call: PendingCall) -> String {
        let input = serde_json::json!({
            "tool": call.tool,
            "arguments": call.arguments,
        });
        self.journal_start(&call.tool, &input);
        match self
            .router
            .dispatch(&call.call_id, &call.tool, &call.arguments)
            .await
        {
            Ok(output) => {
                self.journal_complete(&call.tool, &input, &output);
                self.forward_diff(&output);
                output.as_str().map_or_else(
                    || serde_json::to_string(&output).unwrap_or_default(),
                    str::to_owned,
                )
            }
            Err(worker_error) => format!("(remote worker: {worker_error})"),
        }
    }
}

#[async_trait]
impl ToolExecutor for WorkerToolExecutor {
    async fn execute(
        &mut self,
        _ctx: &mut StageCtx<'_>,
        call: PendingCall,
    ) -> Result<ToolOutcome, StageError> {
        let text = self.execute_call(call).await;
        Ok(ToolOutcome::text(text))
    }
}

/// Local executor for worker-less deployments and tests: returns the
/// call as its own result (echo semantics).
pub struct EchoToolExecutor;

#[async_trait]
impl ToolExecutor for EchoToolExecutor {
    async fn execute(
        &mut self,
        _ctx: &mut StageCtx<'_>,
        call: PendingCall,
    ) -> Result<ToolOutcome, StageError> {
        Ok(ToolOutcome::text(format!(
            "(echo executor: {} {})",
            call.tool,
            serde_json::to_string(&call.arguments).unwrap_or_default()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mediation::MediationPolicy;
    use crate::mediation::PermissionMediator;
    use crate::worker::ToolGrant;
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[test]
    fn mediated_chain_builds_over_default_rules() {
        let mediator = Arc::new(PermissionMediator::new(
            MediationPolicy::FirstResponder,
            Duration::from_secs(30),
            crate::mediation::control_bus(8),
        ));
        // Type-level composition check: the T7 chain accepts the
        // mediated ask policy. Its decision flow is exercised through a
        // real AgentLoop in the attach/integration suites.
        let _chain = chain_over_mediator(mediator);
    }

    #[tokio::test]
    async fn worker_executor_surfaces_failure_as_tool_text() {
        // Split-brain preserve at the STAGE level: no worker connected —
        // the tool result carries the typed error text, not a stage
        // failure; the loop keeps running.
        let router = Arc::new(WorkerRouter::new(Duration::from_millis(50)));
        let (sink, _rx) = mpsc::channel(4);
        let executor = WorkerToolExecutor::new(router, None, sink);
        let outcome = executor
            .execute_call(PendingCall {
                call_id: "c1".into(),
                tool: "bash".into(),
                arguments: serde_json::json!({}),
            })
            .await;
        assert!(outcome.contains("no worker connected"));
    }

    #[tokio::test]
    async fn worker_executor_journals_and_emits_diffs() {
        let router = Arc::new(WorkerRouter::new(Duration::from_secs(5)));
        let (worker_tx, mut worker_rx) = mpsc::channel(2);
        router.register(
            "w1",
            vec![ToolGrant {
                tool: "edit".into(),
            }],
            worker_tx,
        );
        let journal = Arc::new(Mutex::new(Journal::in_memory()));
        let (sink, mut rx) = mpsc::channel(4);
        let executor =
            WorkerToolExecutor::new(Arc::clone(&router), Some(Arc::clone(&journal)), sink);
        let dispatch = tokio::spawn({
            let router = Arc::clone(&router);
            async move {
                if let Some(crate::worker::DaemonToWorker::ToolCall { call_id, .. }) =
                    worker_rx.recv().await
                {
                    router.deliver(
                        &call_id,
                        true,
                        serde_json::json!({"diff": {
                            "path": "src/lib.rs",
                            "before_hash": "aa",
                            "after_hash": "bb",
                            "hunks": ["@@ -1 +1 @@"]
                        }}),
                    );
                }
            }
        });
        let outcome = executor
            .execute_call(PendingCall {
                call_id: "c1".into(),
                tool: "edit".into(),
                arguments: serde_json::json!({"file": "src/lib.rs"}),
            })
            .await;
        dispatch.await.ok();
        assert!(outcome.contains("src/lib.rs"));
        let steps = {
            let guard = journal.lock();
            match guard {
                Ok(g) => g.steps().len(),
                Err(p) => p.into_inner().steps().len(),
            }
        };
        assert_eq!(steps, 2, "activity start + completion journaled");
        match rx.try_recv() {
            Ok(SessionEvent::Diff { path, hunks, .. }) => {
                assert_eq!(path, "src/lib.rs");
                assert_eq!(hunks, vec!["@@ -1 +1 @@"]);
            }
            other => panic!("expected diff event, got {other:?}"),
        }
    }
}
