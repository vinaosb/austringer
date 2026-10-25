//! The named, ordered eviction ladder (plan todo 12).
//!
//! Under test: the Gemini-CLI vocabulary — masking → distillation →
//! compression → truncation as NAMED, ORDERED stages; each stage's trigger
//! condition and budget are explicit in config; the engine walks the
//! ladder in canonical order and STOPS the moment the budget target is
//! met (no over-eviction); the Grok tool-result pruning ladder is embedded
//! as compression (soft-trim head/tail excerpts) → truncation (hard-clear
//! to placeholder + retrieval pointer).

mod common;

use austringer_core::eviction::{EvictionConfig, EvictionEngine, LADDER_ORDER, LadderStage};
use common::evictionkit::{self, EviRng, TestDeps};

#[test]
fn ladder_stages_are_named_and_canonically_ordered() {
    assert_eq!(
        LADDER_ORDER,
        [
            LadderStage::Masking,
            LadderStage::Distillation,
            LadderStage::Compression,
            LadderStage::Truncation
        ]
    );
    assert_eq!(LadderStage::Masking.name(), "masking");
    assert_eq!(LadderStage::Distillation.name(), "distillation");
    assert_eq!(LadderStage::Compression.name(), "compression");
    assert_eq!(LadderStage::Truncation.name(), "truncation");
}

#[test]
fn every_stage_carries_an_explicit_trigger_and_budget_in_config() {
    let config = EvictionConfig::default();
    for stage in LADDER_ORDER {
        let stage_config = config
            .ladder
            .stage(stage)
            .unwrap_or_else(|| panic!("stage {} missing config", stage.name()));
        assert!(
            stage_config.budget_tokens > 0,
            "stage {} must declare a positive budget",
            stage.name()
        );
        assert!(
            stage_config.trigger_percent >= 50 && stage_config.trigger_percent <= 100,
            "stage {} trigger must be an explicit window percent",
            stage.name()
        );
    }
    // Masking, distillation, and compression are finite per-walk budgets;
    // only the last-resort stage is unlimited, so the walk can always
    // finish the job.
    for stage in [
        LadderStage::Masking,
        LadderStage::Distillation,
        LadderStage::Compression,
    ] {
        let budget = config
            .ladder
            .stage(stage)
            .map(|c| c.budget_tokens)
            .unwrap_or_default();
        assert!(
            budget != u64::MAX,
            "stage {} must be rationed",
            stage.name()
        );
    }
    assert_eq!(
        config
            .ladder
            .stage(LadderStage::Truncation)
            .map(|c| c.budget_tokens),
        Some(u64::MAX),
        "the last-resort stage is unlimited"
    );
}

#[tokio::test]
async fn walk_stops_at_first_stage_that_meets_the_budget() {
    // Masking gets an unbounded budget: clearing stale tool outputs to
    // placeholders must reclaim enough on its own — no later stage may
    // fire (no over-eviction).
    let history = evictionkit::bloated_tool_history(&mut EviRng::new(21), 40);
    let (mut config, input) = evictionkit::input_with_window(&history, 6_000);
    if let Some(stage) = config.ladder.stage_mut(LadderStage::Masking) {
        stage.budget_tokens = u64::MAX;
    }
    let outcome = evictionkit::default_engine()
        .evict(&input, &config, TestDeps::new().deps())
        .await
        .expect("eviction succeeds");
    assert!(outcome.budget_met, "masking alone must meet the budget");
    assert_eq!(outcome.applied_stages(), vec![LadderStage::Masking]);
    assert!(outcome.post_tokens <= config.budget.target_tokens());
}

#[tokio::test]
async fn walk_continues_through_the_ladder_in_order_when_one_stage_is_not_enough() {
    // Default masking budget is capped: the walk must continue into
    // compression (soft-trim) and beyond, in canonical order.
    let history = evictionkit::bloated_tool_history(&mut EviRng::new(33), 80);
    let (config, input) = evictionkit::input_with_window(&history, 7_000);
    let outcome = evictionkit::default_engine()
        .evict(&input, &config, TestDeps::new().deps())
        .await
        .expect("eviction succeeds");
    assert!(outcome.budget_met);
    let applied = outcome.applied_stages();
    let mut canonical_rank = 0usize;
    for stage in &applied {
        let rank = LADDER_ORDER
            .iter()
            .position(|candidate| candidate == stage)
            .unwrap_or_default();
        assert!(
            rank >= canonical_rank,
            "stages must apply in canonical order, got {applied:?}"
        );
        canonical_rank = rank;
    }
    assert!(
        applied.contains(&LadderStage::Compression),
        "capped masking forces the soft-trim stage: {applied:?}"
    );
}

#[test]
fn soft_trim_keeps_head_and_tail_excerpts_with_an_omission_marker() {
    let body = "A".repeat(4_000);
    let trimmed = austringer_core::eviction::soft_trim(&body, 64, 64);
    assert!(trimmed.starts_with(&"A".repeat(64)));
    assert!(trimmed.ends_with(&"A".repeat(64)));
    assert!(trimmed.contains("[trimmed "));
    assert!(trimmed.len() < body.len() / 10);
    assert_eq!(
        austringer_core::eviction::soft_trim("short", 64, 64),
        "short"
    );
}

#[tokio::test]
async fn under_trigger_the_engine_is_a_passthrough_with_no_decisions() {
    let history = evictionkit::small_history(&mut EviRng::new(5));
    let (config, input) = evictionkit::input_with_window(&history, 64_000);
    let outcome = evictionkit::default_engine()
        .evict(&input, &config, TestDeps::new().deps())
        .await
        .expect("eviction succeeds");
    assert!(!outcome.triggered);
    assert!(outcome.decisions.is_empty());
    assert_eq!(outcome.projected_history().len(), history.len());
    assert!(outcome.budget_met, "budget trivially met: nothing fired");
}
