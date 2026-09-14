//! The per-model 400-trap feature-detection table (research SS33.1/SS33.7):
//! `budget_tokens` on Anthropic 4.7+; prefill on 4.6+; forced `tool_choice`
//! on Fable 5.1; `thinking_budget`+`thinking_level` together on Gemini;
//! `none`+sampling params on GPT-6 Astra; penalties/`stop` on Grok
//! reasoning models. Legality is detected per MODEL, never per provider —
//! and every violation fails CLOSED with a typed error before any wire
//! request is built.

use harness_intel::catalog::{
    Catalog, CatalogError, EffortValue, Feature, ThinkingLevel, WireKnobs,
};
use harness_intel::dialect::ProtocolDialect;

fn knobs() -> WireKnobs {
    WireKnobs::default()
}

/// The trap feature a validation failure carries (fails the test if
/// validation passed or if the failure was anything but a trap).
fn trap_feature(catalog: &Catalog, model: &str, knobs: &WireKnobs) -> Feature {
    match catalog.validate(model, knobs) {
        Err(CatalogError::Trap(trap)) => trap.feature,
        Err(other) => panic!("expected a trap violation, got: {other}"),
        Ok(()) => panic!("expected {model} to reject these knobs"),
    }
}

#[test]
fn budget_tokens_trap_fires_on_opus_4_7_but_not_4_6() {
    // Given: a request carrying the deprecated thinking-budget knob.
    let catalog = Catalog::seeded();
    let mut k = knobs();
    k.thinking_budget = Some(1024);

    // When/Then: 4.7 rejects (budget_tokens 400s on 4.7+) ...
    assert_eq!(
        trap_feature(&catalog, "claude-opus-4-7", &k),
        Feature::ThinkingBudget
    );
    // ... while 4.6 (budget merely deprecated there) accepts it.
    assert!(catalog.validate("claude-opus-4-6", &k).is_ok());
}

#[test]
fn prefill_trap_on_anthropic_4_6_and_newer() {
    let catalog = Catalog::seeded();
    let mut k = knobs();
    k.prefill = true;

    // Given/When: prefill against a 4.6-family model.
    let err = catalog
        .validate("claude-sonnet-4-6", &k)
        .expect_err("prefill 400s on 4.6+");
    // Then: the violation names the model and the feature.
    let CatalogError::Trap(trap) = err else {
        panic!("expected trap violation")
    };
    assert_eq!(trap.feature, Feature::Prefill);
    assert_eq!(trap.model, "claude-sonnet-4-6");
    // And non-Anthropic models keep prefill legal.
    assert!(catalog.validate("gpt-5-5", &k).is_ok());
}

#[test]
fn forced_tool_choice_trap_on_fable_only() {
    let catalog = Catalog::seeded();
    let mut k = knobs();
    k.forced_tool_choice = true;

    assert_eq!(
        trap_feature(&catalog, "claude-fable-5-1", &k),
        Feature::ForcedToolChoice
    );
    // Opus 5 keeps forced tool_choice legal.
    assert!(catalog.validate("claude-opus-5", &k).is_ok());
}

#[test]
fn gemini_rejects_budget_and_level_together() {
    let catalog = Catalog::seeded();
    let mut both = knobs();
    both.thinking_budget = Some(2048);
    both.thinking_level = Some(ThinkingLevel::High);

    assert_eq!(
        trap_feature(&catalog, "gemini-3-pro", &both),
        Feature::ThinkingBudgetWithLevel
    );

    // Each knob alone remains legal.
    let mut level_only = knobs();
    level_only.thinking_level = Some(ThinkingLevel::High);
    assert!(catalog.validate("gemini-3-pro", &level_only).is_ok());
    let mut budget_only = knobs();
    budget_only.thinking_budget = Some(2048);
    assert!(catalog.validate("gemini-2-5-pro", &budget_only).is_ok());
}

#[test]
fn astra_rejects_effort_none_and_sampling() {
    let catalog = Catalog::seeded();

    let mut none_effort = knobs();
    none_effort.effort = Some(EffortValue::None);
    assert_eq!(
        trap_feature(&catalog, "gpt-6-astra", &none_effort),
        Feature::EffortNone
    );

    let mut low = knobs();
    low.effort = Some(EffortValue::Low);
    assert!(catalog.validate("gpt-6-astra", &low).is_ok());

    let mut sampling = knobs();
    sampling.effort = Some(EffortValue::Low);
    sampling.sampling = true;
    assert_eq!(
        trap_feature(&catalog, "gpt-6-astra", &sampling),
        Feature::Sampling
    );
}

#[test]
fn grok_rejects_penalties_and_stop_sequences() {
    let catalog = Catalog::seeded();
    let mut k = knobs();
    k.penalties = true;
    k.stop_sequences = true;

    assert_eq!(trap_feature(&catalog, "grok-4-6", &k), Feature::Penalties);

    let clean = knobs();
    assert!(catalog.validate("grok-4-6", &clean).is_ok());
}

#[test]
fn effort_grid_is_per_model_not_per_family() {
    // The fragmentation point (research SS33.1): xhigh/max support differs
    // model-by-model inside one family. gpt-5-5 tops out at xhigh (no max);
    // claude-opus-5 reaches max.
    let catalog = Catalog::seeded();

    let mut max = knobs();
    max.effort = Some(EffortValue::Max);
    assert!(matches!(
        trap_feature(&catalog, "gpt-5-5", &max),
        Feature::EffortUnsupported { .. }
    ));
    assert!(catalog.validate("claude-opus-5", &max).is_ok());

    let mut xhigh = knobs();
    xhigh.effort = Some(EffortValue::Xhigh);
    assert!(catalog.validate("gpt-5-5", &xhigh).is_ok());
}

#[test]
fn gemini_pro_thinking_floor_is_enforced() {
    // Cline bug 7735 guard: `minimal` (and budget 0) must never be sent to
    // a Pro-tier Gemini.
    let catalog = Catalog::seeded();

    let mut minimal = knobs();
    minimal.thinking_level = Some(ThinkingLevel::Minimal);
    assert_eq!(
        trap_feature(&catalog, "gemini-3-pro", &minimal),
        Feature::ThinkingBelowFloor
    );

    let mut low = knobs();
    low.thinking_level = Some(ThinkingLevel::Low);
    assert!(catalog.validate("gemini-3-pro", &low).is_ok());

    let mut zero_budget = knobs();
    zero_budget.thinking_budget = Some(0);
    assert!(catalog.validate("gemini-2-5-pro", &zero_budget).is_err());
}

#[test]
fn unknown_model_is_a_typed_error_never_a_guess() {
    let catalog = Catalog::seeded();
    let err = catalog
        .entry("mystery-model")
        .expect_err("unknown models must be typed errors");
    assert_eq!(err.model, "mystery-model");
    assert!(!err.to_string().is_empty());
}

#[test]
fn dialect_is_explicit_per_catalog_entry() {
    // The dsh thinkingFormat lesson: dialect comes from the catalog entry,
    // never from an endpoint URL. Spot-check the seeded table.
    let catalog = Catalog::seeded();
    assert_eq!(
        catalog.entry("deepseek-reasoner").expect("seeded").dialect,
        ProtocolDialect::DeepSeek
    );
    assert_eq!(
        catalog.entry("claude-opus-4-7").expect("seeded").dialect,
        ProtocolDialect::Anthropic
    );
    assert_eq!(
        catalog.entry("grok-4-6").expect("seeded").dialect,
        ProtocolDialect::Xai
    );
}
