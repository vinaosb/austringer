//! Robust-under-load sample reductions for gate metrics.
//!
//! Wall-clock budgets use min-of-N (the least-noise estimator of "the
//! system can do this in T"); frame statistics use nearest-rank
//! percentiles. Both are pure so tests pin their behavior exactly.

use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StatsError {
    #[error("no samples to reduce")]
    EmptySamples,
    #[error("percentile {pct} out of range 1..=100")]
    PercentileOutOfRange { pct: u32 },
}

/// Minimum of `samples` — the robust "capable of" estimator under load.
pub fn min_of(samples: &[Duration]) -> Result<Duration, StatsError> {
    samples
        .iter()
        .min()
        .copied()
        .ok_or(StatsError::EmptySamples)
}

/// Nearest-rank percentile: value at rank `ceil(pct/100 * n)` of the
/// ascending-sorted samples. `pct` must be 1..=100.
pub fn percentile_nearest_rank(samples: &[Duration], pct: u32) -> Result<Duration, StatsError> {
    if pct == 0 || pct > 100 {
        return Err(StatsError::PercentileOutOfRange { pct });
    }
    let mut sorted = Vec::from(samples);
    sorted.sort_unstable();
    let n = sorted.len();
    if n == 0 {
        return Err(StatsError::EmptySamples);
    }
    let rank = (u64::from(pct) * n as u64).div_ceil(100) as usize; // 1-based
    let rank = rank.clamp(1, n);
    Ok(sorted[rank - 1])
}

/// Millisecond rendering with three decimals (reports + debug overlay).
pub fn format_ms(d: Duration) -> String {
    let micros = d.as_micros();
    format!("{}.{:03}ms", micros / 1000, micros % 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_matches_reference_values() {
        // ceil(0.99 * 1) = 1 -> the only sample.
        assert_eq!(
            percentile_nearest_rank(&[Duration::from_millis(7)], 99),
            Ok(Duration::from_millis(7))
        );
        // ceil(0.99 * 100) = 99th of 1..=100 = 99.
        let samples: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
        assert_eq!(
            percentile_nearest_rank(&samples, 99),
            Ok(Duration::from_millis(99))
        );
        // ceil(0.99 * 200) = 198th of 1..=200 = 198.
        let samples: Vec<Duration> = (1..=200).map(Duration::from_millis).collect();
        assert_eq!(
            percentile_nearest_rank(&samples, 99),
            Ok(Duration::from_millis(198))
        );
    }
}
