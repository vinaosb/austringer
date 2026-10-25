//! Expected-failures YAML pinning (plan todo 44). The pin file is a
//! STRICT OWNED YAML SUBSET — fixed schema, flat scalars, single-line
//! strings — parsed by a purpose-built reader so the evals crate adds
//! zero new dependencies. Anything outside the subset is a typed
//! bounded error, never a guess.
//!
//! Normative format (doc-constant; the tests pin it):
//!
//! ```yaml
//! version: 1
//! failures:
//!   - case: fix-auth-3
//!     model: mini
//!     reason: known tool-schema mismatch
//! ```
//!
//! `model` is optional (`None` pins the case on every model). A
//! FAILING case that is not pinned surfaces as an unpinned regression
//! in the report; a pinned case that now PASSES is listed as a stale
//! pin (benchmark honesty: pins are living annotations, not gravity).

use crate::report::SweepReport;
use serde::{Deserialize, Serialize};

/// The only supported pin-file version.
pub const PIN_FORMAT_VERSION: u32 = 1;

/// One pinned known failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PinnedFailure {
    /// Case id.
    pub case: String,
    /// Model scope (`None` = every model).
    #[serde(default)]
    pub model: Option<String>,
    /// Why this failure is known/accepted (annotated in the report).
    pub reason: String,
}

/// A parsed pin file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectedFailures {
    /// Format version (must be [`PIN_FORMAT_VERSION`]).
    pub version: u32,
    /// The pins.
    pub failures: Vec<PinnedFailure>,
}

/// Everything pin parsing can fail with.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PinError {
    /// Malformed YAML-subset document (bounded detail).
    #[error("malformed expected-failures file: {detail}")]
    Malformed {
        /// Bounded detail (never the raw document).
        detail: String,
    },
    /// Unsupported version.
    #[error("unsupported pin file version {found} (expected {PIN_FORMAT_VERSION})")]
    UnknownVersion {
        /// The version found.
        found: u32,
    },
}

/// A failure that was NOT pinned — the surfaced regression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnpinnedRegression {
    /// Case id.
    pub case: String,
    /// Model scope.
    pub model: Option<String>,
}

/// The audit outcome.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PinAudit {
    /// Known failures that occurred and are annotated.
    pub pinned_confirmed: Vec<PinnedFailure>,
    /// Failing cases with no pin — surfaced in the report.
    pub unpinned: Vec<UnpinnedRegression>,
    /// Pins whose case now passes (stale — remove them).
    pub stale: Vec<PinnedFailure>,
}

/// Strips a trailing ` # comment` from a content line (quoted values
/// keep their interior hashes: `"a # b"` survives intact).
fn strip_comment(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.starts_with('#') {
        return String::new();
    }
    if let Some(rest) = trimmed.strip_prefix('"')
        && let Some(close) = rest.find('"')
    {
        return rest[..=close + 1].to_owned();
    }
    match trimmed.find(" #") {
        Some(at) => trimmed[..at].to_owned(),
        None => trimmed.to_owned(),
    }
}

/// Strips one layer of matching quotes from a scalar.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        return value[1..value.len() - 1].to_owned();
    }
    value.to_owned()
}

type Item = (String, Option<String>, String);

fn malformed(detail: &str) -> PinError {
    PinError::Malformed {
        detail: detail.chars().take(200).collect(),
    }
}

fn flush(current: &mut Option<Item>, failures: &mut Vec<PinnedFailure>) -> Result<(), PinError> {
    if let Some((case, model, reason)) = current.take() {
        if case.trim().is_empty() {
            return Err(malformed("list item has an empty `case`"));
        }
        if reason.trim().is_empty() {
            return Err(malformed("list item missing `reason`"));
        }
        failures.push(PinnedFailure {
            case,
            model: model.filter(|m| !m.trim().is_empty()),
            reason,
        });
    }
    Ok(())
}

