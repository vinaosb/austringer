//! Shared fixtures for the austringer-orchestration integration tests.
//!
//! Everything is hermetic: the executor is an in-process fake that journals
//! spawned effects, and the catalog is intel's seeded offline catalog.

#![allow(dead_code, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use austringer_intel::catalog::Catalog;
use austringer_intel::elicitation::TaskClass;
use austringer_intel::model_profile::ModelProfile;
use austringer_orchestration::launch::{
    ChildCapabilities, ChildFailure, ChildFailureClass, ChildJob, ChildReport, ChildTelemetry,
    LaunchConfig, Launcher, ResolvedPlan, SubagentExecutor,
};
use austringer_orchestration::role::{RoleRegistry, Tier};
use austringer_orchestration::write_set::{FileTouch, TouchOp};

/// The catalog is intel's seeded offline set — no network, ever.
pub fn catalog() -> Catalog {
    Catalog::seeded()
}

/// Neutral profiles for every model (observed-axis behavior is intel's
/// concern; the launcher must work with zero observation data).
pub fn profiles() -> BTreeMap<String, ModelProfile> {
    BTreeMap::new()
}

pub fn registry() -> RoleRegistry {
    RoleRegistry::builtin()
}

pub fn config(children: u32, parallel: u32, tokens: u64) -> LaunchConfig {
    LaunchConfig {
        budget_children: children,
        budget_parallel: parallel,
        budget_tokens: tokens,
        ..LaunchConfig::default()
    }
}

/// A spec with three agents across two waves: t1 (explorer, read-only)
/// then t2 (worker, writes) + t3 (deep, explicit bracket knobs).
pub const THREE_AGENT_SPEC: &str = r#"
# Launch spec: ship the orchestration crate
objective: Deliver the spec-driven launcher
budget:
  children: 8
  parallel: 2
  tokens: 64000
task t1:
  role: explorer
  objective: Map the module layout
  class: fact_find
task t2:
  role: worker
  objective: Implement the launcher
  writes: src/**, tests/**
  needs: t1
task t3:
  role: deep
  model: claude-opus-4-7[effort=high,context=200000]
  objective: Review the implementation
  needs: t1
"#;

/// One recorded spawn: the exact job the launcher handed the executor.
pub struct RecordedJob {
    pub job: ChildJob,
}

/// Scripted outcome for one child id.
pub enum Scripted {
    Ok(ChildReport),
    Fail(ChildFailure),
}

pub type RunHook<'a> = Box<dyn Fn(&ChildJob) + Send + 'a>;

/// In-process fake executor: records every job (the spawned-effects
/// evidence) and answers from a script keyed by child id. Children that
/// mutate the spec file mid-run are supported for stale-state tests.
pub struct FakeExecutor {
    pub jobs: Mutex<Vec<RecordedJob>>,
    pub script: Mutex<BTreeMap<String, Scripted>>,
    pub on_run: Mutex<Option<RunHook<'static>>>,
}

impl FakeExecutor {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(Vec::new()),
            script: Mutex::new(BTreeMap::new()),
            on_run: Mutex::new(None),
        }
    }

    /// Scripts a successful child: 1000 input / 500 output tokens, one
    /// touch on `out/{short}.md`, and a fixed summary.
    pub fn ok_child(&self, child_id: &str, summary: &str) {
        let short = child_id.rsplit("-child-").next().unwrap_or(child_id);
        let report = ChildReport {
            child_id: child_id.to_string(),
            summary: summary.to_string(),
            artifacts: Vec::new(),
            telemetry: ChildTelemetry {
                input_tokens: 1000,
                output_tokens: 500,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                cost_micro_usd: 10,
                duration_ms: 25,
                failure_class: None,
            },
            touches: vec![FileTouch {
                path: format!("out/{short}.md"),
                op: TouchOp::Write,
                intent: "fixture touch".to_string(),
                child: child_id.to_string(),
            }],
        };
        self.script
            .lock()
            .unwrap()
            .insert(child_id.to_string(), Scripted::Ok(report));
    }

    pub fn fail_child(&self, child_id: &str, class: ChildFailureClass) {
        self.script.lock().unwrap().insert(
            child_id.to_string(),
            Scripted::Fail(ChildFailure {
                child_id: child_id.to_string(),
                class,
                detail: "scripted failure".to_string(),
                telemetry: Some(ChildTelemetry {
                    input_tokens: 100,
                    output_tokens: 0,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    cost_micro_usd: 1,
                    duration_ms: 5,
                    failure_class: Some(class),
                }),
            }),
        );
    }

    pub fn job_count(&self) -> usize {
        self.jobs.lock().unwrap().len()
    }

    pub fn job_ids(&self) -> Vec<String> {
        self.jobs
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.job.child_id.clone())
            .collect()
    }
}

#[async_trait::async_trait]
impl SubagentExecutor for FakeExecutor {
    async fn run(&self, job: ChildJob) -> Result<ChildReport, ChildFailure> {
        if let Some(hook) = self.on_run.lock().unwrap().as_ref() {
            hook(&job);
        }
        let id = job.child_id.clone();
        self.jobs.lock().unwrap().push(RecordedJob { job });
        match self.script.lock().unwrap().remove(&id) {
            Some(Scripted::Ok(report)) => Ok(report),
            Some(Scripted::Fail(failure)) => Err(failure),
            None => Err(ChildFailure {
                child_id: id,
                class: ChildFailureClass::Tool,
                detail: "no script".to_string(),
                telemetry: None,
            }),
        }
    }
}

/// Mints fresh-authorization grants for every fresh-class child in the
/// plan (the explicit operator step the guards demand).
pub fn grant_fresh(launcher: &mut Launcher, plan: &ResolvedPlan) {
    use austringer_orchestration::authorize::SpawnAuthzClass;
    for job in plan.waves.iter().flatten() {
        if job.authz == SpawnAuthzClass::Fresh {
            launcher.mint_grant(&job.child_id, "test-operator");
        }
    }
}

/// Capabilities with both gates open (tests assert gating separately).
pub fn full_capabilities() -> ChildCapabilities {
    ChildCapabilities {
        task: true,
        todo: true,
    }
}

/// Task class helper for knob assertions.
pub fn complex() -> TaskClass {
    TaskClass::Complex
}

pub fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Explorer => "explorer",
        Tier::Worker => "worker",
        Tier::Deep => "deep",
    }
}

pub fn arc_executor(fake: FakeExecutor) -> Arc<FakeExecutor> {
    Arc::new(fake)
}
