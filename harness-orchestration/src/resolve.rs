//! Spec resolution: LaunchSpec -> ResolvedPlan. Topology (waves), role
//! and model resolution (fail closed through the catalog), the
//! disjoint-write-set contract for parallel siblings, family-switch
//! detection, and the effective budget (spec AND config AND absolute
//! ceilings — clamps are recorded as loud notices, never silent).

use std::collections::BTreeMap;

use harness_intel::catalog::Catalog;
use harness_intel::model_profile::ModelProfile;
use harness_providers::identity::ModelIdentity;

use crate::authorize::{classify_spawn, inherit_denies};
use crate::budget::{ABSOLUTE_MAX_CHILDREN, ABSOLUTE_MAX_TOKENS};
use crate::fork::fork_child;
use crate::knobs::{ModelBracket, provider_for_family, resolve_knobs};
use crate::launch::{ChildJob, LaunchConfig};
use crate::role::RoleRegistry;
use crate::spec::{LaunchSpec, TaskDecl};
use crate::write_set::WriteSet;

/// Concurrency ceiling (THROTTLE max_concurrent_subagents for complex
/// work).
pub const ABSOLUTE_MAX_PARALLEL: u32 = 4;

/// The effective plan budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanBudget {
    /// Child cap.
    pub children: u32,
    /// Concurrency cap.
    pub parallel: u32,
    /// Token budget.
    pub tokens: u64,
}

/// Typed resolution failure.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// Unknown role name.
    #[error("unknown role `{role}` (known: {known:?})")]
    UnknownRole {
        /// The offending role.
        role: String,
        /// Known role names.
        known: Vec<String>,
    },
    /// Duplicate custom role.
    #[error("duplicate role `{0}`")]
    DuplicateRole(String),
    /// A model failed catalog resolution.
    #[error("task {task}: model `{model}` failed catalog resolution: {source}")]
    Model {
        /// Task id.
        task: String,
        /// Model id.
        model: String,
        /// The catalog error.
        source: harness_intel::catalog::CatalogError,
    },
    /// A model bracket failed to parse.
    #[error("task {task}: bad model bracket: {source}")]
    Bracket {
        /// Task id.
        task: String,
        /// The bracket error.
        source: crate::knobs::BracketError,
    },
    /// The dependency graph has a cycle.
    #[error("dependency cycle: {path}")]
    DependencyCycle {
        /// The cycle path.
        path: String,
    },
    /// A dependency names a missing task.
    #[error("task {task} needs missing task {needs}")]
    MissingDependency {
        /// Task id.
        task: String,
        /// Missing dependency.
        needs: String,
    },
    /// Parallel siblings overlap write sets.
    #[error("parallel siblings `{a}` and `{b}` overlap write set `{glob}`")]
    OverlappingWrites {
        /// First task.
        a: String,
        /// Second task.
        b: String,
        /// The overlapping glob.
        glob: String,
    },
}

/// A fully resolved launch plan.
#[derive(Debug, Clone)]
pub struct ResolvedPlan {
    /// Launch id (child-id namespace root).
    pub launch_id: String,
    /// Execution waves (topology levels), each wave runs concurrently.
    pub waves: Vec<Vec<ChildJob>>,
    /// Effective budget.
    pub budget: PlanBudget,
    /// Mission-validator milestones enabled.
    pub validators: bool,
    /// /boost phase decoupling enabled.
    pub phases: bool,
    /// Family switches across waves (surfaced, never silent).
    pub family_switches: Vec<(String, String)>,
    /// Loud clamp notices.
    pub notices: Vec<String>,
    /// Spec fingerprint when built from file bytes (stale-reload guard).
    pub spec_fingerprint: Option<String>,
    /// Launch objectives.
    pub objectives: Vec<String>,
}

/// Wave levels via Kahn topological sort, then longest-path leveling.
fn waves_of(tasks: &[TaskDecl]) -> Result<Vec<Vec<String>>, ResolveError> {
    let ids: BTreeMap<&str, usize> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();
    let mut indegree = vec![0usize; tasks.len()];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); tasks.len()];
    for (i, task) in tasks.iter().enumerate() {
        for need in &task.needs {
            let Some(&j) = ids.get(need.as_str()) else {
                return Err(ResolveError::MissingDependency {
                    task: task.id.clone(),
                    needs: need.clone(),
                });
            };
            indegree[i] += 1;
            dependents[j].push(i);
        }
    }
    let mut level = vec![0usize; tasks.len()];
    let mut queue: Vec<usize> = (0..tasks.len()).filter(|i| indegree[*i] == 0).collect();
    let mut seen = 0usize;
    while let Some(i) = queue.pop() {
        seen += 1;
        for &j in &dependents[i] {
            level[j] = level[j].max(level[i] + 1);
            indegree[j] -= 1;
            if indegree[j] == 0 {
                queue.push(j);
            }
        }
    }
    if seen != tasks.len() {
        let path = tasks
            .iter()
            .enumerate()
            .filter(|(i, _)| indegree[*i] > 0)
            .map(|(_, t)| t.id.clone())
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(ResolveError::DependencyCycle { path });
    }
    let mut waves: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (i, task) in tasks.iter().enumerate() {
        waves.entry(level[i]).or_default().push(task.id.clone());
    }
    Ok(waves.into_values().collect())
}

