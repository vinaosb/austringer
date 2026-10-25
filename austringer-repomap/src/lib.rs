//! PageRank repository map: tag tables in, token-budgeted map out
//! (plan todo 15).
//!
//! Pipeline (each stage independently testable):
//! 1. [`graph`] — `(path, TagTable)` pairs → [`graph::FileGraph`]:
//!    cross-references keyed by symbol NAME (survives renames and node
//!    renumbering — never dangling), derived into a weighted undirected
//!    CSR adjacency.
//! 2. [`pagerank`] — Personalized PageRank `d = 0.85` over that CSR;
//!    the personalization vector puts uniform mass on the
//!    focused/@mentioned files; dangling mass redistributes to it.
//! 3. [`render`] — signature-only `path:` / `name:kind` lines with
//!    ellipsis markers, density picked by binary search to be the
//!    largest render fitting the token budget (chars/4 approximation).
//! 4. [`synthetic`] — seeded synthetic repos for tests, benches, and
//!    evidence runs.
//!
//! The one-call facade is [`repo_map`].

//! The one-call facade is [`repo_map`]: tag tables + focus set in,
//! token-budgeted map (plus the graph and ranking it came from) out.

pub mod graph;
pub mod pagerank;
pub mod render;
pub mod synthetic;

use austringer_ast::tags::TagTable;
pub use graph::{DefSite, FileGraph, GraphStats, SymbolDef, SymbolLinks};
pub use pagerank::{Csr, DAMPING, PageRankConfig, Ranking};
pub use render::{RenderFit, fit_budget, render, token_count};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Facet knobs for [`repo_map`].
#[derive(Debug, Clone)]
pub struct MapOptions {
    /// Focused/@mentioned files: the personalization set. Unknown
    /// paths are skipped (counted in [`MapOutput::focus_matched`]);
    /// an empty match set falls back to an unpersonalized whole-repo
    /// map.
    pub focus: Vec<PathBuf>,
    /// PageRank solver configuration.
    pub pagerank: PageRankConfig,
    /// Render budget in approximate tokens (chars/4); the map is the
    /// largest render fitting it.
    pub token_budget: u32,
}

impl Default for MapOptions {
    fn default() -> Self {
        MapOptions {
            focus: Vec::new(),
            pagerank: PageRankConfig::default(),
            token_budget: 1000,
        }
    }
}

/// Everything [`repo_map`] produced, for rendering, inspection, and
/// evidence.
#[derive(Debug, Clone)]
pub struct MapOutput {
    /// The token-budgeted map text.
    pub text: String,
    /// [`token_count`] of `text` (≤ the requested budget).
    pub tokens: u32,
    /// Winning density (files shown / symbols per file).
    pub density: u32,
    /// The file graph the ranking ran on.
    pub graph: FileGraph,
    /// The solver outcome (ranks aligned with `graph.files()`).
    pub ranking: Ranking,
    /// How many focus paths actually matched graph files.
    pub focus_matched: usize,
    /// Wall time of the budget-search render step alone.
    pub render_duration: Duration,
}

/// Build the repository map: graph → personalized PageRank →
/// token-budgeted render.
pub fn repo_map(
    tables: impl IntoIterator<Item = (PathBuf, TagTable)>,
    options: &MapOptions,
) -> MapOutput {
    let graph = FileGraph::build(tables);
    let focus_ids: Vec<u32> = options
        .focus
        .iter()
        .filter_map(|path| graph.node_id(path))
        .collect();
    let focus_matched = focus_ids.len();
    let personalization = pagerank::uniform_on(graph.files().len(), &focus_ids);
    let ranking = pagerank::solve(graph.csr(), &personalization, &options.pagerank);
    let started = Instant::now();
    let fit = render::fit_budget(&graph, &ranking.values, options.token_budget);
    MapOutput {
        text: fit.text,
        tokens: fit.tokens,
        density: fit.density,
        graph,
        ranking,
        focus_matched,
        render_duration: started.elapsed(),
    }
}
