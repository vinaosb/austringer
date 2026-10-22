//! Curated cheap-pool read-only agents (todo 14, ecosystem
//! alignment): `explore` (fast codebase search — search tools only)
//! and `librarian` (docs/OSS search over the existing research
//! surface) ship as curated role declarations with READ-ONLY tool
//! allowlists — no writes, no command execution, no delegation — and
//! reserved names a spec cannot redeclare.

use harness_orchestration::authorize::CommandExecutionPolicy;
use harness_orchestration::role::{RoleDef, RoleRegistry, Tier};
use harness_orchestration::spec::RoleDecl;

/// Every tool a curated agent must NEVER carry (the write/exec/
/// delegation vocabulary).
const FORBIDDEN: &[&str] = &["edit_lines", "write", "edit", "execute_command", "task"];

#[test]
fn curated_agents_are_registered_with_read_only_allowlists() {
    // Given: the builtin registry (explorer/worker/deep + the curated
    // pair from todo 14).
    let registry = RoleRegistry::builtin();
    // Then: both curated names resolve with read-only shapes.
    let explore = registry.get("explore").expect("explore registered");
    assert_eq!(explore.tier, Tier::Explorer);
    assert_eq!(explore.policy, CommandExecutionPolicy::Off);
    assert_eq!(explore.summary_budget, 1024);
    // Search tools only: the read/query/search surface.
    assert_eq!(
        explore.tools,
        ["view_window", "lsp_query", "search_sessions", "tool_search"]
            .map(String::from)
            .to_vec()
    );
    let librarian = registry.get("librarian").expect("librarian registered");
    assert_eq!(librarian.tier, Tier::Explorer);
    assert_eq!(librarian.policy, CommandExecutionPolicy::Off);
    // Docs/OSS search: the read + search surface (no code-intel LSP).
    assert_eq!(
        librarian.tools,
        ["view_window", "search_sessions", "tool_search"]
            .map(String::from)
            .to_vec()
    );
    // And: neither agent carries any write/exec/delegation tool, and
    // both default to a cheap bracket (the routing table's cheap-pool
    // validation owns the wire-side pinning).
    for name in ["explore", "librarian"] {
        let def = registry.get(name).unwrap_or_else(|| panic!("{name}"));
        assert!(
            def.tools
                .iter()
                .all(|tool| !FORBIDDEN.contains(&tool.as_str())),
            "{name} allowlist must be read-only: {:?}",
            def.tools
        );
        assert_eq!(
            def.model.as_deref(),
            Some("claude-haiku-5-5"),
            "{name} cheap bracket"
        );
    }
    // And: RoleDef::curated answers exactly the reserved pair.
    assert!(RoleDef::curated("explore").is_some());
    assert!(RoleDef::curated("librarian").is_some());
    assert!(RoleDef::curated("worker").is_none());
    assert!(RoleDef::curated("explore-agent").is_none());
}

#[test]
fn curated_names_are_reserved_against_spec_role_declarations() {
    // Given: the builtin registry (which now carries the curated pair)
    // and a spec declaration trying to redefine `explore`.
    let decl = RoleDecl {
        name: "explore".to_owned(),
        model: Some("claude-opus-4-7".to_owned()),
        tier: Some(Tier::Deep),
        tools: vec!["edit_lines".to_owned()],
        summary: Some(4096),
    };
    // When/Then: merging fails typed — the curated names are reserved
    // (a redefinition could smuggle writes or a frontier model past
    // the curated contract).
    let merged = RoleRegistry::with_declarations(&RoleRegistry::builtin(), &[decl]);
    match merged {
        Err(harness_orchestration::ResolveError::DuplicateRole(name)) => {
            assert_eq!(name, "explore");
        }
        other => panic!("expected DuplicateRole, got {other:?}"),
    }
}

#[test]
fn builtin_registry_lists_the_curated_pair_alongside_the_tiers() {
    let mut names = RoleRegistry::builtin().names();
    names.sort();
    assert_eq!(
        names,
        vec![
            "deep".to_owned(),
            "explore".to_owned(),
            "explorer".to_owned(),
            "librarian".to_owned(),
            "worker".to_owned(),
        ]
    );
}
