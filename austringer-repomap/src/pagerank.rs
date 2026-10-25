//! Personalized PageRank over a CSR file adjacency (plan todo 15).
//!
//! # Why CSR, and why scalar
//!
//! The adjacency is compressed sparse row — two flat `Vec`s plus one
//! offset per node — never `Vec<Vec<_>>` per-node adjacency: a 50k-file
//! monorepo graph is a few hundred KB of contiguous memory, walks
//! linearly, and re-derives in one pass. Per-node `Vec` headers cost
//! 24 bytes plus allocator fragmentation per node and defeat the cache.
//!
//! The power iteration below is deliberately SCALAR f64. The loop is
//! gather/scatter bound over a sparse structure (each step touches
//! `col_indices`/`weights` in adjacency order, scattering into a rank
//! vector), not FLOP bound — SIMD lanes do not accelerate irregular
//! indexed scatter, and the measured 50k-node solve already lands in
//! single-digit milliseconds in release (numbers recorded in
//! `.omo/evidence/task-15-rust-agent-harness.txt`). An explicit SIMD
//! path would buy nothing user-visible for its complexity; revisit only
//! if a real-repo profile shows iteration dominating render time.

/// Damping factor default (`d = 0.85`, the plan-pinned value).
pub const DAMPING: f64 = 0.85;

/// Weighted adjacency in compressed sparse row form. Node ids are
/// indices into the owning graph's sorted file list; row `u` spans
/// `row_offsets[u]..row_offsets[u+1]` in `col_indices`/`weights`.
///
/// An undirected edge `{u, v, w}` is stored as two directed entries
/// `(u, v, w)` and `(v, u, w)`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Csr {
    /// `nodes() + 1` prefix offsets into `col_indices`/`weights`.
    pub row_offsets: Vec<u32>,
    /// Neighbor node ids, sorted within each row.
    pub col_indices: Vec<u32>,
    /// Edge weights, parallel to `col_indices`.
    pub weights: Vec<f64>,
}

impl Csr {
    /// Build from undirected weighted edges `(u, v, w)`. Duplicate pairs
    /// merge by summing weights; self-loops (`u == v`) are skipped.
    /// `nodes` sets the row count even for trailing isolated nodes.
    pub fn from_undirected(nodes: u32, edges: &[(u32, u32, f64)]) -> Self {
        let mut directed: Vec<(u32, u32, f64)> = Vec::with_capacity(edges.len() * 2);
        for &(u, v, w) in edges {
            if u == v {
                continue;
            }
            directed.push((u, v, w));
            directed.push((v, u, w));
        }
        directed.sort_unstable_by_key(|&(u, v, _)| (u, v));

        let total = directed.len();
        let mut row_offsets = vec![0_u32; nodes as usize + 1];
        let mut col_indices = Vec::with_capacity(total);
        let mut weights: Vec<f64> = Vec::with_capacity(total);
        let mut row = 0_u32;
        for &(u, v, w) in &directed {
            while row < u {
                row_offsets[row as usize + 1] = col_indices.len() as u32;
                row += 1;
            }
            let row_open = row_offsets[u as usize] < col_indices.len() as u32;
            if row_open && col_indices.last() == Some(&v) {
                if let Some(merged) = weights.last_mut() {
                    *merged += w;
                }
            } else {
                col_indices.push(v);
                weights.push(w);
            }
        }
        let end = col_indices.len() as u32;
        for offset in row_offsets.iter_mut().skip(row as usize + 1) {
            *offset = end;
        }
        Csr {
            row_offsets,
            col_indices,
            weights,
        }
    }

    /// Node count (rows), including isolated ones.
    pub fn nodes(&self) -> usize {
        self.row_offsets.len() - 1
    }

    /// Directed-entry count (each undirected edge counted twice).
    pub fn entries(&self) -> usize {
        self.col_indices.len()
    }

    /// Weight of the directed entry `from -> to`, if present.
    pub fn weight(&self, from: u32, to: u32) -> Option<f64> {
        let start = self.row_offsets.get(from as usize).copied().unwrap_or(0) as usize;
        let end = self
            .row_offsets
            .get(from as usize + 1)
            .map_or(start, |&e| e as usize);
        self.col_indices[start..end]
            .binary_search(&to)
            .ok()
            .and_then(|i| self.weights.get(start + i).copied())
    }
}

