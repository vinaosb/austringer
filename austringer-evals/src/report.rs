//! Report artifacts + the transparency discipline (Qwen README,
//! research §18): every published row carries an exhaustively
//! disclosed Austringer card — model, profile, settings, source
//! fingerprint (commit + DIRTY WORKTREE STATE, so a report built from
//! a dirty tree is distinguishable from a clean one), and a
//! austringer-matched baseline reference. A pair WITHOUT a matched
//! baseline row is rendered `unmatched — DO NOT PUBLISH`: vendor
//! austringer numbers are never comparable to official rows (the
//! +10–25pt inflation lesson).

use std::collections::BTreeMap;
use std::path::Path;

use crate::guard::GuardSet;
use crate::metrics::{PairMetrics, guard_ablation, pair_metrics, trigger_coverage};
use crate::pinning::{ExpectedFailures, PinAudit, audit_pins};
use crate::runner::{PairRun, RankedPair};
use crate::sweep::Trigger;

/// Source state of the austringer build under evaluation.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct SourceFingerprint {
    /// Commit sha when known.
    pub commit: Option<String>,
    /// Whether the worktree had uncommitted changes.
    pub dirty: bool,
    /// The dirty paths (empty when clean).
    pub dirty_files: Vec<String>,
}

/// Exhaustive austringer disclosure for one pair (the "Austringer card").
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct AustringerCard {
    /// Model under test.
    pub model: String,
    /// Profile (austringer configuration) under test.
    pub profile: String,
    /// Every disclosed setting (keyed, sorted).
    pub settings: BTreeMap<String, String>,
    /// Source fingerprint (dirty worktrees are distinguishable).
    pub source: SourceFingerprint,
    /// The austringer-matched baseline reference (`None` ⇒ unmatched).
    pub baseline_ref: Option<String>,
}

impl AustringerCard {
    /// True when the row may be published as comparable (matched
    /// baseline present, clean-or-documented source).
    #[must_use]
    pub fn publishable(&self) -> bool {
        self.baseline_ref.is_some()
    }

    /// The rendered disclosure line for tables.
    #[must_use]
    pub fn disclosure(&self) -> String {
        let base = match &self.baseline_ref {
            Some(b) => format!("baseline={b}"),
            None => "unmatched - DO NOT PUBLISH".to_owned(),
        };
        let source = if self.source.dirty {
            format!("dirty:{} files", self.source.dirty_files.len())
        } else {
            "clean".to_owned()
        };
        format!("{} source={source}", base)
    }
}

/// One pair's report slice: pair + card + metrics + guard evidence.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PairReport {
    /// The evaluated pair.
    pub pair: crate::runner::SweepPair,
    /// The Austringer card.
    pub card: AustringerCard,
    /// Aggregated metrics (Pass^k, Mean@k, tokens/solved, fingerprint).
    pub metrics: PairMetrics,
    /// Leave-one-out ablation per guard.
    pub ablation: Vec<crate::metrics::AblationDelta>,
    /// Trigger coverage per guard (unmeasured noise surfaced).
    pub coverage: Vec<crate::metrics::GuardCoverage>,
}

impl PairReport {
    /// Builds a pair report from a run + card + the guard set.
    #[must_use]
    pub fn from_run(run: PairRun, card: AustringerCard, guards: &GuardSet) -> Self {
        let metrics = pair_metrics(&run.cases);
        let ablation = guards
            .ids()
            .iter()
            .map(|id| guard_ablation(&run.cases, id))
            .collect();
        let coverage = trigger_coverage(&run.cases, &guards.ids());
        Self {
            pair: run.pair,
            card,
            metrics,
            ablation,
            coverage,
        }
    }

    /// The ranked row for this report.
    #[must_use]
    pub fn ranked(&self) -> RankedPair {
        RankedPair {
            pair: self.pair.clone(),
            pass_k: self.metrics.pass_k,
            mean: self.metrics.mean,
            tokens_solved: self.metrics.tokens_solved,
        }
    }
}

/// The full sweep report artifact.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SweepReport {
    /// The trigger that ran the sweep.
    pub trigger: Trigger,
    /// Per-pair reports (config order).
    pub pairs: Vec<PairReport>,
    /// The deterministic ranking.
    pub ranking: Vec<RankedPair>,
    /// Pin audit results (annotated known failures + surfaced
    /// unpinned regressions + stale pins).
    pub pins: PinAudit,
}

