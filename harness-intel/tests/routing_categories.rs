//! RoutingSpec delegation categories (plan todo 13, ecosystem
//! alignment): the `categories[]` config surface, `RouteRequest::Category`
//! resolution, and the fail-closed errors — an unknown category lists the
//! valid ones, duplicate category keys are typed build errors, and the
//! effort grid validates exactly like tier routes.

use harness_intel::catalog::Catalog;
use harness_intel::routing::config::RoutingConfigError;
use harness_intel::routing::{ResolutionError, ResolutionSource, RouteRequest, RoutingTable};

const CATEGORIES: &str = r#"
{
  "provider": "anthropic",
  "cheap_pool": ["claude-sonnet-4-6"],
  "roles": [{ "role": "summarizer", "model": "claude-sonnet-4-6" }],
  "categories": [
    { "category": "quick", "model": "claude-sonnet-4-6", "effort": "low", "score_floor": 0.5 },
    { "category": "deep",  "model": "claude-opus-4-7",  "score_floor": 0.5 }
  ]
}
"#;

fn table(json: &str) -> RoutingTable {
    RoutingTable::from_json(json, &Catalog::seeded())
        .unwrap_or_else(|e| panic!("routing doc rejected: {e:?}"))
}

#[test]
fn category_request_resolves_the_configured_model_and_effort() {
    // Given: a routing doc with a `quick` category on claude-sonnet-4-6
    // at explicit effort low (and a `deep` category at its catalog
    // default effort).
    let table = table(CATEGORIES);
    // When: a Category request resolves.
    let quick = table
        .resolve(RouteRequest::Category("quick".to_owned()))
        .expect("quick resolves");
    // Then: the resolution carries the category's model + EXPLICIT
    // effort, cites the category as its source, and — like role routes,
    // unlike tiers — is NOT auto-routed (the user's config owns the
    // pick, so no routing discount applies).
    assert_eq!(quick.model.model, "claude-sonnet-4-6");
    assert_eq!(quick.model.provider, "anthropic");
    assert_eq!(quick.effort.to_string(), "low");
    assert_eq!(quick.source, ResolutionSource::Category("quick".to_owned()));
    assert!(!quick.auto_routed);

    let deep = table
        .resolve(RouteRequest::Category("deep".to_owned()))
        .expect("deep resolves");
    assert_eq!(deep.model.model, "claude-opus-4-7");
    // Omitted effort = the model's catalog default (claude-opus-4-7
    // defaults to xhigh), never the provider's implicit default.
    assert_eq!(deep.effort.to_string(), "xhigh");
}

#[test]
fn unknown_category_fails_closed_listing_valid_categories() {
    let table = table(CATEGORIES);
    // When: an unknown category key resolves.
    let error = table
        .resolve(RouteRequest::Category("nope".to_owned()))
        .expect_err("unknown category must fail closed");
    // Then: the typed error names the offending key AND lists the valid
    // categories (sorted), so the caller can steer the model.
    match &error {
        ResolutionError::UnconfiguredCategory { category, valid } => {
            assert_eq!(category, "nope");
            assert_eq!(valid, &vec!["deep".to_owned(), "quick".to_owned()]);
        }
        other => panic!("expected UnconfiguredCategory, got {other:?}"),
    }
    assert!(error.to_string().contains("valid categories"));
}

#[test]
fn duplicate_category_keys_are_typed_build_errors() {
    let json = r#"
    {
      "provider": "anthropic",
      "categories": [
        { "category": "quick", "model": "claude-sonnet-4-6", "score_floor": 0.0 },
        { "category": "quick", "model": "claude-opus-4-7", "score_floor": 0.0 }
      ]
    }
    "#;
    let error = RoutingTable::from_json(json, &Catalog::seeded())
        .expect_err("duplicate categories must fail closed");
    // Conflicting routes fail closed, never silently overwrite.
    match error {
        RoutingConfigError::DuplicateCategory { category } => {
            assert_eq!(category, "quick");
        }
        other => panic!("expected DuplicateCategory, got {other:?}"),
    }
}

#[test]
fn category_effort_off_the_grid_fails_closed() {
    // claude-opus-4-7's grid has no `none` rung (catalog discipline R5).
    let json = r#"
    {
      "provider": "anthropic",
      "categories": [
        { "category": "quick", "model": "claude-opus-4-7", "effort": "none", "score_floor": 0.0 }
      ]
    }
    "#;
    let error = RoutingTable::from_json(json, &Catalog::seeded())
        .expect_err("illegal effort must fail closed");
    assert!(
        matches!(error, RoutingConfigError::Trap(_)),
        "expected a trap violation, got {error:?}"
    );
}

#[test]
fn absent_categories_field_keeps_existing_documents_parsing() {
    // Pre-todo-13 documents (roles/tiers only) parse unchanged; an
    // absent categories list is an empty category surface.
    let legacy = r#"
    {
      "provider": "anthropic",
      "cheap_pool": ["claude-sonnet-4-6"],
      "roles": [{ "role": "summarizer", "model": "claude-sonnet-4-6" }]
    }
    "#;
    let table = table(legacy);
    let error = table
        .resolve(RouteRequest::Category("quick".to_owned()))
        .expect_err("no categories configured");
    match error {
        ResolutionError::UnconfiguredCategory { valid, .. } => {
            assert!(valid.is_empty(), "no valid categories: {valid:?}");
        }
        other => panic!("expected UnconfiguredCategory, got {other:?}"),
    }
}
