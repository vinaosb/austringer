//! The routing config surface (plan todo 42): the user-authored JSON
//! spec. Collections are LISTS of keyed entries, not maps, so duplicate
//! keys surface as typed build errors instead of silent overwrites.

use serde::{Deserialize, Serialize};

use crate::catalog::{EffortValue, TrapViolation, UnknownModel};

/// One role route: the model the role runs on and an optional explicit
/// effort (the model's catalog default when omitted — never the
/// provider's implicit default).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoleRouteSpec {
    /// Role key (`"architect"`, `"commit-message"`, ...).
    pub role: String,
    /// Model id (must exist in the catalog).
    pub model: String,
    /// Explicit effort rung; `None` = the catalog default.
    pub effort: Option<EffortValue>,
}

/// One complexity-tier route: the tier's default model + effort plus the
/// benchmark score floor live routing composes with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierRouteSpec {
    /// Tier key (`"light"`, `"medium"`, `"heavy"`).
    pub tier: String,
    /// The tier's default model.
    pub model: String,
    /// Explicit effort; `None` = the model's catalog default.
    pub effort: Option<EffortValue>,
    /// Minimum benchmark score a model must prove for benchmark-driven
    /// routing to select it for this tier.
    pub score_floor: f32,
}

/// One delegation-category route (todo 13): the `task(category)` tool
/// dispatches child sessions on this shape — model + explicit effort
/// plus the benchmark score floor, like tiers (the floor is reserved
/// for category-level live routing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryRouteSpec {
    /// Category key (`"quick"`, `"deep"`, ... — user-authored).
    pub category: String,
    /// The category's default model.
    pub model: String,
    /// Explicit effort; `None` = the model's catalog default.
    pub effort: Option<EffortValue>,
    /// Minimum benchmark score a model must prove for benchmark-driven
    /// routing to select it for this category.
    pub score_floor: f32,
}

/// One rank-verbatim failover chain (forge, research SS26): the fallback
/// order is an explicit verbatim list, never provider round-robin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FallbackChainSpec {
    /// The model this chain belongs to.
    pub model: String,
    /// The verbatim hop order.
    pub chain: Vec<String>,
}

/// One benchmark table row (fed by the evals adapter, todo 44).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkEntrySpec {
    /// The proven model.
    pub model: String,
    /// Its benchmark score.
    pub score: f32,
    /// List price per million tokens (drives "cheapest proven" picks).
    pub price_per_mtok: f64,
}

/// The fusion sidekick pair (Devin, research SS22).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SidekickSpec {
    /// The frontier lead model.
    pub lead: String,
    /// The cheap sidekick (its own persistent cached context).
    pub sidekick: String,
}

fn default_wait_out_secs() -> u64 {
    45
}

fn default_retry_attempts() -> u32 {
    2
}

/// The routing configuration document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingSpec {
    /// Provider name stamped on resolved identities (configuration-owned,
    /// never wire-inferred).
    pub provider: String,
    /// Models utility roles may run on (the cheap pool).
    #[serde(default)]
    pub cheap_pool: Vec<String>,
    /// Role routes.
    #[serde(default)]
    pub roles: Vec<RoleRouteSpec>,
    /// Complexity-tier routes.
    #[serde(default)]
    pub tiers: Vec<TierRouteSpec>,
    /// Delegation-category routes (the `task(category)` vocabulary).
    #[serde(default)]
    pub categories: Vec<CategoryRouteSpec>,
    /// Rank-verbatim fallback chains.
    #[serde(default)]
    pub fallbacks: Vec<FallbackChainSpec>,
    /// Rate-limit wait-out window in seconds (capped at 75).
    #[serde(default = "default_wait_out_secs")]
    pub rate_limit_wait_out_secs: u64,
    /// Retries per failure mode before an acknowledged fallback is asked.
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    /// Benchmark ranking table (empty = tier routing uses declared
    /// models only).
    #[serde(default)]
    pub benchmarks: Vec<BenchmarkEntrySpec>,
    /// The fusion sidekick pair, when configured.
    #[serde(default)]
    pub sidekick: Option<SidekickSpec>,
}

/// Rate-limit wait-out windows never exceed this (forge, research SS26).
pub const WAIT_OUT_CAP_SECS: u64 = 75;

/// Everything routing-table construction can fail with.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RoutingConfigError {
    /// The document is not a valid routing spec (bounded detail).
    #[error("malformed routing config: {detail}")]
    Malformed {
        /// Bounded serde detail (never the raw document).
        detail: String,
    },
    /// A role key that is not in the taxonomy.
    #[error("unknown role {role:?}")]
    UnknownRole {
        /// The offending key.
        role: String,
    },
    /// A tier key that is not light/medium/heavy.
    #[error("unknown tier {tier:?}")]
    UnknownTier {
        /// The offending key.
        tier: String,
    },
    /// The same role routed twice.
    #[error("role {role:?} routed more than once — conflicting routes fail closed")]
    DuplicateRole {
        /// The duplicated key.
        role: String,
    },
    /// The same tier routed twice.
    #[error("tier {tier:?} routed more than once — conflicting routes fail closed")]
    DuplicateTier {
        /// The duplicated key.
        tier: String,
    },
    /// The same category routed twice.
    #[error("category {category:?} routed more than once — conflicting routes fail closed")]
    DuplicateCategory {
        /// The duplicated key.
        category: String,
    },
    /// Two chains declared for one model.
    #[error("duplicate fallback chain for {model:?}")]
    DuplicateOrigin {
        /// The duplicated model.
        model: String,
    },
    /// The same model benchmarked twice.
    #[error("duplicate benchmark entry for {model:?}")]
    DuplicateBenchmark {
        /// The duplicated model.
        model: String,
    },
    /// A utility role routed outside the cheap pool.
    #[error("utility role {role:?} routed to {model:?} which is not in the cheap pool")]
    UtilityOffCheap {
        /// The utility role.
        role: String,
        /// The frontier model it was pointed at.
        model: String,
    },
    /// A chain that hops to its own origin.
    #[error("fallback chain for {model:?} contains itself")]
    SelfReferentialChain {
        /// The chain's own model.
        model: String,
    },
    /// A chain that lists one hop twice.
    #[error("fallback chain for {model:?} lists a hop twice")]
    DuplicateHop {
        /// The chain's own model.
        model: String,
    },
    /// A wait-out window over the 75s cap.
    #[error("rate-limit wait-out {secs}s exceeds the {WAIT_OUT_CAP_SECS}s cap")]
    WaitOutTooLong {
        /// The configured seconds.
        secs: u64,
    },
    /// A sidekick that is not a cheap-pool model.
    #[error(
        "sidekick {model:?} is not in the cheap pool — the sidekick is the cheap cached context"
    )]
    SidekickNotCheap {
        /// The offending sidekick model.
        model: String,
    },
    /// A model the catalog does not know.
    #[error(transparent)]
    UnknownModel(#[from] UnknownModel),
    /// A route whose effort is off the model's grid.
    #[error(transparent)]
    Trap(#[from] TrapViolation),
}

impl From<crate::catalog::CatalogError> for RoutingConfigError {
    fn from(error: crate::catalog::CatalogError) -> Self {
        match error {
            crate::catalog::CatalogError::UnknownModel(unknown) => Self::UnknownModel(unknown),
            crate::catalog::CatalogError::Trap(trap) => Self::Trap(trap),
        }
    }
}
