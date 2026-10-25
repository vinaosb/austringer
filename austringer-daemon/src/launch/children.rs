//! Child-session execution: each launcher child runs as a REAL
//! session — its own [`SessionHost`] with a per-session dir, one
//! engine turn over the objective through the injected factories, and
//! tool execution through the daemon's executor seam. Writes take the
//! normal permission path (a declared write set is NEVER an
//! auto-allow). Cost governance (todo 33): a child's priced spend
//! books into the PARENT session's ledger — a child's preflight gate
//! refuses inference once the parent session's cap is spent.

use std::sync::Arc;
use std::time::Duration;

use austringer_orchestration::launch::{
    ChildFailure, ChildFailureClass, ChildJob, ChildReport, ChildTelemetry, SubagentExecutor,
};

use crate::attach::{HostCommand, SessionHost};
use crate::protocol::{ServerMsg, SessionEvent};
use crate::usage::ComposedInference;

use super::LaunchServices;

/// Wall-clock bound on one child's engine turn.
const CHILD_TURN_TIMEOUT: Duration = Duration::from_secs(30);
/// Child-session hub capacities (one turn is a handful of frames).
const CHILD_HUB_CAPACITY: usize = 32;

fn child_failure(child_id: &str, class: ChildFailureClass, detail: String) -> ChildFailure {
    ChildFailure {
        child_id: child_id.to_owned(),
        class,
        detail,
        telemetry: None,
    }
}

/// Executes one child job as a real child session.
pub struct SessionChildExecutor {
    /// Daemon services children run under.
    pub services: Arc<LaunchServices>,
    /// The PARENT session host: its cost ledger aggregates every
    /// child's spend, and threshold events publish on its stream.
    pub parent: Arc<SessionHost>,
}

#[async_trait::async_trait]
impl SubagentExecutor for SessionChildExecutor {
    async fn run(&self, job: ChildJob) -> Result<ChildReport, ChildFailure> {
        let started = std::time::Instant::now();
        let session_dir = self.services.sessions_root.join(&job.child_id);
        std::fs::create_dir_all(&session_dir).map_err(|e| {
            child_failure(
                &job.child_id,
                ChildFailureClass::Tool,
                format!("session dir: {e}"),
            )
        })?;
        let (host, cmd_rx) = SessionHost::new(
            &job.child_id,
            CHILD_HUB_CAPACITY,
            CHILD_HUB_CAPACITY,
            2,
            self.services.ask_policy.clone(),
            self.services.ask_timeout,
            None,
        );
        let executor = (self.services.executor_factory)(&session_dir, host.event_sink());
        let ComposedInference {
            inference,
            identity,
        } = (self.services.inference_factory)();
        // Identity-bearing child stacks carry the SAME accounting as
        // parent sessions — Usage frames on the child's own hub, priced
        // spend into the PARENT ledger (warning/stop events surface on
        // the parent stream, where the badge lives). Parents without a
        // ledger (bare hosts) launch unledgered children: honest
        // absence, unchanged behavior.
        let inference = match (identity, self.parent.cost_ledger()) {
            (Some(identity), Some(ledger)) => {
                let usage = crate::usage::UsageInference::new(
                    Arc::clone(&host),
                    identity.clone(),
                    None,
                    inference,
                );
                Box::new(crate::cost_ledger::CostLedgerInference::new(
                    Arc::clone(&self.parent),
                    ledger,
                    identity,
                    Box::new(usage),
                ))
            }
            _ => inference,
        };
        host.spawn_engine(
            session_dir,
            host.standard_stages(executor),
            inference,
            cmd_rx,
        )
        .map_err(|e| {
            child_failure(
                &job.child_id,
                ChildFailureClass::Tool,
                format!("engine spawn: {e}"),
            )
        })?;
        let mut live = host.hub_subscribe();
        host.commands()
            .send(HostCommand::Submit(job.objective.clone()))
            .await
            .map_err(|e| {
                child_failure(
                    &job.child_id,
                    ChildFailureClass::Protocol,
                    format!("objective submit: {e}"),
                )
            })?;
        let outcome = match await_turn_finished(&host, &mut live, &job.child_id).await {
            Ok(outcome) => outcome,
            Err(failure) => return Err(failure),
        };
        let telemetry = ChildTelemetry {
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            ..ChildTelemetry::default()
        };
        if outcome == "task_completed" || outcome == "turn_complete" {
            Ok(ChildReport {
                child_id: job.child_id,
                summary: outcome,
                artifacts: Vec::new(),
                telemetry,
                touches: Vec::new(),
            })
        } else {
            let class = if outcome.starts_with("cancelled") {
                ChildFailureClass::Canceled
            } else {
                ChildFailureClass::Model
            };
            Err(ChildFailure {
                child_id: job.child_id,
                class,
                detail: outcome,
                telemetry: Some(telemetry),
            })
        }
    }
}

/// Awaits the child session's `TurnFinished`, bounded by
/// [`CHILD_TURN_TIMEOUT`]. A lagged receiver recovers from the hub
/// snapshot (the ring still holds the finished turn).
async fn await_turn_finished(
    host: &Arc<SessionHost>,
    live: &mut tokio::sync::broadcast::Receiver<ServerMsg>,
    child_id: &str,
) -> Result<String, ChildFailure> {
    let deadline = tokio::time::Instant::now() + CHILD_TURN_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(child_failure(
                child_id,
                ChildFailureClass::Budget,
                "child session timed out".into(),
            ));
        }
        match tokio::time::timeout(remaining, live.recv()).await {
            Err(_) => {
                return Err(child_failure(
                    child_id,
                    ChildFailureClass::Budget,
                    "child session timed out".into(),
                ));
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                return Err(child_failure(
                    child_id,
                    ChildFailureClass::Protocol,
                    "child session stream closed".into(),
                ));
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                if let Some(outcome) =
                    host.snapshot()
                        .iter()
                        .rev()
                        .find_map(|frame| match &frame.event {
                            SessionEvent::TurnFinished { outcome, .. } => Some(outcome.clone()),
                            _ => None,
                        })
                {
                    return Ok(outcome);
                }
            }
            Ok(Ok(ServerMsg::Event { frame })) => {
                if let SessionEvent::TurnFinished { outcome, .. } = frame.event {
                    return Ok(outcome);
                }
            }
            Ok(Ok(_)) => {}
        }
    }
}
