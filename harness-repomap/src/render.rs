//! Signature-only rendering under a token budget (plan todo 15).
//!
//! Output shape (deterministic order: rank desc → path asc → symbol
//! name asc):
//!
//! ```text
//! src/hub.rs:
//!     connect:function
//!     listen:method
//!     ...
//! src/edge.rs:
//! ...
//! ```
//!
//! Lines are `path:` headers plus indented `name:kind` definition
//! lines; `...` marks cut symbol lists (indented) and a cut file list
//! (column zero). Tag tables carry no signature text today, so the
//! signature-only contract degrades to `name:kind` — a future
//! signature column slots into the same line.
//!
//! # Token budget
//!
//! [`token_count`] approximates one token per 4 Unicode scalars
//! (`chars().count().div_ceil(4)`) — deterministic, allocation-free,
//! and within ~±20% of BPE on code-shaped text, which is all the
//! budget search needs. [`fit_budget`] binary-searches the largest
//! density (files shown AND symbols per file) whose render fits the
//! budget; cost is monotone in density by construction, so the search
//! is exact.

use crate::graph::{FileGraph, kind_label};

/// Largest-density render outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderFit {
    /// The rendered map text.
    pub text: String,
    /// [`token_count`] of `text` (always ≤ the requested budget).
    pub tokens: u32,
    /// The winning density: top `density` files, `density` symbols each.
    pub density: u32,
}

/// Deterministic token approximation: 1 token per 4 characters.
pub fn token_count(text: &str) -> u32 {
    (text.chars().count().div_ceil(4)) as u32
}

/// Render the map at a fixed density: the top `density` files (rank
/// desc, ties by path asc), each with its first `density` definitions
/// (name asc). `ranks` is aligned with node ids; missing entries read
/// as 0.
pub fn render(graph: &FileGraph, ranks: &[f64], density: u32) -> String {
    render_ordered(graph, &ordered_files(graph, ranks), density)
}

/// Files in render order: rank desc, ties by path asc. Computed once
/// per render/budget-search; `ranks` entries beyond the file list are
/// missing-by-construction (read as 0).
fn ordered_files(graph: &FileGraph, ranks: &[f64]) -> Vec<u32> {
    let files = graph.files();
    let mut order: Vec<(u32, f64)> = (0..files.len() as u32)
        .map(|id| (id, ranks.get(id as usize).copied().unwrap_or(0.0)))
        .collect();
    order.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| files[a.0 as usize].cmp(&files[b.0 as usize]))
    });
    order.into_iter().map(|(id, _)| id).collect()
}

fn render_ordered(graph: &FileGraph, order: &[u32], density: u32) -> String {
    let files = graph.files();
    let shown = (density as usize).min(files.len());
    let mut lines: Vec<String> = Vec::new();
    for &id in &order[..shown] {
        lines.push(format!("{}:", files[id as usize].display()));
        let defs = graph.definitions_of(id);
        let symbols = (density as usize).min(defs.len());
        for def in &defs[..symbols] {
            lines.push(format!("    {}:{}", def.name, kind_label(&def.kind)));
        }
        if defs.len() > symbols {
            lines.push("    ...".to_owned());
        }
    }
    if shown > 0 && shown < files.len() {
        lines.push("...".to_owned());
    }
    lines.join("\n")
}

/// Binary-search the largest density whose render fits `token_budget`,
/// and return that render. Density tops out at the largest of
/// (file count, per-file definition counts) — the point beyond which
/// more density cannot change the render. An empty graph, or a budget
/// below one file header, yields density 0 and empty text.
pub fn fit_budget(graph: &FileGraph, ranks: &[f64], token_budget: u32) -> RenderFit {
    let files = graph.files().len() as u32;
    let max_defs = (0..files)
        .map(|id| graph.definitions_of(id).len() as u32)
        .max()
        .unwrap_or(0);
    let top = files.max(max_defs);
    let order = ordered_files(graph, ranks);
    let fits = |density: u32| token_count(&render_ordered(graph, &order, density)) <= token_budget;
    let mut low = 0_u32;
    let mut high = top;
    while low < high {
        let mid = (low + high).div_ceil(2);
        if fits(mid) {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    let text = render_ordered(graph, &order, low);
    RenderFit {
        tokens: token_count(&text),
        text,
        density: low,
    }
}
