//! Micro-harness eval metrics (forge + harness-rs discipline, research
//! §26): **Pass^k floors, not pass@1 peaks** — a case counts as solved
//! only when ALL k repetitions passed (the 24pt consistency gap between
//! Mean@k and Pass^k is reported side by side). Also owns failure
//! fingerprints, tokens/solved, guard trigger-coverage, and leave-one-
//! out guard ablation deltas.

use std::collections::BTreeMap;

use crate::guard::{GuardedScore, IntegrityFlag};

/// Failure taxonomy for one repetition (the fingerprint vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    /// Rep passed its threshold.
    Pass,
    /// Ran clean, scored under threshold.
    GraderMismatch,
    /// The grader itself errored (non-fatal, counted separately).
    GraderError,
    /// The subject failed to produce a stream.
    SubjectError,
    /// The NDJSON stream was invalid.
    StreamInvalid,
    /// The turn failed (guard zero).
    TurnFailed,
    /// Canary leakage (guard zero).
    CanaryLeak,
    /// Upstream PR/diff open (guard zero).
    FairInternetUse,
}

impl FailureCategory {
    /// Stable snake_case name (fingerprint keys).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::GraderMismatch => "grader_mismatch",
            Self::GraderError => "grader_error",
            Self::SubjectError => "subject_error",
            Self::StreamInvalid => "stream_invalid",
            Self::TurnFailed => "turn_failed",
            Self::CanaryLeak => "canary_leak",
            Self::FairInternetUse => "fair_internet_use",
        }
    }
}

/// One repetition's result (raw score kept for ablation).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepResult {
    /// 1-based repetition number.
    pub rep: u32,
    /// Raw grader score pre-zeroing (`[0,1]`).
    pub raw_score: f64,
    /// Reported score post-guards (`[0,1]`).
    pub score: f64,
    /// Whether the reported score met the case threshold.
    pub passed: bool,
    /// Failure category (derives from guards/grader/subject).
    pub category: FailureCategory,
    /// Guards that fired.
    pub hits: Vec<(String, IntegrityFlag)>,
    /// Token usage when the stream reported it (tokens/solved).
    pub tokens: Option<u64>,
}

/// Aggregated pair metrics (one model×profile cell).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct PairMetrics {
    /// Fraction of cases with `pass_k == true` (the floor).
    pub pass_k: f64,
    /// Mean of case means (Mean@k across the dataset).
    pub mean: f64,
    /// Tokens per SOLVED case when token data exists.
    pub tokens_solved: Option<u64>,
    /// Failure fingerprint: category name → count across all reps.
    pub fingerprint: BTreeMap<String, u32>,
    /// Per-case metrics (dataset order).
    pub cases: Vec<CaseMetrics>,
}

/// Leave-one-out ablation for one guard: dataset pass^k with the guard
/// on (as measured) vs recomputed with it off, plus the delta.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AblationDelta {
    /// Guard id.
    pub guard_id: String,
    /// pass^k with the guard active.
    pub with_guard: f64,
    /// pass^k recomputed as if the guard never fired.
    pub without_guard: f64,
    /// `without - with` (the guard's measured effect).
    pub delta: f64,
}

/// Trigger coverage for one guard (a guard that never fired is
/// UNMEASURED NOISE — surfaced, not hidden).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GuardCoverage {
    /// Guard id.
    pub guard_id: String,
    /// Reps in which it fired.
    pub fires: u32,
    /// True when `fires == 0` (unmeasured noise).
    pub noise: bool,
}

/// Context the category collapse needs (grouped: one struct beats a
/// five-parameter function).
#[derive(Debug, Clone, Copy)]
pub struct RepContext<'a> {
    /// The guarded score.
    pub guarded: &'a GuardedScore,
    /// Whether the grader itself errored.
    pub grader_failed: bool,
    /// Whether the subject failed before producing a stream.
    pub subject_failed: bool,
    /// Whether the NDJSON stream failed to parse.
    pub stream_invalid: bool,
    /// Whether the reported score met the case threshold.
    pub passed: bool,
}

/// Collapses one repetition into its failure category.
#[must_use]
pub fn categorize(context: &RepContext<'_>) -> FailureCategory {
    if context.subject_failed {
        return FailureCategory::SubjectError;
    }
    if context.stream_invalid {
        return FailureCategory::StreamInvalid;
    }
    if context.grader_failed {
        return FailureCategory::GraderError;
    }
    if let Some(hit) = context.guarded.hits.first() {
        return match hit.flag {
            IntegrityFlag::CanaryLeak => FailureCategory::CanaryLeak,
            IntegrityFlag::FairInternetUse => FailureCategory::FairInternetUse,
            IntegrityFlag::TurnFailed => FailureCategory::TurnFailed,
        };
    }
    if context.passed {
        FailureCategory::Pass
    } else {
        FailureCategory::GraderMismatch
    }
}

