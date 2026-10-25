//! BEST-OF-N multi-version runs (Kilo Agent Manager: up to 4 parallel
//! same-prompt implementations with per-version models; the winner's DIFF
//! is applied from its worktree, not just reported — Qwen Arena) and
//! PLAN-BRANCH model comparison (Plandex: the same spec on different
//! models via branches).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use austringer_intel::catalog::Catalog;
use austringer_intel::elicitation::TaskClass;
use austringer_intel::model_profile::ModelProfile;

use crate::events::{TeamEvent, TeamEventSink};
use crate::journal::{JournalRecord, TeamJournal};
use crate::knobs::{ModelBracket, resolve_knobs};
use crate::launch::{ChildFailure, ChildJob, ChildReport, SubagentExecutor};
use crate::role::{RoleDef, Tier};
use crate::telemetry::clamp_summary;

/// Hard cap on parallel versions.
pub const MAX_BEST_OF_N: usize = 4;

/// Pluggable winner scoring.
pub type Scorer = Arc<dyn Fn(&ChildReport) -> u32 + Send + Sync>;

/// Variant-layer failure.
#[derive(Debug, thiserror::Error)]
pub enum VariantError {
    /// More versions than the cap.
    #[error("best-of-N over cap: {n} > {max}")]
    TooManyVersions {
        /// Requested versions.
        n: usize,
        /// The cap.
        max: usize,
    },
    /// No versions declared.
    #[error("no versions declared")]
    NoVersions,
    /// Knob resolution failed for a version's model.
    #[error("model `{model}` failed resolution: {source}")]
    Catalog {
        /// Model id.
        model: String,
        /// The catalog error.
        source: austringer_intel::catalog::CatalogError,
    },
    /// Copying the winner's diff failed.
    #[error("apply failed for {path}: {detail}")]
    ApplyIo {
        /// Artifact path.
        path: String,
        /// Detail.
        detail: String,
    },
    /// The winner's bytes do not match its declared hash.
    #[error("hash mismatch applying {path}")]
    HashMismatch {
        /// Artifact path.
        path: String,
    },
}

/// One best-of-N variant run.
#[derive(Debug, Clone)]
pub struct VariantRun {
    /// Child id.
    pub child_id: String,
    /// Version's model id.
    pub model: String,
    /// Score.
    pub score: u32,
    /// Report (successful runs).
    pub report: Option<ChildReport>,
    /// Failure (failed runs).
    pub failure: Option<ChildFailure>,
}

/// Best-of-N result.
#[derive(Debug, Clone)]
pub struct BestOfNReport {
    /// All version runs.
    pub runs: Vec<VariantRun>,
    /// Winning child id (highest score above zero).
    pub winner: Option<String>,
}

/// A best-of-N configuration.
pub struct BestOfN {
    prompt: String,
    versions: Vec<ModelBracket>,
    scorer: Scorer,
}

impl BestOfN {
    /// Validates the version count and stores the prompt/models/scorer.
    pub fn new(
        prompt: &str,
        versions: Vec<ModelBracket>,
        scorer: Scorer,
    ) -> Result<Self, VariantError> {
        if versions.is_empty() {
            return Err(VariantError::NoVersions);
        }
        if versions.len() > MAX_BEST_OF_N {
            return Err(VariantError::TooManyVersions {
                n: versions.len(),
                max: MAX_BEST_OF_N,
            });
        }
        Ok(Self {
            prompt: prompt.to_string(),
            versions,
            scorer,
        })
    }
}

fn version_job(prefix: &str, index: usize, knobs: crate::knobs::ChildKnobs) -> ChildJob {
    let worker = RoleDef::builtin(Tier::Worker);
    ChildJob {
        child_id: format!("{prefix}-child-v{index}"),
        role: worker.name,
        tier: worker.tier,
        knobs,
        objective: String::new(),
        fork: crate::fork::fork_child("", &crate::fork::ToolSurface::default(), &worker.tools),
        writes: crate::write_set::WriteSet::default(),
        policy: worker.policy,
        capabilities: crate::authorize::ChildCapabilities::default(),
        deny_rules: Vec::new(),
        authz: crate::authorize::SpawnAuthzClass::Standing,
        escalation: Arc::new(crate::authorize::ParentEscalations::new()),
        summary_budget: worker.summary_budget,
        context_warning: None,
    }
}

