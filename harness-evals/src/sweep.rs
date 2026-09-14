//! Sweep configuration and execution. Two policies are load-bearing:
//!
//! - **Live-model sweeps are SCHEDULED ONLY — never PR gates.** The
//!   [`Trigger`] enum has exactly `Scheduled` and `Manual` variants;
//!   no PR-gate trigger exists to construct, and the pinned test below
//!   keeps that true (an exhaustive match over the enum).
//! - **Joint model×profile evaluation** (research §33): regression
//!   runs pin ONE locked profile across models; harness experiments
//!   require a ≥2×2 model×profile grid so harness variance is
//!   measurable, never assumed away.

use std::collections::BTreeMap;

use crate::dataset::EvalDataset;
use crate::guard::GuardSet;
use crate::report::{HarnessCard, PairReport, SweepReport};
use crate::runner::{Subject, SweepPair, run_pair};

/// What initiated the sweep. There is deliberately NO PR variant
/// (plan todo 44: "Must NOT gate PRs on live-model results").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// Daemon cron (todo 41) — the sanctioned live-model path.
    Scheduled,
    /// An explicit human invocation.
    Manual,
}

/// Sweep mode.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SweepMode {
    /// Regression run: every pair MUST share the single locked profile
    /// (comparability with the baseline).
    Regression {
        /// The one profile every pair runs.
        locked_profile: String,
    },
    /// Harness experiment: requires a ≥2×2 model×profile grid.
    Experiment,
}

/// A validated sweep request.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepConfig {
    /// Trigger (scheduled or manual — never a PR gate).
    pub trigger: Trigger,
    /// Mode + grid policy.
    pub mode: SweepMode,
    /// The model×profile pairs.
    pub pairs: Vec<SweepPair>,
    /// Repetitions per case (k in Pass^k; ≥ 1).
    pub reps: u32,
}

/// Everything sweep validation can fail with.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SweepConfigError {
    /// No pairs declared.
    #[error("sweep has no pairs")]
    NoPairs,
    /// Repetition count of zero.
    #[error("sweep needs at least one repetition per case")]
    NoReps,
    /// A regression run with pairs off the locked profile.
    #[error("regression run: pair {pair} is off the locked profile {locked:?}")]
    RegressionProfileDrift {
        /// The offending pair label.
        pair: String,
        /// The locked profile.
        locked: String,
    },
    /// An experiment grid smaller than 2×2.
    #[error("experiment grid too small: {models} models x {profiles} profiles — need >= 2x2")]
    ExperimentGridTooSmall {
        /// Distinct model count.
        models: usize,
        /// Distinct profile count.
        profiles: usize,
    },
    /// Duplicate pairs.
    #[error("duplicate sweep pair {pair:?}")]
    DuplicatePair {
        /// The duplicated pair label.
        pair: String,
    },
    /// No subject registered for a pair's model.
    #[error("no subject registered for model {model:?}")]
    MissingSubject {
        /// The model with no registered subject.
        model: String,
    },
}

/// Validates the sweep config against the grid policies.
///
/// # Errors
/// typed [`SweepConfigError`].
pub fn validate(config: &SweepConfig) -> Result<(), SweepConfigError> {
    if config.pairs.is_empty() {
        return Err(SweepConfigError::NoPairs);
    }
    if config.reps == 0 {
        return Err(SweepConfigError::NoReps);
    }
    let mut seen = std::collections::BTreeSet::new();
    for pair in &config.pairs {
        if !seen.insert(pair.label()) {
            return Err(SweepConfigError::DuplicatePair { pair: pair.label() });
        }
    }
    match &config.mode {
        SweepMode::Regression { locked_profile } => {
            for pair in &config.pairs {
                if &pair.profile != locked_profile {
                    return Err(SweepConfigError::RegressionProfileDrift {
                        pair: pair.label(),
                        locked: locked_profile.clone(),
                    });
                }
            }
        }
        SweepMode::Experiment => {
            let models: std::collections::BTreeSet<&String> =
                config.pairs.iter().map(|pair| &pair.model).collect();
            let profiles: std::collections::BTreeSet<&String> =
                config.pairs.iter().map(|pair| &pair.profile).collect();
            if models.len() < 2 || profiles.len() < 2 {
                return Err(SweepConfigError::ExperimentGridTooSmall {
                    models: models.len(),
                    profiles: profiles.len(),
                });
            }
        }
    }
    Ok(())
}

