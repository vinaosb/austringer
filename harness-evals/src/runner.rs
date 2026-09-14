//! Pair execution: drives one model×profile pair's [`Subject`] over
//! every dataset case for k repetitions, grades each stream with the
//! deterministic graders, runs the integrity guards, and folds the
//! reps into case metrics. Grader errors are TYPED and NON-FATAL
//! per case; subject errors fingerprint the rep and never abort the
//! sweep.

use std::collections::BTreeMap;

use crate::dataset::{EvalCase, EvalDataset};
use crate::grader::{self, GraderSpec};
use crate::guard::GuardSet;
use crate::metrics::{self, CaseMetrics, FailureCategory, RepContext, RepResult};
use crate::wire::TurnStream;

/// One unit of evaluation: a model×profile PAIR (research §33 — the
/// unit is the harness–model pair, never a bare model).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SweepPair {
    /// Model id.
    pub model: String,
    /// Profile id (the harness configuration under test).
    pub profile: String,
}

impl SweepPair {
    /// `"model/profile"` label.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}/{}", self.model, self.profile)
    }
}

/// A subject under test: produces the NDJSON lines of one headless
/// `harness run --output-json` invocation for (case, rep). The
/// scripted implementation is the hermetic fake inference; the live
/// CLI adapter is built by [`crate::adapter`] and driven by the
/// SCHEDULED sweep runner (never in this crate's tests).
pub trait Subject {
    /// Runs one case repetition, returning the raw NDJSON lines.
    ///
    /// # Errors
    /// typed [`SubjectError`] (fingerprinted, non-fatal).
    fn run(&mut self, case: &EvalCase, rep: u32) -> Result<Vec<String>, SubjectError>;
}

/// A subject failure (bounded detail).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("subject failed: {detail}")]
pub struct SubjectError {
    /// Bounded detail.
    pub detail: String,
}

/// Scripted fake inference: per-case, per-rep recorded NDJSON line
/// scripts (rep index cycles through the per-case list — supplying two
/// alternating scripts models a FLAKY subject, which Pass^k must
/// catch).
#[derive(Default)]
pub struct ScriptedSubject {
    scripts: BTreeMap<String, Vec<Vec<String>>>,
}

impl ScriptedSubject {
    /// An empty subject (add scripts with [`ScriptedSubject::add`]).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the per-repetition scripts for one case.
    pub fn add(&mut self, case_id: &str, rep_scripts: Vec<Vec<String>>) {
        self.scripts.insert(case_id.to_owned(), rep_scripts);
    }
}

impl Subject for ScriptedSubject {
    fn run(&mut self, case: &EvalCase, rep: u32) -> Result<Vec<String>, SubjectError> {
        let scripts = self.scripts.get(&case.id).ok_or_else(|| SubjectError {
            detail: format!("no script for case {:?}", case.id),
        })?;
        if scripts.is_empty() {
            return Err(SubjectError {
                detail: format!("no rep scripts for case {:?}", case.id),
            });
        }
        let index = (rep.saturating_sub(1) as usize) % scripts.len();
        Ok(scripts[index].clone())
    }
}

/// One pair's full run over a dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct PairRun {
    /// The pair.
    pub pair: SweepPair,
    /// Per-case metrics in dataset order.
    pub cases: Vec<CaseMetrics>,
}

/// Ranked row (the deterministic ranking table input).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RankedPair {
    /// The pair.
    pub pair: SweepPair,
    /// Pass^k floor (primary sort key).
    pub pass_k: f64,
    /// Mean@k (secondary).
    pub mean: f64,
    /// Tokens per solved case when known.
    pub tokens_solved: Option<u64>,
}

/// The shared ranking total order: pass^k DESC, mean DESC, pair ASC.
fn compare_ranked(a: &RankedPair, b: &RankedPair) -> std::cmp::Ordering {
    b.pass_k
        .total_cmp(&a.pass_k)
        .then_with(|| b.mean.total_cmp(&a.mean))
        .then_with(|| a.pair.cmp(&b.pair))
}

