//! Resolution (plan todo 42): turning a [`RouteRequest`] into a concrete
//! [`RouteResolution`] (identity + explicit knobs, validated fail-closed
//! against the catalog), complexity-tier and benchmark-driven selection,
//! per-task profile stickiness, and profile-aware suggestions.

use std::collections::BTreeMap;

use harness_providers::identity::ModelIdentity;

use super::config::BenchmarkEntrySpec;
use super::table::{RoleRoute, RoutingTable, TierRoute};
use super::{Role, RouteRequest, Tier};
use crate::catalog::{CatalogError, EffortValue, WireKnobs};

/// Everything resolution can fail with (all typed, all fail-closed).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ResolutionError {
    /// The role has no route in the config.
    #[error("role {role:?} has no route configured — add it before dispatching")]
    UnconfiguredRole {
        /// The unconfigured role.
        role: Role,
    },
    /// The tier has no route in the config.
    #[error("tier {tier:?} has no route configured — add it before dispatching")]
    UnconfiguredTier {
        /// The unconfigured tier.
        tier: Tier,
    },
    /// Benchmark routing found no proven model at the tier's floor.
    #[error(
        "no benchmark-proven model meets the {tier:?} score floor — refusing to route to an unproven model"
    )]
    NoBenchmarkCandidate {
        /// The tier whose floor nothing met.
        tier: Tier,
    },
    /// Catalog validation rejected the resolved knobs.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

/// Why a resolution points where it points.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionSource {
    /// A configured role route.
    Role(Role),
    /// A complexity-tier default.
    Tier(Tier),
    /// Benchmark-driven live routing: cheapest proven model at the tier's
    /// floor, at the tier's effort.
    Benchmark {
        /// The tier whose floor was applied.
        tier: Tier,
        /// The winning model.
        model: String,
        /// The winning model's list price per million tokens.
        price_per_mtok: f64,
    },
    /// A per-task pin (launch profile frozen by stickiness).
    Pinned,
}

/// One resolved route: the identity, the EXPLICIT effort, and the knob
/// set a wire request must carry.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteResolution {
    /// The routed identity.
    pub model: ModelIdentity,
    /// The explicit effort rung.
    pub effort: EffortValue,
    /// The knobs to set on every request under this route.
    pub knobs: WireKnobs,
    /// Why this route.
    pub source: ResolutionSource,
    /// Whether the router (vs the user's config) picked the model.
    pub auto_routed: bool,
}

/// Resolves a request against the table. Knobs are validated against the
/// catalog before the resolution exists (fail closed, exactly like a
/// direct wire request).
pub fn resolve(
    table: &RoutingTable,
    request: RouteRequest,
) -> Result<RouteResolution, ResolutionError> {
    match request {
        RouteRequest::Role(role) => {
            let route = role_route(table, role)?;
            to_resolution(
                table,
                &route.model,
                route.effort,
                ResolutionSource::Role(role),
                false,
            )
        }
        RouteRequest::Tier(tier) => {
            let route = tier_route(table, tier)?;
            to_resolution(
                table,
                &route.model,
                route.effort,
                ResolutionSource::Tier(tier),
                true,
            )
        }
        RouteRequest::Benchmarked(tier) => {
            let route = tier_route(table, tier)?;
            let winner = cheapest_proven(table, tier, route.score_floor)?;
            to_resolution(
                table,
                &winner.model,
                route.effort,
                ResolutionSource::Benchmark {
                    tier,
                    model: winner.model.clone(),
                    price_per_mtok: winner.price_per_mtok,
                },
                true,
            )
        }
    }
}

fn role_route(table: &RoutingTable, role: Role) -> Result<&RoleRoute, ResolutionError> {
    table
        .roles
        .get(&role)
        .ok_or(ResolutionError::UnconfiguredRole { role })
}

fn tier_route(table: &RoutingTable, tier: Tier) -> Result<&TierRoute, ResolutionError> {
    table
        .tiers
        .get(&tier)
        .ok_or(ResolutionError::UnconfiguredTier { tier })
}

fn to_resolution(
    table: &RoutingTable,
    model: &str,
    effort: EffortValue,
    source: ResolutionSource,
    auto_routed: bool,
) -> Result<RouteResolution, ResolutionError> {
    let knobs = WireKnobs {
        effort: Some(effort),
        ..WireKnobs::default()
    };
    table.catalog.validate(model, &knobs)?;
    Ok(RouteResolution {
        model: ModelIdentity::new(&table.provider, model),
        effort,
        knobs,
        source,
        auto_routed,
    })
}

/// Cheapest benchmark-proven model meeting the tier floor. Ties break by
/// model id ascending — deterministic, never provider round-robin.
fn cheapest_proven(
    table: &RoutingTable,
    tier: Tier,
    floor: f32,
) -> Result<&BenchmarkEntrySpec, ResolutionError> {
    let mut candidates: Vec<&BenchmarkEntrySpec> = table
        .benchmarks
        .iter()
        .filter(|entry| entry.score >= floor)
        .collect();
    candidates.sort_by(|a, b| {
        a.price_per_mtok
            .total_cmp(&b.price_per_mtok)
            .then_with(|| a.model.cmp(&b.model))
    });
    candidates
        .into_iter()
        .next()
        .ok_or(ResolutionError::NoBenchmarkCandidate { tier })
}

