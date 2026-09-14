//! The plan runner: waves, spec-freshness checks, phase decoupling,
//! validator milestones, and task_id resumption.

use std::sync::Arc;

use crate::authorize::SpawnAuthzClass;
use crate::budget::BudgetGuard;
use crate::events::TeamEvent;
use crate::journal::{JournalError, JournalRecord, fingerprint_bytes};
use crate::launch::{ChildJob, ChildTelemetry, Launcher, RunState, SubagentExecutor};
use crate::phases::{BoostPhase, CleanContextReview, validator_jobs};
use crate::report::{LaunchError, LaunchOutcome, LaunchReport, SpecSource, StopReason};
use crate::resolve::ResolvedPlan;
use crate::role::Tier;
use crate::write_set::TouchRegistry;

impl Launcher {
    pub(crate) fn announce(&mut self, job: &ChildJob, resumed: bool) -> Result<(), JournalError> {
        self.jr(JournalRecord::ChildStarted {
            child: job.child_id.clone(),
            spec: Self::spawn_spec(job),
            resumed,
        })?;
        self.emit(TeamEvent::Spawned {
            child: job.child_id.clone(),
            role: job.role.clone(),
            model: job.knobs.model.model.clone(),
            effort: job.knobs.effort.to_string(),
        });
        Ok(())
    }

    fn spawn_spec(job: &ChildJob) -> String {
        serde_json::json!({
            "model": job.knobs.model.model,
            "provider": job.knobs.model.provider,
            "effort": job.knobs.effort.to_string(),
            "role": job.role,
            "tier": job.tier.name(),
            "writes": job.writes.globs,
        })
        .to_string()
    }

