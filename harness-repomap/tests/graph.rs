//! File-graph construction (plan todo 15): tag tables → name-keyed
//! cross-references → weighted CSR adjacency. Asserted on CONTENT (edge
//! weights, name keys, def-site lists), never on boolean flags alone.

use harness_ast::tags::SymbolKind;
use harness_repomap::graph::{DefSite, FileGraph, SymbolLinks};
use harness_repomap::synthetic::{self, table};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn p(path: &str) -> PathBuf {
    PathBuf::from(path)
}

/// One hub file defines `connect`; three consumers reference it with
/// different multiplicities. Expected weights are hand-derived.
fn hub_repo() -> Vec<(PathBuf, harness_ast::tags::TagTable)> {
    vec![
        (
            p("src/net.rs"),
            table(&[("connect", SymbolKind::Function)], &[]),
        ),
        (
            p("src/a.rs"),
            table(&[("work", SymbolKind::Function)], &["connect", "connect"]),
        ),
        (
            p("src/b.rs"),
            table(&[("run", SymbolKind::Function)], &["connect"]),
        ),
        (
            p("src/c.rs"),
            table(&[], &["connect", "connect", "connect"]),
        ),
    ]
}

#[test]
fn cross_file_references_become_weighted_edges() {
    let graph = FileGraph::build(hub_repo());

    let net = graph
        .node_id(Path::new("src/net.rs"))
        .unwrap_or_else(|| panic!("net missing"));
    for (file, expected) in [("src/a.rs", 2.0), ("src/b.rs", 1.0), ("src/c.rs", 3.0)] {
        let id = graph
            .node_id(Path::new(file))
            .unwrap_or_else(|| panic!("{file} missing"));
        assert_eq!(
            graph.edge_weight(net, id),
            Some(expected),
            "edge {file} <-> src/net.rs"
        );
    }
    assert_eq!(graph.stats().edges, 3, "three distinct undirected edges");
}

#[test]
fn edges_are_symmetric_and_input_order_is_irrelevant() {
    let mut tables = hub_repo();
    let ordered = FileGraph::build(tables.clone());
    tables.reverse();
    let shuffled = FileGraph::build(tables);

    assert_eq!(
        ordered.files(),
        shuffled.files(),
        "nodes come from sorted paths"
    );
    assert_eq!(
        ordered.csr().row_offsets,
        shuffled.csr().row_offsets,
        "CSR identical after input reordering"
    );
    assert_eq!(ordered.csr().col_indices, shuffled.csr().col_indices);
    assert_eq!(ordered.csr().weights, shuffled.csr().weights);

    let net = ordered.node_id(Path::new("src/net.rs")).unwrap_or_default();
    let a = ordered.node_id(Path::new("src/a.rs")).unwrap_or_default();
    assert_eq!(ordered.edge_weight(net, a), ordered.edge_weight(a, net));
}

#[test]
fn refs_by_name_keys_cross_references_by_symbol_name() {
    let graph = FileGraph::build(hub_repo());

    let links = graph
        .refs_by_name()
        .get("connect")
        .unwrap_or_else(|| panic!("connect not in refs_by_name"));
    assert_eq!(
        links.def_sites,
        vec![DefSite {
            file: p("src/net.rs"),
            kind: SymbolKind::Function
        }],
        "def site carries path + kind, no node id"
    );
    let mut expected_refs = BTreeMap::new();
    expected_refs.insert(p("src/a.rs"), 2);
    expected_refs.insert(p("src/b.rs"), 1);
    expected_refs.insert(p("src/c.rs"), 3);
    assert_eq!(links.refs, expected_refs);

    assert!(
        !graph.refs_by_name().contains_key("HashMap"),
        "names never defined in the repo are not kept"
    );
    assert!(
        !graph.refs_by_name().contains_key("work"),
        "names never referenced cross-file (here: never referenced at all) are not kept"
    );
}

