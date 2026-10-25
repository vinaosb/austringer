//! Versioned tool behaviors and per-session version pinning.
//!
//! Grok Build pattern (research §15): each tool owns a stack of versioned
//! behaviors; a session pins the version set it started with, so upgrading
//! the registry mid-history cannot churn schemas for existing sessions.

use std::collections::BTreeMap;
use std::fmt;

use super::schema::ToolSchema;

/// Monotonic behavior version of a tool contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToolVersion(pub u32);

impl fmt::Display for ToolVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "v{}", self.0)
    }
}

/// Lifecycle of one versioned tool behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lifecycle {
    /// The normal state: pinned sessions use it without notice.
    #[default]
    Active,
    /// Superseded: pinned sessions keep working but see a notice with the
    /// replaced-by pointer.
    Deprecated,
    /// Scheduled for removal: same surfacing as [`Lifecycle::Deprecated`].
    RemovalCandidate,
}

impl fmt::Display for Lifecycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Active => "active",
            Self::Deprecated => "deprecated",
            Self::RemovalCandidate => "a removal candidate",
        };
        formatter.write_str(label)
    }
}

/// One versioned behavior of a tool: the schema the version serializes,
/// its lifecycle, and its deprecation metadata.
#[derive(Debug, Clone)]
pub struct VersionedBehavior {
    /// The behavior's version number.
    pub version: ToolVersion,
    /// The serialized contract for this version.
    pub schema: ToolSchema,
    /// Lifecycle state of this version.
    pub lifecycle: Lifecycle,
    /// When deprecated, the version that replaces this one.
    pub replaced_by: Option<ToolVersion>,
}

/// Surfaced deprecation metadata for a pinned session. Carrying metadata
/// out-of-band is deliberate: it never changes the serialized schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecationNotice {
    /// The tool whose pinned behavior is not [`Lifecycle::Active`].
    pub tool: String,
    /// The pinned version the session runs.
    pub version: ToolVersion,
    /// That version's lifecycle.
    pub lifecycle: Lifecycle,
    /// The version replacing it, when one exists.
    pub replaced_by: Option<ToolVersion>,
}

/// The version set a session started with. Built only by
/// [`super::ToolRegistry::pin_current`]; the registry resolves every
/// render, search, and dispatch against it, per session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PinnedToolSet {
    pub(super) versions: BTreeMap<String, ToolVersion>,
}

impl PinnedToolSet {
    /// Build from a name-to-version map (registry-internal).
    pub(super) fn from_versions(versions: BTreeMap<String, ToolVersion>) -> Self {
        Self { versions }
    }

    /// The pinned version for `tool`, or `None` when the tool was not in
    /// the catalog at pin time.
    #[must_use]
    pub fn version_of(&self, tool: &str) -> Option<ToolVersion> {
        self.versions.get(tool).copied()
    }

    /// The pinned tool names, sorted (BTreeMap order; deterministic).
    pub fn tool_names(&self) -> impl Iterator<Item = &str> {
        self.versions.keys().map(String::as_str)
    }
}
