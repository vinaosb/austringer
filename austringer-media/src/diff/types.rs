//! Public types of the structural diff engine: tolerance profile, diff
//! entries, the report container, and the plan-M5 caps.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Depth cap: trees nested deeper than 64 comparison levels trip the typed
/// [`crate::error::MediaError::DiffTooLarge`] (plan M5). The cap bounds the
/// recursive walk, so the engine's stack use is constant-bounded.
pub const MAX_DIFF_DEPTH: usize = 64;

/// Node cap: more than 100_000 compared nodes (both trees combined) trips
/// the typed [`crate::error::MediaError::DiffTooLarge`] (plan M5). The walk
/// aborts on the first node past the cap - no unbounded allocation, no
/// unbounded work.
pub const MAX_DIFF_NODES: usize = 100_000;

/// Per-format comparison tolerances (the "profile" a round-trip gate runs
/// under). Default is EXACT: zero epsilon, no reordering escapes, no
/// ignored subtrees - tolerance is a per-format profile decision recorded
/// in the M25 tolerance table, never ambient.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Tolerances {
    /// Absolute float tolerance: two numbers within `|a - b| <= epsilon`
    /// compare equal (f32->f64 print cycles; integers meet f64 space, so
    /// `1` and `1.0` are equal at any epsilon, and integers beyond 2^53
    /// compare as their f64 neighbors - documented, deliberate).
    pub float_epsilon: f64,
    /// RFC 6901 pointers of arrays compared ORDER-INSENSITIVELY (multiset
    /// of canonical element forms). This is the ONLY reordering escape:
    /// arrays elsewhere are ordered-exact, because spec-driven writes are
    /// ordered-exact (glTF materials are the use case; parquet
    /// single-row-group writes stay ordered, M22).
    pub unordered_at: BTreeSet<String>,
    /// RFC 6901 pointers of subtrees skipped entirely (volatile clocks,
    /// opaque blobs). Exact-pointer match; the root pointer `""` skips
    /// everything (a vacuous pass the caller owns).
    pub ignored_at: BTreeSet<String>,
}

/// A node present only in the actual tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Addition {
    /// RFC 6901 pointer of the added node (root-relative).
    pub path: String,
    /// The whole added subtree, as it appears in the actual tree.
    pub actual: Value,
}

/// A node present only in the expected tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Removal {
    /// RFC 6901 pointer of the removed node (root-relative).
    pub path: String,
    /// The whole removed subtree, as it appeared in the expected tree.
    pub expected: Value,
}

/// A node present in both trees at the same pointer with a different value
/// or a different JSON kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    /// RFC 6901 pointer of the changed node (root-relative).
    pub path: String,
    /// The expected value (whole subtree).
    pub expected: Value,
    /// The actual value (whole subtree).
    pub actual: Value,
}

/// The full structural difference between two canonical trees. EMPTY means
/// round-trip PASS - semantic equivalence, never byte identity.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StructuralDiff {
    /// Nodes present only in the actual tree.
    pub added: Vec<Addition>,
    /// Nodes present only in the expected tree.
    pub removed: Vec<Removal>,
    /// Nodes present in both trees with different values or kinds.
    pub changed: Vec<Change>,
}

impl StructuralDiff {
    /// True when the trees are semantically equivalent under the given
    /// tolerances - the round-trip PASS verdict.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

impl fmt::Display for StructuralDiff {
    /// One deterministic line per entry (added, then removed, then changed;
    /// within a group, canonical traversal order), values in compact JSON.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return formatter.write_str("no differences\n");
        }
        for entry in &self.added {
            writeln!(formatter, "added   {}: {}", entry.path, entry.actual)?;
        }
        for entry in &self.removed {
            writeln!(formatter, "removed {}: {}", entry.path, entry.expected)?;
        }
        for entry in &self.changed {
            writeln!(
                formatter,
                "changed {}: expected {}, actual {}",
                entry.path, entry.expected, entry.actual
            )?;
        }
        Ok(())
    }
}
