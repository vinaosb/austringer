//! Replay-from-traces benchmarks (Warp Factories, research §25):
//! STORED real-run NDJSON traces are re-scored across scorer
//! variations — same trace, different graders — producing benchmark
//! rows that feed the routing table WITHOUT any live-model call. This
//! is the private-benchmark eval surface: trace once, re-score
//! cheaply, compare scorers on identical subject behavior.

use std::collections::BTreeMap;

use crate::dataset::EvalDataset;
use crate::grader::GraderSpec;
use crate::guard::GuardSet;
use crate::runner::grade_stream;
use crate::wire::TurnStream;

/// One stored trace: the model label + the case it answered + the raw
/// NDJSON lines recorded from a real `austringer run --output-json`.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceRecord {
    /// Model that produced the trace.
    pub model: String,
    /// Case id the trace answers.
    pub case_id: String,
    /// The recorded NDJSON lines.
    pub lines: Vec<String>,
}

/// One re-scored cell: (model, scorer) → mean guarded score.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RescoredCell {
    /// Model label.
    pub model: String,
    /// Scorer variant name.
    pub scorer: String,
    /// Mean guarded score across the model's traces.
    pub score: f64,
}

/// Everything re-scoring can fail with.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RetraceError {
    /// A trace references a case the dataset does not know.
    #[error("trace for unknown case {case_id:?} (model {model:?})")]
    UnknownCase {
        /// The trace's case id.
        case_id: String,
        /// The trace's model.
        model: String,
    },
    /// A stored trace failed to parse.
    #[error("trace for case {case_id:?} (model {model:?}) is invalid")]
    InvalidTrace {
        /// The trace's case id.
        case_id: String,
        /// The trace's model.
        model: String,
    },
}

/// Re-scores stored traces under each scorer variant (guards applied —
/// identical integrity rules to live scoring). Errors are TYPED and
/// terminal (a corrupt trace store is an audit failure, not noise).
///
/// # Errors
/// typed [`RetraceError`].
pub fn rescore_traces(
    dataset: &EvalDataset,
    traces: &[TraceRecord],
    scorer_variants: &BTreeMap<String, GraderSpec>,
    guards: &GuardSet,
) -> Result<Vec<RescoredCell>, RetraceError> {
    let mut cells: BTreeMap<(String, String), (f64, u32)> = BTreeMap::new();
    for trace in traces {
        let case = dataset
            .cases
            .iter()
            .find(|case| case.id == trace.case_id)
            .ok_or_else(|| RetraceError::UnknownCase {
                case_id: trace.case_id.clone(),
                model: trace.model.clone(),
            })?;
        let stream = TurnStream::parse(&trace.lines).map_err(|_| RetraceError::InvalidTrace {
            case_id: trace.case_id.clone(),
            model: trace.model.clone(),
        })?;
        for (name, spec) in scorer_variants {
            let rep = grade_stream(case, &stream, spec, guards);
            let entry = cells
                .entry((trace.model.clone(), name.clone()))
                .or_insert((0.0, 0));
            entry.0 += rep.score;
            entry.1 += 1;
        }
    }
    Ok(cells
        .into_iter()
        .map(|((model, scorer), (sum, count))| RescoredCell {
            model,
            scorer,
            score: if count == 0 {
                0.0
            } else {
                sum / f64::from(count)
            },
        })
        .collect())
}

