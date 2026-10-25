//! Frame-budget discipline (Zed GPUI, research §23): a 10ms frame budget
//! with hang-threshold detection, per-frame foreground-spend attribution,
//! and a debug overlay that renders MEASURED numbers. Classification is a
//! pure function over recorded samples so the gate verdict is exactly
//! reproducible.

use std::time::Duration;

use crate::stats::{StatsError, format_ms, percentile_nearest_rank};

/// One recorded frame: total wall time plus named foreground phases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameSample {
    pub frame: u32,
    pub total: Duration,
    pub phases: Vec<(&'static str, Duration)>,
}

/// A budget violation with the worst (largest) phase attributed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameBreach {
    pub frame: u32,
    pub total: Duration,
    pub worst_phase: Option<(&'static str, Duration)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameVerdict {
    pub frames: usize,
    pub p50: Duration,
    pub p99: Duration,
    pub breaches: Vec<FrameBreach>,
    pub hangs: Vec<FrameBreach>,
}

fn worst_phase(sample: &FrameSample) -> Option<(&'static str, Duration)> {
    sample
        .phases
        .iter()
        .max_by_key(|(_, d)| *d)
        .map(|&(name, d)| (name, d))
}

fn breach_of(sample: &FrameSample) -> FrameBreach {
    FrameBreach {
        frame: sample.frame,
        total: sample.total,
        worst_phase: worst_phase(sample),
    }
}

/// Classify recorded frames against the frame budget and hang threshold.
/// A hang (over `hang`) is also listed as a budget breach. Requires at
/// least one sample — a zero-frame "pass" would be misleading success.
pub fn classify(
    samples: &[FrameSample],
    budget: Duration,
    hang: Duration,
) -> Result<FrameVerdict, StatsError> {
    let totals: Vec<Duration> = samples.iter().map(|s| s.total).collect();
    let p50 = percentile_nearest_rank(&totals, 50)?;
    let p99 = percentile_nearest_rank(&totals, 99)?;
    let breaches: Vec<FrameBreach> = samples
        .iter()
        .filter(|s| s.total > budget)
        .map(breach_of)
        .collect();
    let hangs: Vec<FrameBreach> = samples
        .iter()
        .filter(|s| s.total > hang)
        .map(breach_of)
        .collect();
    Ok(FrameVerdict {
        frames: samples.len(),
        p50,
        p99,
        breaches,
        hangs,
    })
}

/// Debug overlay lines (surfaced in the TUI debug overlay and the gate
/// report): budget, measured percentiles, breach count, hang count, and
/// per-breach foreground-spend attribution.
pub fn overlay_lines(verdict: &FrameVerdict, budget: Duration, hang: Duration) -> Vec<String> {
    let mut lines = vec![
        format!(
            "frame-budget: {} budget | p50 {} | p99 {} | breaches {}/{}",
            format_ms(budget),
            format_ms(verdict.p50),
            format_ms(verdict.p99),
            verdict.breaches.len(),
            verdict.frames
        ),
        format!(
            "hang-threshold: {} | hangs {}",
            format_ms(hang),
            verdict.hangs.len()
        ),
    ];
    for b in &verdict.breaches {
        match b.worst_phase {
            Some((name, d)) => lines.push(format!(
                "frame {}: {} ({} {})",
                b.frame,
                format_ms(b.total),
                name,
                format_ms(d)
            )),
            None => lines.push(format!("frame {}: {}", b.frame, format_ms(b.total))),
        }
    }
    lines
}
