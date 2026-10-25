//! The routing table (plan todo 42): the validated, resolved form of the
//! [`RoutingSpec`]. Construction is the fail-closed gate — unknown roles,
//! unknown models, utility roles off the cheap pool, duplicate routes,
//! self-referential or duplicate-hop chains, and over-cap wait-out
//! windows are all typed errors before any request exists.

use std::collections::BTreeMap;
use std::time::Duration;

use austringer_providers::identity::ModelIdentity;

use super::config::RoutingSpec;
use super::resolve::ResolutionError;
use super::sidekick::SidekickRoute;
use super::{Role, Tier};
use crate::catalog::{Catalog, EffortValue, WireKnobs};

// Defined alongside the spec in `config`; re-exported here so the
// `routing::table` surface keeps its historic paths.
pub use super::config::{RoutingConfigError, WAIT_OUT_CAP_SECS};

/// A resolved role route (config + catalog defaults folded in).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RoleRoute {
    /// The routed model id.
    pub(crate) model: String,
    /// The explicit effort rung.
    pub(crate) effort: EffortValue,
}

/// A resolved tier route.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TierRoute {
    /// The tier's default model id.
    pub(crate) model: String,
    /// The tier's explicit effort rung.
    pub(crate) effort: EffortValue,
    /// Benchmark floor for live routing.
    pub(crate) score_floor: f32,
}

/// A resolved delegation-category route.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CategoryRoute {
    /// The category's model id.
    pub(crate) model: String,
    /// The category's explicit effort rung.
    pub(crate) effort: EffortValue,
    /// Benchmark floor (reserved for category-level live routing).
    pub(crate) score_floor: f32,
}

/// The validated routing table.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutingTable {
    /// Provider stamped on identities.
    pub(crate) provider: String,
    /// Role -> resolved route.
    pub(crate) roles: BTreeMap<Role, RoleRoute>,
    /// Tier -> resolved route.
    pub(crate) tiers: BTreeMap<Tier, TierRoute>,
    /// Category -> resolved route.
    pub(crate) categories: BTreeMap<String, CategoryRoute>,
    /// Model -> verbatim chain.
    pub(crate) chains: BTreeMap<String, Vec<String>>,
    /// The benchmark table (verbatim order kept; selection is
    /// cheapest-proven, deterministic).
    pub(crate) benchmarks: Vec<super::config::BenchmarkEntrySpec>,
    wait_out: Duration,
    retry_attempts: u32,
    sidekick: Option<SidekickRoute>,
    /// Catalog copy for resolution-time validation.
    pub(crate) catalog: Catalog,
}

