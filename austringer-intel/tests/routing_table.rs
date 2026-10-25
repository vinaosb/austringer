//! Routing-table construction and resolution (plan todo 42): roles hit
//! their configured models with EXPLICIT efforts, utility roles stay on
//! the cheap pool, complexity-tier routing (Droid) and benchmark-driven
//! live routing (Kilo) resolve per todo semantics, and every config
//! mistake fails closed with a typed error.

use std::time::Duration;

use austringer_intel::catalog::{Catalog, EffortValue};
use austringer_intel::routing::config::RoutingSpec;
use austringer_intel::routing::resolve::ResolutionError;
use austringer_intel::routing::table::RoutingConfigError;
use austringer_intel::routing::{Role, RoleClass, RouteRequest, RoutingTable, Tier};
use austringer_intel::taxonomy::FailureTaxonomy;
use austringer_providers::identity::ModelIdentity;

/// The reference fixture: every role routed, three tiers, verbatim
/// fallback chains, a benchmark table, a sidekick pair.
const VALID: &str = r#"
{
  "provider": "anthropic",
  "cheap_pool": ["gpt-5-5", "deepseek-reasoner"],
  "roles": [
    { "role": "architect",     "model": "claude-opus-4-7" },
    { "role": "editor",        "model": "deepseek-reasoner" },
    { "role": "reviewer",      "model": "claude-sonnet-4-6", "effort": "high" },
    { "role": "hydrator",      "model": "gpt-5-5", "effort": "low" },
    { "role": "summarizer",    "model": "gpt-5-5", "effort": "low" },
    { "role": "default",       "model": "claude-opus-5" },
    { "role": "subagent",      "model": "claude-sonnet-4-6" },
    { "role": "inline",        "model": "gpt-5-5", "effort": "medium" },
    { "role": "commit-message","model": "gpt-5-5", "effort": "none" },
    { "role": "session-title", "model": "gpt-5-5", "effort": "none" },
    { "role": "auto-continue", "model": "deepseek-reasoner", "effort": "low" },
    { "role": "dedup-folding", "model": "gpt-5-5", "effort": "low" },
    { "role": "summary",       "model": "gpt-5-5", "effort": "low" },
    { "role": "compaction",    "model": "gpt-5-5", "effort": "low" }
  ],
  "tiers": [
    { "tier": "light",  "model": "gpt-5-5",          "effort": "low",   "score_floor": 0.60 },
    { "tier": "medium", "model": "claude-sonnet-4-6",                     "score_floor": 0.80 },
    { "tier": "heavy",  "model": "claude-opus-4-7",                      "score_floor": 0.92 }
  ],
  "fallbacks": [
    { "model": "claude-opus-4-7",   "chain": ["claude-opus-5", "claude-sonnet-4-6"] },
    { "model": "deepseek-reasoner", "chain": ["claude-sonnet-4-6"] }
  ],
  "rate_limit_wait_out_secs": 45,
  "retry_attempts": 2,
  "benchmarks": [
    { "model": "claude-sonnet-4-6", "score": 0.85, "price_per_mtok": 3.0 },
    { "model": "gpt-5-5",           "score": 0.70, "price_per_mtok": 1.2 },
    { "model": "claude-opus-4-7",   "score": 0.95, "price_per_mtok": 15.0 }
  ],
  "sidekick": { "lead": "claude-opus-5", "sidekick": "gpt-5-5" }
}
"#;

fn table(json: &str) -> RoutingTable {
    RoutingTable::from_json(json, &Catalog::seeded())
        .unwrap_or_else(|e| panic!("valid config rejected: {e:?}"))
}

fn valid() -> RoutingTable {
    table(VALID)
}

/// Patch one role route inside the fixture (string replace on the model).
fn with_role_model(role: &str, model: &str) -> String {
    let spec: RoutingSpec = serde_json::from_str(VALID).expect("fixture parses");
    let mut patched = spec;
    patched
        .roles
        .iter_mut()
        .find(|r| r.role == role)
        .expect("role present")
        .model = model.to_owned();
    serde_json::to_string(&patched).expect("reserializes")
}

