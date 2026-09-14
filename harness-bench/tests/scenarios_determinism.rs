//! Scenario runners: deterministic outputs across runs, correct skip
//! typing for missing inputs, and metric wiring (values present, policies
//! attached). Wall-clock enforcement belongs to the gate binary; these
//! tests pin the deterministic surface. Under `--features dhat` each test
//! owns a profiler (nextest: one test process = dhat's one profiler).

use std::collections::BTreeSet;

use harness_bench::gate::{Policy, evaluate_metric};
use harness_bench::report::MetricStatus;
use harness_bench::scenarios::{
    ScenarioContext, ScenarioError, eviction, idle, process, prompt, pty, repomap, sessions, tui,
};

fn ctx() -> ScenarioContext {
    ScenarioContext::for_tests()
}

#[test]
fn repomap_is_deterministic_and_measures_tokens() {
    #[cfg(feature = "dhat")]
    let _profiler = harness_bench::scenarios::heap::start_profiler();
    let a = repomap::run(&ctx()).expect("run a");
    let b = repomap::run(&ctx()).expect("run b");
    let ta = a.value("repomap.tokens").expect("tokens metric");
    let tb = b.value("repomap.tokens").expect("tokens metric");
    assert_eq!(ta, tb, "same seed -> same token count");
    assert!(ta > 0.0, "tokens must be MEASURED, not a constant zero");
}

#[test]
fn prompt_prefix_bytes_are_deterministic() {
    #[cfg(feature = "dhat")]
    let _profiler = harness_bench::scenarios::heap::start_profiler();
    let a = prompt::run(&ctx()).expect("run a");
    let b = prompt::run(&ctx()).expect("run b");
    let pa = a
        .value("prompt.stable_prefix_bytes")
        .expect("prefix metric");
    let pb = b
        .value("prompt.stable_prefix_bytes")
        .expect("prefix metric");
    assert_eq!(pa, pb);
    assert!(pa > 0.0);
}

#[test]
fn pty_observation_is_deterministic_and_bounded() {
    #[cfg(feature = "dhat")]
    let _profiler = harness_bench::scenarios::heap::start_profiler();
    let a = pty::run(&ctx()).expect("run a");
    let b = pty::run(&ctx()).expect("run b");
    let oa = a
        .value("pty.observation_bytes")
        .expect("observation metric");
    let ob = b
        .value("pty.observation_bytes")
        .expect("observation metric");
    assert_eq!(oa, ob);
    assert!(oa > 0.0);
}

#[test]
fn sessions_page_bytes_are_deterministic() {
    #[cfg(feature = "dhat")]
    let _profiler = harness_bench::scenarios::heap::start_profiler();
    let a = sessions::run(&ctx()).expect("run a");
    let b = sessions::run(&ctx()).expect("run b");
    let ba = a.value("sessions.page_bytes").expect("page bytes");
    let bb = b.value("sessions.page_bytes").expect("page bytes");
    assert_eq!(ba, bb);
    assert!(ba > 0.0);
}

#[test]
fn eviction_projection_is_deterministic() {
    #[cfg(feature = "dhat")]
    let _profiler = harness_bench::scenarios::heap::start_profiler();
    let a = eviction::run(&ctx()).expect("run a");
    let b = eviction::run(&ctx()).expect("run b");
    let ea = a
        .value("eviction.projected_events")
        .expect("projection metric");
    let eb = b
        .value("eviction.projected_events")
        .expect("projection metric");
    assert_eq!(ea, eb);
    assert!(ea > 0.0, "eviction must project a non-empty history");
}

#[test]
fn tui_emits_frame_metrics_with_budget_policies() {
    #[cfg(feature = "dhat")]
    let _profiler = harness_bench::scenarios::heap::start_profiler();
    let out = tui::run(&ctx()).expect("tui run");
    let p99 = out.value("tui.p99_frame_ms").expect("p99 metric");
    let breaches = out.value("tui.frame_breaches").expect("breach metric");
    let hangs = out.value("tui.hangs").expect("hang metric");
    assert!(p99 > 0.0, "p99 must be measured");
    assert!(breaches >= 0.0);
    assert!(hangs >= 0.0);
    let record = evaluate_metric(
        "tui.p99_frame_ms",
        "ms",
        p99,
        Policy::Budget { budget: 16.0 },
        None,
        10,
    );
    assert_eq!(
        record.status,
        MetricStatus::Pass,
        "debug TestBackend p99 {p99}ms must sit well under 16ms"
    );
}

#[test]
fn idle_drains_100k_noop_events_with_zero_renders() {
    let out = idle::run(&ctx()).expect("idle run");
    let renders = out.value("idle.renders").expect("renders metric");
    assert_eq!(
        renders, 0.0,
        "no-op events must never render (idle CPU 0.0%)"
    );
}

#[test]
fn missing_process_inputs_are_typed_skips() {
    let err = process::cold_start(&ctx()).expect_err("no cli bin");
    assert!(
        matches!(err, ScenarioError::Skipped(ref why) if why.contains("--cli-bin")),
        "{err}"
    );
    let err = process::daemon_rss(&ctx()).expect_err("no daemon bin");
    assert!(
        matches!(err, ScenarioError::Skipped(ref why) if why.contains("--daemon-bin")),
        "{err}"
    );
}

#[test]
fn binary_size_measures_real_file_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("fake-bin.exe");
    let payload = vec![0u8; 1024 * 1024]; // exactly 1 MiB
    std::fs::write(&file, &payload).expect("write fake bin");
    let mut context = ctx();
    context.binaries = vec![("fake".to_owned(), file)];
    let out = process::binary_size(&context).expect("binary size run");
    let mb = out.value("binary.fake_mb").expect("size metric");
    assert!((mb - 1.0).abs() < 0.01, "1 MiB file measures {mb} MB");
}

#[test]
fn self_test_measurement_breaches_its_budget() {
    // End-to-end: the synthetic slow scenario MEASURES >= 3x budget, and
    // gate evaluation flags it — the anti misleading-success proof.
    let out = process::self_test(&ctx()).expect("self test run");
    let measured = out.value("self_test.cold_start_ms").expect("measured ms");
    assert!(
        measured >= 150.0,
        "busy-well must exceed 3x50ms, measured {measured}"
    );
    let record = evaluate_metric(
        "self_test.cold_start_ms",
        "ms",
        measured,
        Policy::Budget { budget: 50.0 },
        None,
        10,
    );
    assert_eq!(record.status, MetricStatus::Breach);
}

#[test]
fn gate_baseline_metric_names_are_exact() {
    let expected: BTreeSet<&str> = harness_bench::scenarios::GATE_BASELINE_METRICS
        .iter()
        .copied()
        .collect();
    assert_eq!(
        expected,
        [
            "repomap.tokens",
            "eviction.projected_events",
            "prompt.stable_prefix_bytes",
            "pty.observation_bytes",
            "sessions.page_bytes",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>()
    );
}
