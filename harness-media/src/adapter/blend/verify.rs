//! The blend round-trip gate (plan M16): extract the produced bytes
//! through the same subprocess dump lane and diff against the
//! INDEPENDENT expected tree under the blend tolerance profile
//! (unordered-by-name objects, eps 1e-5). Byte identity is explicitly
//! NOT the gate - .blend is structurally reproducible only (digest
//! 3.3).

use std::collections::BTreeSet;

use serde_json::Value;

use super::extract::extract;
use crate::diff::{structural_diff, RoundTripVerdict, Tolerances};
use crate::error::MediaError;
use crate::executor::{OpOutput, Produced};
use crate::extract::ExtractBudgets;
use crate::tool::ToolExecutor;

/// The blend diff profile: floats at eps 1e-5, objects matched as an
/// unordered multiset by name-bearing content (scene graph order is not
/// a contract).
#[must_use]
pub fn profile() -> Tolerances {
    Tolerances {
        float_epsilon: 1e-5,
        unordered_at: BTreeSet::from(["/objects".to_owned()]),
        ignored_at: BTreeSet::new(),
    }
}

/// Verify produced .blend bytes against the independent expected tree
/// (from [`BlendSpec::canonical_tree`](super::BlendSpec)), through the
/// blender-backed extract.
///
/// # Errors
/// The routed extract/tool failures, or
/// [`MediaError::DiffTooLarge`] when the diff caps trip. A non-empty
/// diff is the [`RoundTripVerdict::Fail`] verdict, never an error.
pub fn verify(
    tools: &ToolExecutor,
    expected: &Value,
    bytes: &[u8],
) -> Result<Produced, MediaError> {
    let produced = extract(tools, bytes, ExtractBudgets::default())?;
    let (tree, producer) = match produced.inner() {
        OpOutput::Extract(extracted) => (extracted.tree.clone(), produced.producer().clone()),
        _ => return Err(MediaError::ParseFailure),
    };
    let diff = structural_diff(expected, &tree, &profile())?;
    let verdict = if diff.is_empty() {
        RoundTripVerdict::Pass
    } else {
        RoundTripVerdict::Fail(diff)
    };
    Ok(Produced::new(OpOutput::Verify(verdict), producer))
}