#[test]
fn roles_hit_configured_models_with_explicit_efforts() {
    // Given/When: the reference table.
    let table = valid();
    let cases: &[(&str, &str, EffortValue)] = &[
        // architect: catalog default effort, never the provider implicit one.
        ("architect", "claude-opus-4-7", EffortValue::Xhigh),
        ("editor", "deepseek-reasoner", EffortValue::Medium),
        // reviewer: explicit effort beats the catalog default.
        ("reviewer", "claude-sonnet-4-6", EffortValue::High),
        ("hydrator", "gpt-5-5", EffortValue::Low),
        ("commit-message", "gpt-5-5", EffortValue::None),
        ("session-title", "gpt-5-5", EffortValue::None),
        ("auto-continue", "deepseek-reasoner", EffortValue::Low),
        ("compaction", "gpt-5-5", EffortValue::Low),
    ];
    for (role, model, effort) in cases {
        let role = Role::from_key(role).expect("known role key");
        let resolution = table.resolve(RouteRequest::Role(role)).expect("resolves");
        // Then: OBSERVED resolution carries the configured identity and knob.
        assert_eq!(
            resolution.model,
            ModelIdentity::new("anthropic", model),
            "{role:?}"
        );
        assert_eq!(resolution.effort, *effort, "{role:?}");
        assert_eq!(resolution.knobs.effort, Some(*effort), "{role:?}");
        assert_eq!(resolution.model.model, *model);
    }
}

#[test]
fn utility_roles_are_classified_and_stay_cheap() {
    // Given: the taxonomy — lead vs utility (Plandex + Zed union).
    let utility = [
        "hydrator",
        "summarizer",
        "commit-message",
        "session-title",
        "auto-continue",
        "dedup-folding",
        "summary",
        "compaction",
    ];
    let lead = [
        "architect",
        "editor",
        "reviewer",
        "default",
        "subagent",
        "inline",
    ];
    for key in utility {
        assert_eq!(
            Role::from_key(key).expect("key").class(),
            RoleClass::Utility
        );
    }
    for key in lead {
        assert_eq!(Role::from_key(key).expect("key").class(), RoleClass::Lead);
    }
    // And unknown keys never guess.
    assert!(Role::from_key("wizard").is_none());
    assert!(Role::from_key("\u{2728}architect").is_none());
}

#[test]
fn utility_role_off_cheap_pool_fails_closed() {
    // Given: hydrator (utility) routed to a frontier model.
    let json = with_role_model("hydrator", "claude-opus-4-7");
    // When/Then: build fails typed — utility roles live on cheap models.
    let err = RoutingTable::from_json(&json, &Catalog::seeded()).expect_err("fail closed");
    match err {
        RoutingConfigError::UtilityOffCheap { role, model } => {
            assert_eq!(role, "hydrator");
            assert_eq!(model, "claude-opus-4-7");
        }
        other => panic!("wrong error: {other:?}"),
    }
}

#[test]
fn unknown_role_and_unknown_model_are_typed() {
    let bad_role = r#"{ "provider": "p", "roles": [ { "role": "wizard", "model": "gpt-5-5" } ] }"#;
    match RoutingTable::from_json(bad_role, &Catalog::seeded()) {
        Err(RoutingConfigError::UnknownRole { role }) => assert_eq!(role, "wizard"),
        other => panic!("wrong outcome: {other:?}"),
    }
    let bad_model = with_role_model("architect", "gpt-4");
    match RoutingTable::from_json(&bad_model, &Catalog::seeded()) {
        Err(RoutingConfigError::UnknownModel(e)) => assert_eq!(e.model, "gpt-4"),
        other => panic!("wrong outcome: {other:?}"),
    }
}

#[test]
fn duplicate_role_entries_conflict_fails_closed() {
    let dup = r#"{ "provider": "p",
        "roles": [ { "role": "editor", "model": "gpt-5-5" },
                   { "role": "editor", "model": "deepseek-reasoner" } ] }"#;
    match RoutingTable::from_json(dup, &Catalog::seeded()) {
        Err(RoutingConfigError::DuplicateRole { role }) => assert_eq!(role, "editor"),
        other => panic!("wrong outcome: {other:?}"),
    }
    let dup_tier = r#"{ "provider": "p",
        "tiers": [ { "tier": "light", "model": "gpt-5-5", "score_floor": 0.0 },
                   { "tier": "light", "model": "deepseek-reasoner", "score_floor": 0.0 } ] }"#;
    match RoutingTable::from_json(dup_tier, &Catalog::seeded()) {
        Err(RoutingConfigError::DuplicateTier { tier }) => assert_eq!(tier, "light"),
        other => panic!("wrong outcome: {other:?}"),
    }
}