/// Runs every version concurrently, scores them, and selects the winner.
#[allow(clippy::too_many_arguments)]
pub async fn run_best_of_n(
    best: &BestOfN,
    prefix: &str,
    catalog: &Catalog,
    profiles: &BTreeMap<String, ModelProfile>,
    executor: Arc<dyn SubagentExecutor>,
    sink: Arc<dyn TeamEventSink>,
    mut journal: TeamJournal,
) -> Result<BestOfNReport, VariantError> {
    let mut jobs = Vec::new();
    for (index, bracket) in best.versions.iter().enumerate() {
        let profile = profiles.get(&bracket.model).cloned().unwrap_or_else(|| {
            ModelProfile::neutral(austringer_providers::identity::ModelIdentity::new(
                "any",
                &bracket.model,
            ))
        });
        let knobs =
            resolve_knobs(bracket, TaskClass::Complex, catalog, &profile).map_err(|source| {
                VariantError::Catalog {
                    model: bracket.model.clone(),
                    source: source.0,
                }
            })?;
        let mut job = version_job(prefix, index, knobs);
        job.objective = best.prompt.clone();
        jobs.push(job);
    }

    for job in &jobs {
        let spec = serde_json::json!({
            "model": job.knobs.model.model,
            "effort": job.knobs.effort.to_string(),
            "variant": true,
        })
        .to_string();
        journal
            .record(JournalRecord::ChildStarted {
                child: job.child_id.clone(),
                spec,
                resumed: false,
            })
            .map_err(|e| VariantError::ApplyIo {
                path: String::new(),
                detail: e.to_string(),
            })?;
        sink.emit(TeamEvent::Spawned {
            child: job.child_id.clone(),
            role: job.role.clone(),
            model: job.knobs.model.model.clone(),
            effort: job.knobs.effort.to_string(),
        });
    }

    let mut set = tokio::task::JoinSet::new();
    for job in jobs {
        let executor = Arc::clone(&executor);
        set.spawn(async move { executor.run(job).await });
    }
    let mut by_id: BTreeMap<String, Result<ChildReport, ChildFailure>> = BTreeMap::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(result) = joined {
            if let Ok(report) = &result {
                by_id.insert(report.child_id.clone(), result);
            } else if let Err(failure) = &result {
                by_id.insert(failure.child_id.clone(), result);
            }
        }
    }

    let mut runs = Vec::new();
    let mut winner: Option<(String, u32)> = None;
    for (index, bracket) in best.versions.iter().enumerate() {
        let child_id = format!("{prefix}-child-v{index}");
        match by_id.remove(&child_id) {
            Some(Ok(mut report)) => {
                report.summary = clamp_summary(&report.summary, crate::telemetry::SUMMARY_MAX);
                let score = (best.scorer)(&report);
                if score > 0 && winner.as_ref().is_none_or(|(_, s)| score > *s) {
                    winner = Some((child_id.clone(), score));
                }
                runs.push(VariantRun {
                    child_id,
                    model: bracket.model.clone(),
                    score,
                    report: Some(report),
                    failure: None,
                });
            }
            Some(Err(failure)) => {
                runs.push(VariantRun {
                    child_id,
                    model: bracket.model.clone(),
                    score: 0,
                    report: None,
                    failure: Some(failure),
                });
            }
            None => {
                runs.push(VariantRun {
                    child_id,
                    model: bracket.model.clone(),
                    score: 0,
                    report: None,
                    failure: None,
                });
            }
        }
    }

    if let Some((child, score)) = &winner {
        sink.emit(TeamEvent::BestOfNSelected {
            winner: child.clone(),
            score: *score,
        });
    }
    Ok(BestOfNReport {
        runs,
        winner: winner.map(|(child, _)| child),
    })
}

/// One applied winner artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedDiff {
    /// Artifact path.
    pub path: String,
    /// Verified content hash.
    pub content_hash: String,
}

/// Applies the winner's DIFF: copies each artifact's source bytes into the
/// shared scope, verifying the declared hash along the way.
pub fn apply_winner(
    report: &ChildReport,
    scope_root: &Path,
) -> Result<Vec<AppliedDiff>, VariantError> {
    let mut applied = Vec::new();
    for artifact in &report.artifacts {
        let bytes = std::fs::read(&artifact.source).map_err(|e| VariantError::ApplyIo {
            path: artifact.path.clone(),
            detail: e.to_string(),
        })?;
        let hash = blake3::hash(&bytes).to_hex().to_string();
        if hash != artifact.content_hash {
            return Err(VariantError::HashMismatch {
                path: artifact.path.clone(),
            });
        }
        let target = scope_root.join(&artifact.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| VariantError::ApplyIo {
                path: artifact.path.clone(),
                detail: e.to_string(),
            })?;
        }
        std::fs::write(&target, &bytes).map_err(|e| VariantError::ApplyIo {
            path: artifact.path.clone(),
            detail: e.to_string(),
        })?;
        applied.push(AppliedDiff {
            path: artifact.path.clone(),
            content_hash: hash,
        });
    }
    Ok(applied)
}

pub use crate::plan_branch::{BranchRun, PlanBranch, PlanBranchReport, run_plan_branch};