/// One task's identity in the stickiness map.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(String);

impl TaskId {
    /// Creates a task id.
    pub fn new(id: &str) -> Self {
        Self(id.to_owned())
    }

    /// The id as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The launch profile frozen for one task: the original request plus its
/// resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct PinnedProfile {
    /// The request the task launched under.
    pub request: RouteRequest,
    /// The frozen resolution.
    pub resolution: RouteResolution,
}

/// A re-pin of a pinned task to a DIFFERENT profile (silent model drift)
/// — fail closed; unpin first for a deliberate change.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error(
    "task {task:?} pinned to {pinned_model:?} refuses re-route to {requested_model:?} — unpin for a deliberate change"
)]
pub struct PinConflict {
    /// The pinned task.
    pub task: TaskId,
    /// The pinned launch model.
    pub pinned_model: String,
    /// The model the re-route wanted.
    pub requested_model: String,
}

/// A subtask was spawned under an unknown parent.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("unknown parent task {task:?} — nothing to inherit")]
pub struct UnknownTask {
    /// The missing parent.
    pub task: TaskId,
}

/// PER-TASK PROFILE STICKINESS (Roo): a task pins its launch profile
/// across resume, and subtasks inherit the parent's pin.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stickiness {
    pins: BTreeMap<TaskId, PinnedProfile>,
}

impl Stickiness {
    /// An empty stickiness map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pins a task's launch profile. Re-pinning a pinned task to a
    /// different model is a typed [`PinConflict`].
    pub fn pin(&mut self, task: TaskId, profile: PinnedProfile) -> Result<(), PinConflict> {
        if let Some(existing) = self.pins.get(&task)
            && existing.resolution.model != profile.resolution.model
        {
            return Err(PinConflict {
                pinned_model: existing.resolution.model.model.clone(),
                requested_model: profile.resolution.model.model.clone(),
                task,
            });
        }
        self.pins.insert(task, profile);
        Ok(())
    }

    /// Pins `profile` only when the task is unpinned; always returns the
    /// EFFECTIVE profile (the stored pin when one exists).
    #[must_use]
    pub fn launch(&mut self, task: TaskId, profile: PinnedProfile) -> PinnedProfile {
        self.pins.entry(task).or_insert(profile).clone()
    }

    /// The task's pinned profile (resume path).
    #[must_use]
    pub fn resolve(&self, task: &TaskId) -> Option<&PinnedProfile> {
        self.pins.get(task)
    }

    /// A subtask inherits its parent's pin.
    pub fn spawn_subtask(&mut self, parent: &TaskId, child: TaskId) -> Result<(), UnknownTask> {
        let Some(inherited) = self.pins.get(parent).cloned() else {
            return Err(UnknownTask {
                task: parent.clone(),
            });
        };
        self.pins.entry(child).or_insert(inherited);
        Ok(())
    }

    /// Removes a task's pin (the deliberate re-route path). Returns
    /// whether a pin existed.
    pub fn unpin(&mut self, task: &TaskId) -> bool {
        self.pins.remove(task).is_some()
    }
}

/// Weakness gate for profile-aware suggestions (observed-bias threshold
/// mirrors the T34 elicitation's edit-rejection gate).
const EDIT_REJECTION_SUGGESTION_FLOOR: f32 = 0.25;

/// A surfaced (never auto-applied) model replacement suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteSuggestion {
    /// The model being run today.
    pub from: String,
    /// The suggested replacement (the first verbatim hop).
    pub to: String,
    /// The observed evidence behind the suggestion.
    pub reason: String,
}

/// Profile-aware suggestions: when the role's configured model shows an
/// observed weakness the taxonomy quantifies, surface the model's first
/// verbatim fallback hop as a suggested replacement. A suggestion never
/// switches anything — the confirmation flow owns switches.
pub fn suggest_replacement(
    table: &RoutingTable,
    role: Role,
    observed: &crate::taxonomy::FailureTaxonomy,
) -> Vec<RouteSuggestion> {
    let Ok(resolution) = table.resolve(RouteRequest::Role(role)) else {
        return Vec::new();
    };
    if observed.edit_rejection_rate <= EDIT_REJECTION_SUGGESTION_FLOOR {
        return Vec::new();
    }
    let chain = table.fallback_chain(&resolution.model.model);
    let Some(first_hop) = chain.first() else {
        return Vec::new();
    };
    vec![RouteSuggestion {
        from: resolution.model.model.clone(),
        to: first_hop.clone(),
        reason: format!(
            "observed edit rejection rate {:.2} above {:.2} — first verbatim hop suggested",
            observed.edit_rejection_rate, EDIT_REJECTION_SUGGESTION_FLOOR
        ),
    }]
}