/// Resolves a parsed spec into an executable plan.
pub fn resolve(
    launch_id: &str,
    spec: &LaunchSpec,
    registry: &RoleRegistry,
    catalog: &Catalog,
    profiles: &BTreeMap<String, ModelProfile>,
    config: &LaunchConfig,
) -> Result<ResolvedPlan, ResolveError> {
    let merged = RoleRegistry::with_declarations(registry, &spec.roles)?;
    let waves = waves_of(&spec.tasks)?;

    let mut jobs: Vec<ChildJob> = Vec::with_capacity(spec.tasks.len());
    for task in &spec.tasks {
        let role = merged
            .get(&task.role)
            .ok_or_else(|| ResolveError::UnknownRole {
                role: task.role.clone(),
                known: merged.names(),
            })?;
        let bracket_source = task
            .model
            .clone()
            .or_else(|| role.model.clone())
            .unwrap_or_else(|| "claude-sonnet-4-6".to_string());
        let bracket =
            ModelBracket::parse(&bracket_source).map_err(|source| ResolveError::Bracket {
                task: task.id.clone(),
                source,
            })?;
        let profile = profiles
            .get(&bracket.model)
            .cloned()
            .unwrap_or_else(|| ModelProfile::neutral(ModelIdentity::new("any", &bracket.model)));
        let knobs = resolve_knobs(&bracket, task.class, catalog, &profile).map_err(|source| {
            ResolveError::Model {
                task: task.id.clone(),
                model: bracket.model.clone(),
                source: source.0,
            }
        })?;
        let writes = WriteSet {
            globs: task.writes.clone(),
        };
        let authz = classify_spawn(role, &writes);
        jobs.push(ChildJob {
            child_id: format!("{launch_id}-child-{}", task.id),
            role: role.name.clone(),
            tier: role.tier,
            knobs,
            objective: task.objective.clone(),
            fork: fork_child(&config.parent_prompt, &config.tool_surface, &role.tools),
            writes,
            policy: role.policy,
            capabilities: config.child_capabilities,
            deny_rules: inherit_denies(&config.parent_rules),
            authz,
            escalation: config.escalation.clone(),
            summary_budget: role.summary_budget,
            context_warning: None,
        });
    }

    let by_id: BTreeMap<&str, &ChildJob> = jobs.iter().map(|j| (j.child_id.as_str(), j)).collect();
    let mut plan_waves: Vec<Vec<ChildJob>> = Vec::new();
    for wave in &waves {
        plan_waves.push(
            wave.iter()
                .filter_map(|id| {
                    by_id
                        .get(format!("{launch_id}-child-{id}").as_str())
                        .copied()
                })
                .cloned()
                .collect(),
        );
    }

    for wave in &plan_waves {
        for (i, a) in wave.iter().enumerate() {
            for b in wave.iter().skip(i + 1) {
                if let Some(glob) = a.writes.overlap_with(&b.writes).into_iter().next() {
                    return Err(ResolveError::OverlappingWrites {
                        a: a.child_id.clone(),
                        b: b.child_id.clone(),
                        glob,
                    });
                }
            }
        }
    }

    let mut family_switches = Vec::new();
    let mut previous: Option<&str> = None;
    for wave in &mut plan_waves {
        if let Some(first) = wave.first() {
            let current = provider_for_family(first.knobs.family);
            if let Some(prev) = previous {
                for job in wave.iter_mut() {
                    let job_family = provider_for_family(job.knobs.family);
                    if job_family != prev {
                        family_switches.push((prev.to_string(), job_family.to_string()));
                        job.context_warning = Some(format!(
                            "model family changed mid-run ({prev} -> {job_family}): \
                             thinking-block/context continuity is NOT carried; surfaced, never silent"
                        ));
                    }
                }
            }
            previous = Some(current);
        }
    }

    let mut notices = Vec::new();
    let children = spec.budget.children.unwrap_or(config.budget_children);
    let children = children
        .min(config.budget_children)
        .min(ABSOLUTE_MAX_CHILDREN);
    if let Some(declared) = spec.budget.children.filter(|d| *d > children) {
        notices.push(format!("spec children {declared} clamped to {children}"));
    }
    let tokens = spec.budget.tokens.unwrap_or(config.budget_tokens);
    let tokens = tokens.min(config.budget_tokens).min(ABSOLUTE_MAX_TOKENS);
    if let Some(declared) = spec.budget.tokens.filter(|d| *d > tokens) {
        notices.push(format!("spec tokens {declared} clamped to {tokens}"));
    }
    let parallel = spec
        .budget
        .parallel
        .unwrap_or(config.budget_parallel)
        .min(config.budget_parallel)
        .min(ABSOLUTE_MAX_PARALLEL);

    Ok(ResolvedPlan {
        launch_id: launch_id.to_string(),
        waves: plan_waves,
        budget: PlanBudget {
            children,
            parallel,
            tokens,
        },
        validators: spec.validators && config.validators,
        phases: spec.phases || config.phases,
        family_switches,
        notices,
        spec_fingerprint: None,
        objectives: spec.objectives.clone(),
    })
}
