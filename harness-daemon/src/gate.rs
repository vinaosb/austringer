//! Completion gate for unattended loops: an autonomous run ends only
//! when an independent reviewer accepts the evidence bundle against
//! the goal criteria. Verdicts use the CONFIRMED / REFUTED /
//! INCONCLUSIVE vocabulary and carry evidence links. The deterministic
//! [`CriteriaGate`] is the default reviewer; an adversarial (LLM)
//! reviewer composes later through the same [`CompletionGate`] seam.
//!
//! Todo 18 binds test evidence: a criterion of the form
//! `tests: <suite> green` (also `tests: suite <suite> green`) is
//! covered ONLY by a structured [`TestRunEvidence`] run of that suite
//! — never by a prose claim — a red run REFUTES citing the failing
//! test ids, and a missing run leaves the criterion uncovered
//! (INCONCLUSIVE, never CONFIRMED without bound evidence).

use crate::tools::run_tests::TestReport;
use harness_core::event::ContentBlock;

/// One piece of evidence and what it claims to establish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceLink {
    /// What the evidence is (journal path, session event range, …).
    pub label: String,
    /// Criteria this link claims to cover (verbatim criterion text).
    pub claims: Vec<String>,
}

/// One structured test-run evidence (todo 18): a `run_tests`
/// TestReport observed at a recorded session event. `suite` is the
/// report's framework (`cargo-nextest` / `cargo-libtest` /
/// `override`) — the identifier a `tests:` criterion binds.
#[derive(Debug, Clone, PartialEq)]
pub struct TestRunEvidence {
    /// The session event id whose recorded `run_tests` ToolResult
    /// carried the report (what a verdict cites).
    pub event_id: u64,
    /// The suite the run belongs to (the report's framework).
    pub suite: String,
    /// The parsed task-17 report.
    pub report: TestReport,
}

impl TestRunEvidence {
    /// Parse the evidence from a `run_tests` tool observation: the
    /// FIRST line of the first text block is the TestReport JSON
    /// (line 2 is the `[safety]` suffix). `None` = the blocks are not
    /// a parseable observation.
    #[must_use]
    pub fn from_observation(event_id: u64, blocks: &[ContentBlock]) -> Option<Self> {
        let text = blocks.iter().find_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })?;
        let first_line = text.lines().next()?;
        let report: TestReport = serde_json::from_str(first_line).ok()?;
        Some(Self {
            event_id,
            suite: report.framework.clone(),
            report,
        })
    }
}

