//! TSS golden-prefix replay — the drift gate (plan todo 44): the FIRST
//! TWO TOOL CALLS of a run are pinned as a golden prefix and compared
//! against the pinned reference; any drift (different tool, different
//! order, missing prefix) is a typed verdict surfaced in the report
//! before it silently changes behavior. Prefixes shorter than two are
//! pinned as-is (a run with one tool call pins one name).

use crate::wire::TurnStream;
use serde::{Deserialize, Serialize};

/// A pinned golden prefix (first-two tool-call names).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldenPrefix {
    /// The pinned tool-call names (0–2 entries).
    pub tool_calls: Vec<String>,
}

/// The conformance verdict for one stream against its pin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceVerdict {
    /// The first-two tool calls match the pin.
    Conforms,
    /// Drift detected — expected vs actual, surfaced not absorbed.
    Drift {
        /// The pinned prefix.
        expected: Vec<String>,
        /// What the run actually did.
        actual: Vec<String>,
    },
}

impl ConformanceVerdict {
    /// True only for [`ConformanceVerdict::Conforms`].
    #[must_use]
    pub fn is_conformant(&self) -> bool {
        matches!(self, Self::Conforms)
    }
}

/// Pins a stream's golden prefix (first two tool calls).
#[must_use]
pub fn pin_prefix(stream: &TurnStream) -> GoldenPrefix {
    GoldenPrefix {
        tool_calls: stream.first_two_tool_calls(),
    }
}

/// Checks a stream against a pinned prefix.
#[must_use]
pub fn check(stream: &TurnStream, pin: &GoldenPrefix) -> ConformanceVerdict {
    let actual = stream.first_two_tool_calls();
    if actual == pin.tool_calls {
        ConformanceVerdict::Conforms
    } else {
        ConformanceVerdict::Drift {
            expected: pin.tool_calls.clone(),
            actual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::ToolCallObserved;

    fn stream(tools: &[&str]) -> TurnStream {
        TurnStream {
            assistant_text: String::new(),
            tool_calls: tools
                .iter()
                .map(|tool| ToolCallObserved {
                    call_id: "c".to_owned(),
                    tool: (*tool).to_owned(),
                    arguments_json: "{}".to_owned(),
                })
                .collect(),
            outcome: "task_completed".to_owned(),
            outcome_ok: true,
        }
    }

    #[test]
    fn matching_prefix_conforms_and_drift_is_typed() {
        let pin = pin_prefix(&stream(&["tool_search", "view_window", "edit_lines"]));
        assert_eq!(pin.tool_calls, vec!["tool_search", "view_window"]);
        assert!(check(&stream(&["tool_search", "view_window", "x"]), &pin).is_conformant());
        let verdict = check(&stream(&["view_window", "tool_search"]), &pin);
        assert_eq!(
            verdict,
            ConformanceVerdict::Drift {
                expected: vec!["tool_search".to_owned(), "view_window".to_owned()],
                actual: vec!["view_window".to_owned(), "tool_search".to_owned()],
            }
        );
    }

    #[test]
    fn short_runs_pin_short_prefixes_and_drift_on_growth() {
        let pin = pin_prefix(&stream(&["tool_search"]));
        assert_eq!(pin.tool_calls, vec!["tool_search"]);
        assert!(check(&stream(&["tool_search"]), &pin).is_conformant());
        assert!(!check(&stream(&["tool_search", "edit_lines"]), &pin).is_conformant());
    }
}
