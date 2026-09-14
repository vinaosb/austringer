//! Eval datasets: typed JSON documents of cases. Malformed documents
//! are typed bounded errors (the raw document never rides the error);
//! duplicate case ids and empty datasets fail closed at parse time.
//! Loading is content-hashed (blake3) so a changed file is detected as
//! stale and re-parsed — never served from cache (todo 44 stale_state).

use std::collections::BTreeMap;
use std::path::Path;

use crate::grader::{self, GraderSpec};

/// One eval case. `prompt` is the ONLY field ever shown to the subject;
/// `grader`, `threshold`, and `canary` are scoring-side secrets by
/// construction (the runner never injects them into a prompt).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvalCase {
    /// Unique case id.
    pub id: String,
    /// The prompt handed to the subject.
    pub prompt: String,
    /// How the output is scored.
    pub grader: GraderSpec,
    /// Pass threshold in `(0.0, 1.0]` (default 1.0 — strict).
    #[serde(default = "default_threshold")]
    pub threshold: f64,
    /// A never-shown marker; if it appears in output the subject saw
    /// the dataset (leakage) and the run is zeroed.
    #[serde(default)]
    pub canary: Option<String>,
    /// Free-form tags (e.g. `mined`).
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Default pass threshold: strict full score.
fn default_threshold() -> f64 {
    1.0
}

/// A parsed dataset.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvalDataset {
    /// Cases in file order (deterministic).
    pub cases: Vec<EvalCase>,
}

/// Everything dataset handling can fail with.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DatasetError {
    /// Malformed document (bounded serde detail).
    #[error("malformed dataset: {detail}")]
    Malformed {
        /// Bounded detail (never the raw document).
        detail: String,
    },
    /// A dataset with zero cases.
    #[error("dataset has no cases")]
    Empty,
    /// Duplicate case id.
    #[error("duplicate case id {id:?}")]
    DuplicateCase {
        /// The duplicated id.
        id: String,
    },
    /// An unusable grader spec.
    #[error(transparent)]
    Grader(#[from] grader::GraderError),
    /// Read failure (bounded detail).
    #[error("dataset read failed: {detail}")]
    Io {
        /// Bounded detail.
        detail: String,
    },
}

impl EvalDataset {
    /// Parses and validates a JSON dataset document.
    ///
    /// # Errors
    /// typed [`DatasetError`] — never a panic, never the raw document.
    pub fn parse(json: &str) -> Result<Self, DatasetError> {
        let dataset: EvalDataset =
            serde_json::from_str(json).map_err(|e| DatasetError::Malformed {
                detail: e.to_string().chars().take(300).collect(),
            })?;
        if dataset.cases.is_empty() {
            return Err(DatasetError::Empty);
        }
        let mut seen = std::collections::BTreeSet::new();
        for case in &dataset.cases {
            if !seen.insert(case.id.as_str()) {
                return Err(DatasetError::DuplicateCase {
                    id: case.id.clone(),
                });
            }
            if case.threshold <= 0.0 || case.threshold > 1.0 {
                return Err(DatasetError::Malformed {
                    detail: format!("case {:?} threshold outside (0.0, 1.0]", case.id)
                        .chars()
                        .take(300)
                        .collect(),
                });
            }
            grader::compile(&case.grader)?;
        }
        Ok(dataset)
    }

    /// Loads a dataset from disk (content-hashed).
    ///
    /// # Errors
    /// typed [`DatasetError`].
    pub fn load(path: &Path) -> Result<(Self, DatasetFingerprint), DatasetError> {
        let bytes = std::fs::read(path).map_err(|e| DatasetError::Io {
            detail: e.to_string().chars().take(200).collect(),
        })?;
        let fingerprint = DatasetFingerprint(blake3::hash(&bytes).to_hex().to_string());
        let text = String::from_utf8_lossy(&bytes).into_owned();
        Ok((Self::parse(&text)?, fingerprint))
    }
}

/// Content fingerprint of a dataset file (blake3, hex).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetFingerprint(pub String);

/// Result of a cache lookup: `Fresh` (hash unchanged) or `Reloaded`
/// (on-disk content changed — the stale parse was discarded).
#[derive(Debug, Clone, PartialEq)]
pub enum CachedDataset {
    /// Served from the cached parse.
    Fresh(EvalDataset),
    /// On-disk hash changed; this is a freshly parsed dataset.
    Reloaded(EvalDataset),
}

