//! Frame-budget discipline (Zed GPUI, research §23): 10ms frame budget,
//! hang-threshold detection, per-frame foreground-spend attribution, and a
//! debug overlay rendering MEASURED numbers.

use std::time::Duration;

use austringer_bench::frame::{FrameSample, classify, overlay_lines};

fn sample(frame: u32, total_ms: u64, phases: &[(&'static str, u64)]) -> FrameSample {
    FrameSample {
        frame,
        total: Duration::from_millis(total_ms),
        phases: phases
            .iter()
            .map(|(n, ms)| (*n, Duration::from_millis(*ms)))
            .collect(),
    }
}

#[test]
fn all_frames_under_budget_report_no_breaches() {
    let samples = [
        sample(0, 2, &[("layout", 1), ("draw", 1)]),
        sample(1, 4, &[]),
    ]; // max 4ms < 10ms
    let verdict = classify(
        &samples,
        Duration::from_millis(10),
        Duration::from_millis(100),
    )
    .expect("classify");
    assert!(verdict.breaches.is_empty());
    assert!(verdict.hangs.is_empty());
    assert_eq!(verdict.frames, 2);
}

#[test]
fn frame_over_budget_is_a_breach_with_phase_attribution() {
    let samples = [
        sample(0, 3, &[("draw", 2)]),
        sample(1, 15, &[("layout", 4), ("draw", 9)]), // 15ms > 10ms budget
    ];
    let verdict = classify(
        &samples,
        Duration::from_millis(10),
        Duration::from_millis(100),
    )
    .expect("classify");
    assert_eq!(verdict.breaches.len(), 1);
    let breach = &verdict.breaches[0];
    assert_eq!(breach.frame, 1);
    assert_eq!(breach.total, Duration::from_millis(15));
    // Worst-phase attribution points at the largest foreground spend.
    assert_eq!(breach.worst_phase, Some(("draw", Duration::from_millis(9))));
}

#[test]
fn hang_threshold_is_detected_separately_from_budget_breach() {
    let samples = [sample(0, 120, &[("draw", 118)])]; // > 100ms hang threshold
    let verdict = classify(
        &samples,
        Duration::from_millis(10),
        Duration::from_millis(100),
    )
    .expect("classify");
    assert_eq!(verdict.hangs.len(), 1, "120ms frame is a hang");
    assert_eq!(
        verdict.breaches.len(),
        1,
        "the same frame also breached the 10ms budget"
    );
}

#[test]
fn percentiles_come_from_the_measured_totals() {
    let samples: Vec<FrameSample> = (1..=100u32)
        .map(|i| sample(i - 1, u64::from(i), &[]))
        .collect();
    let verdict = classify(
        &samples,
        Duration::from_millis(10),
        Duration::from_millis(100),
    )
    .expect("classify");
    assert_eq!(verdict.p50, Duration::from_millis(50));
    assert_eq!(verdict.p99, Duration::from_millis(99));
}

#[test]
fn overlay_renders_measured_numbers_not_constants() {
    // 100 samples of exactly 1..=100ms -> p99 MUST render as 99.000ms in the
    // overlay. A canned/constant overlay would fail this by construction.
    let samples: Vec<FrameSample> = (1..=100u32)
        .map(|i| sample(i - 1, u64::from(i), &[("draw", 1)]))
        .collect();
    let verdict = classify(
        &samples,
        Duration::from_millis(10),
        Duration::from_millis(100),
    )
    .expect("classify");
    let overlay = overlay_lines(
        &verdict,
        Duration::from_millis(10),
        Duration::from_millis(100),
    );
    let joined = overlay.join("\n");
    assert!(joined.contains("p99 99.000ms"), "overlay: {joined}");
    assert!(
        joined.contains("breaches 90/100"),
        "90 samples (11..=100ms) exceed 10ms: {joined}"
    );
    assert!(joined.contains("hangs 0"), "overlay: {joined}");
    // The attribution line names the worst frame and its phase.
    assert!(
        joined.contains("draw"),
        "overlay must surface foreground spend: {joined}"
    );
}

#[test]
fn overlay_lists_each_breaching_frame_with_attribution() {
    let samples = [sample(7, 15, &[("layout", 5), ("draw", 9)])];
    let verdict = classify(
        &samples,
        Duration::from_millis(10),
        Duration::from_millis(100),
    )
    .expect("classify");
    let overlay = overlay_lines(
        &verdict,
        Duration::from_millis(10),
        Duration::from_millis(100),
    );
    let joined = overlay.join("\n");
    assert!(joined.contains("frame 7: 15.000ms"), "overlay: {joined}");
    assert!(joined.contains("draw 9.000ms"), "overlay: {joined}");
}