/// What a reviewer examines.
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceBundle {
    /// The run's goal.
    pub goal: String,
    /// Criteria acceptance requires.
    pub criteria: Vec<String>,
    /// Collected evidence.
    pub links: Vec<EvidenceLink>,
    /// Structured test-run evidence (todo 18). Only these bind
    /// `tests:` criteria.
    pub test_runs: Vec<TestRunEvidence>,
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

/// Deterministic default reviewer. A plain criterion is covered when
/// some evidence link claims it verbatim. A `tests:` criterion is
/// covered only by a bound structured run (see the module docs): the
/// LATEST run of the named suite decides — green covers, red REFUTES
/// citing the failing test ids, absent leaves it uncovered. All
/// covered = Confirmed; any gap = Inconclusive; any red bound run =
/// Refuted. Semantic review composes through the [`CompletionGate`]
/// seam.
pub struct CriteriaGate;

impl CompletionGate for CriteriaGate {
    fn review(&self, bundle: &EvidenceBundle) -> GateVerdict {
        let mut covered: Vec<String> = Vec::new();
        let mut refutations: Vec<String> = Vec::new();
        for criterion in &bundle.criteria {
            match parse_test_criterion(criterion) {
                Some(suite) => {
                    match bundle.test_runs.iter().rev().find(|run| run.suite == suite) {
                        Some(run) if run.report.summary.failed == 0 => {
                            covered.push(criterion.clone());
                        }
                        Some(run) => refutations.push(refute_run(suite, &run.report)),
                        None => {}
                    }
                }
                None => {
                    let claimed = bundle
                        .links
                        .iter()
                        .any(|link| link.claims.iter().any(|claim| claim == criterion));
                    if claimed {
                        covered.push(criterion.clone());
                    }
                }
            }
        }
        if !refutations.is_empty() {
            return GateVerdict::Refuted {
                reason: refutations.join("; "),
                covered,
            };
        }
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

/// The suite a `tests:` criterion binds: text of the form
/// `tests: <suite> green` or `tests: suite <suite> green`. `None` =
/// not a test criterion (plain verbatim-claim coverage applies).
fn parse_test_criterion(criterion: &str) -> Option<&str> {
    let rest = criterion.strip_prefix("tests:")?.strip_suffix("green")?;
    let trimmed = rest.trim();
    let suite = trimmed.strip_prefix("suite ").unwrap_or(trimmed).trim();
    (!suite.is_empty()).then_some(suite)
}

/// A refutation MUST cite the failing test ids when the run parsed
/// them; a summary count without entries (possible only for
/// hand-authored reports) still names the count.
fn refute_run(suite: &str, report: &TestReport) -> String {
    let failing: Vec<&str> = report
        .tests
        .iter()
        .filter(|entry| entry.status == "failed")
        .map(|entry| entry.name.as_str())
        .collect();
    if failing.is_empty() {
        format!(
            "tests: {suite} green — {} failing (ids unlisted; see report)",
            report.summary.failed
        )
    } else {
        format!(
            "tests: {suite} green — {} failing test(s): {}",
            failing.len(),
            failing.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::run_tests::{TestEntry, TestReport, TestSummary};

    fn link(label: &str, claims: &[&str]) -> EvidenceLink {
        EvidenceLink {
            label: label.into(),
            claims: claims.iter().map(|c| (*c).to_owned()).collect(),
        }
    }

    fn bundle(criteria: &[&str], links: Vec<EvidenceLink>) -> EvidenceBundle {
        bundle_with_runs(criteria, links, Vec::new())
    }

    fn bundle_with_runs(
        criteria: &[&str],
        links: Vec<EvidenceLink>,
        test_runs: Vec<TestRunEvidence>,
    ) -> EvidenceBundle {
        EvidenceBundle {
            goal: "nightly review".into(),
            criteria: criteria.iter().map(|c| (*c).to_owned()).collect(),
            links,
            test_runs,
        }
    }

    fn suite_run(
        event_id: u64,
        suite: &str,
        failing: &[&str],
        passing: &[&str],
    ) -> TestRunEvidence {
        let tests: Vec<TestEntry> = passing
            .iter()
            .map(|name| TestEntry {
                name: (*name).into(),
                status: "passed".into(),
                duration_ms: Some(2),
            })
            .chain(failing.iter().map(|name| TestEntry {
                name: (*name).into(),
                status: "failed".into(),
                duration_ms: Some(5),
            }))
            .collect();
        let summary = TestSummary::of(&tests);
        TestRunEvidence {
            event_id,
            suite: suite.into(),
            report: TestReport {
                framework: suite.into(),
                command: format!("cargo nextest run -p {suite}"),
                exit_code: Some(if failing.is_empty() { 0 } else { 100 }),
                timed_out: false,
                suite_duration_ms: 42,
                tests,
                summary,
            },
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

    // --- todo 18: tests: criteria bindings over TestReports ----------

    #[test]
    fn bound_green_suite_confirms_with_structured_run() {
        let gate = CriteriaGate;
        let b = bundle_with_runs(
            &["tests: cargo-nextest green"],
            vec![],
            vec![suite_run(
                4,
                "cargo-nextest",
                &[],
                &["rtfx_a passes", "rtfx_b passes"],
            )],
        );
        let GateVerdict::Confirmed { covered } = gate.review(&b) else {
            panic!("green bound suite must confirm");
        };
        assert_eq!(covered, vec!["tests: cargo-nextest green".to_owned()]);
    }

    #[test]
    fn bound_red_suite_refutes_citing_failing_test_ids() {
        let gate = CriteriaGate;
        let b = bundle_with_runs(
            &["tests: cargo-nextest green"],
            vec![],
            vec![suite_run(
                3,
                "cargo-nextest",
                &["rtfx_a fails", "rtfx_b fails"],
                &["rtfx_c passes"],
            )],
        );
        let GateVerdict::Refuted { reason, .. } = gate.review(&b) else {
            panic!("red bound suite must refute");
        };
        assert!(reason.contains("rtfx_a fails"), "reason: {reason}");
        assert!(reason.contains("rtfx_b fails"), "reason: {reason}");
    }

    #[test]
    fn missing_test_run_is_inconclusive_despite_verbatim_claim() {
        // The binding's hard rule: a tests: criterion is covered ONLY
        // by a structured run — a link claiming the criterion text
        // verbatim (a prose "done" claim) must never confirm it.
        let gate = CriteriaGate;
        let b = bundle_with_runs(
            &["tests: cargo-nextest green"],
            vec![link("session:events", &["tests: cargo-nextest green"])],
            vec![],
        );
        assert!(matches!(
            gate.review(&b),
            GateVerdict::Inconclusive { .. }
        ));
    }

    #[test]
    fn latest_bound_run_decides_the_suite_state() {
        let gate = CriteriaGate;
        let recovered = bundle_with_runs(
            &["tests: cargo-nextest green"],
            vec![],
            vec![
                suite_run(2, "cargo-nextest", &["rtfx_a fails"], &[]),
                suite_run(5, "cargo-nextest", &[], &["rtfx_a passes"]),
            ],
        );
        assert!(gate.review(&recovered).accepted());
        let regressed = bundle_with_runs(
            &["tests: cargo-nextest green"],
            vec![],
            vec![
                suite_run(2, "cargo-nextest", &[], &["rtfx_a passes"]),
                suite_run(5, "cargo-nextest", &["rtfx_a fails"], &[]),
            ],
        );
        assert!(matches!(
            gate.review(&regressed),
            GateVerdict::Refuted { .. }
        ));
    }

    #[test]
    fn suite_binding_must_match_the_run_suite() {
        let gate = CriteriaGate;
        let b = bundle_with_runs(
            &["tests: cargo-libtest green"],
            vec![],
            vec![suite_run(4, "cargo-nextest", &[], &["rtfx_a passes"])],
        );
        assert!(matches!(
            gate.review(&b),
            GateVerdict::Inconclusive { .. }
        ));
    }

    #[test]
    fn suite_keyword_form_binds_the_named_suite() {
        let gate = CriteriaGate;
        let b = bundle_with_runs(
            &["tests: suite cargo-libtest green"],
            vec![],
            vec![suite_run(7, "cargo-libtest", &[], &["a passes"])],
        );
        assert!(gate.review(&b).accepted());
    }

    #[test]
    fn mixed_test_and_plain_criteria_confirm_together() {
        let gate = CriteriaGate;
        let b = bundle_with_runs(
            &["tests: cargo-nextest green", "report written"],
            vec![link("journal:turn-1", &["report written"])],
            vec![suite_run(4, "cargo-nextest", &[], &["rtfx_a passes"])],
        );
        let GateVerdict::Confirmed { covered } = gate.review(&b) else {
            panic!("mixed coverage must confirm");
        };
        assert_eq!(covered.len(), 2);
    }

    #[test]
    fn test_run_evidence_parses_only_the_first_observation_line() {
        use harness_core::event::ContentBlock;
        let report = TestReport {
            framework: "cargo-nextest".into(),
            command: "cargo nextest run".into(),
            exit_code: Some(0),
            timed_out: false,
            suite_duration_ms: 9,
            tests: vec![TestEntry {
                name: "rtfx_a passes".into(),
                status: "passed".into(),
                duration_ms: Some(1),
            }],
            summary: TestSummary {
                passed: 1,
                failed: 0,
                ignored: 0,
            },
        };
        let json = serde_json::to_string(&report).expect("serialize");
        let blocks = vec![ContentBlock::Text {
            text: format!(
                "{json}\n[safety] backend=windows-appcontainer requested=online enforced=offline"
            ),
        }];
        let parsed = TestRunEvidence::from_observation(11, &blocks).expect("parses");
        assert_eq!(parsed.event_id, 11);
        assert_eq!(parsed.suite, "cargo-nextest");
        assert_eq!(parsed.report.summary.failed, 0);
        let not_json = vec![ContentBlock::Text {
            text: "plain prose\n[safety] x".into(),
        }];
        assert!(TestRunEvidence::from_observation(12, &not_json).is_none());
    }
}