    /// Runs the plan. Checks spec freshness before every wave when a
    /// source is supplied (fail closed on mid-run mutation).
    pub async fn run(
        &mut self,
        plan: &ResolvedPlan,
        executor: Arc<dyn SubagentExecutor>,
        spec_source: Option<&SpecSource>,
    ) -> Result<LaunchReport, LaunchError> {
        let mut state = RunState {
            children: Vec::new(),
            failures: Vec::new(),
            child_ids: Vec::new(),
            touches: TouchRegistry::new(),
        };
        let mut stopped: Option<StopReason> = None;
        let mut phase_trace: Vec<String> = Vec::new();
        let mut waves_executed = 0u32;
        let mut budget = BudgetGuard::new(
            plan.budget.children,
            plan.budget.parallel,
            plan.budget.tokens,
        );
        let permits = Arc::new(tokio::sync::Semaphore::new(plan.budget.parallel as usize));

        let baseline = match spec_source {
            Some(source) => {
                let bytes = source.read().map_err(|e| LaunchError::Io(e.to_string()))?;
                Some(fingerprint_bytes(&bytes))
            }
            None => None,
        };

        for (from, to) in &plan.family_switches {
            self.jr(JournalRecord::FamilySwitch {
                from: from.clone(),
                to: to.clone(),
            })?;
            self.emit(TeamEvent::FamilySwitch {
                from: from.clone(),
                to: to.clone(),
            });
        }

        if plan.phases {
            phase_trace.push(BoostPhase::Strategy.name().to_string());
            self.emit(TeamEvent::PhaseTransition {
                phase: BoostPhase::Strategy.name().to_string(),
            });
            if let Some(first) = plan.waves.iter().flatten().next() {
                let mut strategy = first.clone();
                strategy.child_id = format!("{}-child-strategy", plan.launch_id);
                strategy.role = "strategy".to_string();
                strategy.tier = Tier::Explorer;
                strategy.writes = crate::write_set::WriteSet::default();
                strategy.authz = SpawnAuthzClass::Standing;
                strategy.objective =
                    format!("Produce a strategy for: {}", plan.objectives.join("; "));
                stopped = self
                    .run_single(strategy, &mut budget, &executor, &permits, &mut state)
                    .await?;
            }
            phase_trace.push(BoostPhase::Execution.name().to_string());
        }

        let last_wave = plan.waves.len().saturating_sub(1);
        for (index, wave) in plan.waves.iter().enumerate() {
            if stopped.is_some() {
                break;
            }
            if index > 0
                && let (Some(source), Some(base)) = (spec_source, &baseline)
            {
                match source.read() {
                    Ok(bytes) => {
                        let fp = fingerprint_bytes(&bytes);
                        if fp != *base {
                            self.jr(JournalRecord::SpecStale {
                                expected: base.clone(),
                                actual: fp.clone(),
                            })?;
                            self.emit(TeamEvent::SpecStale {
                                expected: base.clone(),
                                actual: fp.clone(),
                            });
                            stopped = Some(StopReason::StaleSpec {
                                expected: base.clone(),
                                actual: fp,
                            });
                            break;
                        }
                    }
                    Err(e) => return Err(LaunchError::Io(e.to_string())),
                }
            }

            waves_executed += 1;
            let ids: Vec<String> = wave.iter().map(|j| j.child_id.clone()).collect();
            self.jr(JournalRecord::WaveStarted {
                index: index as u32,
                children: ids.clone(),
            })?;
            self.emit(TeamEvent::WaveStarted {
                index: index as u32,
                children: ids,
            });

            stopped = self
                .execute_wave(wave, &mut budget, &executor, &permits, &mut state)
                .await?;

            if plan.validators && index < last_wave && stopped.is_none() {
                let wave_refs: Vec<&ChildJob> = wave.iter().collect();
                for (_, validator) in validator_jobs(&wave_refs, &plan.launch_id) {
                    if stopped.is_some() {
                        break;
                    }
                    stopped = self
                        .run_single(validator, &mut budget, &executor, &permits, &mut state)
                        .await?;
                }
            }
            for id in wave.iter().map(|j| j.child_id.clone()) {
                state.touches.retire(&id);
            }
        }

        if plan.phases && stopped.is_none() && !state.children.is_empty() {
            phase_trace.push(BoostPhase::Verification.name().to_string());
            self.emit(TeamEvent::PhaseTransition {
                phase: BoostPhase::Verification.name().to_string(),
            });
            let manifest = state
                .children
                .iter()
                .flat_map(|c| {
                    c.artifacts
                        .iter()
                        .map(|a| format!("{} {}", a.path, a.content_hash))
                })
                .collect::<Vec<_>>()
                .join("\n");
            let review = CleanContextReview::from_artifacts(&manifest, &plan.objectives.join(" "));
            let reviewer = review.reviewer_job(&plan.launch_id, crate::telemetry::SUMMARY_MAX);
            if let Some(base) = plan.waves.iter().flatten().next() {
                let mut job = base.clone();
                job.child_id = reviewer.child_id;
                job.role = "reviewer".to_string();
                job.tier = Tier::Explorer;
                job.objective = reviewer.objective;
                job.fork = reviewer.fork;
                job.writes = crate::write_set::WriteSet::default();
                job.authz = SpawnAuthzClass::Standing;
                job.summary_budget = reviewer.summary_budget;
                self.run_single(job, &mut budget, &executor, &permits, &mut state)
                    .await?;
            }
        }

        let telemetry: Vec<ChildTelemetry> = state.children.iter().map(|c| c.telemetry).collect();
        let rollup = crate::telemetry::rollup(&telemetry);
        let synthesis =
            (!state.children.is_empty()).then(|| crate::synthesis::synthesize(&state.children));
        let complete = stopped.is_none();
        let outcome = if complete {
            LaunchOutcome::Complete
        } else {
            LaunchOutcome::Partial
        };
        self.jr(JournalRecord::LaunchFinished {
            summary: format!(
                "{} children, {} failures",
                state.children.len(),
                state.failures.len()
            ),
            complete,
        })?;
        self.emit(TeamEvent::LaunchFinished {
            children: state.children.len(),
            complete,
        });

        Ok(LaunchReport {
            launch_id: plan.launch_id.clone(),
            outcome,
            children: state.children,
            failures: state.failures,
            stopped_early: stopped,
            waves_executed,
            family_switches: plan.family_switches.clone(),
            synthesis,
            child_ids: state.child_ids,
            phase_trace,
            rollup,
        })
    }
}
