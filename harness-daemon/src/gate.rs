//! Completion gate for unattended loops: an autonomous run ends only
//! when an independent reviewer accepts the evidence bundle against
//! the goal criteria. Verdicts use the CONFIRMED / REFUTED /
//! INCONCLUSIVE vocabulary and carry evidence links. The deterministic
//! [`CriteriaGate`] is the default reviewer; an adversarial (LLM)
//! reviewer composes later through the same [`CompletionGate`] seam.

/// One piece of evidence and what it claims to establish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceLink {
    /// What the evidence is (journal path, session event range, …).
    pub label: String,
    /// Criteria this link claims to cover (verbatim criterion text).
    pub claims: Vec<String>,
}

/// What a reviewer examines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceBundle {
    /// The run's goal.
    pub goal: String,
    /// Criteria acceptance requires.
    pub criteria: Vec<String>,
    /// Collected evidence.
    pub links: Vec<EvidenceLink>,
}

/// Review verdict (Droid /verify vocabulary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    /// Evidence bundle accepted: the run may end.
    Confirmed {
        /// Criteria the evidence covered.
        covered: Vec<String>,
    },
    /// Evidence contradicts the goal: the run is rejected.
    Refuted {
        /// Why.
        reason: String,
        /// Criteria covered despite the refutation.
        covered: Vec<String>,
    },
    /// Evidence insufficient: neither accepted nor rejected.
    Inconclusive {
        /// Why.
        reason: String,
        /// Criteria covered so far.
        covered: Vec<String>,
    },
}

impl GateVerdict {
    /// True only for [`GateVerdict::Confirmed`].
    #[must_use]
    pub fn accepted(&self) -> bool {
        matches!(self, Self::Confirmed { .. })
    }

    /// Stable wire name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Confirmed { .. } => "confirmed",
            Self::Refuted { .. } => "refuted",
            Self::Inconclusive { .. } => "inconclusive",
        }
    }
}

/// Independent reviewer pass over an evidence bundle.
pub trait CompletionGate: Send + Sync {
    /// Review the bundle against its criteria.
    fn review(&self, bundle: &EvidenceBundle) -> GateVerdict;
}

/// Deterministic default reviewer: a criterion is covered when some
/// evidence link claims it verbatim. All covered = Confirmed; any gap
/// = Inconclusive (a coverage check cannot derive contradiction —
/// Refuted is reserved for adversarial reviewers that affirmatively
/// refute). Semantic review composes through the [`CompletionGate`]
/// seam.
pub struct CriteriaGate;

impl CompletionGate for CriteriaGate {
    fn review(&self, bundle: &EvidenceBundle) -> GateVerdict {
        let covered: Vec<String> = bundle
            .criteria
            .iter()
            .filter(|criterion| {
                bundle
                    .links
                    .iter()
                    .any(|link| link.claims.iter().any(|claim| claim == *criterion))
            })
            .cloned()
            .collect();
        if bundle.criteria.is_empty() || covered.len() == bundle.criteria.len() {
            GateVerdict::Confirmed { covered }
        } else {
            GateVerdict::Inconclusive {
                reason: format!(
                    "{}/{} criteria covered; needs independent review",
                    covered.len(),
                    bundle.criteria.len()
                ),
                covered,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(label: &str, claims: &[&str]) -> EvidenceLink {
        EvidenceLink {
            label: label.into(),
            claims: claims.iter().map(|c| (*c).to_owned()).collect(),
        }
    }

    fn bundle(criteria: &[&str], links: Vec<EvidenceLink>) -> EvidenceBundle {
        EvidenceBundle {
            goal: "nightly review".into(),
            criteria: criteria.iter().map(|c| (*c).to_owned()).collect(),
            links,
        }
    }

    #[test]
    fn verdict_vocabulary_is_stable() {
        let confirmed = GateVerdict::Confirmed {
            covered: vec!["c".into()],
        };
        let refuted = GateVerdict::Refuted {
            reason: "none".into(),
            covered: vec![],
        };
        let inconclusive = GateVerdict::Inconclusive {
            reason: "partial".into(),
            covered: vec!["c".into()],
        };
        assert_eq!(confirmed.as_str(), "confirmed");
        assert_eq!(refuted.as_str(), "refuted");
        assert_eq!(inconclusive.as_str(), "inconclusive");
        assert!(confirmed.accepted());
        assert!(!refuted.accepted());
        assert!(!inconclusive.accepted());
    }

    #[test]
    fn criteria_gate_confirms_full_coverage() {
        let gate = CriteriaGate;
        let b = bundle(
            &["report written", "summary saved"],
            vec![
                link("journal:turn-1", &["report written"]),
                link("session:events", &["summary saved"]),
            ],
        );
        let verdict = gate.review(&b);
        assert!(verdict.accepted());
        let GateVerdict::Confirmed { covered } = verdict else {
            panic!("not confirmed");
        };
        assert_eq!(covered.len(), 2);
    }

    #[test]
    fn criteria_gate_marks_zero_coverage_inconclusive() {
        let gate = CriteriaGate;
        let b = bundle(
            &["report written"],
            vec![link("journal:turn-1", &["something else"])],
        );
        let GateVerdict::Inconclusive { covered, .. } = gate.review(&b) else {
            panic!("not inconclusive");
        };
        assert!(covered.is_empty());
    }

    #[test]
    fn criteria_gate_marks_partial_coverage_inconclusive() {
        let gate = CriteriaGate;
        let b = bundle(
            &["report written", "summary saved"],
            vec![link("journal:turn-1", &["report written"])],
        );
        let GateVerdict::Inconclusive { covered, .. } = gate.review(&b) else {
            panic!("not inconclusive");
        };
        assert_eq!(covered, vec!["report written".to_owned()]);
    }

    #[test]
    fn empty_criteria_confirm_without_evidence() {
        let gate = CriteriaGate;
        let b = bundle(&[], vec![]);
        assert!(gate.review(&b).accepted());
    }
}
