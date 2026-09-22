//! Per-model media input capabilities (plan M7): the capability flag
//! type, the restriction-only override errors, and the [`Catalog`]
//! lookup/override surface. The types are re-exported by
//! [`super::catalog`] so the public API path stays
//! `harness_intel::catalog::...` — this module is the LOC-ceiling split
//! home for the M7 surface, not a new import path callers must learn.
//!
//! Flags are fail-closed like the trap table: `true` ONLY when the
//! entry's cited source row documents support (see the sidecar table
//! in [`super::seed`]) — absent or unknown support is `false`, so a
//! model never receives media it cannot use. Flags are PER-MODEL
//! catalog facts, never inferred from [`super::catalog::Family`].

use serde::{Deserialize, Serialize};

use super::catalog::{Catalog, UnknownModel};

/// Per-model media input capabilities (plan M7). Fail-closed like the
/// trap table: a flag is `true` ONLY when the entry's cited source row
/// documents support (see the sidecar table in [`super::seed`]) —
/// absent or unknown support is `false`, so a model never receives
/// media it cannot use. Flags are PER-MODEL catalog facts, never
/// inferred from [`super::catalog::Family`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Accepts image input.
    pub vision: bool,
    /// Accepts audio input.
    pub audio_input: bool,
    /// Accepts video input.
    pub video_input: bool,
}

/// A runtime override attempted to RAISE a capability flag above the
/// entry's catalog-owned value. Capability truth comes from cited
/// source rows; overrides may only restrict (lower), never expand.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error(
    "cannot raise media capabilities on {model}: flags are catalog-owned \
     source-cited data (requested {requested:?} over {current:?}) — \
     overrides may only lower them"
)]
pub struct CapabilityRaiseRefused {
    /// The model whose flags were targeted.
    pub model: String,
    /// The offending requested flags.
    pub requested: Capabilities,
    /// The catalog-owned flags at refusal time.
    pub current: Capabilities,
}

/// Everything a runtime capability override can fail with (plan M7).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OverrideError {
    /// Unknown model.
    #[error(transparent)]
    UnknownModel(#[from] UnknownModel),
    /// A raise was attempted (catalog-owned, source-cited data).
    #[error(transparent)]
    Raise(#[from] CapabilityRaiseRefused),
}

impl Catalog {
    /// The model's media input capabilities (plan M7): `true` only where
    /// the entry's cited source row documents support. Unknown models
    /// are typed errors; the answer is the entry's own PER-MODEL data —
    /// there is deliberately no family-keyed lookup path.
    pub fn capabilities(&self, model: &str) -> Result<Capabilities, UnknownModel> {
        Ok(self.entry(model)?.capabilities)
    }

    /// Restriction-only runtime capability override: `requested` may
    /// only LOWER flags below the entry's current catalog-owned values —
    /// any `false -> true` transition (including restoring a previously
    /// lowered flag) is refused with a typed [`CapabilityRaiseRefused`],
    /// leaving the entry untouched. Unknown models are typed errors.
    pub fn override_capabilities(
        &mut self,
        model: &str,
        requested: Capabilities,
    ) -> Result<(), OverrideError> {
        let entry = self.entries.get_mut(model).ok_or_else(|| UnknownModel {
            model: model.to_string(),
        })?;
        let raises = (requested.vision && !entry.capabilities.vision)
            || (requested.audio_input && !entry.capabilities.audio_input)
            || (requested.video_input && !entry.capabilities.video_input);
        if raises {
            return Err(CapabilityRaiseRefused {
                model: model.to_string(),
                requested,
                current: entry.capabilities,
            }
            .into());
        }
        entry.capabilities = requested;
        Ok(())
    }
}
