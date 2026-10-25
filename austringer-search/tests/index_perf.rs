//! Performance acceptance (plan todo 17): 10k-file cold index, warm query,
//! and one-file incremental refresh. Budget assertions are deliberately
//! generous (CI hosts vary) while the plan budgets (5 s cold / 20 ms warm /
//! 500 ms incremental) are recorded in the evidence via measured durations.

mod common;

use std::time::{Duration, Instant};

use austringer_search::CodeIndex;
use common::{
    XorShift64, enable_gitignore, write_adversarial_files, write_file, write_synth_corpus,
};

const COUNT: u32 = 10_000;
/// Synthetic corpus: 10k modules + adversarial UTF-8 file + huge line file.
/// The binary blob is skipped, the gitignored file is excluded.
const EXPECTED_DOCS: usize = COUNT as usize + 2;
/// Wide CI-safe bound; the plan's real 5 s budget is recorded, not gated.
const COLD_BUDGET: Duration = Duration::from_secs(15);
/// Wide CI-safe bound; the plan's real 20 ms budget is recorded, not gated.
const WARM_BUDGET: Duration = Duration::from_millis(200);
/// Wide CI-safe bound; the plan's real 500 ms budget is recorded, not gated.
/// Also generous enough for a loaded dev machine running the full local CI
/// gate in parallel (the plan budget itself is enforced by perf-gate, not
/// here).
const INCREMENTAL_BUDGET: Duration = Duration::from_secs(10);

fn setup(label: &str, seed: u64) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let outer = tempfile::tempdir().expect("tempdir");
    let corpus = outer.path().join("corpus");
    let index_dir = outer.path().join(label);
    std::fs::create_dir_all(&corpus).expect("mkdir corpus");
    std::fs::create_dir_all(&index_dir).expect("mkdir index");
    let mut rng = XorShift64::new(seed);
    write_synth_corpus(&corpus, COUNT, &mut rng);
    write_adversarial_files(&corpus);
    enable_gitignore(&corpus);
    write_file(
        &corpus,
        "ignored_dir/secret.rs",
        "pub fn hidden_marker() {}\n",
    );
    (outer, corpus, index_dir)
}

#[test]
fn acceptance_10k_cold_index_then_warm_query_within_budgets() {
    let (_keep, corpus, index_dir) = setup("perf-cold", 2026_0912);
    let index = CodeIndex::open(&index_dir).expect("open");

    let cold_started = Instant::now();
    let report = index.refresh(&corpus).expect("cold refresh");
    let cold_elapsed = cold_started.elapsed();

    assert_eq!(
        report.added, EXPECTED_DOCS as u32,
        "doc count must be exact"
    );
    assert_eq!(report.skipped_binary, 1, "binary blob skipped");
    assert_eq!(
        report.docs_indexed, EXPECTED_DOCS,
        "live docs must be exact"
    );
    assert!(
        cold_elapsed < COLD_BUDGET,
        "cold index took {cold_elapsed:?}"
    );

    // Warm up caches, then measure the best of three identical queries.
    index.search("widget_4242", 5).expect("warmup query");
    let mut warm_elapsed = Duration::MAX;
    for _ in 0..3 {
        let started = Instant::now();
        let hits = index.search("widget_4242", 5).expect("warm query");
        warm_elapsed = warm_elapsed.min(started.elapsed());
        assert_eq!(hits.len(), 1, "symbol query has one right answer");
        assert_eq!(hits[0].path, "mod_042/file_04242.rs");
    }
    assert!(
        warm_elapsed < WARM_BUDGET,
        "warm query took {warm_elapsed:?}"
    );

    let top5: Vec<String> = index
        .search("quartz rune", 5)
        .expect("ranked query")
        .iter()
        .map(|h| format!("{}  score={:.3}", h.path, h.score))
        .collect();
    eprintln!(
        "task-17 acceptance: cold={cold_elapsed:?} (plan 5s), warm={warm_elapsed:?} \
         (plan 20ms), docs={}",
        report.docs_indexed
    );
    eprintln!("task-17 top-5 for 'quartz rune':");
    for line in &top5 {
        eprintln!("  {line}");
    }
}

#[test]
fn incremental_one_file_of_10k_within_budget() {
    let (_keep, corpus, index_dir) = setup("perf-incremental", 0x1CE_BABE);
    let index = CodeIndex::open(&index_dir).expect("open");
    index.refresh(&corpus).expect("initial refresh");

    // 500 % 100 = 0: the synth corpus wrote file_00500.rs into mod_000.
    write_file(
        &corpus,
        "mod_000/file_00500.rs",
        "pub fn delta_marker() {}\n",
    );
    let started = Instant::now();
    let report = index.refresh(&corpus).expect("incremental refresh");
    let elapsed = started.elapsed();

    assert_eq!(
        report.touched(),
        1,
        "exactly one doc touched, no rebuild — report: {report:?}"
    );
    assert_eq!(report.updated, 1);
    assert_eq!(report.docs_indexed, EXPECTED_DOCS);
    assert!(
        elapsed < INCREMENTAL_BUDGET,
        "incremental refresh took {elapsed:?}"
    );
    eprintln!(
        "task-17 incremental: {elapsed:?} (plan 500ms), touched={} docs={}",
        report.touched(),
        report.docs_indexed
    );
}
