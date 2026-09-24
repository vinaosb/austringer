//! The svg round-trip gate (plan M11): the M5 structural diff under the
//! svg tolerance profile PLUS the usvg strict parse as the digest's
//! stage-1 verify hook. The stage-2 visual check (resvg render ->
//! perceptual hash) is named deferred in the honesty matrix - font
//! availability makes visual comparison a rendering-engine statement,
//! not a document-structure one, and v0.2 claims only structure.
//!
//! usvg is built text-less (no fontdb/harfrust tree): `<text>` elements
//! are dropped by its converter at this stage, which is honest - glyph
//! conversion belongs to the deferred render stage, while OUR quick-xml
//! extractor still fully parses and gates text content.

use std::collections::BTreeSet;

use serde_json::Value;

use super::extract::extract;
use crate::diff::{structural_diff, RoundTripVerdict, Tolerances};
use crate::error::MediaError;
use crate::extract::ExtractBudgets;

/// The svg diff profile: float epsilon only. No unordered pointers
/// (spec-driven writes are ordered-exact), no ignored subtrees.
#[must_use]
pub fn profile() -> Tolerances {
    Tolerances {
        float_epsilon: 1e-9,
        unordered_at: BTreeSet::new(),
        ignored_at: BTreeSet::new(),
    }
}

/// Verify produced svg bytes against the INDEPENDENT expected tree:
/// strict-parse first (typed `ParseFailure` on structural violation -
/// never a silent pass), then extract + diff under [`profile`].
///
/// # Errors
/// [`MediaError::ParseFailure`] when usvg rejects the bytes or extraction
/// fails; [`MediaError::DiffTooLarge`] when the diff caps trip.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    strict_parse(bytes)?;
    let extracted = extract(bytes, ExtractBudgets::default())?;
    let diff = structural_diff(expected, &extracted.tree, &profile())?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}

fn strict_parse(bytes: &[u8]) -> Result<(), MediaError> {
    let text = std::str::from_utf8(bytes).map_err(|_| MediaError::ParseFailure)?;
    let options = usvg::Options::default();
    usvg::Tree::from_str(text, &options)
        .map(|_| ())
        .map_err(|_| MediaError::ParseFailure)
}
