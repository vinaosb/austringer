//! The committed hermetic scenario pack (todo 26): the ecosystem-
//! alignment plan's headline mechanisms as evalable artifacts. The
//! pack lives in `austringer-evals/scenarios/` and is LOADED, never
//! re-implemented: [`dataset.json`] (EvalCase datasets for plan-gate
//! refusal, run_tests report shape, search_sessions scoping),
//! `cassettes/*.ndjson` (recorded `austringer run --output-json` streams
//! under the scripted fake provider, one per case), and
//! [`conformance-pins.json`] (golden prefixes for ask-first fallback
//! and task(category) fan-out, pinned via
//! [`crate::conformance::pin_prefix`]).
//!
//! Loading is fail-loud: a corrupted cassette, a missing or orphan
//! cassette, or a pin set that is not exactly [`PINNED_CASES`] is a
//! typed error naming the file — a pack that silently skipped a
//! mechanism would be a receipt that isn't.

use std::collections::BTreeMap;
use std::path::Path;

use crate::conformance::{self, ConformanceVerdict, GoldenPrefix};
use crate::dataset::EvalDataset;
use crate::pinning::ExpectedFailures;
use crate::runner::ScriptedSubject;
use crate::wire::TurnStream;

/// The exact pin set the pack must ship (doc-constant): the two
/// plan mechanisms pinned by golden prefixes. Anything else fails
/// [`ScenarioError::PinSetMismatch`].
pub const PINNED_CASES: [&str; 2] = ["ask-first-fallback", "task-category-fan-out"];

/// Everything scenario-pack loading can fail with. Every variant
/// names the offending file.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ScenarioError {
    /// The dataset document is unusable.
    #[error("scenario pack dataset: {0}")]
    Dataset(#[from] crate::dataset::DatasetError),
    /// A cassette is not a valid NDJSON stream (the loud corrupted-
    /// cassette failure; the path is named).
    #[error("cassette {path} is not a valid NDJSON stream: {detail}")]
    Cassette {
        /// The offending cassette path.
        path: String,
        /// Bounded wire detail (never the raw line).
        detail: String,
    },
    /// A dataset case with no cassette on disk.
    #[error("dataset case {case_id} has no cassette at {path}")]
    MissingCassette {
        /// The case id.
        case_id: String,
        /// The expected cassette path.
        path: String,
    },
    /// A cassette file with no dataset case.
    #[error("cassette {path} has no dataset case")]
    OrphanCassette {
        /// The orphan cassette path.
        path: String,
    },
    /// A pin referencing a case the dataset does not contain.
    #[error("conformance pin references unknown case {case_id}")]
    PinForUnknownCase {
        /// The unknown case id.
        case_id: String,
    },
    /// The pin set is not exactly [`PINNED_CASES`].
    #[error("conformance pin set must be exactly {PINNED_CASES:?}, found {found:?}")]
    PinSetMismatch {
        /// The pin keys found on disk.
        found: Vec<String>,
    },
    /// The pins or expected-failures document is malformed.
    #[error("scenario pack {file}: {detail}")]
    Malformed {
        /// The offending file name.
        file: &'static str,
        /// Bounded detail.
        detail: String,
    },
    /// A pack file could not be read.
    #[error("scenario pack read failed: {detail}")]
    Io {
        /// Bounded detail.
        detail: String,
    },
}

/// One golden-prefix verdict for a pinned case (the named drift the
/// sweep surfaces).
#[derive(Debug, Clone)]
pub struct ConformanceFinding {
    /// The pinned case id.
    pub case_id: String,
    /// The cassette file name checked.
    pub cassette: String,
    /// The verdict.
    pub verdict: ConformanceVerdict,
}

impl std::fmt::Display for ConformanceFinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.verdict {
            ConformanceVerdict::Conforms => {
                write!(f, "{} conforms ({})", self.case_id, self.cassette)
            }
            ConformanceVerdict::Drift { expected, actual } => write!(
                f,
                "golden-prefix drift in {} (cassette {}): expected {expected:?}, actual {actual:?}",
                self.case_id, self.cassette
            ),
        }
    }
}

/// One parsed cassette: raw NDJSON lines (the subject script) plus
/// the parsed stream (conformance input).
struct Cassette {
    file_name: String,
    lines: Vec<String>,
    stream: TurnStream,
}

