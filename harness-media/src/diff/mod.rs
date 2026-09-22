//! # Structural diff engine + round-trip verifier (plan M5)
//!
//! Format-agnostic structural comparison over CANONICAL `serde_json`
//! trees (sorted object keys - serde_json's default `BTreeMap` backing;
//! the `preserve_order` feature is banned workspace-wide, so a non-canonical
//! key order is unrepresentable). This is a STRUCTURAL diff, never a
//! textual/line diff, and it allocates nothing unbounded: the walk stops at
//! the plan caps ([`MAX_DIFF_DEPTH`] 64, [`MAX_DIFF_NODES`] 100_000 compared
//! nodes) with the typed [`MediaError::DiffTooLarge`].
//!
//! ## Round-trip semantics
//!
//! An EMPTY [`StructuralDiff`] is the round-trip PASS verdict: the produced
//! document is semantically equivalent to the expectation. Semantic
//! equivalence is deliberately weaker than byte identity - floats compare
//! under the profile's absolute epsilon (f32->f64 print cycles), and arrays
//! under an `unordered_at` pointer compare as multisets. Everywhere else
//! arrays are ORDERED-EXACT, because spec-driven writes are ordered-exact
//! (parquet single-row-group writes included - M22 aligned); `unordered_at`
//! is the ONLY reordering escape (glTF materials are the motivating case).
//! External-file extracts verify a head sample only - an adapter concern
//! noted here because it shapes how expected trees are scoped.
//!
//! ## Expected-tree provenance (plan amendment - CONTRACT)
//!
//! Expected trees come from an INDEPENDENT spec->tree projection, NEVER
//! from running the adapter's own extractor over its own output. A circular
//! expected tree makes the gate vacuous: the extractor's quirks
//! (watermark keys, reordering, lossy projections) would verify
//! themselves. [`spec_tree`] is the generic M5 projection (canonical
//! `{format, params}` wrap); format-specific projections arrive with the
//! adapters (M11+) and are separate functions from extraction, separately
//! tested.
//!
//! ## Pinned decisions
//!
//! - **Non-finite floats**: serde_json cannot hold NaN/inf -
//!   `Number::from_f64` returns `None` and `Value::from` maps them to
//!   `Null`. Canonical trees therefore never carry non-finite numbers; a
//!   writer emitting one lands as `Null` and diffs as a plain type change
//!   at that pointer.
//! - **Integer/float meeting point**: all numbers compare in f64 space
//!   after an exact-representation shortcut, so `1` == `1.0` at any
//!   epsilon and integers beyond 2^53 compare as their f64 neighbors.
//! - **null vs missing**: `null` is a PRESENT node; expected-`null` vs
//!   actual-missing is a Removal (and the mirror an Addition), never
//!   equality.
//! - **Unordered leftovers**: unmatched elements pair up in sorted
//!   canonical order and compare under the ACTUAL element's index, so a
//!   mutated element reports a sub-pointer, not a whole-array change.
//! - **One-sided subtrees** (added/removed) are reported whole without
//!   traversal: the caps bound comparison work; storage stays bounded by
//!   the caller's input.

mod types;
mod walk;

pub use types::{Addition, Change, Removal, StructuralDiff, Tolerances, MAX_DIFF_DEPTH, MAX_DIFF_NODES};

use serde_json::Value;

use crate::error::MediaError;
use crate::extract::Extracted;
use crate::generate::GenerationSpec;

/// Compare two canonical trees under a tolerance profile. Every difference
/// lands in the report with its exact RFC 6901 pointer; an empty diff means
/// semantic equivalence. Tripping either cap is the typed
/// [`MediaError::DiffTooLarge`].
pub fn structural_diff(
    expected: &Value,
    actual: &Value,
    tolerances: &Tolerances,
) -> Result<StructuralDiff, MediaError> {
    walk::run(expected, actual, tolerances)
}

/// The independent generic spec->tree projection (see the provenance
/// contract above): canonical `{format, params}`. Deterministic, sorted-key
/// canonical by construction; format-specific projections supersede it per
/// format as the adapters land.
#[must_use]
pub fn spec_tree(spec: &GenerationSpec) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("format".into(), Value::String(spec.format.to_string()));
    map.insert("params".into(), spec.params.clone());
    Value::Object(map)
}

/// Round-trip verdict: empty diff = [`RoundTripVerdict::Pass`].
#[derive(Debug, Clone, PartialEq)]
pub enum RoundTripVerdict {
    /// The produced bytes are semantically equivalent to the expected tree
    /// under the profile.
    Pass,
    /// The differences, with exact pointers.
    Fail(StructuralDiff),
}

impl RoundTripVerdict {
    /// True when the round trip passed.
    #[must_use]
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }
}

/// Composition helper for the per-format acceptance gate: run the format's
/// extractor over the produced bytes, diff its canonical tree against the
/// INDEPENDENT expected tree, and translate the result into a verdict.
/// `extract_fn` owns its [`crate::extract::ExtractBudgets`] by closure; its
/// typed errors (parse failures, budget trips) propagate - never a silent
/// pass.
pub fn verify_roundtrip<F>(
    expected_tree: &Value,
    produced_bytes: &[u8],
    extract_fn: F,
    profile: &Tolerances,
) -> Result<RoundTripVerdict, MediaError>
where
    F: FnOnce(&[u8]) -> Result<Extracted, MediaError>,
{
    let extracted = extract_fn(produced_bytes)?;
    let diff = structural_diff(expected_tree, &extracted.tree, profile)?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}
