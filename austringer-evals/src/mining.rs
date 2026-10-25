//! Cursor-blame-style eval mining (research §19): real commit-to-task
//! histories become eval cases — the commit SUBJECT is the user
//! intent, the changed FILE PATHS are the graded expectations (the
//! proxy for "edited the right place"). Mined cases carry the `mined`
//! tag; resolution-rate is the review-agent metric (the Bugbot
//! 52→76–78% arc is resolution rate over MINED intent, not synthetic
//! prompts).

use crate::dataset::{EvalCase, EvalDataset};
use crate::grader::GraderSpec;

/// One mined commit record (typed input — the caller owns the git
/// walk; nothing here shells out).
#[derive(Debug, Clone, PartialEq)]
pub struct CommitRecord {
    /// Commit sha.
    pub sha: String,
    /// Commit subject (the human intent).
    pub subject: String,
    /// Files the commit touched (the graded expectation).
    pub files: Vec<String>,
}

/// Mines commit records into eval cases. Records with no files or an
/// empty subject are skipped (not mined — no expectation, no case).
/// Case ids are `mine-<first 8 sha hex>` (deterministic; duplicates
/// fail dataset validation upstream).
#[must_use]
pub fn mine_cases(records: &[CommitRecord]) -> Vec<EvalCase> {
    records
        .iter()
        .filter(|record| !record.subject.trim().is_empty() && !record.files.is_empty())
        .map(|record| EvalCase {
            id: format!("mine-{}", record.sha.chars().take(8).collect::<String>()),
            prompt: format!("Implement: {}", record.subject),
            grader: GraderSpec::AllOf {
                expected: record.files.clone(),
            },
            threshold: 1.0,
            canary: None,
            tags: vec!["mined".to_owned()],
        })
        .collect()
}

/// Wraps mined cases into a dataset (validated — duplicate shas fail
/// closed).
///
/// # Errors
/// typed dataset errors (duplicate ids, empty set).
pub fn mine_dataset(records: &[CommitRecord]) -> Result<EvalDataset, crate::dataset::DatasetError> {
    let dataset = EvalDataset {
        cases: mine_cases(records),
    };
    if dataset.cases.is_empty() {
        return Err(crate::dataset::DatasetError::Empty);
    }
    let mut seen = std::collections::BTreeSet::new();
    for case in &dataset.cases {
        if !seen.insert(case.id.as_str()) {
            return Err(crate::dataset::DatasetError::DuplicateCase {
                id: case.id.clone(),
            });
        }
        crate::grader::compile(&case.grader)?;
    }
    Ok(dataset)
}

/// Resolution rate: solved cases over total (the review-agent metric).
#[must_use]
pub fn resolution_rate(solved: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        solved as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records() -> Vec<CommitRecord> {
        vec![
            CommitRecord {
                sha: "0123456789abcdef".to_owned(),
                subject: "fix(auth): refresh-token loop".to_owned(),
                files: vec!["src/auth/refresh.rs".to_owned()],
            },
            CommitRecord {
                sha: "fedcba9876543210".to_owned(),
                subject: "docs: readme".to_owned(),
                files: vec![],
            },
        ]
    }

    #[test]
    fn mining_maps_intent_to_graded_expectations() {
        let cases = mine_cases(&records());
        assert_eq!(cases.len(), 1, "fileless records are not mined");
        let case = &cases[0];
        assert_eq!(case.id, "mine-01234567");
        assert!(case.prompt.contains("fix(auth): refresh-token loop"));
        assert_eq!(
            case.grader,
            GraderSpec::AllOf {
                expected: vec!["src/auth/refresh.rs".to_owned()]
            }
        );
        assert!(case.tags.contains(&"mined".to_owned()));
        assert!(mine_dataset(&records()).is_ok());
    }

    #[test]
    fn resolution_rate_is_solved_over_total_and_zero_on_empty() {
        assert!((resolution_rate(3, 4) - 0.75).abs() < 1e-9);
        assert!((resolution_rate(0, 0)).abs() < 1e-9);
    }
}