/// Sorts ranked rows into the canonical total order (shared with the
/// report assembler).
pub fn sort_ranked(rows: &mut [RankedPair]) {
    rows.sort_by(compare_ranked);
}

/// Ranks pair runs: pass^k DESC, then mean DESC, then model/profile
/// ASC (a total order — identical inputs always rank identically).
#[must_use]
pub fn rank(runs: &[PairRun]) -> Vec<RankedPair> {
    let mut rows: Vec<RankedPair> = runs
        .iter()
        .map(|run| {
            let metrics = metrics::pair_metrics(&run.cases);
            RankedPair {
                pair: run.pair.clone(),
                pass_k: metrics.pass_k,
                mean: metrics.mean,
                tokens_solved: metrics.tokens_solved,
            }
        })
        .collect();
    sort_ranked(&mut rows);
    rows
}

/// Grades one parsed stream against one spec with the guard set
/// (shared with the re-trace re-scorer). Grader compile/score errors
/// are TYPED and non-fatal: the rep fingerprints `grader_error`.
#[must_use]
pub fn grade_stream(
    case: &EvalCase,
    stream: &TurnStream,
    spec: &GraderSpec,
    guards: &GuardSet,
) -> RepResult {
    let graded = grader::compile(spec).and_then(|grader| grader.score(&stream.assistant_text));
    match graded {
        Err(_) => RepResult {
            rep: 1,
            raw_score: 0.0,
            score: 0.0,
            passed: false,
            category: FailureCategory::GraderError,
            hits: Vec::new(),
            tokens: None,
        },
        Ok(raw) => {
            let guarded = guards.apply(case, stream, raw);
            let passed = guarded.score >= case.threshold;
            let context = RepContext {
                guarded: &guarded,
                grader_failed: false,
                subject_failed: false,
                stream_invalid: false,
                passed,
            };
            RepResult {
                rep: 1,
                raw_score: guarded.raw_score,
                score: guarded.score,
                passed,
                category: metrics::categorize(&context),
                hits: guarded
                    .hits
                    .iter()
                    .map(|hit| (hit.guard_id.clone(), hit.flag))
                    .collect(),
                tokens: None,
            }
        }
    }
}

/// Runs one repetition end-to-end (subject → stream → grade).
fn run_rep(case: &EvalCase, rep: u32, subject: &mut dyn Subject, guards: &GuardSet) -> RepResult {
    let lines = match subject.run(case, rep) {
        Err(_) => {
            return RepResult {
                rep,
                raw_score: 0.0,
                score: 0.0,
                passed: false,
                category: FailureCategory::SubjectError,
                hits: Vec::new(),
                tokens: None,
            };
        }
        Ok(lines) => lines,
    };
    match TurnStream::parse(&lines) {
        Err(_) => RepResult {
            rep,
            raw_score: 0.0,
            score: 0.0,
            passed: false,
            category: FailureCategory::StreamInvalid,
            hits: Vec::new(),
            tokens: None,
        },
        Ok(stream) => RepResult {
            rep,
            ..grade_stream(case, &stream, &case.grader, guards)
        },
    }
}

/// Executes one pair. Grader compile failures (impossible for parsed
/// datasets — validated at parse — but possible for re-scored traces)
/// and grading errors fingerprint the rep as `grader_error` and
/// continue.
#[must_use]
pub fn run_pair(
    pair: &SweepPair,
    dataset: &EvalDataset,
    reps: u32,
    subject: &mut dyn Subject,
    guards: &GuardSet,
) -> PairRun {
    let cases = dataset
        .cases
        .iter()
        .map(|case| {
            let rep_results: Vec<RepResult> = (1..=reps)
                .map(|rep| run_rep(case, rep, subject, guards))
                .collect();
            metrics::case_metrics(&case.id, case.threshold, rep_results)
        })
        .collect();
    PairRun {
        pair: pair.clone(),
        cases,
    }
}
