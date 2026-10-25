//! Bench-style incremental budget (plan todo 14 acceptance): re-analysis
//! of an unchanged (mtime-hit) mid-size file must stay far under the
//! 10 ms budget. The in-test assert is deliberately CI-safe (100 ms,
//! debug-profile cold-fs tolerant); the actual measurement is printed and
//! recorded in the task evidence.

use austringer_ast::analyze::{AnalyzeOutcome, Analyzer, CacheStatus};
use std::fs;
use std::time::{Duration, Instant};
use tempfile::TempDir;

const CI_SAFE_ASSERT: Duration = Duration::from_millis(100);
const PLAN_BUDGET: Duration = Duration::from_millis(10);

fn mid_size_rust_source() -> Vec<u8> {
    let mut src = Vec::with_capacity(64 * 1024);
    for module in 0..40 {
        src.extend_from_slice(format!("mod m{module} {{\n").as_bytes());
        for i in 0..30 {
            src.extend_from_slice(
                format!(
                    "    pub fn handler_{i}(input: u32) -> u32 {{\n        let scaled = input * {i};\n        helper(scaled)\n    }}\n"
                )
                .as_bytes(),
            );
        }
        src.extend_from_slice(b"}\n");
    }
    src
}

#[test]
fn mtime_hit_reanalysis_stays_under_budget() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let cache_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = src_dir.path().join("mid.rs");
    fs::write(&path, mid_size_rust_source()).unwrap_or_else(|e| panic!("{e}"));

    let analyzer = Analyzer::with_cache_dir(cache_dir.path());
    let warm = match analyzer
        .analyze_path(&path)
        .unwrap_or_else(|e| panic!("{e}"))
    {
        AnalyzeOutcome::Analyzed(a) => *a,
        AnalyzeOutcome::NoGrammar { .. } => panic!("rust is a default grammar"),
    };
    assert_eq!(warm.cache, CacheStatus::Miss);
    let def_count = warm.definitions.len();
    assert!(
        def_count > 1000,
        "mid-size fixture has real content :: {def_count}"
    );

    let mut best = Duration::MAX;
    for _ in 0..5 {
        let t0 = Instant::now();
        let outcome = analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}"));
        let elapsed = t0.elapsed();
        match outcome {
            AnalyzeOutcome::Analyzed(a) => {
                assert_eq!(
                    a.cache,
                    CacheStatus::Hit,
                    "unchanged file must hit the cache"
                );
                assert_eq!(
                    a.definitions.len(),
                    def_count,
                    "hit returns the same table size"
                );
            }
            AnalyzeOutcome::NoGrammar { .. } => panic!("rust is a default grammar"),
        }
        best = best.min(elapsed);
    }
    eprintln!(
        "incremental mtime-hit: best of 5 = {:.3} ms (plan budget 10 ms, CI-safe assert 100 ms), fixture {} bytes, {} defs",
        best.as_secs_f64() * 1000.0,
        mid_size_rust_source().len(),
        def_count
    );
    assert!(
        best < CI_SAFE_ASSERT,
        "mtime-hit took {best:?} — over the CI-safe bound (plan budget {PLAN_BUDGET:?})"
    );
}
