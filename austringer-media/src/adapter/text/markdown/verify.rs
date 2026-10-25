//! The markdown round-trip gate (plan M30): reparse the produced bytes
//! (comrak, strict CommonMark + GFM - the same dialect the writer
//! renders in) and diff the census against the INDEPENDENT expected
//! tree under the text tolerance profile.

use serde_json::Value;

use super::extract::extract;
use crate::adapter::text::tolerances;
use crate::diff::{RoundTripVerdict, Tolerances, structural_diff};
use crate::error::MediaError;
use crate::extract::ExtractBudgets;

/// The markdown diff profile: float epsilon only; no unordered
/// pointers (canonical writes are ordered-exact), no ignored subtrees.
#[must_use]
pub fn profile() -> Tolerances {
    tolerances()
}

/// Verify produced markdown bytes against the INDEPENDENT expected
/// tree: extract (reparse) then M5 structural diff.
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
