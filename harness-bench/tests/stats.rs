//! Stats helpers for the perf gate: robust-under-load reduction (min-of-N)
//! and nearest-rank percentiles over measured samples.

use std::time::Duration;

use harness_bench::stats::{StatsError, format_ms, min_of, percentile_nearest_rank};

#[test]
fn min_of_returns_the_minimum_sample() {
    let samples = [
        Duration::from_millis(30),
        Duration::from_millis(7),
        Duration::from_millis(12),
    ];
    assert_eq!(min_of(&samples), Ok(Duration::from_millis(7)));
}

#[test]
fn min_of_rejects_empty_input_with_typed_error() {
    let err = min_of(&[]).expect_err("empty input must be a typed error");
    assert!(matches!(err, StatsError::EmptySamples));
}

#[test]
fn percentile_nearest_rank_p99_of_100_ordered_samples() {
    // 1ms..=100ms: nearest-rank p99 = ceil(0.99 * 100) = 99th sample = 99ms.
    let samples: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
    assert_eq!(
        percentile_nearest_rank(&samples, 99),
        Ok(Duration::from_millis(99))
    );
}

#[test]
fn percentile_nearest_rank_sorts_unordered_input() {
    let samples = [
        Duration::from_millis(40),
        Duration::from_millis(10),
        Duration::from_millis(30),
        Duration::from_millis(20),
    ];
    // ceil(0.5 * 4) = 2nd smallest = 20ms.
    assert_eq!(
        percentile_nearest_rank(&samples, 50),
        Ok(Duration::from_millis(20))
    );
}

#[test]
fn percentile_nearest_rank_p100_is_the_maximum() {
    let samples = [Duration::from_micros(500), Duration::from_millis(2)];
    assert_eq!(
        percentile_nearest_rank(&samples, 100),
        Ok(Duration::from_millis(2))
    );
}

#[test]
fn percentile_nearest_rank_rejects_empty_input() {
    let err = percentile_nearest_rank(&[], 99).expect_err("empty input must be a typed error");
    assert!(matches!(err, StatsError::EmptySamples));
}

#[test]
fn percentile_nearest_rank_rejects_out_of_range_percentile() {
    let samples = [Duration::from_millis(1)];
    assert!(matches!(
        percentile_nearest_rank(&samples, 0),
        Err(StatsError::PercentileOutOfRange { pct: 0 })
    ));
    assert!(matches!(
        percentile_nearest_rank(&samples, 101),
        Err(StatsError::PercentileOutOfRange { pct: 101 })
    ));
}

#[test]
fn format_ms_renders_sub_millisecond_durations_with_precision() {
    assert_eq!(format_ms(Duration::from_micros(1)), "0.001ms");
    assert_eq!(format_ms(Duration::from_millis(1580)), "1580.000ms");
}
