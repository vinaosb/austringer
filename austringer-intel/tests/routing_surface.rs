//! Routing surfaces (plan todo 42): the on-demand model-catalog search
//! tool (bounded hydration — the full catalog is NEVER injected into
//! context), the fusion sidekick pair with compaction-boundary switch
//! gating, routing-incentive pricing, and the surfaced hop audit.

use austringer_intel::catalog::Catalog;
use austringer_intel::routing::search::{
    MATCH_LIMIT, MODEL_CATALOG_SEARCH_TOOL, render_observation, search,
};
use austringer_intel::routing::sidekick::{SidekickRoute, gate_model_switch};
use austringer_intel::routing::table::RoutingConfigError;
use austringer_intel::routing::{
    RoutingTable,
    dispatch::{AUTO_ROUTE_DISCOUNT, price_usage},
};
use austringer_providers::identity::ModelIdentity;

const VALID: &str = r#"
{
  "provider": "anthropic",
  "cheap_pool": ["gpt-5-5"],
  "roles": [ { "role": "architect", "model": "claude-opus-4-7" } ],
  "fallbacks": [
    { "model": "claude-opus-4-7", "chain": ["claude-opus-5", "claude-sonnet-4-6"] }
  ],
  "sidekick": { "lead": "claude-opus-5", "sidekick": "gpt-5-5" }
}
"#;

fn table() -> RoutingTable {
    RoutingTable::from_json(VALID, &Catalog::seeded())
        .unwrap_or_else(|e| panic!("valid config rejected: {e:?}"))
}

#[test]
fn catalog_search_hydrates_bounded_matches() {
    // Given: the seeded catalog behind the search tool.
    let catalog = Catalog::seeded();
    // When: the model searches for opus models.
    let matches = search(&catalog, "opus");
    // Then: bounded, sorted, structured results — never the full catalog.
    let ids: Vec<&str> = matches.iter().map(|m| m.model.as_str()).collect();
    assert_eq!(
        ids,
        [
            "claude-opus-4-6",
            "claude-opus-4-7",
            "claude-opus-5",
            "claude-opus-5-5"
        ]
    );
    assert!(matches.len() <= MATCH_LIMIT);
    assert_eq!(matches[0].family, "anthropic");
    assert_eq!(matches[0].default_effort.to_string(), "high");
    assert_eq!(MODEL_CATALOG_SEARCH_TOOL, "model_catalog_search");
    // And family-word queries match too (todo-10 wave added a fourth
    // gemini and a second grok).
    assert_eq!(search(&catalog, "gemini").len(), 4);
    assert_eq!(search(&catalog, "grok").len(), 2);
}

#[test]
fn catalog_search_never_dumps_the_full_catalog() {
    let catalog = Catalog::seeded();
    // Empty and whitespace queries hydrate NOTHING.
    assert!(search(&catalog, "").is_empty());
    assert!(search(&catalog, "   ").is_empty());
    // Misses hydrate nothing.
    assert!(search(&catalog, "banana").is_empty());
    // Unicode garbage is safe and hydrates nothing.
    assert!(search(&catalog, "\u{1f916}\u{2728}").is_empty());
    // The render is bounded by the matches it was handed (two grok
    // models since the todo-10 wave).
    let none = render_observation(&[]);
    assert!(none.contains("no models matched"));
    let some = render_observation(&search(&catalog, "grok"));
    assert_eq!(some.lines().count(), 2);
    assert!(some.contains("grok-4-6"));
    assert!(some.contains("grok-4-7"));
    assert!(some.contains("xai"));
}