impl CachedDataset {
    /// The dataset regardless of freshness.
    #[must_use]
    pub fn dataset(&self) -> &EvalDataset {
        match self {
            Self::Fresh(d) | Self::Reloaded(d) => d,
        }
    }
}

/// A single-entry content-addressed dataset cache. Serves the cached
/// parse only while the file's blake3 hash is unchanged; any change
/// forces a reload (stale parses are never served).
#[derive(Default)]
pub struct DatasetCache {
    entry: Option<(std::path::PathBuf, DatasetFingerprint, EvalDataset)>,
}

impl DatasetCache {
    /// A new empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads `path`, serving the cached parse only on an identical
    /// content hash.
    ///
    /// # Errors
    /// typed [`DatasetError`].
    pub fn get(&mut self, path: &Path) -> Result<CachedDataset, DatasetError> {
        let (dataset, fingerprint) = EvalDataset::load(path)?;
        let stale = match &self.entry {
            Some((cached_path, cached_hash, _)) => {
                cached_path == path && *cached_hash != fingerprint
            }
            None => false,
        };
        self.entry = Some((path.to_owned(), fingerprint, dataset.clone()));
        Ok(if stale {
            CachedDataset::Reloaded(dataset)
        } else {
            CachedDataset::Fresh(dataset)
        })
    }
}

/// Serializes case-id → prompt (the dataset's subject-facing view;
/// scoring secrets never appear).
///
/// # Errors
/// serialization failure (bounded).
pub fn prompt_view(dataset: &EvalDataset) -> Result<BTreeMap<String, String>, DatasetError> {
    Ok(dataset
        .cases
        .iter()
        .map(|case| (case.id.clone(), case.prompt.clone()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case_json(id: &str, expected: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "prompt": format!("solve {id}"),
            "grader": { "kind": "all_of", "expected": [expected] }
        })
    }

    #[test]
    fn parses_valid_documents_and_defaults_the_threshold() {
        let doc = serde_json::json!({ "cases": [case_json("a", "42")] }).to_string();
        let dataset = EvalDataset::parse(&doc).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(dataset.cases.len(), 1);
        assert_eq!(dataset.cases[0].threshold, 1.0);
        assert!(dataset.cases[0].canary.is_none());
    }

    #[test]
    fn malformed_and_empty_and_duplicate_are_typed() {
        let malformed = EvalDataset::parse("{ nope");
        assert!(matches!(malformed, Err(DatasetError::Malformed { .. })));
        let empty = EvalDataset::parse(r#"{"cases": []}"#);
        assert_eq!(empty.err(), Some(DatasetError::Empty));
        let dup = EvalDataset::parse(
            &serde_json::json!({ "cases": [case_json("a", "1"), case_json("a", "2")] }).to_string(),
        );
        assert!(matches!(dup, Err(DatasetError::DuplicateCase { .. })));
    }

    #[test]
    fn prompt_view_never_leaks_scoring_secrets() {
        let doc = serde_json::json!({
            "cases": [{
                "id": "secret-case",
                "prompt": "what is the passcode",
                "grader": { "kind": "all_of", "expected": ["hunter2"] },
                "canary": "canary-xyz"
            }]
        })
        .to_string();
        let dataset = EvalDataset::parse(&doc).unwrap_or_else(|e| panic!("{e}"));
        let view = prompt_view(&dataset).unwrap_or_default();
        let rendered = format!("{view:?}");
        assert!(!rendered.contains("hunter2"));
        assert!(!rendered.contains("canary-xyz"));
    }

    #[test]
    fn threshold_must_be_in_range() {
        let doc = serde_json::json!({
            "cases": [{ "id": "t", "prompt": "p",
                "grader": { "kind": "all_of", "expected": ["x"] },
                "threshold": 0.0 }]
        })
        .to_string();
        assert!(EvalDataset::parse(&doc).is_err());
    }

    #[test]
    fn grader_spec_shape_is_rejected_when_invalid() {
        let doc = serde_json::json!({
            "cases": [{ "id": "t", "prompt": "p",
                "grader": { "kind": "all_of", "expected": [] } }]
        })
        .to_string();
        assert!(matches!(
            EvalDataset::parse(&doc),
            Err(DatasetError::Grader(_))
        ));
    }
}
