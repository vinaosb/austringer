//! PLAN-BRANCH model comparison (Plandex): the same spec run to
//! completion on different models via branches, compared per model.

use std::collections::BTreeMap;
use std::sync::Arc;

use austringer_intel::catalog::Catalog;
use austringer_intel::model_profile::ModelProfile;

use crate::events::{MemoryEventSink, TeamEventSink};
use crate::journal::TeamJournal;
use crate::knobs::ModelBracket;
use crate::launch::{
    LaunchConfig, LaunchOutcome, Launcher, ResolvedPlan, SubagentExecutor, resolve,
};
use crate::role::RoleRegistry;
use crate::variants::VariantError;

/// One plan-branch run (same spec, one model).
#[derive(Debug, Clone)]
pub struct BranchRun {
    /// Branch model id.
    pub model: String,
    /// Children completed.
    pub children_completed: usize,
    /// Children failed.
    pub children_failed: usize,
    /// Ran to completion.
    pub complete: bool,
}

/// Plan-branch comparison report.
#[derive(Debug, Clone)]
pub struct PlanBranchReport {
    /// One run per branch model.
    pub per_model: Vec<BranchRun>,
}

/// A plan-branch configuration: the spec plus the branch models.
pub struct PlanBranch {
    spec: crate::spec::LaunchSpec,
    models: Vec<ModelBracket>,
}

impl PlanBranch {
    /// Stores the spec and branch models.
    pub fn new(spec: crate::spec::LaunchSpec, models: Vec<ModelBracket>) -> Self {
        Self { spec, models }
    }
}

/// Runs the same spec once per branch model and compares them.
#[allow(clippy::too_many_arguments)]
pub async fn run_plan_branch(
    branch: &PlanBranch,
    prefix: &str,
    registry: &RoleRegistry,
    catalog: &Catalog,
    profiles: &BTreeMap<String, ModelProfile>,
    executor: Arc<dyn SubagentExecutor>,
    config: LaunchConfig,
    sink: Arc<dyn TeamEventSink>,
) -> Result<PlanBranchReport, VariantError> {
    let mut per_model = Vec::new();
    for bracket in &branch.models {
        let mut spec = branch.spec.clone();
        for task in &mut spec.tasks {
            task.model = Some(bracket.model.clone());
        }
        let launch_id = format!("{prefix}-{}", bracket.model);
        let plan: ResolvedPlan = resolve(&launch_id, &spec, registry, catalog, profiles, &config)
            .map_err(|e| VariantError::ApplyIo {
            path: launch_id.clone(),
            detail: e.to_string(),
        })?;
        let mut launcher = Launcher::new(
            config.clone(),
            Arc::new(MemoryEventSink::new()),
            TeamJournal::in_memory(),
        );
        for job in plan.waves.iter().flatten() {
            if job.authz == crate::authorize::SpawnAuthzClass::Fresh {
                launcher.mint_grant(&job.child_id, "plan-branch");
            }
        }
        let report = launcher
            .run(&plan, Arc::clone(&executor), None)
            .await
            .map_err(|e| VariantError::ApplyIo {
                path: launch_id.clone(),
                detail: e.to_string(),
            })?;
        per_model.push(BranchRun {
            model: bracket.model.clone(),
            children_completed: report.children.len(),
            children_failed: report.failures.len(),
            complete: report.outcome == LaunchOutcome::Complete,
        });
        let _ = &sink;
    }
    Ok(PlanBranchReport { per_model })
}
