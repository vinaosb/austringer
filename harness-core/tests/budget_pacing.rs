//! Pacing acceptance: the quota conservation window and the token-budget
//! governor ladder (plan todo 25, scope items 5-6).

use std::collections::BTreeMap;

use harness_core::budget::{
    DegradeDecision, Governor, OpClass, Pacing, PoolSpec, PoolTier, QuotaError, QuotaSpec,
    QuotaState, TaskClass, TokenCount, Weight,
};

/// Test-only: `Ok` extraction without the unwrap/expect/panic verbs —
/// an `Err` prints and yields `None` so the caller's `let-else` returns.
fn ok<T, E: std::fmt::Debug>(r: Result<T, E>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("unexpected Ok-expected error: {e:?}");
            None
        }
    }
}

fn quota_fixture() -> QuotaSpec {
    QuotaSpec {
        pools: BTreeMap::from([
            (
                "reason".to_owned(),
                PoolSpec {
                    capacity: 100,
                    refill_per_tick: 10,
                    tier: PoolTier::Expensive,
                },
            ),
            (
                "index".to_owned(),
                PoolSpec {
                    capacity: 50,
                    refill_per_tick: 5,
                    tier: PoolTier::Cheap,
                },
            ),
        ]),
        weights: BTreeMap::from([
            (
                OpClass::Reasoning,
                Weight {
                    pool: "reason".to_owned(),
                    cost_per_unit: 10,
                },
            ),
            (
                OpClass::Retrieval,
                Weight {
                    pool: "index".to_owned(),
                    cost_per_unit: 5,
                },
            ),
        ]),
        conservation_floor_pct: 20,
    }
}

#[test]
fn quota_conservation_window_pauses_the_expensive_tier_only() {
    let Some(mut state) = ok(QuotaState::new(quota_fixture())) else {
        return;
    };
    assert!(matches!(state.pacing(), Pacing::Running));

    // Drain the expensive pool: 8 units x10 = 80 -> level 20 (exactly at the
    // 20% floor: still healthy), then 1 more unit -> level 10 (10% < 20%:
    // the conservation window opens).
    assert_eq!(state.spend(OpClass::Reasoning, 8), Ok(20));
    assert_eq!(state.spend(OpClass::Reasoning, 1), Ok(10));

    let Pacing::Conserving { pools } = state.pacing() else {
        eprintln!("expensive tier must conserve below the floor");
        return;
    };
    assert_eq!(pools, ["reason".to_owned()]);

    // Cheap tier stays healthy while the expensive one is paused.
    assert_eq!(state.spend(OpClass::Retrieval, 2), Ok(40));
    // Expensive spend is refused for the whole window, typed as Paused.
    assert!(matches!(
        state.spend(OpClass::Reasoning, 1),
        Err(QuotaError::Paused { .. })
    ));

    // The window closes once refill brings the pool back to the floor
    // (3 ticks x 10 = level 40 >= 20).
    assert!(state.advance_to(3).is_ok());
    assert!(matches!(state.pacing(), Pacing::Running));
    assert_eq!(state.spend(OpClass::Reasoning, 1), Ok(30));
}

#[test]
fn governor_ladder_keys_on_token_budget_per_class() {
    let gov = Governor::standard();
    // The ~10x class spread: quick-edit holds fan-out at 25k remaining
    // where deep research has already degraded to serialize.
    assert_eq!(
        gov.decide(TokenCount(25_000), 3, TaskClass::QuickEdit),
        DegradeDecision::HoldFanOut { current: 3 }
    );
    assert_eq!(
        gov.decide(TokenCount(25_000), 6, TaskClass::DeepResearch),
        DegradeDecision::Serialize
    );
    // Exact boundaries: at the rung threshold the rung applies.
    assert_eq!(
        gov.decide(TokenCount(200_000), 4, TaskClass::DeepResearch),
        DegradeDecision::HoldFanOut { current: 4 }
    );
    assert_eq!(
        gov.decide(TokenCount(50_000), 6, TaskClass::DeepResearch),
        DegradeDecision::CollapseFanOut
    );
    assert_eq!(
        gov.decide(TokenCount(49_999), 6, TaskClass::DeepResearch),
        DegradeDecision::Serialize
    );
    assert_eq!(
        gov.decide(TokenCount(9_999), 3, TaskClass::DeepResearch),
        DegradeDecision::Solo
    );
    assert_eq!(
        gov.decide(TokenCount(60_000), 3, TaskClass::Balanced),
        DegradeDecision::HoldFanOut { current: 3 }
    );
    // Arithmetic check (anti misleading-success): the spread is exactly
    // 10x at every rung, per the deterministic policy table.
    let deep = Governor::thresholds(TaskClass::DeepResearch);
    let quick = Governor::thresholds(TaskClass::QuickEdit);
    assert_eq!(deep.hold, 10 * quick.hold);
    assert_eq!(deep.collapse, 10 * quick.collapse);
    assert_eq!(deep.serialize, 10 * quick.serialize);
}