/// Solver knobs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageRankConfig {
    /// Damping `d`; rank flows `d` along edges, `1 - d` teleports to
    /// the personalization vector.
    pub damping: f64,
    /// L1 convergence tolerance: stop when `Σ|x_{t+1} - x_t|` drops
    /// below this.
    pub tolerance: f64,
    /// Iteration cap; hitting it returns the current vector with
    /// `converged == false` (bounded, never hangs).
    pub max_iterations: u32,
}

impl Default for PageRankConfig {
    fn default() -> Self {
        PageRankConfig {
            damping: DAMPING,
            tolerance: 1e-8,
            max_iterations: 200,
        }
    }
}

/// Solve outcome: ranks aligned with CSR node ids plus run facts.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranking {
    /// Rank per node; sums to ~1.
    pub values: Vec<f64>,
    /// Updates actually performed.
    pub iterations: u32,
    /// Whether the L1 delta dropped below `tolerance` before the cap.
    pub converged: bool,
}

/// Personalization placing uniform mass on the listed node ids
/// (duplicates add mass per listing; out-of-range ids are skipped).
/// No focus → uniform over every node.
pub fn uniform_on(nodes: usize, focus: &[u32]) -> Vec<f64> {
    let mut vector = vec![0.0_f64; nodes];
    let mut hits = 0_usize;
    for &id in focus {
        if let Some(slot) = vector.get_mut(id as usize) {
            *slot += 1.0;
            hits += 1;
        }
    }
    if hits == 0 {
        if nodes == 0 {
            return Vec::new();
        }
        vector.fill(1.0 / nodes as f64);
        return vector;
    }
    let mass = 1.0 / hits as f64;
    for slot in &mut vector {
        *slot *= mass;
    }
    vector
}

/// Personalized PageRank by power iteration:
/// `x_{t+1} = d·WᵀD⁻¹x_t + (1 - d + d·dangling)·p`.
///
/// `personalization` entries are expected non-negative; entries beyond
/// the node count are ignored, missing ones read as 0, an all-zero (or
/// absent) vector falls back to uniform. Dangling mass (rank sitting on
/// out-degree-0 nodes) teleports back to `p` — the standard
/// personalization-consistent handling. Iteration order over the CSR is
/// fixed, so results are bit-deterministic for identical inputs.
pub fn solve(csr: &Csr, personalization: &[f64], config: &PageRankConfig) -> Ranking {
    let n = csr.nodes();
    if n == 0 {
        return Ranking {
            values: Vec::new(),
            iterations: 0,
            converged: true,
        };
    }
    let mut p = vec![0.0_f64; n];
    let mut total = 0.0;
    for (slot, weight) in p.iter_mut().zip(personalization) {
        *slot = *weight;
        total += *weight;
    }
    if total <= 0.0 {
        p.fill(1.0 / n as f64);
    } else {
        for slot in &mut p {
            *slot /= total;
        }
    }

    let mut out_weight = Vec::with_capacity(n);
    for row in csr.row_offsets.windows(2) {
        let (start, end) = (row[0] as usize, row[1] as usize);
        out_weight.push(csr.weights[start..end].iter().sum::<f64>());
    }

    let d = config.damping;
    let mut x = p.clone();
    let mut y = vec![0.0_f64; n];
    let mut iterations = 0_u32;
    let mut converged = false;
    while iterations < config.max_iterations {
        iterations += 1;
        y.fill(0.0);
        let mut dangling = 0.0;
        for u in 0..n {
            if out_weight[u] > 0.0 {
                let share = d * x[u] / out_weight[u];
                let start = csr.row_offsets[u] as usize;
                let end = csr.row_offsets[u + 1] as usize;
                for k in start..end {
                    y[csr.col_indices[k] as usize] += share * csr.weights[k];
                }
            } else {
                dangling += x[u];
            }
        }
        let teleport = (1.0 - d) + d * dangling;
        for i in 0..n {
            y[i] += teleport * p[i];
        }
        let delta: f64 = (0..n).map(|i| (y[i] - x[i]).abs()).sum();
        std::mem::swap(&mut x, &mut y);
        if delta < config.tolerance {
            converged = true;
            break;
        }
    }
    Ranking {
        values: x,
        iterations,
        converged,
    }
}
