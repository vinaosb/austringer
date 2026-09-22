//! Format descriptors: the per-format capability table and its container.
//!
//! A [`FormatDescriptor`] states, for one [`FormatId`], its identity (mime,
//! extensions) and its per-operation support: every operation is either
//! [`OpSupport::Supported`] or [`OpSupport::Unsupported`] with the honest
//! reason. Unsupported is a typed fact, never a fake. The M2 probe registry
//! fills these tables; the M6 executor dispatches over them. This module is
//! only the contract surface - no format logic lives here.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::MediaError;
use crate::format::FormatId;

/// The four adapter operations a format's contract is stated over.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Identify bytes/name (M2 magic sniffing).
    Probe,
    /// Typed structural extraction + text projection.
    Extract,
    /// Spec-driven generation.
    Generate,
    /// Round-trip structural verification (M5 diff engine).
    Verify,
}

impl fmt::Display for Operation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Probe => "probe",
            Self::Extract => "extract",
            Self::Generate => "generate",
            Self::Verify => "verify",
        };
        formatter.write_str(name)
    }
}

/// Whether a format supports one operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpSupport {
    /// The operation is implemented by the format's adapter.
    Supported,
    /// The operation is not implemented; `reason` is the honest why and is
    /// surfaced verbatim in errors and the honesty matrix.
    Unsupported {
        /// Human-readable, non-empty justification.
        reason: String,
    },
}

/// Static per-format contract: identity plus the capability table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatDescriptor {
    /// The format this descriptor speaks for.
    pub id: FormatId,
    /// Canonical MIME type (`image/avif`, `model/gltf+json`, ...).
    pub mime: String,
    /// File extensions (lowercase, dot-free), e.g. `["png"]`.
    pub extensions: Vec<String>,
    /// Per-operation support. Sorted-key serialized (BTreeMap; the
    /// `preserve_order` serde_json feature must never be enabled).
    pub ops: BTreeMap<Operation, OpSupport>,
}

/// Container of registered format descriptors.
///
/// The registry starts empty and is filled by adapter wiring: M2 registers
/// probe support for the full slate, M6 attaches executor entries. A lookup
/// against a format with no descriptor is the typed
/// [`MediaError::UnsupportedFormat`] - never a guess, never a panic.
#[derive(Debug, Clone, Default)]
pub struct FormatRegistry {
    descriptors: BTreeMap<FormatId, FormatDescriptor>,
}

impl FormatRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a descriptor. Returns `false` (and keeps the existing
    /// entry) when a descriptor is already registered for the format: two
    /// adapters claiming one format is a wiring bug the caller surfaces,
    /// not a condition this container papers over.
    pub fn register(&mut self, descriptor: FormatDescriptor) -> bool {
        use std::collections::btree_map::Entry;
        match self.descriptors.entry(descriptor.id) {
            Entry::Vacant(slot) => {
                slot.insert(descriptor);
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    /// Look up a format's descriptor; typed
    /// [`MediaError::UnsupportedFormat`] when nothing is registered for it.
    pub fn lookup(&self, id: &FormatId) -> Result<&FormatDescriptor, MediaError> {
        self.descriptors.get(id).ok_or(MediaError::UnsupportedFormat)
    }
}
