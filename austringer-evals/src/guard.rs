//! Integrity guards: the eval-gaming defenses. Every guard is
//! test-pinned and names the measured pathology it prevents (forge
//! micro-harness discipline, research §26):
//!
//! - [`CanaryGuard`] — prevents DATASET LEAKAGE: a canary is never in
//!   the prompt, so its presence in output proves the subject saw the
//!   dataset; the case is zeroed.
//! - [`FairInternetUseGuard`] — prevents UPSTREAM CONTAMINATION
//!   (FrontierCode integrity rule, research §22): a run that opens
//!   upstream PRs/diffs (github `/pull/`, `/compare/`,
//!   `patch-diff.githubusercontent.com`) is zeroed automatically.
//! - [`TurnOutcomeGuard`] — prevents PARTIAL-OUTPUT INFLATION: text
//!   that happens to satisfy the grader still scores zero when the
//!   turn itself failed (`turn_result.ok == false`).
//!
//! Guard zeroing is measurable: the report carries per-guard
//! trigger-coverage (a guard that never fires is unmeasured noise) and
//! leave-one-out ablation deltas (see [`crate::metrics`]).

use crate::dataset::EvalCase;
use crate::wire::TurnStream;
use serde::{Deserialize, Serialize};

/// Why a guard zeroed (or annotated) a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityFlag {
    /// Output contained the case canary (dataset leakage).
    CanaryLeak,
    /// A tool call opened an upstream PR/diff (fair-internet-use).
    FairInternetUse,
    /// The turn itself failed (`turn_result.ok == false`).
    TurnFailed,
}

impl IntegrityFlag {
    /// Stable snake_case name (report keys).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::CanaryLeak => "canary_leak",
            Self::FairInternetUse => "fair_internet_use",
            Self::TurnFailed => "turn_failed",
        }
    }
}

/// URL/path fragments whose presence in a tool call means the run
/// opened an upstream PR or diff (the zeroing rule; doc-constant).
pub const FAIR_INTERNET_USE_PATTERNS: [&str; 3] =
    ["/pull/", "/compare/", "patch-diff.githubusercontent.com"];

/// One integrity guard.
pub trait IntegrityGuard: Send + Sync {
    /// Stable guard id (report keys, ablation grouping).
    fn id(&self) -> &'static str;
    /// The flag when this guard fires on the (case, stream) pair.
    fn check(&self, case: &EvalCase, stream: &TurnStream) -> Option<IntegrityFlag>;
}

/// Canary-leak guard (dataset leakage).
#[derive(Debug, Clone, Copy, Default)]
pub struct CanaryGuard;

impl IntegrityGuard for CanaryGuard {
    fn id(&self) -> &'static str {
        "canary"
    }
    fn check(&self, case: &EvalCase, stream: &TurnStream) -> Option<IntegrityFlag> {
        let leaked = case
            .canary
            .as_ref()
            .is_some_and(|canary| stream.assistant_text.contains(canary.as_str()));
        leaked.then_some(IntegrityFlag::CanaryLeak)
    }
}

/// Fair-internet-use guard (upstream PR/diff zeroing).
#[derive(Debug, Clone, Copy, Default)]
pub struct FairInternetUseGuard;

/// True when any tool-call arguments text opens an upstream PR/diff.
#[must_use]
pub fn fair_internet_use_hit(arguments_json: &str) -> bool {
    FAIR_INTERNET_USE_PATTERNS
        .iter()
        .any(|pattern| arguments_json.contains(pattern))
}

impl IntegrityGuard for FairInternetUseGuard {
    fn id(&self) -> &'static str {
        "fair_internet_use"
    }
    fn check(&self, _case: &EvalCase, stream: &TurnStream) -> Option<IntegrityFlag> {
        stream
            .tool_calls
            .iter()
            .any(|call| fair_internet_use_hit(&call.arguments_json))
            .then_some(IntegrityFlag::FairInternetUse)
    }
}

/// Turn-outcome guard (partial-output inflation).
#[derive(Debug, Clone, Copy, Default)]
pub struct TurnOutcomeGuard;

impl IntegrityGuard for TurnOutcomeGuard {
    fn id(&self) -> &'static str {
        "turn_outcome"
    }
    fn check(&self, _case: &EvalCase, stream: &TurnStream) -> Option<IntegrityFlag> {
        (!stream.outcome_ok).then_some(IntegrityFlag::TurnFailed)
    }
}

/// One guard firing, with its id for coverage/ablation grouping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardHit {
    /// Guard id.
    pub guard_id: String,
    /// The flag raised.
    pub flag: IntegrityFlag,
}

/// A score after the guard set ran: the RAW (grader) score is kept for
/// leave-one-out ablation; the reported score is zero when any guard
/// fired.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardedScore {
    /// Raw grader score (bounded `[0,1]`), pre-zeroing.
    pub raw_score: f64,
    /// Reported score (zero when any guard fired).
    pub score: f64,
    /// Every guard that fired.
    pub hits: Vec<GuardHit>,
}