#[test]
fn rebuild_after_rename_and_reorder_edges_survive_via_name_keys() {
    let before = FileGraph::build(hub_repo());

    // Rename the hub file and one consumer, and shuffle ingestion order:
    // node ids change, but the symbol-name join rebuilds the same links.
    let renamed = vec![
        (
            p("src/c.rs"),
            table(&[], &["connect", "connect", "connect"]),
        ),
        (
            p("src/a.rs"),
            table(&[("work", SymbolKind::Function)], &["connect", "connect"]),
        ),
        (
            p("src/renamed/net-hub.rs"),
            table(&[("connect", SymbolKind::Function)], &[]),
        ),
        (
            p("src/b.rs"),
            table(&[("run", SymbolKind::Function)], &["connect"]),
        ),
    ];
    let after = FileGraph::build(renamed);

    let links: &SymbolLinks = after
        .refs_by_name()
        .get("connect")
        .unwrap_or_else(|| panic!("connect link lost across rename"));
    assert_eq!(
        links.def_sites,
        vec![DefSite {
            file: p("src/renamed/net-hub.rs"),
            kind: SymbolKind::Function
        }],
        "def site follows the renamed path"
    );

    let new_hub = after
        .node_id(Path::new("src/renamed/net-hub.rs"))
        .unwrap_or_default();
    let c = after.node_id(Path::new("src/c.rs")).unwrap_or_default();
    let before_hub = before.node_id(Path::new("src/net.rs")).unwrap_or_default();
    let before_c = before.node_id(Path::new("src/c.rs")).unwrap_or_default();
    assert_eq!(
        after.edge_weight(new_hub, c),
        before.edge_weight(before_hub, before_c),
        "weight survives rename through the name-keyed index"
    );
}

#[test]
fn multi_def_site_symbols_link_every_defining_file() {
    // `new` defined in two files (a re-export pattern): a reference from a
    // third file must reach BOTH def sites with the full reference count —
    // documented resolution rule (no directory-proximity guesswork).
    let graph = FileGraph::build(vec![
        (p("src/v1.rs"), table(&[("new", SymbolKind::Function)], &[])),
        (p("src/v2.rs"), table(&[("new", SymbolKind::Function)], &[])),
        (p("src/user.rs"), table(&[], &["new", "new"])),
    ]);

    let user = graph.node_id(Path::new("src/user.rs")).unwrap_or_default();
    let v1 = graph.node_id(Path::new("src/v1.rs")).unwrap_or_default();
    let v2 = graph.node_id(Path::new("src/v2.rs")).unwrap_or_default();
    assert_eq!(graph.edge_weight(user, v1), Some(2.0));
    assert_eq!(graph.edge_weight(user, v2), Some(2.0));
    assert_eq!(
        graph.edge_weight(v1, v2),
        None,
        "def sites are not linked to each other: no reference connects them"
    );

    let links = graph
        .refs_by_name()
        .get("new")
        .unwrap_or_else(|| panic!("new missing"));
    assert_eq!(links.def_sites.len(), 2, "both def sites recorded");
}

#[test]
fn duplicate_same_file_definitions_collapse_to_one_def_site() {
    let graph = FileGraph::build(vec![
        (
            p("src/impls.rs"),
            table(
                &[("fetch", SymbolKind::Method), ("fetch", SymbolKind::Method)],
                &[],
            ),
        ),
        (p("src/caller.rs"), table(&[], &["fetch"])),
    ]);

    let links = graph
        .refs_by_name()
        .get("fetch")
        .unwrap_or_else(|| panic!());
    assert_eq!(links.def_sites.len(), 1, "same-file duplicates collapse");
    assert_eq!(graph.stats().edges, 1);
}