/// Per-case metrics across the k repetitions.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CaseMetrics {
    /// Case id.
    pub case_id: String,
    /// The case's pass threshold (kept for ablation recompute).
    pub threshold: f64,
    /// All rep results (deterministic order).
    pub reps: Vec<RepResult>,
    /// Mean reported score across reps (Mean@k).
    pub mean_at_k: f64,
    /// True only when EVERY rep passed (Pass^k floor).
    pub pass_k: bool,
    /// Summed tokens when every rep reported them.
    pub tokens: Option<u64>,
}

/// Case metrics from the k rep results (dataset-order preserved).
#[must_use]
pub fn case_metrics(case_id: &str, threshold: f64, reps: Vec<RepResult>) -> CaseMetrics {
    let mean_at_k = if reps.is_empty() {
        0.0
    } else {
        reps.iter().map(|rep| rep.score).sum::<f64>() / reps.len() as f64
    };
    let pass_k = !reps.is_empty() && reps.iter().all(|rep| rep.score >= threshold);
    let tokens = (!reps.is_empty() && reps.iter().all(|rep| rep.tokens.is_some()))
        .then(|| reps.iter().map(|rep| rep.tokens.unwrap_or(0)).sum());
    CaseMetrics {
        case_id: case_id.to_owned(),
        threshold,
        reps,
        mean_at_k,
        pass_k,
        tokens,
    }
}

/// Pass^k fraction over cases (0.0 on an empty set).
fn fraction(count: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        count as f64 / total as f64
    }
}

/// Pair metrics from case metrics.
#[must_use]
pub fn pair_metrics(cases: &[CaseMetrics]) -> PairMetrics {
    let solved = cases.iter().filter(|case| case.pass_k).count();
    let solved_tokens: Option<Vec<u64>> = cases
        .iter()
        .filter(|case| case.pass_k)
        .map(|case| case.tokens)
        .collect();
    let mut fingerprint: BTreeMap<String, u32> = BTreeMap::new();
    for case in cases {
        for rep in &case.reps {
            *fingerprint
                .entry(rep.category.name().to_owned())
                .or_insert(0) += 1;
        }
    }
    PairMetrics {
        pass_k: fraction(solved, cases.len()),
        mean: if cases.is_empty() {
            0.0
        } else {
            cases.iter().map(|case| case.mean_at_k).sum::<f64>() / cases.len() as f64
        },
        tokens_solved: solved_tokens.map(|tokens| tokens.into_iter().sum()),
        fingerprint,
        cases: cases.to_vec(),
    }
}

/// Recomputes one case as if `guard_id` never fired (other guards
/// still zero; the ablated guard's zeroing is undone from raw_score).
fn ablate_case(case: &CaseMetrics, guard_id: &str) -> CaseMetrics {
    let reps: Vec<RepResult> = case
        .reps
        .iter()
        .map(|rep| {
            let hits: Vec<(String, IntegrityFlag)> = rep
                .hits
                .iter()
                .filter(|(id, _)| id != guard_id)
                .cloned()
                .collect();
            let score = if hits.is_empty() { rep.raw_score } else { 0.0 };
            RepResult {
                score,
                passed: score >= case.threshold,
                hits,
                ..rep.clone()
            }
        })
        .collect();
    case_metrics(&case.case_id, case.threshold, reps)
}

/// Leave-one-out ablation for one guard across a pair's cases.
#[must_use]
pub fn guard_ablation(cases: &[CaseMetrics], guard_id: &str) -> AblationDelta {
    let with_guard = fraction(cases.iter().filter(|case| case.pass_k).count(), cases.len());
    let without: Vec<CaseMetrics> = cases
        .iter()
        .map(|case| ablate_case(case, guard_id))
        .collect();
    let without_guard = fraction(
        without.iter().filter(|case| case.pass_k).count(),
        without.len(),
    );
    AblationDelta {
        guard_id: guard_id.to_owned(),
        with_guard,
        without_guard,
        delta: without_guard - with_guard,
    }
}

/// Trigger coverage for the given guard ids across all reps.
#[must_use]
pub fn trigger_coverage(cases: &[CaseMetrics], guard_ids: &[&str]) -> Vec<GuardCoverage> {
    guard_ids
        .iter()
        .map(|id| {
            let fires: u32 = cases
                .iter()
                .map(|case| {
                    case.reps
                        .iter()
                        .filter(|rep| rep.hits.iter().any(|(gid, _)| gid == id))
                        .count() as u32
                })
                .sum();
            GuardCoverage {
                guard_id: (*id).to_owned(),
                fires,
                noise: fires == 0,
            }
        })
        .collect()
}
