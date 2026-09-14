//! The launcher: runs a [`ResolvedPlan`] through gates (budget, pacing,
//! spawn authorization), journals every spawned effect, gathers partials,
//! and synthesizes. Children are isolated task_id-resumable sessions in
//! the `{launch}-child-{task}` namespace (hidden from the main list).
//! Parallel children run concurrently under a semaphore bounded by the
//! plan's parallel cap; wave results journal in child-id order so the
//! on-disk truth is deterministic.

use std::collections::BTreeMap;
use std::sync::Arc;

pub use crate::authorize::ChildCapabilities;
use crate::authorize::{SpawnAuthzClass, SpawnRefusal};
use crate::budget::{BudgetGuard, Pacer, SpawnLease};
use crate::events::{TeamEvent, TeamEventSink};
pub use crate::job::{
    Artifact, ChildFailure, ChildJob, ChildReport, LaunchConfig, SubagentExecutor,
};
use crate::journal::{JournalError, JournalRecord, TeamJournal};
pub use crate::report::{
    LaunchError, LaunchOutcome, LaunchReport, ResumeError, SpecSource, StopReason,
};
pub use crate::resolve::{PlanBudget, ResolveError, ResolvedPlan, resolve};
use crate::telemetry::clamp_summary;
pub use crate::telemetry::{ChildFailureClass, ChildTelemetry};
use crate::write_set::TouchRegistry;

/// Result of the per-child gate.
enum Authorized {
    Lease(SpawnLease),
    Stopped(StopReason),
}

pub(crate) struct RunState {
    pub(crate) children: Vec<ChildReport>,
    pub(crate) failures: Vec<ChildFailure>,
    pub(crate) child_ids: Vec<String>,
    pub(crate) touches: TouchRegistry,
}

/// The launcher. One instance per launch; owns the journal, the event
/// sink, the pacing state, and the authorization ledger.
pub struct Launcher {
    config: LaunchConfig,
    pacer: Pacer,
    sink: Arc<dyn TeamEventSink>,
    pub(crate) journal: TeamJournal,
    ledger: crate::authorize::AuthorizationLedger,
}

impl Launcher {
    /// Builds a launcher over a config, an event sink, and a journal.
    pub fn new(config: LaunchConfig, sink: Arc<dyn TeamEventSink>, journal: TeamJournal) -> Self {
        let pacer = config.pacer.clone();
        Self {
            config,
            pacer,
            sink,
            journal,
            ledger: crate::authorize::AuthorizationLedger::new(),
        }
    }

    /// Mints a fresh-authorization grant for a child scope.
    pub fn mint_grant(&mut self, scope: &str, granted_by: &str) {
        self.ledger.mint(scope, granted_by);
    }

    pub(crate) fn emit(&self, event: TeamEvent) {
        self.sink.emit(event);
    }

    pub(crate) fn jr(&mut self, record: JournalRecord) -> Result<(), JournalError> {
        self.journal.record(record)
    }

    fn authorize(
        &mut self,
        budget: &mut BudgetGuard,
        job: &ChildJob,
        completed: &[String],
    ) -> Result<Authorized, LaunchError> {
        let lease = match budget.authorize_spawn() {
            Ok(lease) => lease,
            Err(breach) => {
                self.report_stop(&breach.to_string(), completed)?;
                return Ok(Authorized::Stopped(StopReason::Budget(breach)));
            }
        };
        if let Err(denied) = self.pacer.try_acquire((self.config.clock)()) {
            budget.release(lease);
            self.report_stop(&denied.to_string(), completed)?;
            return Ok(Authorized::Stopped(StopReason::Pacing(denied)));
        }
        if job.authz == SpawnAuthzClass::Fresh && !self.ledger.has(&job.child_id) {
            budget.release(lease);
            return Err(LaunchError::SpawnRefusal(SpawnRefusal {
                role: job.role.clone(),
                writes: job.writes.globs.clone(),
                scope: job.child_id.clone(),
            }));
        }
        Ok(Authorized::Lease(lease))
    }

    fn report_stop(&mut self, reason: &str, completed: &[String]) -> Result<(), LaunchError> {
        self.jr(JournalRecord::BudgetStopped {
            reason: reason.to_string(),
            completed: completed.to_vec(),
        })?;
        self.emit(TeamEvent::BudgetStopped {
            reason: reason.to_string(),
        });
        Ok(())
    }

