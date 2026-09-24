//! The html round-trip gate (plan M30): html5gum reparse (the extract
//! walk) then M5 structural diff against the INDEPENDENT expected
//! tree. There is no strict-parse failure stage by design - the WHATWG
//! tokenizer is total - so the census diff IS the reparse gate.

use serde_json::Value;

use super::extract::extract;
use crate::adapter::text::tolerances;
use crate::diff::{structural_diff, RoundTripVerdict, Tolerances};
use crate::error::MediaError;
use crate::extract::ExtractBudgets;

/// The html diff profile: float epsilon only; no unordered pointers,
/// no ignored subtrees.
#[must_use]
pub fn profile() -> Tolerances {
    tolerances()
}

/// Verify produced html bytes against the INDEPENDENT expected tree:
/// reparse (extract) then M5 structural diff.
///
/// # Errors
/// The extractor's typed failures (non-utf8 is `ParseFailure`);
/// [`MediaError::DiffTooLarge`] when the diff caps trip.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let extracted = extract(bytes, ExtractBudgets::default())?;
    let diff = structural_diff(expected, &extracted.tree, &profile())?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}