impl SweepReport {
    /// Assembles a report (empty pin audit — attach via
    /// [`SweepReport::apply_pins`]).
    #[must_use]
    pub fn new(trigger: Trigger, pairs: Vec<PairReport>) -> Self {
        let mut ranking: Vec<RankedPair> = pairs.iter().map(PairReport::ranked).collect();
        crate::runner::sort_ranked(&mut ranking);
        Self {
            trigger,
            pairs,
            ranking,
            pins: PinAudit::default(),
        }
    }

    /// Runs a full run-set through ranking + report assembly.
    #[must_use]
    pub fn from_runs(
        trigger: Trigger,
        runs: Vec<PairRun>,
        cards: &BTreeMap<String, AustringerCard>,
        guards: &GuardSet,
    ) -> Self {
        let pairs = runs
            .into_iter()
            .map(|run| {
                let card = cards.get(&run.pair.label()).cloned().unwrap_or_default();
                PairReport::from_run(run, card, guards)
            })
            .collect();
        Self::new(trigger, pairs)
    }

    /// Applies expected-failure pins (annotates known failures,
    /// surfaces unpinned regressions, lists stale pins).
    pub fn apply_pins(&mut self, pins: &ExpectedFailures) {
        self.pins = audit_pins(self, pins);
    }

    /// Serialized JSON artifact (sorted keys, stable).
    ///
    /// # Errors
    /// serialization failure.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    /// The human-readable ranking table (the nightly artifact view).
    #[must_use]
    pub fn render_table(&self) -> String {
        let mut out = format!("sweep trigger={}\n", trigger_name(self.trigger));
        out.push_str("model/profile\tpass^k\tmean@k\ttokens/solved\tdisclosure\n");
        for row in &self.ranking {
            let tokens = row
                .tokens_solved
                .map(|t| t.to_string())
                .unwrap_or_else(|| "-".to_owned());
            let disclosure = self
                .pairs
                .iter()
                .find(|pair| pair.pair == row.pair)
                .map(|pair| pair.card.disclosure())
                .unwrap_or_else(|| "no card".to_owned());
            out.push_str(&format!(
                "{}\t{:.4}\t{:.4}\t{}\t{}\n",
                row.pair.label(),
                row.pass_k,
                row.mean,
                tokens,
                disclosure
            ));
        }
        for pinned in &self.pins.pinned_confirmed {
            out.push_str(&format!(
                "pinned (annotated): {} — {}\n",
                pinned.case, pinned.reason
            ));
        }
        for regression in &self.pins.unpinned {
            out.push_str(&format!("UNPINNED REGRESSION: {}\n", regression.case));
        }
        out
    }

    /// Writes the JSON artifact to `path`.
    ///
    /// # Errors
    /// I/O failure (bounded detail).
    pub fn write_artifact(&self, path: &Path) -> Result<(), String> {
        let json = self.to_json()?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }
}

fn trigger_name(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Scheduled => "scheduled",
        Trigger::Manual => "manual",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(dirty: bool, baseline: Option<&str>) -> AustringerCard {
        AustringerCard {
            model: "m".to_owned(),
            profile: "p".to_owned(),
            settings: BTreeMap::new(),
            source: SourceFingerprint {
                commit: Some("abc".to_owned()),
                dirty,
                dirty_files: if dirty {
                    vec!["src/x.rs".to_owned()]
                } else {
                    vec![]
                },
            },
            baseline_ref: baseline.map(str::to_owned),
        }
    }

    /// PINNED: dirty and clean worktrees are distinguishable in the
    /// rendered report — a dirty-tree result can never masquerade as
    /// a clean one.
    #[test]
    fn dirty_worktrees_are_distinguishable_from_clean() {
        let clean = card(false, Some("official-v1"));
        let dirty = card(true, Some("official-v1"));
        assert_ne!(clean, dirty);
        assert!(clean.disclosure().contains("clean"));
        assert!(dirty.disclosure().contains("dirty:1 files"));
        assert_ne!(clean.disclosure(), dirty.disclosure());
    }

    /// PINNED (transparency discipline): unmatched rows carry the
    /// DO-NOT-PUBLISH marker.
    #[test]
    fn unmatched_baseline_rows_are_marked_do_not_publish() {
        let matched = card(false, Some("official-v1"));
        let unmatched = card(false, None);
        assert!(matched.publishable());
        assert!(!unmatched.publishable());
        assert!(unmatched.disclosure().contains("DO NOT PUBLISH"));
    }
}