#[test]
fn self_references_and_empty_names_are_skipped_bounded() {
    let graph = FileGraph::build(vec![
        (
            p("src/selfish.rs"),
            table(&[("me", SymbolKind::Function)], &["me", "me", ""]),
        ),
        (
            p("src/other.rs"),
            table(&[("", SymbolKind::Function)], &["me"]),
        ),
    ]);

    let selfish = graph
        .node_id(Path::new("src/selfish.rs"))
        .unwrap_or_default();
    let other = graph.node_id(Path::new("src/other.rs")).unwrap_or_default();
    assert_eq!(graph.edge_weight(selfish, selfish), None, "no self edges");
    assert_eq!(
        graph.edge_weight(selfish, other),
        Some(1.0),
        "cross-file ref still links"
    );
    assert!(
        !graph.refs_by_name().contains_key(""),
        "empty symbol names never enter the index"
    );
    assert_eq!(
        graph.stats().skipped_empty_names,
        2,
        "one empty ref + one empty def counted, not silently dropped"
    );
}

#[test]
fn unicode_names_and_weird_paths_link_normally() {
    let graph = FileGraph::build(vec![
        (
            p("src/café régional/Über.rs"),
            table(
                &[
                    ("日本語メソッド", SymbolKind::Function),
                    ("naïve", SymbolKind::Struct),
                ],
                &[],
            ),
        ),
        (
            p("src/sp ace/con su mer.rs"),
            table(&[], &["日本語メソッド", "naïve", "naïve"]),
        ),
    ]);

    let uber = graph
        .node_id(Path::new("src/café régional/Über.rs"))
        .unwrap_or_default();
    let consumer = graph
        .node_id(Path::new("src/sp ace/con su mer.rs"))
        .unwrap_or_default();
    assert_eq!(graph.edge_weight(uber, consumer), Some(3.0));
    assert!(graph.refs_by_name().contains_key("日本語メソッド"));
}

#[test]
fn zero_reference_repo_yields_edgeless_graph() {
    let graph = FileGraph::build(vec![
        (p("src/a.rs"), table(&[("one", SymbolKind::Function)], &[])),
        (p("src/b.rs"), table(&[("two", SymbolKind::Struct)], &[])),
    ]);

    assert_eq!(graph.stats().edges, 0);
    assert_eq!(graph.csr().col_indices.len(), 0);
    assert_eq!(graph.files().len(), 2, "isolated nodes still present");
}

#[test]
fn empty_repo_builds_empty_graph() {
    let graph = FileGraph::build(Vec::<(PathBuf, harness_ast::tags::TagTable)>::new());
    assert_eq!(graph.files().len(), 0);
    assert_eq!(
        graph.csr().row_offsets.len(),
        1,
        "just the terminating offset"
    );
    assert_eq!(graph.stats().edges, 0);
}

#[test]
fn duplicate_input_paths_are_rejected_visibly_not_merged() {
    // Two tables for the same path would make node identity ambiguous;
    // the builder keeps the FIRST table per path and counts the collision.
    let graph = FileGraph::build(vec![
        (p("src/a.rs"), table(&[("one", SymbolKind::Function)], &[])),
        (p("src/a.rs"), table(&[("two", SymbolKind::Function)], &[])),
    ]);
    assert_eq!(graph.files().len(), 1);
    assert_eq!(graph.stats().duplicate_paths, 1);
    let defs = graph.definitions_of(graph.node_id(Path::new("src/a.rs")).unwrap_or_default());
    assert_eq!(defs.len(), 1, "first table wins, deterministic");
}

#[test]
fn synthetic_repos_are_reproducible_and_shaped() {
    let a = FileGraph::build(synthetic::repo(50, 7));
    let b = FileGraph::build(synthetic::repo(50, 7));
    let c = FileGraph::build(synthetic::repo(50, 8));
    assert_eq!(a.files().len(), 50);
    assert_eq!(a, b, "same seed => identical graph");
    assert_ne!(a, c, "different seed => different graph");
    assert!(
        a.stats().edges > 0,
        "generator must produce a connected repo"
    );
}