#[test]
fn rebuilt_table_reflects_a_reloaded_config() {
    // Given: a table built from the fixture.
    let before = valid();
    let architect_before = before
        .resolve(RouteRequest::Role(Role::Architect))
        .expect("resolves");
    assert_eq!(architect_before.model.model, "claude-opus-4-7");
    // When: the config file changes (architect moves) and is reloaded.
    let edited = with_role_model("architect", "claude-opus-5");
    let after = table(&edited);
    // Then: the rebuilt table routes to the NEW model — no stale
    // resolution survives a reload.
    let architect_after = after
        .resolve(RouteRequest::Role(Role::Architect))
        .expect("resolves");
    assert_eq!(architect_after.model.model, "claude-opus-5");
    assert_eq!(architect_after.effort, EffortValue::High);
}

#[test]
fn wait_out_over_cap_and_bad_chains_fail_closed() {
    let over = r#"{ "provider": "p", "rate_limit_wait_out_secs": 76 }"#;
    match RoutingTable::from_json(over, &Catalog::seeded()) {
        Err(RoutingConfigError::WaitOutTooLong { secs }) => assert_eq!(secs, 76),
        other => panic!("wrong outcome: {other:?}"),
    }
    let self_ref = r#"{ "provider": "p", "fallbacks": [
        { "model": "gpt-5-5", "chain": ["gpt-5-5", "claude-sonnet-4-6"] } ] }"#;
    match RoutingTable::from_json(self_ref, &Catalog::seeded()) {
        Err(RoutingConfigError::SelfReferentialChain { model }) => assert_eq!(model, "gpt-5-5"),
        other => panic!("wrong outcome: {other:?}"),
    }
    let dup_hop = r#"{ "provider": "p", "fallbacks": [
        { "model": "gpt-5-5", "chain": ["claude-sonnet-4-6", "claude-sonnet-4-6"] } ] }"#;
    match RoutingTable::from_json(dup_hop, &Catalog::seeded()) {
        Err(RoutingConfigError::DuplicateHop { model }) => assert_eq!(model, "gpt-5-5"),
        other => panic!("wrong outcome: {other:?}"),
    }
}

#[test]
fn malformed_config_is_bounded_and_typed() {
    for json in ["{ not json", r#"{ "provider": 42 }"#, r#"{ "roles": 7 }"#] {
        match RoutingTable::from_json(json, &Catalog::seeded()) {
            Err(RoutingConfigError::Malformed { detail }) => {
                assert!(detail.len() < 512, "bounded detail, got {}", detail.len());
            }
            other => panic!("wrong outcome for {json}: {other:?}"),
        }
    }
}

#[test]
fn complexity_tiers_route_per_tier_defaults() {
    // Given: Droid-style light/medium/heavy tiers in the table.
    let table = valid();
    // When/Then: each tier resolves to its declared model+effort; medium
    // inherits the catalog default effort (explicit, not implicit).
    let light = table
        .resolve(RouteRequest::Tier(Tier::Light))
        .expect("light");
    assert_eq!(light.model.model, "gpt-5-5");
    assert_eq!(light.effort, EffortValue::Low);
    assert!(light.auto_routed);
    let medium = table
        .resolve(RouteRequest::Tier(Tier::Medium))
        .expect("medium");
    assert_eq!(medium.model.model, "claude-sonnet-4-6");
    assert_eq!(medium.effort, EffortValue::Medium);
    let heavy = table
        .resolve(RouteRequest::Tier(Tier::Heavy))
        .expect("heavy");
    assert_eq!(heavy.model.model, "claude-opus-4-7");
    assert_eq!(heavy.effort, EffortValue::Xhigh);
}

