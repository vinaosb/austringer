//! The quota vocabulary (forge §26): pools, burn-rate weights per
//! operation class, and typed failures. Live pacing (levels, refill,
//! conservation windows) lives in [`super::pacing`]; time there is a
//! caller-injected tick counter, so every decision is deterministic.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Operation class a burn weight keys on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OpClass {
    /// Model reasoning calls (the expensive tier's typical draw).
    Reasoning,
    /// Retrieval / search calls.
    Retrieval,
    /// Tool execution / local work.
    Execution,
}

/// Costliness tier — conservation pauses expensive tiers only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoolTier {
    /// Conservation-eligible: paused below the floor.
    Expensive,
    /// Never paused by conservation.
    Cheap,
}

/// One paced pool: capacity and refill rate per tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolSpec {
    /// Maximum units the pool holds.
    pub capacity: u64,
    /// Units refilled per elapsed tick.
    pub refill_per_tick: u64,
    /// The pool's tier.
    pub tier: PoolTier,
}

/// The burn-rate weight of an operation class: which pool it draws from
/// and the per-unit cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Weight {
    /// The pool the class draws from.
    pub pool: String,
    /// Cost per unit of the operation.
    pub cost_per_unit: u64,
}

/// The quota configuration (config-loaded shape).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotaSpec {
    /// The pools, keyed by name.
    pub pools: BTreeMap<String, PoolSpec>,
    /// Burn weights per operation class.
    pub weights: BTreeMap<OpClass, Weight>,
    /// Conservation floor, in percent of capacity: below it an expensive
    /// pool pauses until refill recovers it.
    pub conservation_floor_pct: u32,
}

/// A typed quota failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QuotaError {
    /// The operation class carries no burn weight.
    #[error("operation class {class:?} has no burn weight")]
    NoWeight {
        /// The unweighted class.
        class: OpClass,
    },
    /// A burn weight targets a pool that does not exist.
    #[error("burn weight targets unknown pool {pool}")]
    UnknownPool {
        /// The missing pool name.
        pool: String,
    },
    /// A pool declared with zero capacity.
    #[error("pool {pool} must have positive capacity")]
    EmptyCapacity {
        /// The offending pool name.
        pool: String,
    },
    /// A burn weight declared with zero cost.
    #[error("burn weight for pool {pool} must cost at least 1 per unit")]
    ZeroCost {
        /// The offending pool name.
        pool: String,
    },
    /// The conservation floor is not a percentage.
    #[error("conservation floor {0}% is not a percentage")]
    FloorTooHigh(u32),
    /// The computed cost exceeds the u64 level space.
    #[error("quota arithmetic overflow")]
    Overflow,
    /// The tick counter moved backwards.
    #[error("tick moved backwards: {from} -> {to}")]
    TimeWentBackwards {
        /// The previous tick.
        from: u64,
        /// The attempted tick.
        to: u64,
    },
    /// The pool is inside a conservation window.
    #[error("pool {pool} is paused (conservation window): level {level}")]
    Paused {
        /// The paused pool.
        pool: String,
        /// The level when the pause was hit.
        level: u64,
    },
    /// The pool lacks the required level for the spend.
    #[error("pool {pool} exhausted: deficit {deficit}")]
    Exhausted {
        /// The exhausted pool.
        pool: String,
        /// How short the level was.
        deficit: u128,
    },
}
