//! Personalized PageRank on the CSR adjacency (plan todo 15):
//! personalization puts uniform mass on the focus set, dangling mass
//! returns to it, iteration runs to a configurable L1 tolerance.
//! Assertions are on rank ORDER, exact symmetry values, and mass
//! conservation — not booleans.

use austringer_repomap::pagerank::{self, Csr, DAMPING, PageRankConfig};

fn cfg() -> PageRankConfig {
    PageRankConfig::default()
}

fn solve(csr: &Csr, p: &[f64]) -> pagerank::Ranking {
    pagerank::solve(csr, p, &cfg())
}

#[test]
fn focus_file_outranks_its_satellites() {
    // Star: 0 - 1, 0 - 2, focus on 0. Hand-derived fixed point:
    //   x0 = (1-d) + d*(x1 + x2);  x1 = x2 = d/2 * x0
    //   =>  x1 = x2 ~ 0.2297,  x0 ~ 0.5405  (x0 + 2*x1 = 1).
    let csr = Csr::from_undirected(3, &[(0, 1, 1.0), (0, 2, 1.0)]);
    let ranking = solve(&csr, &[1.0, 0.0, 0.0]);

    assert!(
        ranking.converged,
        "small graph must converge within default budget"
    );
    let values = &ranking.values;
    assert!(
        values[0] > values[1] && values[0] > values[2],
        "focus node top: {values:?}"
    );
    assert_eq!(
        values[1], values[2],
        "symmetric satellites are bit-identical"
    );
    assert!(
        (values[0] - 0.5405405).abs() < 1e-4,
        "x0 ~= 20/37: {values:?}"
    );
    let mass: f64 = values.iter().sum();
    assert!(
        (mass - 1.0).abs() < 1e-6,
        "probability mass conserved: {mass}"
    );
}

#[test]
fn edge_weights_steer_rank_flow() {
    // Focus 0 linked to 1 with weight 3 and to 2 with weight 1:
    // the heavier reference path carries proportionally more rank.
    let csr = Csr::from_undirected(3, &[(0, 1, 3.0), (0, 2, 1.0)]);
    let values = &solve(&csr, &[1.0, 0.0, 0.0]).values;
    assert!(values[1] > values[2], "weight 3 beats weight 1: {values:?}");
    let total = values[1] + values[2];
    assert!(
        (values[1] / total - 0.75).abs() < 1e-6,
        "rank splits 3:1 along the weights: {values:?}"
    );
}

#[test]
fn dangling_mass_returns_to_the_personalization_vector() {
    // All-dangling graph: every step returns (1-d)*p + d*1.0*p = p.
    let csr = Csr::from_undirected(3, &[]);
    let ranking = solve(&csr, &[1.0, 0.0, 0.0]);
    assert_eq!(ranking.values, vec![1.0, 0.0, 0.0], "p is the fixed point");
    assert!(ranking.converged);

    // Partly dangling: isolated node 2 never gains mass (p[2] = 0),
    // the connected pair behaves like the plain 2-node case.
    let csr = Csr::from_undirected(3, &[(0, 1, 1.0)]);
    let values = &solve(&csr, &[1.0, 0.0, 0.0]).values;
    assert_eq!(values[2], 0.0, "no edge, no personalization, no rank");
    assert!((values[0] - 0.5405405).abs() < 1e-4);
    assert!(values.iter().all(|v| v.is_finite()));
}

#[test]
fn zero_personalization_falls_back_to_uniform() {
    let csr = Csr::from_undirected(2, &[(0, 1, 1.0)]);
    let values = &solve(&csr, &[0.0, 0.0]).values;
    assert!(
        (values[0] - 0.5).abs() < 1e-6 && (values[1] - 0.5).abs() < 1e-6,
        "{values:?}"
    );
}

#[test]
fn solve_is_deterministic_and_config_bounded() {
    let csr = Csr::from_undirected(4, &[(0, 1, 2.0), (1, 2, 1.0), (2, 3, 4.0), (0, 3, 1.0)]);
    let a = solve(&csr, &[1.0, 0.0, 0.0, 0.0]);
    let b = solve(&csr, &[1.0, 0.0, 0.0, 0.0]);
    assert_eq!(a.values, b.values, "bit-identical across runs");

    // Huge tolerance: the first delta already satisfies it.
    let loose = pagerank::solve(
        &csr,
        &[1.0, 0.0, 0.0, 0.0],
        &PageRankConfig {
            tolerance: 1e9,
            ..PageRankConfig::default()
        },
    );
    assert_eq!(loose.iterations, 1);
    assert!(loose.converged);

    // Zero-iteration budget returns the (normalized) start vector.
    let capped = pagerank::solve(
        &csr,
        &[1.0, 0.0, 0.0, 0.0],
        &PageRankConfig {
            max_iterations: 0,
            ..PageRankConfig::default()
        },
    );
    assert_eq!(capped.iterations, 0);
    assert!(!capped.converged);
    assert_eq!(capped.values, vec![1.0, 0.0, 0.0, 0.0]);

    assert_eq!(DAMPING, 0.85, "plan-pinned damping");
}

#[test]
fn empty_graph_and_short_personalization_are_handled() {
    let empty = Csr::from_undirected(0, &[]);
    let ranking = solve(&empty, &[]);
    assert_eq!(ranking.values, Vec::<f64>::new());
    assert!(ranking.converged);

    // Shorter-than-nodes personalization: missing entries read as 0.
    let csr = Csr::from_undirected(2, &[(0, 1, 1.0)]);
    let values = &solve(&csr, &[1.0]).values;
    assert!(
        values[0] > values[1],
        "equivalent to focus-on-0: {values:?}"
    );
}

#[test]
fn uniform_on_builds_the_focus_vector() {
    let vector = pagerank::uniform_on(4, &[3, 1]);
    assert_eq!(vector, vec![0.0, 0.5, 0.0, 0.5]);
    assert_eq!(
        pagerank::uniform_on(4, &[]),
        vec![0.25; 4],
        "no focus => uniform"
    );
    assert_eq!(
        pagerank::uniform_on(0, &[0]),
        Vec::<f64>::new(),
        "no nodes => empty"
    );
    let clamped = pagerank::uniform_on(2, &[0, 9]);
    assert_eq!(
        clamped,
        vec![1.0, 0.0],
        "out-of-range focus ids are skipped"
    );
}
