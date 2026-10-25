//! Curated-agent role taxonomy (todo 14, ecosystem alignment):
//! `explore` and `librarian` are UTILITY roles, so the routing table's
//! cheap-pool validation (fail-closed at build) is LIVE for them — an
//! explore/librarian route pinned to a frontier model is rejected
//! before any request exists, and an on-pool route resolves under
//! `RouteRequest::Role` exactly like every other role.

use austringer_intel::catalog::Catalog;
use austringer_intel::routing::resolve::ResolutionError;
use austringer_intel::routing::table::RoutingConfigError;
use austringer_intel::routing::{Role, RoleClass, RouteRequest, RoutingTable};

#[test]
fn curated_agent_keys_parse_as_utility_roles() {
    // Given/When: the curated keys.
    // Then: they parse, round-trip their keys, and classify UTILITY —
    // the class the cheap-pool validation keys on.
    for key in ["explore", "librarian"] {
        let role = Role::from_key(key).unwrap_or_else(|| panic!("{key} must parse"));
        assert_eq!(role.key(), key);
        assert_eq!(role.class(), RoleClass::Utility, "{key} must be utility");
    }
    // And unknown keys still never guess.
    assert!(Role::from_key("explorer-agent").is_none());
}

#[test]
fn table_rejects_explore_pinned_off_the_cheap_pool() {
    // Given: an explore role route pointing at a frontier model while
    // the cheap pool holds a different one.
    let json = r#"
    {
      "provider": "anthropic",
      "cheap_pool": ["gpt-5-5"],
      "roles": [ { "role": "explore", "model": "claude-opus-4-7" } ]
    }
    "#;
    // When/Then: build fails typed — the curated agents are utility
    // roles, the utility-roles-must-be-cheap rule fires at build.
    match RoutingTable::from_json(json, &Catalog::seeded()) {
        Err(RoutingConfigError::UtilityOffCheap { role, model }) => {
            assert_eq!(role, "explore");
            assert_eq!(model, "claude-opus-4-7");
        }
        other => panic!("expected UtilityOffCheap, got {other:?}"),
    }
}

#[test]
fn table_rejects_librarian_pinned_off_the_cheap_pool() {
    let json = r#"
    {
      "provider": "anthropic",
      "cheap_pool": ["gpt-5-5"],
      "roles": [ { "role": "librarian", "model": "claude-sonnet-4-6" } ]
    }
    "#;
    match RoutingTable::from_json(json, &Catalog::seeded()) {
        Err(RoutingConfigError::UtilityOffCheap { role, model }) => {
            assert_eq!(role, "librarian");
            assert_eq!(model, "claude-sonnet-4-6");
        }
        other => panic!("expected UtilityOffCheap, got {other:?}"),
    }
}

#[test]
fn curated_roles_resolve_on_the_cheap_pool() {
    // Given: both curated roles routed on the cheap pool at explicit
    // efforts.
    let json = r#"
    {
      "provider": "anthropic",
      "cheap_pool": ["gpt-5-5", "deepseek-reasoner"],
      "roles": [
        { "role": "explore",   "model": "gpt-5-5", "effort": "low" },
        { "role": "librarian", "model": "deepseek-reasoner", "effort": "low" }
      ]
    }
    "#;
    let table =
        RoutingTable::from_json(json, &Catalog::seeded()).unwrap_or_else(|e| panic!("{e:?}"));
    // When/Then: both resolve under Role requests with their configured
    // models — user-config routes, never auto-routed.
    for (role, model) in [
        (Role::Explore, "gpt-5-5"),
        (Role::Librarian, "deepseek-reasoner"),
    ] {
        let resolution = table
            .resolve(RouteRequest::Role(role))
            .unwrap_or_else(|e| panic!("{role:?}: {e:?}"));
        assert_eq!(resolution.model.model, model);
        assert_eq!(resolution.model.provider, "anthropic");
        assert!(!resolution.auto_routed);
    }
    // And: an unrouted curated role refuses typed (no guessing).
    let bare = r#"{ "provider": "p", "cheap_pool": ["gpt-5-5"] }"#;
    let bare =
        RoutingTable::from_json(bare, &Catalog::seeded()).unwrap_or_else(|e| panic!("{e:?}"));
    match bare.resolve(RouteRequest::Role(Role::Explore)) {
        Err(ResolutionError::UnconfiguredRole { role }) => assert_eq!(role, Role::Explore),
        other => panic!("expected UnconfiguredRole, got {other:?}"),
    }
}
