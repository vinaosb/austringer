//! Committed regression baselines. Only machine-independent deterministic
//! metrics belong here (allocation counts, render counts, byte sizes) —
//! wall-clock numbers are noise-bound and stay in absolute budgets
//! (noise-margin discipline, todo 43).

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const BASELINE_SCHEMA: u32 = 1;

#[derive(Debug, Error)]
pub enum BaselineError {
    #[error("malformed baseline: {0}")]
    Malformed(#[source] serde_json::Error),
    #[error(
        "baseline schema mismatch: found {found}, this gate speaks {expected} (re-record the baseline)"
    )]
    Schema { found: u32, expected: u32 },
    #[error(
        "stale baseline (scenario set changed): missing {missing:?}, unknown {unknown:?} — re-record with --record"
    )]
    Stale {
        missing: Vec<String>,
        unknown: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    pub schema: u32,
    /// Recording platform provenance (e.g. "windows"); informational only.
    pub recorded_on: Option<String>,
    /// "scenario.metric" -> deterministic measured value.
    pub metrics: BTreeMap<String, f64>,
}

impl Baseline {
    pub fn from_json_str(json: &str) -> Result<Self, BaselineError> {
        let baseline: Self = serde_json::from_str(json).map_err(BaselineError::Malformed)?;
        if baseline.schema != BASELINE_SCHEMA {
            return Err(BaselineError::Schema {
                found: baseline.schema,
                expected: BASELINE_SCHEMA,
            });
        }
        Ok(baseline)
    }

    pub fn to_json_pretty(&self) -> Result<String, BaselineError> {
        serde_json::to_string_pretty(self).map_err(BaselineError::Malformed)
    }

    pub fn get(&self, metric: &str) -> Option<f64> {
        self.metrics.get(metric).copied()
    }

    /// The metric set the runner produces must match the baseline exactly;
    /// drift in either direction is stale (forces a deliberate re-record,
    /// never a silent partial comparison).
    pub fn check_freshness(&self, expected: &BTreeSet<String>) -> Result<(), BaselineError> {
        let recorded: BTreeSet<&String> = self.metrics.keys().collect();
        let expected_ref: BTreeSet<&String> = expected.iter().collect();
        let missing: Vec<String> = expected_ref
            .difference(&recorded)
            .map(|s| (*s).clone())
            .collect();
        let unknown: Vec<String> = recorded
            .difference(&expected_ref)
            .map(|s| (*s).clone())
            .collect();
        if missing.is_empty() && unknown.is_empty() {
            Ok(())
        } else {
            Err(BaselineError::Stale { missing, unknown })
        }
    }

    /// Freshness for runs with allowed skips: every produced metric must
    /// exist in the baseline, and baseline keys must belong to the
    /// registry's known names (entries for skipped scenarios may remain,
    /// alien names may not).
    pub fn check_scope(
        &self,
        registry_names: &BTreeSet<String>,
        produced: &BTreeSet<String>,
    ) -> Result<(), BaselineError> {
        let missing: Vec<String> = produced
            .iter()
            .filter(|m| !self.metrics.contains_key(*m))
            .cloned()
            .collect();
        let unknown: Vec<String> = self
            .metrics
            .keys()
            .filter(|k| !registry_names.contains(*k))
            .cloned()
            .collect();
        if missing.is_empty() && unknown.is_empty() {
            Ok(())
        } else {
            Err(BaselineError::Stale { missing, unknown })
        }
    }
}
