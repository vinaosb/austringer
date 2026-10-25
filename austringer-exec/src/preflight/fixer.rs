//! The injected fixer seam (plan todo 22): the corrective sub-cycle's
//! policy hook. There is NO LLM here by design — production ships the
//! deterministic [`SurfaceAfterTwo`] default; richer policies are wired
//! by the daemon later.

use austringer_vfs::{ShadowEdit, ShadowHandle};

use super::error::PreFlightFailure;

#[derive(Debug, thiserror::Error)]
pub enum FixerError {
    #[error("fixer policy error: {reason}")]
    Policy { reason: String },
}

/// Deterministic corrective policy: receives the single-slot failure and
/// returns whole-file revisions to apply INSIDE the shadow. Edits anchor
/// on CURRENT shadow content (see `ShadowHandle::revise`). An empty
/// result is a no-op: the pipeline stops re-running provably identical
/// stages and surfaces.
pub trait FixerSeam {
    fn fix(
        &mut self,
        failure: &PreFlightFailure,
        shadow: &ShadowHandle,
    ) -> Result<Vec<ShadowEdit>, FixerError>;
}

/// The default policy: never fix. Keeps the pipeline fully
/// deterministic; the ≤2-iteration cap holds trivially (an empty fix set
/// cannot change the outcome, so re-running is skipped).
pub struct SurfaceAfterTwo;

impl FixerSeam for SurfaceAfterTwo {
    fn fix(
        &mut self,
        _failure: &PreFlightFailure,
        _shadow: &ShadowHandle,
    ) -> Result<Vec<ShadowEdit>, FixerError> {
        Ok(Vec::new())
    }
}
