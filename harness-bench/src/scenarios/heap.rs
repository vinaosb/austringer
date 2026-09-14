//! dhat-backed allocation metrics (feature `dhat`). Scenarios snapshot
//! cumulative totals before/after their measured work; the deltas are the
//! deterministic allocation counts archived in heap_profile.json.
//! Without the feature this module is inert — call sites cfg-gate both
//! ends, so no dead code exists in either build.

#[cfg(feature = "dhat")]
pub struct HeapSnapshot {
    pub total_bytes: u64,
    pub total_blocks: u64,
}

#[cfg(feature = "dhat")]
pub fn snapshot() -> HeapSnapshot {
    let stats = dhat::HeapStats::get();
    HeapSnapshot {
        total_bytes: stats.total_bytes,
        total_blocks: stats.total_blocks,
    }
}

/// In-process consumers (integration tests) must own a profiler before
/// any snapshot; nextest runs each test in its own process, so one
/// profiler per test process is the singleton dhat expects.
#[cfg(feature = "dhat")]
pub fn start_profiler() -> dhat::Profiler {
    dhat::Profiler::builder().build()
}

/// Record `<scenario>.alloc_bytes` / `.alloc_count` regression metrics
/// from a before/after pair. dhat totals are monotonic under its global
/// allocator, so the deltas are well-defined by construction.
#[cfg(feature = "dhat")]
pub fn record(out: &mut crate::scenarios::ScenarioOutput, scenario: &str, before: HeapSnapshot) {
    let after = snapshot();
    out.push(
        &format!("{scenario}.alloc_bytes"),
        (after.total_bytes - before.total_bytes) as f64,
        "bytes",
        crate::gate::Policy::Regression,
    );
    out.push(
        &format!("{scenario}.alloc_count"),
        (after.total_blocks - before.total_blocks) as f64,
        "allocs",
        crate::gate::Policy::Regression,
    );
}
