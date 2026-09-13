//! Eval-before-activate: a deterministic replay harness. The draft's
//! rendered SKILL.md is parsed by the REAL T30 parser and routed by the
//! REAL T30 router against a scripted replay of the originating session
//! type — no model calls, no flakiness. Activation gates on
//! [`EvalReport::is_pass`].

use crate::forge::document::{extract_body, parse_body_sections};
use crate::routing::{RouteQuery, route};
use crate::skill::parse_skill_document;

use std::path::Path;

/// One scripted replay step.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReplayCase {
    /// Prompt to route.
    pub prompt: String,
    /// Whether the skill must route on this prompt.
    pub should_route: bool,
    /// Optional `## section` that must exist in the body.
    pub expect_section: Option<String>,
}

/// One failed replay step.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayFailure {
    /// The prompt that failed.
    pub prompt: String,
    /// What routing was expected.
    pub expected_route: bool,
    /// What routing actually did.
    pub actual_route: bool,
    /// Why it failed.
    pub reason: String,
}

/// Replay outcome: passes, typed failures.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EvalReport {
    /// Steps that passed.
    pub passed: usize,
    /// Steps that failed (empty ⇒ pass).
    pub failed: Vec<ReplayFailure>,
}

impl EvalReport {
    /// True when no step failed.
    pub fn is_pass(&self) -> bool {
        self.failed.is_empty()
    }

    /// Human-readable report (used verbatim in the blocked-activation
    /// error and the QA evidence).
    pub fn render(&self) -> String {
        let mut out = format!(
            "replay eval: {} passed, {} failed\n",
            self.passed,
            self.failed.len()
        );
        for failure in &self.failed {
            out.push_str(&format!(
                "  FAIL prompt {:?} expected_route={} actual_route={} — {}\n",
                failure.prompt, failure.expected_route, failure.actual_route, failure.reason
            ));
        }
        out
    }
}

/// Replays a rendered draft document against scripted cases using the
/// real parser and the real router. A document that fails to parse
/// fails every case with the parse error as the reason.
pub fn replay_draft(document: &str, cases: &[ReplayCase]) -> EvalReport {
    let parsed = parse_skill_document(Path::new("replay"), "replay", document.as_bytes());
    let mut report = EvalReport::default();
    let meta = match parsed {
        Ok((meta, _)) => meta,
        Err(err) => {
            report.failed = cases
                .iter()
                .map(|case| ReplayFailure {
                    prompt: case.prompt.clone(),
                    expected_route: case.should_route,
                    actual_route: false,
                    reason: format!("draft document failed to parse: {err}"),
                })
                .collect();
            return report;
        }
    };
    let sections = parse_body_sections(extract_body(document));
    for case in cases {
        let decision = route(
            std::slice::from_ref(&meta),
            &RouteQuery {
                text: &case.prompt,
                work_path: None,
            },
        );
        let actual = decision.matched.iter().any(|m| m.name == meta.name);
        let mut failure = (!actual == case.should_route).then(|| ReplayFailure {
            prompt: case.prompt.clone(),
            expected_route: case.should_route,
            actual_route: actual,
            reason: if case.should_route {
                "originating-session prompt no longer routes after applying this skill".to_string()
            } else {
                "skill routes on a prompt it must stay silent for".to_string()
            },
        });
        if failure.is_none() && let Some(expected) = &case.expect_section {
            let missing = !sections
                .iter()
                .any(|s| &s.heading == expected && !s.body.is_empty());
            if missing {
                failure = Some(ReplayFailure {
                    prompt: case.prompt.clone(),
                    expected_route: case.should_route,
                    actual_route: actual,
                    reason: format!("expected section {expected:?} missing or empty"),
                });
            }
        }
        match failure {
            Some(f) => report.failed.push(f),
            None => report.passed += 1,
        }
    }
    report
}
