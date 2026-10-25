//! Live quota pacing: pool levels, burn accounting, and the conservation
//! view (forge §26). Time is a caller-injected tick counter — no clock, no
//! wall time, so every pacing decision is deterministic.

use std::collections::BTreeMap;

use super::quota::{OpClass, PoolTier, QuotaError, QuotaSpec};

/// The pacing view over all pools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pacing {
    /// Every pool healthy.
    Running,
    /// Conservation windows are open on the named pools (cheap tiers keep
    /// running).
    Conserving {
        /// The paused expensive pools, sorted by name.
        pools: Vec<String>,
    },
}

/// Live pool levels over a validated [`QuotaSpec`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaState {
    spec: QuotaSpec,
    level: BTreeMap<String, u64>,
    tick: u64,
}

impl QuotaState {
    /// Creates the state with every pool full, validating the spec.
    pub fn new(spec: QuotaSpec) -> Result<Self, QuotaError> {
        for (pool, pool_spec) in &spec.pools {
            if pool_spec.capacity == 0 {
                return Err(QuotaError::EmptyCapacity { pool: pool.clone() });
            }
        }
        if spec.conservation_floor_pct > 100 {
            return Err(QuotaError::FloorTooHigh(spec.conservation_floor_pct));
        }
        for weight in spec.weights.values() {
            if !spec.pools.contains_key(&weight.pool) {
                return Err(QuotaError::UnknownPool {
                    pool: weight.pool.clone(),
                });
            }
            if weight.cost_per_unit == 0 {
                return Err(QuotaError::ZeroCost {
                    pool: weight.pool.clone(),
                });
            }
        }
        let level = spec
            .pools
            .iter()
            .map(|(pool, pool_spec)| (pool.clone(), pool_spec.capacity))
            .collect();
        Ok(Self {
            spec,
            level,
            tick: 0,
        })
    }

    /// Advances to `tick`, refilling every pool (saturating at capacity).
    pub fn advance_to(&mut self, tick: u64) -> Result<(), QuotaError> {
        if tick < self.tick {
            return Err(QuotaError::TimeWentBackwards {
                from: self.tick,
                to: tick,
            });
        }
        let elapsed = tick - self.tick;
        for (pool, pool_spec) in &self.spec.pools {
            let Some(level) = self.level.get_mut(pool) else {
                continue;
            };
            let refill = elapsed.saturating_mul(pool_spec.refill_per_tick);
            *level = u64::try_from(u128::from(*level) + u128::from(refill))
                .unwrap_or(pool_spec.capacity)
                .min(pool_spec.capacity);
        }
        self.tick = tick;
        Ok(())
    }

    /// Burns an operation class's cost from its pool; returns the pool's
    /// remaining level. A paused pool refuses typed; an insufficient level
    /// refuses typed with the deficit.
    pub fn spend(&mut self, class: OpClass, units: u64) -> Result<u64, QuotaError> {
        let weight = self
            .spec
            .weights
            .get(&class)
            .ok_or(QuotaError::NoWeight { class })?;
        let level =
            self.level
                .get(&weight.pool)
                .copied()
                .ok_or_else(|| QuotaError::UnknownPool {
                    pool: weight.pool.clone(),
                })?;
        if self.conserving(&weight.pool) {
            return Err(QuotaError::Paused {
                pool: weight.pool.clone(),
                level,
            });
        }
        let cost = u128::from(units) * u128::from(weight.cost_per_unit);
        let Ok(cost) = u64::try_from(cost) else {
            return Err(QuotaError::Overflow);
        };
        if level < cost {
            return Err(QuotaError::Exhausted {
                pool: weight.pool.clone(),
                deficit: u128::from(cost - level),
            });
        }
        let next = level - cost;
        self.level.insert(weight.pool.clone(), next);
        Ok(next)
    }

    /// The current level of `pool`.
    #[must_use]
    pub fn level(&self, pool: &str) -> Option<u64> {
        self.level.get(pool).copied()
    }

    /// The pacing view: which expensive pools are inside a conservation
    /// window right now.
    #[must_use]
    pub fn pacing(&self) -> Pacing {
        let pools = self
            .spec
            .pools
            .iter()
            .filter(|(pool, _)| self.conserving(pool))
            .map(|(pool, _)| pool.clone())
            .collect::<Vec<_>>();
        if pools.is_empty() {
            Pacing::Running
        } else {
            Pacing::Conserving { pools }
        }
    }

    /// Cross-multiplied fill check: strictly below the floor.
    fn conserving(&self, pool: &str) -> bool {
        let Some(pool_spec) = self.spec.pools.get(pool) else {
            return false;
        };
        let Some(level) = self.level.get(pool) else {
            return false;
        };
        pool_spec.tier == PoolTier::Expensive
            && u128::from(*level) * 100
                < u128::from(pool_spec.capacity) * u128::from(self.spec.conservation_floor_pct)
    }
}

#[cfg(test)]
mod tests {
    use super::{OpClass, Pacing, QuotaError, QuotaState};
    use crate::budget::{PoolSpec, PoolTier, QuotaSpec, Weight};
    use std::collections::BTreeMap;

    fn spec() -> QuotaSpec {
        QuotaSpec {
            pools: BTreeMap::from([(
                "p".to_owned(),
                PoolSpec {
                    capacity: 100,
                    refill_per_tick: 1,
                    tier: PoolTier::Cheap,
                },
            )]),
            weights: BTreeMap::from([(
                OpClass::Execution,
                Weight {
                    pool: "p".to_owned(),
                    cost_per_unit: 1,
                },
            )]),
            conservation_floor_pct: 20,
        }
    }

    #[test]
    fn unpriced_class_and_unknown_pool_are_typed_rejections() {
        let made = QuotaState::new(spec());
        assert!(made.is_ok(), "valid spec rejected: {made:?}");
        let Ok(mut state) = made else {
            return;
        };
        assert!(matches!(
            state.spend(OpClass::Reasoning, 1),
            Err(QuotaError::NoWeight { .. })
        ));
        let bad = QuotaSpec {
            weights: BTreeMap::from([(
                OpClass::Execution,
                Weight {
                    pool: "ghost".to_owned(),
                    cost_per_unit: 1,
                },
            )]),
            ..spec()
        };
        assert!(matches!(
            QuotaState::new(bad),
            Err(QuotaError::UnknownPool { .. })
        ));
    }

    #[test]
    fn ticks_never_move_backwards() {
        let made = QuotaState::new(spec());
        assert!(made.is_ok(), "valid spec rejected: {made:?}");
        let Ok(mut state) = made else {
            return;
        };
        assert!(state.advance_to(5).is_ok());
        assert!(matches!(
            state.advance_to(4),
            Err(QuotaError::TimeWentBackwards { from: 5, to: 4 })
        ));
    }

    #[test]
    fn cheap_pools_never_conserve() {
        let made = QuotaState::new(spec());
        assert!(made.is_ok(), "valid spec rejected: {made:?}");
        let Ok(mut state) = made else {
            return;
        };
        assert_eq!(state.spend(OpClass::Execution, 100), Ok(0));
        assert!(matches!(state.pacing(), Pacing::Running));
        assert!(matches!(
            state.spend(OpClass::Execution, 1),
            Err(QuotaError::Exhausted { deficit: 1, .. })
        ));
    }
}
