//! Stage-B tests: per-model cost tables, exact integer arithmetic in micro-USD,
//! the unpriced fallback, and runtime price-override merging.

use harness_providers::cost::{CostReport, CostTable, ModelPrices, Priced};
use harness_providers::event::Usage;
use harness_providers::identity::ModelIdentity;

fn usage_all_kinds() -> Usage {
    Usage {
        input_tokens: 1_000_000,
        output_tokens: 500_000,
        cache_read_tokens: 200_000,
        cache_write_tokens: 100_000,
    }
}

#[test]
fn builtin_table_prices_each_provider_and_arithmetic_is_exact() {
    let table = CostTable::builtin();
    let identity = ModelIdentity::new("anthropic", "claude-sonnet-4-5");
    let report = table.account(&identity, usage_all_kinds());
    // claude-sonnet-4-5 list prices (USD/MTok): 3 in, 15 out, 0.30 cache-read,
    // 3.75 cache-write. With the usage above: 3 + 7.5 + 0.06 + 0.375 USD.
    let Priced::Priced(costs) = report.priced else {
        panic!("claude-sonnet-4-5 must be priced in the builtin table");
    };
    assert_eq!(costs.input_usd_micros, 3_000_000);
    assert_eq!(costs.output_usd_micros, 7_500_000);
    assert_eq!(costs.cache_read_usd_micros, 60_000);
    assert_eq!(costs.cache_write_usd_micros, 375_000);
    assert_eq!(costs.total_usd_micros, 10_935_000_u128);
}

#[test]
fn dated_model_ids_match_their_stable_alias_entry() {
    let table = CostTable::builtin();
    let stable = table.lookup("anthropic", "claude-sonnet-4-5");
    let dated = table.lookup("anthropic", "claude-sonnet-4-5-20250929");
    assert!(stable.is_some(), "stable alias must be in the table");
    assert_eq!(
        stable, dated,
        "dated snapshot ids resolve to the alias entry"
    );
}

#[test]
fn prefix_match_never_crosses_a_version_boundary() {
    let table = CostTable::builtin();
    assert!(
        table.lookup("anthropic", "claude-sonnet-4").is_none(),
        "'claude-sonnet-4' is a different model, not a prefix of the 4-5 entry"
    );
    assert!(
        table.lookup("anthropic", "claude-sonnet-4-5x").is_none(),
        "a suffix that does not start a new boundary segment must not match"
    );
}

#[test]
fn unknown_model_reports_tokens_only_with_unpriced_marker() {
    let table = CostTable::builtin();
    let identity = ModelIdentity::new("openai-compat", "local-custom-70b");
    let report = table.account(&identity, usage_all_kinds());
    assert_eq!(report.priced, Priced::Unpriced, "never invented costs");
    assert_eq!(report.usage, usage_all_kinds(), "tokens still accounted");
}

#[test]
fn user_price_overrides_merge_at_runtime() {
    let table = CostTable::builtin().with_override(
        "openai-compat",
        "local-custom-70b",
        // 0.50 in / 2 out USD per MTok, no cache pricing.
        ModelPrices::new(500_000, 2_000_000, 0, 0),
    );
    let identity = ModelIdentity::new("openai-compat", "local-custom-70b");
    let report = table.account(&identity, usage_all_kinds());
    let Priced::Priced(costs) = report.priced else {
        panic!("override must make the model priced");
    };
    assert_eq!(costs.input_usd_micros, 500_000);
    assert_eq!(costs.output_usd_micros, 1_000_000);
    assert_eq!(costs.total_usd_micros, 1_500_000_u128);
    // Overrides merge, not replace: untouched builtin entries stay.
    assert!(table.lookup("anthropic", "claude-opus-4-5").is_some());
}

#[test]
fn every_builtin_provider_has_entries_and_zero_usage_costs_zero() {
    let table = CostTable::builtin();
    for provider in ["anthropic", "openai", "google", "xai"] {
        let report = table.account(
            &ModelIdentity::new(provider, first_model(&table, provider).as_str()),
            Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        );
        let Priced::Priced(costs) = report.priced else {
            panic!("{provider} must have builtin prices");
        };
        assert_eq!(costs.total_usd_micros, 0_u128);
    }
}

fn first_model(table: &CostTable, provider: &str) -> String {
    table
        .models(provider)
        .next()
        .unwrap_or_else(|| panic!("{provider} table must not be empty"))
        .to_owned()
}

#[test]
fn report_serializes_for_the_cost_hud() {
    let table = CostTable::builtin();
    let report: CostReport =
        table.account(&ModelIdentity::new("xai", "grok-4"), Usage::new(10, 20));
    let json = serde_json::to_value(report).expect("reports serialize");
    assert!(json.get("usage").is_some());
    assert!(json.get("priced").is_some());
}