#[test]
fn sidekick_pair_resolves_and_must_be_cheap() {
    // Given: the fusion sidekick config.
    let table = table();
    let pair = table.sidekick().expect("sidekick configured");
    assert_eq!(
        pair,
        &SidekickRoute {
            lead: ModelIdentity::new("anthropic", "claude-opus-5"),
            sidekick: ModelIdentity::new("anthropic", "gpt-5-5"),
        }
    );
    // When: the sidekick targets a frontier model.
    let greedy = VALID.replace(
        "\"sidekick\": \"gpt-5-5\"",
        "\"sidekick\": \"claude-opus-4-7\"",
    );
    // Then: build fails typed — the sidekick is the cheap cached context.
    match RoutingTable::from_json(&greedy, &Catalog::seeded()) {
        Err(RoutingConfigError::SidekickNotCheap { model }) => assert_eq!(model, "claude-opus-4-7"),
        other => panic!("wrong outcome: {other:?}"),
    }
}

#[test]
fn sidekick_lead_switches_only_at_compaction_boundaries() {
    let current = ModelIdentity::new("anthropic", "claude-opus-5");
    let target = ModelIdentity::new("anthropic", "claude-sonnet-4-6");
    // At a compaction boundary the switch is free (cache miss happens
    // anyway) and executes.
    assert_eq!(
        gate_model_switch(&current, &target, true).expect("gated"),
        target
    );
    // Mid-run it is rejected typed — never a silent cache-busting switch.
    let rejected = gate_model_switch(&current, &target, false).expect_err("fail closed");
    assert_eq!(rejected.current, "claude-opus-5");
    assert_eq!(rejected.target, "claude-sonnet-4-6");
}

#[test]
fn auto_routed_usage_is_priced_below_manual_picks() {
    // The ~10% Copilot-style discount: user economics favor the router.
    assert!((AUTO_ROUTE_DISCOUNT - 0.10).abs() < 1e-9);
    let auto = price_usage(100.0, true);
    assert_eq!(auto.list_cost, 100.0);
    assert_eq!(auto.effective_cost, 90.0);
    assert_eq!(auto.savings, 10.0);
    let manual = price_usage(100.0, false);
    assert_eq!(manual.effective_cost, 100.0);
    assert_eq!(manual.savings, 0.0);
}

#[test]
fn every_hop_is_surfaced_in_the_audit_trail() {
    use austringer_intel::routing::dispatch::Dispatcher;
    use austringer_intel::routing::fallback::{DispatchFailure, FallbackDecision, TurnOutcome};
    use austringer_intel::routing::resolve::TaskId;
    use austringer_intel::routing::{Role, RouteRequest};

    // Given: an architect task whose model outage walks one acknowledged hop.
    let mut d = Dispatcher::new(table());
    let task = TaskId::new("t");
    let dispatch = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    let ask = match d.observe_turn(
        &task,
        &dispatch.model,
        TurnOutcome::Failed {
            failure: DispatchFailure::Outage,
        },
    ) {
        FallbackDecision::AskFallback(fr) => fr,
        other => panic!("expected ask, got {other:?}"),
    };
    d.acknowledge_fallback(&task, ask.acknowledge())
        .expect("ack");
    // Then: the audit trail surfaces EVERY hop in order — proposal,
    // acknowledgement — nothing switches silently.
    let audit = d.audit();
    assert_eq!(audit.len(), 2, "audit: {audit:?}");
    assert_eq!(audit[0].kind(), "hop_proposed");
    assert_eq!(audit[0].from(), "claude-opus-4-7");
    assert_eq!(audit[0].to(), "claude-opus-5");
    assert_eq!(audit[1].kind(), "hop_acknowledged");
    assert_eq!(audit[1].from(), "claude-opus-4-7");
    assert_eq!(audit[1].to(), "claude-opus-5");
    // And a declined proposal surfaces too.
    let dispatch2 = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    match d.observe_turn(
        &task,
        &dispatch2.model,
        TurnOutcome::Failed {
            failure: DispatchFailure::Outage,
        },
    ) {
        FallbackDecision::AskFallback(_) => {}
        other => panic!("expected ask, got {other:?}"),
    }
    d.decline_fallback(&task);
    let audit = d.audit();
    assert_eq!(audit.len(), 4, "audit: {audit:?}");
    assert_eq!(audit[3].kind(), "hop_declined");
}