/// A subject factory per pair (the sweep borrows one subject per pair).
pub type Subjects<'a> = BTreeMap<String, &'a mut dyn Subject>;

/// One harness card per pair (transparency disclosure), keyed by pair
/// label.
pub type Cards = BTreeMap<String, HarnessCard>;

/// Executes a validated sweep: runs every pair over the dataset and
/// assembles the report (ranking, ablation, coverage, pins).
///
/// # Errors
/// re-validation failures (the config is validated before running).
pub fn execute(
    config: &SweepConfig,
    dataset: &EvalDataset,
    subjects: &mut Subjects<'_>,
    cards: &Cards,
    guards: &GuardSet,
) -> Result<SweepReport, SweepConfigError> {
    validate(config)?;
    let mut pairs = Vec::new();
    for pair in &config.pairs {
        let subject =
            subjects
                .get_mut(&pair.model)
                .ok_or_else(|| SweepConfigError::MissingSubject {
                    model: pair.model.clone(),
                })?;
        let run = run_pair(pair, dataset, config.reps, *subject, guards);
        let card = cards.get(&pair.label()).cloned().unwrap_or_default();
        pairs.push(PairReport::from_run(run, card, guards));
    }
    Ok(SweepReport::new(config.trigger, pairs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(model: &str, profile: &str) -> SweepPair {
        SweepPair {
            model: model.to_owned(),
            profile: profile.to_owned(),
        }
    }

    fn config(mode: SweepMode, pairs: Vec<SweepPair>) -> SweepConfig {
        SweepConfig {
            trigger: Trigger::Scheduled,
            mode,
            pairs,
            reps: 3,
        }
    }

    /// PINNED: the trigger vocabulary has no PR-gate variant. Adding
    /// one must fail this match (and be rejected in review).
    #[test]
    fn trigger_vocabulary_contains_no_pr_gate_variant() {
        for trigger in [Trigger::Scheduled, Trigger::Manual] {
            let name = serde_json::to_string(&trigger).unwrap_or_default();
            assert!(!name.to_lowercase().contains("pr"), "{name}");
            match trigger {
                Trigger::Scheduled | Trigger::Manual => {}
            }
        }
    }

    #[test]
    fn regression_requires_one_locked_profile() {
        let ok = config(
            SweepMode::Regression {
                locked_profile: "baseline".to_owned(),
            },
            vec![pair("m1", "baseline"), pair("m2", "baseline")],
        );
        assert!(validate(&ok).is_ok());
        let drifted = config(
            SweepMode::Regression {
                locked_profile: "baseline".to_owned(),
            },
            vec![pair("m1", "baseline"), pair("m2", "turbo")],
        );
        assert!(matches!(
            validate(&drifted),
            Err(SweepConfigError::RegressionProfileDrift { .. })
        ));
    }

    #[test]
    fn experiments_need_at_least_a_2x2_grid() {
        let small = config(
            SweepMode::Experiment,
            vec![pair("m1", "p1"), pair("m1", "p2")],
        );
        assert!(matches!(
            validate(&small),
            Err(SweepConfigError::ExperimentGridTooSmall { .. })
        ));
        let grid = config(
            SweepMode::Experiment,
            vec![
                pair("m1", "p1"),
                pair("m1", "p2"),
                pair("m2", "p1"),
                pair("m2", "p2"),
            ],
        );
        assert!(validate(&grid).is_ok());
    }

    #[test]
    fn empty_pairs_zero_reps_and_duplicates_fail_closed() {
        let empty = config(SweepMode::Experiment, vec![]);
        assert!(matches!(validate(&empty), Err(SweepConfigError::NoPairs)));
        let mut zero = config(SweepMode::Experiment, vec![pair("m", "p")]);
        zero.reps = 0;
        assert!(matches!(validate(&zero), Err(SweepConfigError::NoReps)));
        let dup = config(SweepMode::Experiment, vec![pair("m", "p"), pair("m", "p")]);
        assert!(matches!(
            validate(&dup),
            Err(SweepConfigError::DuplicatePair { .. })
        ));
    }
}
