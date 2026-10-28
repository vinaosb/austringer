//! Perf scenarios: one named runner per measured surface. Every scenario
//! emits MEASURED metrics (deterministic counts, dhat allocation deltas,
//! min-of-N walls); wall numbers are Advisory or Budget-only — never
//! baseline-regression-gated (noise-margin discipline).

use std::collections::BTreeMap;
use std::path::PathBuf;

use thiserror::Error;

use crate::budgets::BudgetConfig;
use crate::gate::Policy;

pub mod eviction;
pub mod fixtures;
pub mod heap;
pub mod idle;
pub mod process;
pub mod prompt;
pub mod pty;
pub mod repomap;
pub mod sessions;
pub mod tui;
pub mod wire_prefix;

#[derive(Debug, Error)]
pub enum ScenarioError {
    #[error("skipped: {0}")]
    Skipped(String),
    #[error("scenario failed: {0}")]
    Failed(String),
}

/// One measured number with its gating policy. `name` is the full
/// "scenario.metric" identifier used in baselines and reports.
#[derive(Debug)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub unit: &'static str,
    pub policy: Policy,
}

#[derive(Debug, Default)]
pub struct ScenarioOutput {
    pub metrics: Vec<Metric>,
}

impl ScenarioOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, name: &str, value: f64, unit: &'static str, policy: Policy) {
        self.metrics.push(Metric {
            name: name.to_owned(),
            value,
            unit,
            policy,
        });
    }

    pub fn value(&self, name: &str) -> Option<f64> {
        self.metrics
            .iter()
            .find(|m| m.name == name)
            .map(|m| m.value)
    }
}

/// Inputs the process-level scenarios need; absent inputs are typed skips.
pub struct ScenarioContext {
    pub budgets: BudgetConfig,
    pub cli_bin: Option<PathBuf>,
    pub daemon_bin: Option<PathBuf>,
    /// Named binaries for the size budget (name -> path).
    pub binaries: Vec<(String, PathBuf)>,
    /// Min-of-N sample count for wall measurements (default 7).
    pub wall_runs: u32,
}

impl ScenarioContext {
    pub fn for_tests() -> Self {
        Self {
            budgets: BudgetConfig::plan_defaults(),
            cli_bin: None,
            daemon_bin: None,
            binaries: Vec::new(),
            wall_runs: 3,
        }
    }
}

pub type ScenarioResult = Result<ScenarioOutput, ScenarioError>;
pub type ScenarioFn = fn(&ScenarioContext) -> ScenarioResult;

/// Fixed registry; baseline freshness is defined against this set.
pub const REGISTRY: &[(&str, ScenarioFn)] = &[
    ("repomap", repomap::run),
    ("eviction", eviction::run),
    ("prompt", prompt::run),
    ("pty", pty::run),
    ("sessions", sessions::run),
    ("tui", tui::run),
    ("wire_prefix", wire_prefix::run),
    ("idle", idle::run),
    ("cold_start", process::cold_start),
    ("rss", process::daemon_rss),
    ("binary_size", process::binary_size),
    ("self_test", process::self_test),
];

/// Metric names the DEFAULT build compares against gate_baseline.json
/// (machine-independent, allocator-independent). dhat allocation metrics
/// live in the heap-profile baseline instead.
pub const GATE_BASELINE_METRICS: &[&str] = &[
    "repomap.tokens",
    "eviction.projected_events",
    "prompt.stable_prefix_bytes",
    "wire.stable_surface_bytes",
    "pty.observation_bytes",
    "sessions.page_bytes",
];

/// dhat metric names the heap-profile build compares against
/// heap_profile.json.
pub const HEAP_BASELINE_METRICS: &[&str] = &[
    "repomap.alloc_bytes",
    "repomap.alloc_count",
    "eviction.alloc_bytes",
    "eviction.alloc_count",
    "prompt.alloc_bytes",
    "prompt.alloc_count",
    "pty.alloc_bytes",
    "pty.alloc_count",
    "sessions.alloc_bytes",
    "sessions.alloc_count",
    "tui.alloc_bytes",
    "tui.alloc_count",
];

/// Collect a deterministic metric map from a run (for baselines and
/// freshness checks).
pub fn deterministic_metrics(
    outputs: &BTreeMap<String, ScenarioOutput>,
    names: &[&str],
) -> BTreeMap<String, f64> {
    let mut map = BTreeMap::new();
    for &name in names {
        if let Some(value) = outputs.values().find_map(|o| o.value(name)) {
            map.insert(name.to_owned(), value);
        }
    }
    map
}