/// Parses the strict YAML-subset pin document.
///
/// # Errors
/// typed [`PinError`].
pub fn parse_expected_failures(yaml: &str) -> Result<ExpectedFailures, PinError> {
    let mut version: Option<u32> = None;
    let mut failures: Vec<PinnedFailure> = Vec::new();
    let mut in_failures = false;
    let mut current: Option<Item> = None;
    for raw in yaml.lines() {
        let line = strip_comment(raw);
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("version:") {
            if version.is_some() {
                return Err(malformed("duplicate `version` key"));
            }
            version = Some(
                unquote(rest.trim())
                    .parse::<u32>()
                    .map_err(|_| malformed("version is not an integer"))?,
            );
            continue;
        }
        if let Some(rest) = line.strip_prefix("failures:") {
            if in_failures {
                return Err(malformed("duplicate `failures` key"));
            }
            in_failures = true;
            match rest.trim() {
                "[]" => continue,
                "" => continue,
                _ => {
                    return Err(malformed(
                        "expected empty list `[]` or nothing after `failures:`",
                    ));
                }
            }
        }
        if let Some(rest) = line.strip_prefix("- ") {
            flush(&mut current, &mut failures)?;
            let mut parts = rest.splitn(2, ':');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();
            if key != "case" || value.is_empty() {
                return Err(malformed("list items must start with `case: <id>`"));
            }
            current = Some((unquote(value), None, String::new()));
            continue;
        }
        if in_failures && let Some((case, model, reason)) = current.as_mut() {
            let mut parts = line.splitn(2, ':');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();
            match key {
                "model" if model.is_none() => *model = Some(unquote(value)),
                "reason" if reason.is_empty() => *reason = unquote(value),
                _ => return Err(malformed("unexpected or duplicate list key")),
            }
            let _ = case;
            continue;
        }
        return Err(malformed("unexpected line outside the pin schema"));
    }
    flush(&mut current, &mut failures)?;
    let found = version.ok_or_else(|| malformed("missing `version` key"))?;
    if found != PIN_FORMAT_VERSION {
        return Err(PinError::UnknownVersion { found });
    }
    Ok(ExpectedFailures {
        version: found,
        failures,
    })
}

/// Serializes pins back to the normative format (round-trip stable).
#[must_use]
pub fn render_expected_failures(pins: &ExpectedFailures) -> String {
    if pins.failures.is_empty() {
        return format!("version: {}\nfailures: []\n", pins.version);
    }
    let mut out = format!("version: {}\nfailures:\n", pins.version);
    for pin in &pins.failures {
        out.push_str(&format!("  - case: {}\n", pin.case));
        if let Some(model) = &pin.model {
            out.push_str(&format!("    model: {model}\n"));
        }
        out.push_str(&format!("    reason: {}\n", pin.reason));
    }
    out
}

/// Audits a finished report against the pins: failing+pinned ⇒
/// annotated; failing+unpinned ⇒ surfaced regression; passing+pinned ⇒
/// stale.
#[must_use]
pub fn audit_pins(report: &SweepReport, pins: &ExpectedFailures) -> PinAudit {
    let mut audit = PinAudit::default();
    let mut used: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for pair in &report.pairs {
        let model = pair.pair.model.clone();
        for case in &pair.metrics.cases {
            if case.pass_k {
                continue;
            }
            let pin_index = pins.failures.iter().position(|pin| {
                pin.case == case.case_id
                    && (pin.model.is_none() || pin.model.as_deref() == Some(model.as_str()))
            });
            match pin_index {
                Some(index) if used.insert(index) => {
                    audit.pinned_confirmed.push(pins.failures[index].clone());
                }
                Some(_) => {}
                None => audit.unpinned.push(UnpinnedRegression {
                    case: case.case_id.clone(),
                    model: Some(model.clone()),
                }),
            }
        }
    }
    audit.stale = pins
        .failures
        .iter()
        .enumerate()
        .filter(|(index, _)| !used.contains(index))
        .map(|(_, pin)| pin.clone())
        .collect();
    audit
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "version: 1\nfailures:\n  - case: fix-auth-3\n    model: mini\n    reason: known tool-schema mismatch\n";

    #[test]
    fn parses_the_normative_subset_and_round_trips() {
        let pins = parse_expected_failures(DOC).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(pins.version, PIN_FORMAT_VERSION);
        assert_eq!(pins.failures.len(), 1);
        assert_eq!(pins.failures[0].case, "fix-auth-3");
        assert_eq!(pins.failures[0].model.as_deref(), Some("mini"));
        let rendered = render_expected_failures(&pins);
        assert_eq!(rendered, DOC);
        let reparsed = parse_expected_failures(&rendered).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(reparsed, pins);
    }

    #[test]
    fn quoted_values_missing_fields_and_bad_versions_are_typed_errors() {
        let quoted =
            "version: 1\nfailures:\n  - case: \"quoted-case\"\n    reason: \"quoted reason\"\n";
        let pins = parse_expected_failures(quoted).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(pins.failures[0].case, "quoted-case");
        assert_eq!(pins.failures[0].model, None);
        let missing_reason = "version: 1\nfailures:\n  - case: x\n";
        assert!(matches!(
            parse_expected_failures(missing_reason),
            Err(PinError::Malformed { .. })
        ));
        let bad_version = "version: 9\nfailures: []\n";
        assert!(matches!(
            parse_expected_failures(bad_version),
            Err(PinError::UnknownVersion { found: 9 })
        ));
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let doc = "# pin file\n\nversion: 1  # v1\nfailures: []\n";
        let pins = parse_expected_failures(doc).unwrap_or_else(|e| panic!("{e}"));
        assert!(pins.failures.is_empty());
    }
}
