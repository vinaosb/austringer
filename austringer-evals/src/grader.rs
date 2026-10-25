//! Deterministic graders. A grader maps one untrusted model output to a
//! score in `[0, 1]` — binary and strict, never partial credit (partial
//! credit invites gaming; the guards in [`crate::guard`] own integrity,
//! the graders own correctness only). All substring matching is
//! `str::contains`, which is char-boundary correct for multibyte text.

use serde::{Deserialize, Serialize};

/// Serializable grader specification (dataset-embedded, validated at
/// dataset parse time).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraderSpec {
    /// Score 1.0 iff every expected substring is present.
    AllOf {
        /// Required substrings (non-empty).
        expected: Vec<String>,
    },
    /// Score 1.0 iff at least one expected substring is present.
    AnyOf {
        /// Acceptable substrings (non-empty).
        expected: Vec<String>,
    },
    /// Score 1.0 iff none of the forbidden substrings is present.
    NotContains {
        /// Forbidden substrings (non-empty).
        forbidden: Vec<String>,
    },
}

/// Everything grading can fail with. Typed, bounded, and always
/// NON-FATAL per case at the runner level (one bad grader never aborts
/// a sweep).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GraderError {
    /// The spec itself is invalid (empty expectation, ...).
    #[error("invalid grader spec: {detail}")]
    Invalid {
        /// Bounded detail.
        detail: String,
    },
    /// A grader implementation failed at scoring time (open trait:
    /// replay-from-traces re-scorers arrive from configuration, not
    /// from validated datasets).
    #[error("grader failed: {detail}")]
    Failed {
        /// Bounded detail.
        detail: String,
    },
}

/// One compiled grader.
pub trait Grader: Send + Sync {
    /// Scores one output in `[0, 1]`.
    ///
    /// # Errors
    /// typed [`GraderError`] (non-fatal per case).
    fn score(&self, output: &str) -> Result<f64, GraderError>;
    /// The spec kind name (`all_of`, ...).
    fn kind(&self) -> &'static str;
}

struct AllOf(Vec<String>);
struct AnyOf(Vec<String>);
struct NotContains(Vec<String>);

impl Grader for AllOf {
    fn score(&self, output: &str) -> Result<f64, GraderError> {
        Ok(f64::from(u8::from(
            self.0.iter().all(|needle| output.contains(needle)),
        )))
    }
    fn kind(&self) -> &'static str {
        "all_of"
    }
}

impl Grader for AnyOf {
    fn score(&self, output: &str) -> Result<f64, GraderError> {
        Ok(f64::from(u8::from(
            self.0.iter().any(|needle| output.contains(needle)),
        )))
    }
    fn kind(&self) -> &'static str {
        "any_of"
    }
}

impl Grader for NotContains {
    fn score(&self, output: &str) -> Result<f64, GraderError> {
        Ok(f64::from(u8::from(
            !self.0.iter().any(|needle| output.contains(needle)),
        )))
    }
    fn kind(&self) -> &'static str {
        "not_contains"
    }
}

/// Clamps any score into `[0, 1]` — untrusted outputs can never push a
/// score outside the bounded range.
#[must_use]
pub fn clamp_score(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

/// Validates and compiles a spec. Empty expectation lists are invalid
/// here so they can never reach scoring time from a dataset.
///
/// # Errors
/// [`GraderError::Invalid`] on an unusable spec.
pub fn compile(spec: &GraderSpec) -> Result<Box<dyn Grader>, GraderError> {
    match spec {
        GraderSpec::AllOf { expected } if !expected.is_empty() => {
            Ok(Box::new(AllOf(expected.clone())))
        }
        GraderSpec::AnyOf { expected } if !expected.is_empty() => {
            Ok(Box::new(AnyOf(expected.clone())))
        }
        GraderSpec::NotContains { forbidden } if !forbidden.is_empty() => {
            Ok(Box::new(NotContains(forbidden.clone())))
        }
        _ => Err(GraderError::Invalid {
            detail: "empty expectation list".to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_of_scores_strictly_and_handles_unicode() {
        let grader = compile(&GraderSpec::AllOf {
            expected: vec![" metres".to_owned(), "メートル".to_owned()],
        })
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(grader.score("3 metres ≈ 3メートル").ok(), Some(1.0));
        assert_eq!(grader.score("3 metres only").ok(), Some(0.0));
    }

    #[test]
    fn any_of_and_not_contains_score_deterministically() {
        let any = compile(&GraderSpec::AnyOf {
            expected: vec!["yes".to_owned()],
        })
        .unwrap_or_else(|e| panic!("{e}"));
        let not = compile(&GraderSpec::NotContains {
            forbidden: vec!["TODO".to_owned()],
        })
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(any.score("yes indeed").ok(), Some(1.0));
        assert_eq!(any.score("no").ok(), Some(0.0));
        assert_eq!(not.score("clean").ok(), Some(1.0));
        assert_eq!(not.score("has TODO left").ok(), Some(0.0));
    }

    #[test]
    fn empty_expectations_are_invalid_and_scores_are_bounded() {
        assert!(compile(&GraderSpec::AllOf { expected: vec![] }).is_err());
        assert!(compile(&GraderSpec::AnyOf { expected: vec![] }).is_err());
        assert!(compile(&GraderSpec::NotContains { forbidden: vec![] }).is_err());
        assert_eq!(clamp_score(2.5), 1.0);
        assert_eq!(clamp_score(-1.0), 0.0);
    }
}