/// A loaded, validated scenario pack.
pub struct ScenarioPack {
    /// The eval dataset (dataset order is the sweep order).
    pub dataset: EvalDataset,
    /// The expected-failures pins (ships green: `failures: []`).
    pub expected_failures: ExpectedFailures,
    /// Golden-prefix pins (exactly [`PINNED_CASES`] keys).
    pub pins: BTreeMap<String, GoldenPrefix>,
    cassettes: BTreeMap<String, Cassette>,
}

fn read(dir: &Path, file: &str) -> Result<String, ScenarioError> {
    std::fs::read_to_string(dir.join(file)).map_err(|e| ScenarioError::Io {
        detail: format!(
            "{file}: {}",
            e.to_string().chars().take(200).collect::<String>()
        ),
    })
}

fn bounded(detail: String) -> String {
    detail.chars().take(200).collect()
}

/// Loads and validates the scenario pack at `dir`.
///
/// # Errors
/// typed [`ScenarioError`] — every failure names the offending file.
pub fn load(dir: &Path) -> Result<ScenarioPack, ScenarioError> {
    let dataset = EvalDataset::parse(&read(dir, "dataset.json")?)?;
    let mut cassettes = BTreeMap::new();
    for case in &dataset.cases {
        let file_name = format!("{}.ndjson", case.id);
        let path = dir.join("cassettes").join(&file_name);
        let text = std::fs::read_to_string(&path).map_err(|_| ScenarioError::MissingCassette {
            case_id: case.id.clone(),
            path: path.display().to_string(),
        })?;
        let lines: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect();
        let stream = TurnStream::parse(&lines).map_err(|e| ScenarioError::Cassette {
            path: path.display().to_string(),
            detail: bounded(e.to_string()),
        })?;
        cassettes.insert(
            case.id.clone(),
            Cassette {
                file_name,
                lines,
                stream,
            },
        );
    }
    for entry in std::fs::read_dir(dir.join("cassettes")).map_err(|e| ScenarioError::Io {
        detail: bounded(e.to_string()),
    })? {
        let entry = entry.map_err(|e| ScenarioError::Io {
            detail: bounded(e.to_string()),
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let stem = name.strip_suffix(".ndjson").unwrap_or(&name);
        if !dataset.cases.iter().any(|case| case.id == stem) {
            return Err(ScenarioError::OrphanCassette {
                path: entry.path().display().to_string(),
            });
        }
    }
    let pins: BTreeMap<String, GoldenPrefix> =
        serde_json::from_str(&read(dir, "conformance-pins.json")?).map_err(|e| {
            ScenarioError::Malformed {
                file: "conformance-pins.json",
                detail: bounded(e.to_string()),
            }
        })?;
    for case_id in pins.keys() {
        if !dataset.cases.iter().any(|case| &case.id == case_id) {
            return Err(ScenarioError::PinForUnknownCase {
                case_id: case_id.clone(),
            });
        }
    }
    let found: Vec<String> = pins.keys().cloned().collect();
    if found != PINNED_CASES.map(str::to_owned) {
        return Err(ScenarioError::PinSetMismatch { found });
    }
    let expected_failures =
        crate::pinning::parse_expected_failures(&read(dir, "expected-failures.yaml")?).map_err(
            |e| ScenarioError::Malformed {
                file: "expected-failures.yaml",
                detail: bounded(e.to_string()),
            },
        )?;
    Ok(ScenarioPack {
        dataset,
        expected_failures,
        pins,
        cassettes,
    })
}

impl ScenarioPack {
    /// The pinned case ids (sorted; exactly [`PINNED_CASES`]).
    #[must_use]
    pub fn pinned_case_ids(&self) -> Vec<&str> {
        self.pins.keys().map(String::as_str).collect()
    }

    /// Builds the scripted fake-provider subject: one recorded
    /// cassette script per case, one repetition.
    #[must_use]
    pub fn build_subject(&self) -> ScriptedSubject {
        let mut subject = ScriptedSubject::new();
        for (case_id, cassette) in &self.cassettes {
            subject.add(case_id, vec![cassette.lines.clone()]);
        }
        subject
    }

    /// Checks every pinned case's cassette against its golden
    /// prefix; non-conformant cases are returned as named findings.
    #[must_use]
    pub fn conformance_findings(&self) -> Vec<ConformanceFinding> {
        self.cassettes
            .iter()
            .filter_map(|(case_id, cassette)| {
                let pin = self.pins.get(case_id)?;
                let verdict = conformance::check(&cassette.stream, pin);
                (!verdict.is_conformant()).then(|| ConformanceFinding {
                    case_id: case_id.clone(),
                    cassette: cassette.file_name.clone(),
                    verdict,
                })
            })
            .collect()
    }
}
