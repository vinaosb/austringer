//! Failure-path suite (plan todo 28): degenerate inputs flow through
//! the FULL facade (graph → PageRank → render) as typed, bounded,
//! degraded output — never a panic. Goes beyond the malformed-tables
//! test (repomap.rs `malformed_tables_flow_through_the_facade_bounded`,
//! empty names + ghost refs only) by covering an empty graph under a
//! focus set, self-loop-shaped references, and symbol-less sources.

use harness_ast::tags::SymbolKind;
use harness_repomap::synthetic::table;
use harness_repomap::{MapOptions, repo_map};
use std::path::{Path, PathBuf};

#[test]
fn empty_graph_with_focus_degrades_to_typed_empty_map() {
    // repomap.rs already renders the no-focus empty repo; here the
    // empty graph also carries a focus set, so the focus join itself
    // must degrade typed: counted as unmatched, not indexed or fatal.
    let output = repo_map(
        Vec::<(PathBuf, harness_ast::tags::TagTable)>::new(),
        &MapOptions {
            focus: vec![PathBuf::from("src/gone.rs")],
            ..MapOptions::default()
        },
    );
    assert_eq!(
        output.focus_matched, 0,
        "unknown focus is counted, not fatal"
    );
    assert_eq!(
        (output.text.as_str(), output.tokens, output.density),
        ("", 0, 0),
        "empty graph renders nothing at density 0"
    );
    assert!(
        output.ranking.values.is_empty() && output.ranking.converged,
        "empty solve is trivially converged"
    );
    let stats = output.graph.stats();
    assert_eq!(
        (
            stats.files,
            stats.symbols,
            stats.edges,
            stats.skipped_empty_names,
            stats.duplicate_paths
        ),
        (0, 0, 0, 0, 0),
        "every counter typed-zero, nothing silently invented"
    );
}

#[test]
fn self_loop_only_repo_drops_intra_file_refs_and_ranks_uniformly() {
    // Every reference either names a symbol defined in the SAME file
    // (self-loop) or a name defined nowhere (ghost): neither may
    // produce an edge, yet the repo still ranks and renders bounded.
    let output = repo_map(
        vec![
            (
                PathBuf::from("src/b.rs"),
                table(
                    &[("beta", SymbolKind::Function)],
                    &["beta", "beta", "std_ghost"],
                ),
            ),
            (
                PathBuf::from("src/a.rs"),
                table(&[("alpha", SymbolKind::Struct)], &["alpha"]),
            ),
        ],
        &MapOptions::default(),
    );

    let stats = output.graph.stats();
    assert_eq!(stats.edges, 0, "self-refs and ghosts never become edges");
    assert_eq!(stats.files, 2, "both files stay in the graph");
    assert!(
        !output.graph.refs_by_name().contains_key("alpha")
            && !output.graph.refs_by_name().contains_key("beta"),
        "names whose references never leave their definer are not kept"
    );

    let values = &output.ranking.values;
    assert_eq!(values.len(), 2);
    assert_eq!(
        values[0], values[1],
        "symmetric all-dangling graph is bit-uniform"
    );
    assert!((values[0] - 0.5).abs() < 1e-9, "uniform ranks: {values:?}");
    assert!(output.ranking.converged);

    assert_eq!(
        output.text, "src/a.rs:\n    alpha:struct\nsrc/b.rs:\n    beta:function",
        "uniform ranks => pure path order; definitions still render"
    );
}

#[test]
fn self_loops_coexist_with_cross_edges_without_false_adjacency() {
    // The hub references its own symbol (self-loop) while a real
    // consumer cross-references it: the self-loop must not create hub
    // self-adjacency, and the consumer edge must keep its full weight
    // and steer the ranking through the facade.
    let output = repo_map(
        vec![
            (
                PathBuf::from("src/hub.rs"),
                table(
                    &[("connect", SymbolKind::Function)],
                    &["connect", "connect"],
                ),
            ),
            (
                PathBuf::from("src/user.rs"),
                table(&[], &["connect", "connect", "connect"]),
            ),
        ],
        &MapOptions::default(),
    );

    let hub = output
        .graph
        .node_id(Path::new("src/hub.rs"))
        .unwrap_or_else(|| panic!("hub missing"));
    let user = output
        .graph
        .node_id(Path::new("src/user.rs"))
        .unwrap_or_else(|| panic!("user missing"));
    assert_eq!(
        output.graph.edge_weight(hub, hub),
        None,
        "no self adjacency"
    );
    assert_eq!(
        output.graph.edge_weight(user, user),
        None,
        "no self adjacency"
    );
    assert_eq!(
        output.graph.edge_weight(hub, user),
        Some(3.0),
        "cross-file references keep their full count as weight"
    );
    assert_eq!(output.graph.stats().edges, 1);
    assert!(output.ranking.values.iter().all(|v| v.is_finite()) && output.ranking.converged);
}

#[test]
fn symbol_less_source_renders_bare_header_and_stays_focusable() {
    // A source whose tag table carries neither definitions nor
    // references (empty, minified, or binary-ish input) keeps its
    // node, renders a bare header, and remains a legal focus target —
    // degraded, not dropped — while healthy siblings rank normally.
    let output = repo_map(
        vec![
            (PathBuf::from("src/empty.rs"), table(&[], &[])),
            (
                PathBuf::from("src/data.rs"),
                table(&[("query", SymbolKind::Function)], &[]),
            ),
            (
                PathBuf::from("src/user.rs"),
                table(&[("run", SymbolKind::Function)], &["query", "query"]),
            ),
        ],
        &MapOptions {
            focus: vec![PathBuf::from("src/empty.rs")],
            ..MapOptions::default()
        },
    );

    let empty = output
        .graph
        .node_id(Path::new("src/empty.rs"))
        .unwrap_or_else(|| panic!("symbol-less source dropped from the graph"));
    assert!(output.graph.definitions_of(empty).is_empty());
    assert_eq!(
        output.focus_matched, 1,
        "symbol-less file is still focusable"
    );

    // Focused empty.rs holds all personalization mass (all-dangling
    // returns to p), so it renders first; the data↔user pair ties at
    // zero and orders by path.
    assert_eq!(
        output.text,
        "src/empty.rs:\nsrc/data.rs:\n    query:function\nsrc/user.rs:\n    run:function",
        "bare header for the symbol-less file, full lines elsewhere"
    );
    assert!(output.ranking.values.iter().all(|v| v.is_finite()));
    assert!(output.ranking.converged);
}