/// Converts re-scored cells into routing feeder rows (best scorer per
/// model, `total_cmp` deterministic; prices required — never
/// invented).
///
/// # Errors
/// a model missing from `prices`.
pub fn benchmark_cells(
    cells: &[RescoredCell],
    prices: &BTreeMap<String, f64>,
) -> Result<Vec<crate::benchmark::BenchmarkRow>, String> {
    let mut best: BTreeMap<&str, f64> = BTreeMap::new();
    for cell in cells {
        let entry = best.entry(cell.model.as_str()).or_insert(f64::MIN);
        if cell.score.total_cmp(entry) == std::cmp::Ordering::Greater {
            *entry = cell.score;
        }
    }
    best.into_iter()
        .map(|(model, score)| {
            let price = prices
                .get(model)
                .ok_or_else(|| format!("no price for model {model:?}"))?;
            Ok(crate::benchmark::BenchmarkRow {
                model: model.to_owned(),
                score: score as f32,
                price_per_mtok: *price,
                retracted: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::EvalCase;

    fn trace_dataset() -> EvalDataset {
        EvalDataset {
            cases: vec![EvalCase {
                id: "sum".to_owned(),
                prompt: "sum 2+2".to_owned(),
                grader: GraderSpec::AllOf {
                    expected: vec!["4".to_owned()],
                },
                threshold: 1.0,
                canary: None,
                tags: vec![],
            }],
        }
    }

    fn trace_lines(text: &str) -> Vec<String> {
        vec![
            serde_json::json!({
                "event": { "id": 1, "event": {
                    "type": "assistant",
                    "blocks": [ { "type": "text", "text": text } ]
                }}
            })
            .to_string(),
            r#"{"type":"turn_result","ok":true,"outcome":"task_completed","turn":1}"#.to_owned(),
        ]
    }

    #[test]
    fn same_trace_scores_differently_under_scorer_variants() {
        let dataset = trace_dataset();
        let traces = vec![TraceRecord {
            model: "mini".to_owned(),
            case_id: "sum".to_owned(),
            lines: trace_lines("the answer is four"),
        }];
        let mut variants = BTreeMap::new();
        variants.insert(
            "strict".to_owned(),
            GraderSpec::AllOf {
                expected: vec!["4".to_owned()],
            },
        );
        variants.insert(
            "lenient".to_owned(),
            GraderSpec::AnyOf {
                expected: vec!["4".to_owned(), "four".to_owned()],
            },
        );
        let guards = GuardSet::standard();
        let cells =
            rescore_traces(&dataset, &traces, &variants, &guards).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(cells.len(), 2);
        let strict = cells
            .iter()
            .find(|c| c.scorer == "strict")
            .cloned()
            .unwrap_or_default();
        let lenient = cells
            .iter()
            .find(|c| c.scorer == "lenient")
            .cloned()
            .unwrap_or_default();
        assert!((strict.score).abs() < 1e-9);
        assert!((lenient.score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn unknown_cases_and_corrupt_traces_are_typed_errors() {
        let dataset = trace_dataset();
        let guards = GuardSet::standard();
        let mut variants = BTreeMap::new();
        variants.insert(
            "v".to_owned(),
            GraderSpec::AllOf {
                expected: vec!["4".to_owned()],
            },
        );
        let unknown = vec![TraceRecord {
            model: "m".to_owned(),
            case_id: "ghost".to_owned(),
            lines: trace_lines("4"),
        }];
        assert!(matches!(
            rescore_traces(&dataset, &unknown, &variants, &guards),
            Err(RetraceError::UnknownCase { .. })
        ));
        let corrupt = vec![TraceRecord {
            model: "m".to_owned(),
            case_id: "sum".to_owned(),
            lines: vec!["{bad".to_owned()],
        }];
        assert!(matches!(
            rescore_traces(&dataset, &corrupt, &variants, &guards),
            Err(RetraceError::InvalidTrace { .. })
        ));
    }

    #[test]
    fn benchmark_cells_feed_routing_rows_with_best_scorer_scores() {
        let cells = vec![
            RescoredCell {
                model: "mini".to_owned(),
                scorer: "strict".to_owned(),
                score: 0.25,
            },
            RescoredCell {
                model: "mini".to_owned(),
                scorer: "lenient".to_owned(),
                score: 0.75,
            },
        ];
        let mut prices = BTreeMap::new();
        prices.insert("mini".to_owned(), 0.25);
        let rows = benchmark_cells(&cells, &prices).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(rows.len(), 1);
        assert!((rows[0].score - 0.75).abs() < 1e-6);
        assert!(rows[0].live());
        prices.remove("mini");
        assert!(benchmark_cells(&cells, &prices).is_err());
    }
}