#[test]
fn benchmark_routing_picks_cheapest_proven_model() {
    // Given: the benchmark table (light floor 0.60, heavy floor 0.92).
    let table = valid();
    // When: benchmark-driven live routing.
    let light = table
        .resolve(RouteRequest::Benchmarked(Tier::Light))
        .expect("light");
    // Then: cheapest model meeting the floor wins (gpt-5-5 at 1.2, not
    // sonnet at 3.0, not the tier default by rank).
    assert_eq!(light.model.model, "gpt-5-5");
    assert_eq!(light.effort, EffortValue::Low); // tier effort composes
    let heavy = table
        .resolve(RouteRequest::Benchmarked(Tier::Heavy))
        .expect("heavy");
    assert_eq!(heavy.model.model, "claude-opus-4-7"); // only >= 0.92
}

#[test]
fn benchmark_routing_without_candidate_fails_closed() {
    // Given: a floor no benchmarked model reaches.
    let json = VALID.replace("\"score_floor\": 0.92", "\"score_floor\": 0.99");
    let table = table(&json);
    // When/Then: typed miss — never a silent downgrade to an unproven model.
    match table.resolve(RouteRequest::Benchmarked(Tier::Heavy)) {
        Err(ResolutionError::NoBenchmarkCandidate { tier }) => assert_eq!(tier, Tier::Heavy),
        other => panic!("wrong outcome: {other:?}"),
    }
}

#[test]
fn missing_role_or_tier_resolution_is_typed() {
    // Given: a minimal table with only the editor role and the light tier.
    let minimal = r#"{ "provider": "p",
        "roles": [ { "role": "editor", "model": "deepseek-reasoner" } ],
        "tiers": [ { "tier": "light", "model": "gpt-5-5", "score_floor": 0.0 } ] }"#;
    let table = table(minimal);
    match table.resolve(RouteRequest::Role(Role::Architect)) {
        Err(ResolutionError::UnconfiguredRole { role }) => assert_eq!(role, Role::Architect),
        other => panic!("wrong outcome: {other:?}"),
    }
    match table.resolve(RouteRequest::Tier(Tier::Heavy)) {
        Err(ResolutionError::UnconfiguredTier { tier }) => assert_eq!(tier, Tier::Heavy),
        other => panic!("wrong outcome: {other:?}"),
    }
}

#[test]
fn table_exposes_verbatim_chain_and_wait_out() {
    let table = valid();
    assert_eq!(
        table.fallback_chain("claude-opus-4-7"),
        ["claude-opus-5".to_owned(), "claude-sonnet-4-6".to_owned()]
    );
    assert!(table.fallback_chain("grok-4-6").is_empty());
    assert_eq!(table.wait_out(), Duration::from_secs(45));
    assert_eq!(table.retry_attempts(), 2);
}

#[test]
fn suggestions_are_profile_aware() {
    // Given: the editor's model shows a weak-editor observed taxonomy.
    let table = valid();
    let weak_editor = FailureTaxonomy {
        turns: 40,
        edit_rejection_rate: 0.40,
        syntax_error_rate: 0.0,
        tool_misuse_rate: 0.0,
        context_overflow_rate: 0.0,
        retry_storm_rate: 0.0,
        tokens_per_turn: 900.0,
    };
    // When: profile-aware suggestions for the editor role.
    let suggestions =
        austringer_intel::routing::resolve::suggest_replacement(&table, Role::Editor, &weak_editor);
    // Then: the first verbatim hop of the model's chain is suggested with
    // the observed reason — a suggestion, never an automatic switch.
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].from, "deepseek-reasoner");
    assert_eq!(suggestions[0].to, "claude-sonnet-4-6");
    assert!(suggestions[0].reason.contains("edit rejection"));
    // And a healthy taxonomy suggests nothing.
    let healthy = FailureTaxonomy {
        turns: 40,
        edit_rejection_rate: 0.01,
        syntax_error_rate: 0.0,
        tool_misuse_rate: 0.0,
        context_overflow_rate: 0.0,
        retry_storm_rate: 0.0,
        tokens_per_turn: 900.0,
    };
    assert!(
        austringer_intel::routing::resolve::suggest_replacement(&table, Role::Editor, &healthy)
            .is_empty()
    );
}
