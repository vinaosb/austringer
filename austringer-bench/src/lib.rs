//! Performance suite + CI gates (plan todo 43).
//!
//! Two surfaces over the same scenario code:
//!
//! * `cargo bench -p austringer-bench` — criterion benches over the six plan
//!   surfaces (repomap, eviction, prompt-builder, PTY scrape, session
//!   paging, TUI render). Informational + regression-trend tooling.
//! * `perf-gate` binary — the CI gate. Evaluates the todo-43 budgets
//!   (cold start <50 ms, idle renders, RSS <80 MB, p99 frame <16 ms,
//!   binary <40 MB, 10 ms frame budget w/ hang detection) plus >10%
//!   regressions against committed machine-independent baselines
//!   (allocation counts, render counts, byte sizes). Exits non-zero on
//!   any breach. Wall-clock statistics are min-of-N / min-of-runs to stay
//!   robust on loaded machines; only deterministic counters are compared
//!   against baselines (noise-margin discipline — see `docs` in the
//!   perf-gate binary header).
//! * `heap-profile` binary (feature `dhat`) — archived dhat heap profiles
//!   and the allocation-count regression check.
//!
//! `mimalloc` feature swaps the gate binaries onto the mimalloc allocator
//! (allocation-discipline option; default stays the system allocator).

pub mod baseline;
pub mod budgets;
pub mod frame;
pub mod gate;
pub mod report;
pub mod runner;
pub mod scenarios;
pub mod stats;

// Both features install competing global allocators; asking for both is
// a configuration error, caught at compile time.
#[cfg(all(feature = "dhat", feature = "mimalloc"))]
compile_error!(
    "features `dhat` and `mimalloc` both install a global allocator; enable at most one"
);
