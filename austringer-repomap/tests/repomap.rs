//! End-to-end facade + acceptance (plan todo 15): 10k-file synthetic
//! repo ≤1k tokens, focus files rank top, degenerate repos render
//! without panicking, byte-identical reproducibility. Timings are
//! printed for the evidence file.

use austringer_ast::tags::SymbolKind;
use austringer_repomap::synthetic::{self, table};
use austringer_repomap::{MapOptions, repo_map};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[test]
fn ten_k_file_map_fits_thousand_tokens_and_focus_ranks_top() {
    let tables = synthetic::repo(10_000, 42);

    // Focus on the definer of hub symbol sym0 (every 97th pool slot is
    // referenced by ~1/3 of files): deterministic hub lookup.
    let focus: Vec<PathBuf> = tables
        .iter()
        .filter(|(_, t)| t.definitions.iter().any(|d| d.name == "sym0"))
        .map(|(p, _)| p.clone())
        .take(2)
        .collect();
    assert!(!focus.is_empty(), "generator guarantees sym0 is defined");

    let t0 = Instant::now();
    let output = repo_map(
        tables,
        &MapOptions {
            focus: focus.clone(),
            ..MapOptions::default()
        },
    );
    let total = t0.elapsed();

    assert!(
        output.tokens <= 1000,
        "map must fit 1k tokens: {}",
        output.tokens
    );
    assert!(output.density > 0);
    assert!(
        output.ranking.converged,
        "10k graph must converge inside default budget"
    );
    assert_eq!(output.focus_matched, focus.len());

    let mut ranked: Vec<(usize, &str, f64)> = output
        .ranking
        .values
        .iter()
        .enumerate()
        .map(|(id, rank)| (id, output.graph.files()[id].to_str().unwrap_or("?"), *rank))
        .collect();
    ranked.sort_by(|a, b| b.2.total_cmp(&a.2));
    for (position, (_, path, rank)) in ranked.iter().enumerate().take(10) {
        println!("top{position:>2}  {path:<24} rank={rank:.6}");
    }
    for focused in &focus {
        let position = ranked
            .iter()
            .position(|(_, path, _)| Path::new(*path) == focused)
            .unwrap_or_else(|| panic!("focus file {focused:?} not ranked"));
        println!("focus {focused:?} ranks #{position}");
        assert!(
            position < 5,
            "focus files must rank top-5 of 10k (position {position})"
        );
    }
    println!(
        "tokens={} density={} build+rank+render={:?} render-only-fits={:?} iterations={}",
        output.tokens, output.density, total, output.render_duration, output.ranking.iterations
    );

    // Plan budget: <50 ms cold render (release). Debug builds get the
    // generous CI-safe bound; release must meet the real budget.
    let render_budget_ms: u64 = if cfg!(debug_assertions) { 5_000 } else { 50 };
    assert!(
        output.render_duration.as_millis() as u64 <= render_budget_ms,
        "render took {:?} (budget {} ms)",
        output.render_duration,
        render_budget_ms
    );
    assert!(
        total.as_secs() < 60,
        "10k build+rank+render must stay CI-safe: {total:?}"
    );
}

#[test]
#[ignore = "50k-node evidence stress: run explicitly (cargo test --release -- --ignored)"]
fn fifty_k_node_stress_stays_bounded() {
    let tables = synthetic::repo(50_000, 7);
    let t0 = Instant::now();
    let output = repo_map(tables, &MapOptions::default());
    let total = t0.elapsed();
    println!(
        "50k nodes: edges={} iterations={} tokens={} total={:?} render={:?}",
        output.graph.stats().edges,
        output.ranking.iterations,
        output.tokens,
        total,
        output.render_duration
    );
    assert!(output.ranking.converged || output.ranking.iterations >= 200);
    assert!(output.tokens <= 1000);
    assert!(output.ranking.values.iter().all(|v| v.is_finite()));
    assert!(total.as_secs() < 120, "stress must stay bounded: {total:?}");
}

#[test]
fn map_renders_identically_for_identical_input() {
    let options = MapOptions::default();
    let a = repo_map(synthetic::repo(500, 9), &options);
    let b = repo_map(synthetic::repo(500, 9), &options);
    assert_eq!(a.text, b.text);
    assert_eq!(a.ranking.values, b.ranking.values);
}

#[test]
fn empty_repo_renders_empty_map_without_panicking() {
    let output = repo_map(
        Vec::<(PathBuf, austringer_ast::tags::TagTable)>::new(),
        &MapOptions::default(),
    );
    assert_eq!(
        (output.text.as_str(), output.tokens, output.density),
        ("", 0, 0)
    );
    assert_eq!(output.focus_matched, 0, "no focus can match an empty repo");
    assert!(output.ranking.converged);
}

#[test]
fn single_file_repo_maps_itself() {
    let output = repo_map(
        vec![(
            PathBuf::from("src/main.rs"),
            table(
                &[
                    ("main", SymbolKind::Function),
                    ("helper", SymbolKind::Function),
                ],
                &["main"],
            ),
        )],
        &MapOptions::default(),
    );
    assert_eq!(
        output.text, "src/main.rs:\n    helper:function\n    main:function",
        "self-references excluded, symbols name-asc"
    );
}

#[test]
fn zero_reference_graph_renders_all_files_in_path_order() {
    let tables = vec![
        (
            PathBuf::from("src/z.rs"),
            table(&[("zeta", SymbolKind::Function)], &[]),
        ),
        (
            PathBuf::from("src/a.rs"),
            table(&[("alpha", SymbolKind::Struct)], &[]),
        ),
        (
            PathBuf::from("src/m.rs"),
            table(&[("mid", SymbolKind::Method)], &[]),
        ),
    ];
    let output = repo_map(tables, &MapOptions::default());
    assert_eq!(
        output.text,
        "src/a.rs:\n    alpha:struct\nsrc/m.rs:\n    mid:method\nsrc/z.rs:\n    zeta:function",
        "no edges => uniform ranks => pure path order, no ellipsis"
    );
    assert!(output.ranking.values.iter().all(|v| v.is_finite()));
}

#[test]
fn unknown_focus_files_are_counted_not_fatal() {
    let output = repo_map(
        synthetic::repo(30, 1),
        &MapOptions {
            focus: vec![PathBuf::from("src/does/not/exist.rs")],
            ..MapOptions::default()
        },
    );
    assert_eq!(output.focus_matched, 0);
    assert!(
        !output.text.is_empty(),
        "falls back to an unpersonalized whole-repo map"
    );
}

#[test]
fn malformed_tables_flow_through_the_facade_bounded() {
    let output = repo_map(
        vec![
            (
                PathBuf::from("src/bad.rs"),
                table(&[("", SymbolKind::Function)], &["", "ghost"]),
            ),
            (
                PathBuf::from("src/good.rs"),
                table(&[("real", SymbolKind::Function)], &["", "real"]),
            ),
        ],
        &MapOptions::default(),
    );
    assert_eq!(output.graph.stats().skipped_empty_names, 3);
    assert_eq!(
        output.text, "src/bad.rs:\nsrc/good.rs:\n    real:function",
        "empty names never render; bad.rs keeps a bare header"
    );
}