impl RoutingTable {
    /// Builds and validates a table from a spec. Every route is checked
    /// against the catalog (fail closed) and every collection conflict
    /// is a typed error.
    pub fn build(spec: &RoutingSpec, catalog: &Catalog) -> Result<Self, RoutingConfigError> {
        if spec.rate_limit_wait_out_secs > WAIT_OUT_CAP_SECS {
            return Err(RoutingConfigError::WaitOutTooLong {
                secs: spec.rate_limit_wait_out_secs,
            });
        }
        let cheap: std::collections::BTreeSet<&str> =
            spec.cheap_pool.iter().map(String::as_str).collect();
        for model in &spec.cheap_pool {
            catalog.entry(model)?;
        }
        let mut roles = BTreeMap::new();
        for route in &spec.roles {
            let Some(role) = Role::from_key(&route.role) else {
                return Err(RoutingConfigError::UnknownRole {
                    role: route.role.clone(),
                });
            };
            if roles.contains_key(&role) {
                return Err(RoutingConfigError::DuplicateRole {
                    role: route.role.clone(),
                });
            }
            let entry = catalog.entry(&route.model)?;
            if role.class() == super::RoleClass::Utility && !cheap.contains(route.model.as_str()) {
                return Err(RoutingConfigError::UtilityOffCheap {
                    role: route.role.clone(),
                    model: route.model.clone(),
                });
            }
            let effort = route.effort.unwrap_or(entry.default_effort);
            catalog.validate(&route.model, &knobs_for(effort))?;
            roles.insert(
                role,
                RoleRoute {
                    model: route.model.clone(),
                    effort,
                },
            );
        }
        let mut tiers = BTreeMap::new();
        for route in &spec.tiers {
            let Some(tier) = Tier::from_key(&route.tier) else {
                return Err(RoutingConfigError::UnknownTier {
                    tier: route.tier.clone(),
                });
            };
            if tiers.contains_key(&tier) {
                return Err(RoutingConfigError::DuplicateTier {
                    tier: route.tier.clone(),
                });
            }
            let entry = catalog.entry(&route.model)?;
            let effort = route.effort.unwrap_or(entry.default_effort);
            catalog.validate(&route.model, &knobs_for(effort))?;
            tiers.insert(
                tier,
                TierRoute {
                    model: route.model.clone(),
                    effort,
                    score_floor: route.score_floor,
                },
            );
        }
        let mut categories = BTreeMap::new();
        for route in &spec.categories {
            if categories.contains_key(&route.category) {
                return Err(RoutingConfigError::DuplicateCategory {
                    category: route.category.clone(),
                });
            }
            let entry = catalog.entry(&route.model)?;
            let effort = route.effort.unwrap_or(entry.default_effort);
            catalog.validate(&route.model, &knobs_for(effort))?;
            categories.insert(
                route.category.clone(),
                CategoryRoute {
                    model: route.model.clone(),
                    effort,
                    score_floor: route.score_floor,
                },
            );
        }
        let mut chains = BTreeMap::new();
        for chain in &spec.fallbacks {
            catalog.entry(&chain.model)?;
            if chains.contains_key(&chain.model) {
                return Err(RoutingConfigError::DuplicateOrigin {
                    model: chain.model.clone(),
                });
            }
            let mut seen = std::collections::BTreeSet::new();
            for hop in &chain.chain {
                catalog.entry(hop)?;
                if *hop == chain.model {
                    return Err(RoutingConfigError::SelfReferentialChain {
                        model: chain.model.clone(),
                    });
                }
                if !seen.insert(hop.as_str()) {
                    return Err(RoutingConfigError::DuplicateHop {
                        model: chain.model.clone(),
                    });
                }
            }
            chains.insert(chain.model.clone(), chain.chain.clone());
        }
        let mut bench_seen = std::collections::BTreeSet::new();
        for bench in &spec.benchmarks {
            catalog.entry(&bench.model)?;
            if !bench_seen.insert(bench.model.as_str()) {
                return Err(RoutingConfigError::DuplicateBenchmark {
                    model: bench.model.clone(),
                });
            }
        }
        let sidekick = match &spec.sidekick {
            Some(pair) => {
                catalog.entry(&pair.lead)?;
                catalog.entry(&pair.sidekick)?;
                if !cheap.contains(pair.sidekick.as_str()) {
                    return Err(RoutingConfigError::SidekickNotCheap {
                        model: pair.sidekick.clone(),
                    });
                }
                Some(SidekickRoute {
                    lead: ModelIdentity::new(&spec.provider, &pair.lead),
                    sidekick: ModelIdentity::new(&spec.provider, &pair.sidekick),
                })
            }
            None => None,
        };
        Ok(Self {
            provider: spec.provider.clone(),
            roles,
            tiers,
            categories,
            chains,
            benchmarks: spec.benchmarks.clone(),
            wait_out: Duration::from_secs(spec.rate_limit_wait_out_secs),
            retry_attempts: spec.retry_attempts,
            sidekick,
            catalog: catalog.clone(),
        })
    }

    /// Parses and builds from a JSON document. Malformed documents are
    /// bounded typed errors (the raw document never rides the error).
    pub fn from_json(json: &str, catalog: &Catalog) -> Result<Self, RoutingConfigError> {
        let spec: RoutingSpec = serde_json::from_str(json).map_err(|e| {
            let detail = e.to_string();
            RoutingConfigError::Malformed {
                detail: detail.chars().take(300).collect(),
            }
        })?;
        Self::build(&spec, catalog)
    }

    /// The verbatim chain for a model (empty when none configured).
    #[must_use]
    pub fn fallback_chain(&self, model: &str) -> &[String] {
        self.chains.get(model).map_or(&[], Vec::as_slice)
    }

    /// The configured rate-limit wait-out window.
    #[must_use]
    pub fn wait_out(&self) -> Duration {
        self.wait_out
    }

    /// The retry budget per failure mode.
    #[must_use]
    pub fn retry_attempts(&self) -> u32 {
        self.retry_attempts
    }

    /// The fusion sidekick pair, when configured.
    #[must_use]
    pub fn sidekick(&self) -> Option<&SidekickRoute> {
        self.sidekick.as_ref()
    }

    /// The catalog routes resolve against.
    #[must_use]
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Resolves a request (see [`super::resolve`]).
    pub fn resolve(
        &self,
        request: super::RouteRequest,
    ) -> Result<super::resolve::RouteResolution, ResolutionError> {
        super::resolve::resolve(self, request)
    }
}

/// The routed knob set for one effort rung (everything else unset).
fn knobs_for(effort: EffortValue) -> WireKnobs {
    WireKnobs {
        effort: Some(effort),
        ..WireKnobs::default()
    }
}