/// The standard guard set (all three, deterministic order).
#[derive(Default)]
pub struct GuardSet {
    guards: Vec<Box<dyn IntegrityGuard>>,
}

impl GuardSet {
    /// The standard set: canary, fair-internet-use, turn-outcome.
    #[must_use]
    pub fn standard() -> Self {
        Self {
            guards: vec![
                Box::<CanaryGuard>::default(),
                Box::<FairInternetUseGuard>::default(),
                Box::<TurnOutcomeGuard>::default(),
            ],
        }
    }

    /// Guard ids in set order.
    #[must_use]
    pub fn ids(&self) -> Vec<&'static str> {
        self.guards.iter().map(|g| g.id()).collect()
    }

    /// Runs every guard over the (case, stream) pair and produces the
    /// guarded score. Any hit zeroes the score.
    #[must_use]
    pub fn apply(&self, case: &EvalCase, stream: &TurnStream, raw_score: f64) -> GuardedScore {
        let hits: Vec<GuardHit> = self
            .guards
            .iter()
            .filter_map(|guard| {
                guard.check(case, stream).map(|flag| GuardHit {
                    guard_id: guard.id().to_owned(),
                    flag,
                })
            })
            .collect();
        let raw = crate::grader::clamp_score(raw_score);
        let score = if hits.is_empty() { raw } else { 0.0 };
        GuardedScore {
            raw_score: raw,
            score,
            hits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::EvalCase;
    use crate::grader::GraderSpec;

    fn case(canary: Option<&str>) -> EvalCase {
        EvalCase {
            id: "c".to_owned(),
            prompt: "p".to_owned(),
            grader: GraderSpec::AllOf {
                expected: vec!["ok".to_owned()],
            },
            threshold: 1.0,
            canary: canary.map(str::to_owned),
            tags: vec![],
        }
    }

    fn stream(text: &str, tool_args: &[&str], outcome_ok: bool) -> TurnStream {
        TurnStream {
            assistant_text: text.to_owned(),
            tool_calls: tool_args
                .iter()
                .map(|args| crate::wire::ToolCallObserved {
                    call_id: "c1".to_owned(),
                    tool: "read_url".to_owned(),
                    arguments_json: (*args).to_owned(),
                })
                .collect(),
            outcome: if outcome_ok {
                "task_completed".to_owned()
            } else {
                "failed::X".to_owned()
            },
            outcome_ok,
        }
    }

    /// PINNED: the canary guard fires on leakage and zeroes.
    #[test]
    fn canary_guard_fires_on_leak_and_stays_silent_without_canary() {
        let guard = CanaryGuard;
        let stream = stream("answer ok canary-xyz", &[], true);
        assert_eq!(
            guard.check(&case(Some("canary-xyz")), &stream),
            Some(IntegrityFlag::CanaryLeak)
        );
        assert_eq!(guard.check(&case(None), &stream), None);
        assert_eq!(guard.check(&case(Some("other")), &stream), None);
    }

    /// PINNED: fair-internet-use zeroes upstream PR/diff opens.
    #[test]
    fn fair_internet_use_guard_fires_on_upstream_pr_or_diff() {
        let guard = FairInternetUseGuard;
        let pr = stream("ok", &[r#"{"url":"https://github.com/o/r/pull/41"}"#], true);
        let diff = stream("ok", &["patch-diff.githubusercontent.com/o/r/raw/x"], true);
        let benign = stream(
            "ok",
            &[r#"{"url":"https://github.com/o/r/blob/main/x"}"#],
            true,
        );
        assert_eq!(
            guard.check(&case(None), &pr),
            Some(IntegrityFlag::FairInternetUse)
        );
        assert_eq!(
            guard.check(&case(None), &diff),
            Some(IntegrityFlag::FairInternetUse)
        );
        assert_eq!(guard.check(&case(None), &benign), None);
    }

    /// PINNED: turn-outcome guard zeroes grader-satisfying text on a
    /// failed turn (the partial-output inflation it prevents).
    #[test]
    fn turn_outcome_guard_zeroes_grader_satisfying_text_on_failed_turns() {
        let guards = GuardSet::standard();
        let failed = stream("ok", &[], false);
        let guarded = guards.apply(&case(None), &failed, 1.0);
        assert_eq!(guarded.score, 0.0);
        assert_eq!(guarded.raw_score, 1.0);
        assert_eq!(guarded.hits.len(), 1);
        assert_eq!(guarded.hits[0].flag, IntegrityFlag::TurnFailed);
        let ok = guards.apply(&case(None), &stream("ok", &[], true), 1.0);
        assert_eq!(ok.score, 1.0);
        assert!(ok.hits.is_empty());
    }
}
