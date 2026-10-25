//! The deterministic eviction SIM across synthetic 200-turn sessions
//! (plan todo 12 acceptance).
//!
//! Under test: seeded synthetic sessions; per-stage decisions recorded;
//! budgets met, the active plan preserved, tool return codes intact, and
//! the ladder walked in canonical order EVERY turn; re-simulating the
//! same session yields identical decisions (stale_state); flaky-resistance
//! across three seeds; degenerate inputs (empty session, single message,
//! zero window) are typed errors, not panics; a second, minimal engine
//! proves the trait is pluggable.

mod common;

use austringer_core::eviction::{
    DegenerateInput, EvictionConfig, EvictionEngine, EvictionError, LADDER_ORDER, SimReport,
    simulate_session,
};
use common::evictionkit::EviRng;

const SEEDS: [u64; 3] = [1, 0xBADC0DE, 2026_0912];

#[tokio::test]
async fn two_hundred_turn_sim_holds_every_invariant_every_turn() {
    for &seed in &SEEDS {
        // Window sized above the session-end residual: folded markers and
        // call events accumulate by design (codes + parseability
        // invariants); reclaiming that mass is todo 13's compactor.
        let config = EvictionConfig::default().with_window(50_000);
        let report = simulate_session(&config, seed, 200)
            .await
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(report.turns.len(), 200, "seed {seed}");
        let triggered = report.turns.iter().filter(|turn| turn.triggered).count();
        assert!(
            triggered >= 3,
            "seed {seed}: the window must force evictions (got {triggered})"
        );
        let late_triggers = report
            .turns
            .iter()
            .filter(|turn| turn.triggered && turn.turn > 100)
            .count();
        assert!(
            late_triggers >= 1,
            "seed {seed}: sustained operation (late triggers)"
        );
        let flushes = report.turns.iter().filter(|turn| turn.flush_fired).count();
        assert!(
            flushes >= 3,
            "seed {seed}: pre-compact flushes fired ({flushes})"
        );
        for turn in &report.turns {
            assert!(
                turn.budget_met,
                "seed {seed} turn {}: budget met",
                turn.turn
            );
            assert!(
                turn.plan_preserved,
                "seed {seed} turn {}: active plan preserved",
                turn.turn
            );
            assert!(
                turn.tool_codes_intact,
                "seed {seed} turn {}: tool return codes intact",
                turn.turn
            );
            assert!(
                turn.invariants_ok,
                "seed {seed} turn {}: structural invariants",
                turn.turn
            );
            if turn.triggered {
                assert!(!turn.decisions.is_empty(), "decisions recorded");
            }
            let mut canonical_rank = 0usize;
            for stage in &turn.applied_stages {
                let rank = LADDER_ORDER
                    .iter()
                    .position(|candidate| candidate == stage)
                    .unwrap_or_default();
                assert!(
                    rank >= canonical_rank,
                    "seed {seed} turn {}: ladder order {:?}",
                    turn.turn,
                    turn.applied_stages
                );
                canonical_rank = rank;
            }
        }
    }
}

#[tokio::test]
async fn re_simulating_the_same_session_yields_identical_decisions() {
    for &seed in &SEEDS {
        let config = EvictionConfig::default().with_window(50_000);
        let first: SimReport = simulate_session(&config, seed, 200)
            .await
            .expect("first run");
        let second: SimReport = simulate_session(&config, seed, 200)
            .await
            .expect("second run");
        assert_eq!(first, second, "seed {seed}: stale-state resistance");
    }
}

#[tokio::test]
async fn sim_survives_malformed_and_degenerate_sessions_typed() {
    let config = EvictionConfig::default();
    let zero = EvictionConfig::default().with_window(0);
    match simulate_session(&zero, 7, 10).await {
        Err(EvictionError::Degenerate(DegenerateInput::ZeroWindow)) => {}
        other => panic!("zero window must be typed, got {other:?}"),
    }
    let report = simulate_session(&config, 7, 0)
        .await
        .expect("zero turns is well-formed");
    assert!(report.turns.is_empty());
    let report = simulate_session(&config, 9, 1)
        .await
        .expect("single turn is well-formed");
    assert_eq!(report.turns.len(), 1);
    assert!(report.turns[0].invariants_ok);
}

#[tokio::test]
async fn a_second_minimal_engine_proves_the_trait_is_pluggable() {
    let history = common::evictionkit::bloated_tool_history(&mut EviRng::new(5150), 60);
    let (config, input) = common::evictionkit::input_with_window(&history, 5_000);
    let outcome = common::evictionkit::minimal_engine()
        .evict(&input, &config, common::evictionkit::TestDeps::new().deps())
        .await
        .expect("minimal engine runs");
    assert!(outcome.budget_met, "the minimal engine also meets budgets");
    assert!(
        !outcome.projected_history().is_empty(),
        "projection is non-empty"
    );
    let default_outcome = common::evictionkit::default_engine()
        .evict(&input, &config, common::evictionkit::TestDeps::new().deps())
        .await
        .expect("default engine runs");
    assert!(default_outcome.budget_met);
}
