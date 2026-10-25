//! The autonomy ask wrapper (plan todo 8): decorates any
//! [`AskPolicy`] with the two store-related matrix behaviors.
//!
//! - a refused ask records a DENIAL in the permission store (the `n`
//!   half of y/a/n semantics) feeding repetition feedback — a
//!   persistence failure here is non-fatal feedback loss, never a
//!   safety downgrade;
//! - when the [`Capability::GrantMemory`] toggle is off, a
//!   `GrantAlways` answer is downgraded to `GrantOnce` BEFORE the chain
//!   sees it, so nothing is persisted (the chain's own record step
//!   never fires).

use async_trait::async_trait;

use crate::autonomy::matrix::{AutonomyHandle, Capability};
use crate::inspectors::store::{AskPolicy, AskRequest, AskResolution, PermissionStore};
use std::sync::Arc;

/// The wrapping policy.
pub struct AutonomyAskPolicy {
    inner: Box<dyn AskPolicy>,
    store: Arc<dyn PermissionStore>,
    matrix: AutonomyHandle,
}

impl AutonomyAskPolicy {
    /// Wraps `inner` with denial recording and grant-memory semantics.
    #[must_use]
    pub fn new(
        inner: Box<dyn AskPolicy>,
        store: Arc<dyn PermissionStore>,
        matrix: AutonomyHandle,
    ) -> Self {
        Self {
            inner,
            store,
            matrix,
        }
    }
}

#[async_trait]
impl AskPolicy for AutonomyAskPolicy {
    async fn resolve(&mut self, request: AskRequest) -> AskResolution {
        match self.inner.resolve(request.clone()).await {
            AskResolution::Refuse => {
                // The refusal itself is the safety action; the denial
                // counter is feedback — its loss never widens access.
                let _ = self
                    .store
                    .record_denial(&request.tool, request.target.as_deref());
                AskResolution::Refuse
            }
            AskResolution::GrantAlways(_scope)
                if !self
                    .matrix
                    .read(|matrix| matrix.allows(Capability::GrantMemory)) =>
            {
                AskResolution::GrantOnce
            }
            resolution => resolution,
        }
    }
}
