//! Signature-only rendering under a token budget (plan todo 15):
//! `path:` + `name:kind` lines, ellipsis markers, deterministic order
//! (rank desc, path asc, symbol name asc), largest density fitting the
//! budget found by binary search. Assertions on exact rendered CONTENT.

use harness_ast::tags::SymbolKind;
use harness_repomap::graph::FileGraph;
use harness_repomap::render::{RenderFit, fit_budget, render, token_count};
use harness_repomap::synthetic::table;
use std::path::PathBuf;

fn hub_graph() -> FileGraph {
    FileGraph::build(vec![
        (
            PathBuf::from("src/net.rs"),
            table(&[("connect", SymbolKind::Function)], &[]),
        ),
        (
            PathBuf::from("src/a.rs"),
            table(&[("work", SymbolKind::Function)], &["connect", "connect"]),
        ),
        (
            PathBuf::from("src/b.rs"),
            table(&[("run", SymbolKind::Function)], &["connect"]),
        ),
        (
            PathBuf::from("src/c.rs"),
            table(&[], &["connect", "connect", "connect"]),
        ),
    ])
}

/// node ids are sorted-path positions: a=0, b=1, c=2, net=3.
fn hub_ranks() -> Vec<f64> {
    vec![0.3, 0.2, 0.1, 0.4]
}

#[test]
fn renders_paths_and_signature_only_symbols_in_rank_order() {
    let text = render(&hub_graph(), &hub_ranks(), 10);
    assert_eq!(
        text,
        "src/net.rs:\n    connect:function\nsrc/a.rs:\n    work:function\nsrc/b.rs:\n    run:function\nsrc/c.rs:",
        "rank desc, signature-only name:kind, bare path for def-less file"
    );
}

#[test]
fn rank_ties_break_by_path_ascending() {
    let tied = vec![0.25; 4];
    let text = render(&hub_graph(), &tied, 10);
    let lines: Vec<&str> = text.split('\n').collect();
    assert_eq!(
        lines,
        vec![
            "src/a.rs:",
            "    work:function",
            "src/b.rs:",
            "    run:function",
            "src/c.rs:",
            "src/net.rs:",
            "    connect:function"
        ],
        "equal ranks order purely by path"
    );
}

#[test]
fn symbols_within_a_file_order_by_name_and_carry_kind() {
    let graph = FileGraph::build(vec![(
        PathBuf::from("src/many.rs"),
        table(
            &[
                ("zed", SymbolKind::Function),
                ("alpha", SymbolKind::Struct),
                ("mid", SymbolKind::Other("macro".to_owned())),
            ],
            &[],
        ),
    )]);
    let text = render(&graph, &[1.0], 10);
    assert_eq!(
        text, "src/many.rs:\n    alpha:struct\n    mid:macro\n    zed:function",
        "name-ascending symbols, Other kinds label verbatim"
    );
}

#[test]
fn density_truncates_files_and_symbols_with_ellipsis_markers() {
    let graph = FileGraph::build(vec![(
        PathBuf::from("src/many.rs"),
        table(
            &[
                ("a1", SymbolKind::Function),
                ("b2", SymbolKind::Function),
                ("c3", SymbolKind::Function),
            ],
            &[],
        ),
    )]);
    let text = render(&graph, &[1.0], 1);
    assert_eq!(
        text, "src/many.rs:\n    a1:function\n    ...",
        "symbol ellipsis is indented"
    );

    let text = render(&hub_graph(), &hub_ranks(), 2);
    assert_eq!(
        text, "src/net.rs:\n    connect:function\nsrc/a.rs:\n    work:function\n...",
        "file ellipsis at column zero after the cut"
    );
}

#[test]
fn token_count_is_chars_divided_by_four_rounded_up() {
    assert_eq!(token_count(""), 0);
    assert_eq!(token_count("abc"), 1);
    assert_eq!(token_count("abcd"), 1);
    assert_eq!(token_count("abcde"), 2);
    assert_eq!(token_count("日本語"), 1, "unicode scalar count, not bytes");
    assert_eq!(token_count("日本語x"), 1, "4 scalars = exactly 1 token");
    assert_eq!(token_count("日本語xy"), 2);
}

#[test]
fn budget_binary_search_finds_the_largest_fitting_density() {
    let graph = hub_graph();
    let ranks = hub_ranks();

    let two = render(&graph, &ranks, 2);
    let fit: RenderFit = fit_budget(&graph, &ranks, token_count(&two));
    assert_eq!(fit.density, 2, "budget exactly fitting density 2 picks 2");
    assert_eq!(fit.text, two);
    assert_eq!(fit.tokens, token_count(&two));
    assert!(fit.tokens <= token_count(&two) as u32);

    let full = render(&graph, &ranks, 10);
    let fit = fit_budget(&graph, &ranks, token_count(&full));
    assert_eq!(fit.density, 4, "density caps at file count (plateau start)");
    assert_eq!(fit.text, full);

    let starved = fit_budget(&graph, &ranks, 1);
    assert_eq!(starved.density, 0);
    assert_eq!(starved.text, "", "budget below one file renders nothing");
}

#[test]
fn token_cost_is_monotonic_in_density() {
    let graph = FileGraph::build(harness_repomap::synthetic::repo(40, 3));
    let ranks = vec![0.5, 0.5];
    let mut previous = 0_u32;
    for density in 0_u32..45 {
        let cost = token_count(&render(&graph, &ranks, density));
        assert!(cost >= previous, "density {density}: {cost} < {previous}");
        previous = cost;
    }
}

#[test]
fn empty_graph_and_short_ranks_render_nothing_or_leniently() {
    let empty = FileGraph::build(Vec::<(PathBuf, harness_ast::tags::TagTable)>::new());
    let fit = fit_budget(&empty, &[], 1000);
    assert_eq!((fit.density, fit.tokens, fit.text.as_str()), (0, 0, ""));

    // ranks shorter than the file list: missing entries read 0.
    let text = render(&hub_graph(), &[1.0, 0.5, 0.0], 10);
    assert!(
        text.starts_with("src/a.rs:\n    work:function\nsrc/b.rs:\n    run:function"),
        "explicit ranks order a (1.0) then b (0.5): {text}"
    );
    assert!(
        text.ends_with("src/c.rs:\nsrc/net.rs:\n    connect:function"),
        "c and net tie at 0 (net's entry missing); path breaks the tie: {text}"
    );
}