    async fn run_gated(
        &self,
        gated: Vec<(SpawnLease, ChildJob)>,
        budget: &mut BudgetGuard,
        executor: &Arc<dyn SubagentExecutor>,
        permits: &Arc<tokio::sync::Semaphore>,
    ) -> BTreeMap<String, Result<ChildReport, ChildFailure>> {
        let mut set = tokio::task::JoinSet::new();
        let leases: Vec<SpawnLease> = gated.iter().map(|(lease, _)| *lease).collect();
        for (_, job) in gated {
            let executor = Arc::clone(executor);
            let permits = Arc::clone(permits);
            set.spawn(async move {
                let _permit = permits.acquire().await;
                (job.child_id.clone(), executor.run(job).await)
            });
        }
        let mut results = BTreeMap::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((child_id, outcome)) => {
                    results.insert(child_id, outcome);
                }
                Err(err) => {
                    results.insert(
                        format!("join-error-{err}"),
                        Err(ChildFailure {
                            child_id: String::new(),
                            class: ChildFailureClass::Tool,
                            detail: format!("child task join error: {err}"),
                            telemetry: None,
                        }),
                    );
                }
            }
        }
        for lease in leases {
            budget.release(lease);
        }
        results
    }

    fn absorb(
        &mut self,
        results: BTreeMap<String, Result<ChildReport, ChildFailure>>,
        budget: &mut BudgetGuard,
        state: &mut RunState,
        stopped: &mut Option<StopReason>,
    ) -> Result<(), LaunchError> {
        for (_, outcome) in results {
            match outcome {
                Ok(mut report) => {
                    report.summary = clamp_summary(&report.summary, crate::telemetry::SUMMARY_MAX);
                    let spend = report.telemetry.total_tokens();
                    self.jr(JournalRecord::ChildCompleted {
                        child: report.child_id.clone(),
                        result: report.summary.clone(),
                    })?;
                    self.emit(TeamEvent::Completed {
                        child: report.child_id.clone(),
                    });
                    if let Err(breach) = budget.record_spend(spend)
                        && stopped.is_none()
                    {
                        *stopped = Some(StopReason::Budget(breach));
                    }
                    for touch in &report.touches {
                        self.emit(TeamEvent::FileTouched {
                            child: touch.child.clone(),
                            path: touch.path.clone(),
                            op: touch.op.name().to_string(),
                        });
                        if let Some(collision) = state.touches.record(touch.clone()) {
                            self.jr(JournalRecord::Collision {
                                path: collision.path.clone(),
                                a: collision.a.clone(),
                                b: collision.b.clone(),
                            })?;
                            self.emit(TeamEvent::Collision(collision.clone()));
                        }
                    }
                    state.children.push(report);
                }
                Err(failure) => {
                    if let Some(t) = failure.telemetry {
                        let _ = budget.record_spend(t.total_tokens());
                    }
                    self.jr(JournalRecord::ChildFailed {
                        child: failure.child_id.clone(),
                        class: failure.class.name().to_string(),
                        detail: failure.detail.clone(),
                    })?;
                    self.emit(TeamEvent::Failed {
                        child: failure.child_id.clone(),
                        class: failure.class.name().to_string(),
                    });
                    state.failures.push(failure);
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn run_single(
        &mut self,
        job: ChildJob,
        budget: &mut BudgetGuard,
        executor: &Arc<dyn SubagentExecutor>,
        permits: &Arc<tokio::sync::Semaphore>,
        state: &mut RunState,
    ) -> Result<Option<StopReason>, LaunchError> {
        let completed: Vec<String> = state.children.iter().map(|c| c.child_id.clone()).collect();
        match self.authorize(budget, &job, &completed)? {
            Authorized::Stopped(reason) => Ok(Some(reason)),
            Authorized::Lease(lease) => {
                self.announce(&job, false)?;
                state.child_ids.push(job.child_id.clone());
                let results = self
                    .run_gated(vec![(lease, job)], budget, executor, permits)
                    .await;
                let mut stopped = None;
                self.absorb(results, budget, state, &mut stopped)?;
                Ok(stopped)
            }
        }
    }

    pub(crate) async fn execute_wave(
        &mut self,
        wave: &[ChildJob],
        budget: &mut BudgetGuard,
        executor: &Arc<dyn SubagentExecutor>,
        permits: &Arc<tokio::sync::Semaphore>,
        state: &mut RunState,
    ) -> Result<Option<StopReason>, LaunchError> {
        let completed: Vec<String> = state.children.iter().map(|c| c.child_id.clone()).collect();
        let mut gated: Vec<(SpawnLease, ChildJob)> = Vec::new();
        let mut stopped = None;
        for job in wave {
            match self.authorize(budget, job, &completed)? {
                Authorized::Lease(lease) => gated.push((lease, job.clone())),
                Authorized::Stopped(reason) => {
                    stopped = Some(reason);
                    break;
                }
            }
        }
        for (_, job) in &gated {
            self.announce(job, false)?;
            state.child_ids.push(job.child_id.clone());
        }
        let results = self.run_gated(gated, budget, executor, permits).await;
        let mut inner = stopped;
        self.absorb(results, budget, state, &mut inner)?;
        Ok(inner)
    }
}
