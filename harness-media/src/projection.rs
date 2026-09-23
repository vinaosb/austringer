//! The deterministic projection seam (plan M10):
//! [`ProjectionProvider{kind, project(bytes, ctx)}`] plus its registry.
//!
//! Deterministic per-format projectors are THIN WRAPS over
//! [`crate::extract::Extracted::text_projection`] and register alongside
//! their format adapters as the adapter waves land (M11+). Until then the
//! registry is honestly EMPTY: every lookup answers the typed
//! [`crate::error::MediaError::NoProjector`] — the seam exists, the
//! fabrications do not (MUST NOT: no stub transcript text, ever).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use harness_core::event::ProjectionKind;

use crate::error::MediaError;
use crate::extract::ExtractBudgets;
use crate::format::FormatId;

/// Everything a deterministic projector needs besides the bytes: the
/// caller-provided safety budgets (a projector that trips them fails the
/// typed [`MediaError::ExtractTooLarge`], never hangs) and an optional
/// filename hint for format disambiguation.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionContext<'a> {
    /// Wall-clock deadline + cumulative size cap for the projection.
    pub budgets: ExtractBudgets,
    /// Original filename, when the caller has one.
    pub name_hint: Option<&'a str>,
}

/// A deterministic projector for one format and one projection kind.
///
/// Implementations are pure functions of `(bytes, ctx)` — no model calls,
/// no I/O, no clocks. Model-mediated projection is NOT this trait: it
/// lives in [`crate::pipeline`] behind a capability check.
pub trait ProjectionProvider: Send + Sync {
    /// The projection kind this provider produces; a registry lookup for
    /// any other kind on this format is the typed `NoProjector`.
    fn kind(&self) -> ProjectionKind;
    /// Projects `bytes` deterministically, or fails typed (corrupt input
    /// is [`MediaError::ParseFailure`] — never a fabricated projection).
    fn project(&self, bytes: &[u8], context: &ProjectionContext<'_>) -> Result<String, MediaError>;
}

/// Wiring bugs the projection registry refuses, each naming the fix
/// (mirrors the executor registry's discipline).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectionWiringError {
    /// Two projectors claimed one format.
    #[error("two projection providers claimed format {format}: one format, one projector - fix the wiring")]
    DuplicateProjector {
        /// The contested format.
        format: FormatId,
    },
}

/// Registry of deterministic per-format projectors. One format, one
/// projector; lookups are by (format, kind) and anything unmatched is
/// the typed [`MediaError::NoProjector`] — never a guess.
#[derive(Default, Clone)]
pub struct ProjectionRegistry {
    providers: BTreeMap<FormatId, Arc<dyn ProjectionProvider>>,
}

impl fmt::Debug for ProjectionRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.providers.keys()).finish()
    }
}

impl ProjectionRegistry {
    /// An empty registry (the shipped state until the adapter waves).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `provider` for `format`; a second registration for the
    /// same format is refused typed and the original survives.
    ///
    /// # Errors
    /// [`ProjectionWiringError::DuplicateProjector`] on contention.
    pub fn register(
        &mut self,
        format: FormatId,
        provider: Arc<dyn ProjectionProvider>,
    ) -> Result<(), ProjectionWiringError> {
        use std::collections::btree_map::Entry;
        match self.providers.entry(format) {
            Entry::Vacant(slot) => {
                slot.insert(provider);
                Ok(())
            }
            Entry::Occupied(slot) => Err(ProjectionWiringError::DuplicateProjector {
                format: *slot.key(),
            }),
        }
    }

    /// Projects `bytes` through the registered provider for
    /// `(format, kind)`.
    ///
    /// # Errors
    /// [`MediaError::NoProjector`] when no provider matches (including a
    /// kind mismatch against the registered provider); the provider's
    /// own typed failure otherwise.
    pub fn project(
        &self,
        format: &FormatId,
        kind: ProjectionKind,
        bytes: &[u8],
        context: &ProjectionContext<'_>,
    ) -> Result<String, MediaError> {
        let provider = self
            .providers
            .get(format)
            .filter(|provider| provider.kind() == kind)
            .ok_or(MediaError::NoProjector)?;
        provider.project(bytes, context)
    }
}
