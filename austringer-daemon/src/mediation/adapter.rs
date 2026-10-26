//! The adapter onto austringer-core's ask vocabulary: mediation
//! resolutions mapped to [`AskResolution`] grants and the
//! [`AskPolicy`] stage piece that routes inspector asks through the
//! mediator.

use std::sync::Arc;

use async_trait::async_trait;

use austringer_core::inspectors::store::{AskPolicy, AskRequest, AskResolution, GrantScope};

use super::PermissionMediator;
use crate::protocol::{Answer, Resolution};

/// Maps a mediation resolution onto the austringer-core ask vocabulary.
#[must_use]
pub fn to_ask_resolution(resolution: &Resolution, answer_hint: Answer) -> AskResolution {
    match resolution {
        Resolution::Allowed { .. } => match answer_hint {
            Answer::AllowSession => AskResolution::GrantAlways(GrantScope::Session),
            _ => AskResolution::GrantOnce,
        },
        Resolution::Denied { .. } | Resolution::Cancelled { .. } | Resolution::Timeout => {
            AskResolution::Refuse
        }
    }
}

/// The [`AskPolicy`] adapter: plug into
/// `InspectorChain::with_ask_policy` to route asks through mediation.
pub struct MediatedAskPolicy {
    mediator: Arc<PermissionMediator>,
}

impl MediatedAskPolicy {
    /// Wrap a shared mediator.
    #[must_use]
    pub fn new(mediator: Arc<PermissionMediator>) -> Self {
        Self { mediator }
    }
}

#[async_trait]
impl AskPolicy for MediatedAskPolicy {
    async fn resolve(&mut self, request: AskRequest) -> AskResolution {
        let resolution = self.mediator.mediate(request).await;
        let hint = match &resolution {
            Resolution::Allowed { .. } => Answer::AllowOnce,
            _ => Answer::Deny,
        };
        to_ask_resolution(&resolution, hint)
    }
}

#[cfg(test)]
mod tests {
    use super::{MediatedAskPolicy, to_ask_resolution};
    use crate::protocol::{Answer, Resolution};
    use austringer_core::inspectors::store::{AskResolution, GrantScope};

    #[test]
    fn resolution_mapping_covers_ask_vocabulary() {
        let allowed = Resolution::Allowed { by: "x".into() };
        assert_eq!(
            to_ask_resolution(&allowed, Answer::AllowSession),
            AskResolution::GrantAlways(GrantScope::Session)
        );
        assert_eq!(
            to_ask_resolution(&allowed, Answer::AllowOnce),
            AskResolution::GrantOnce
        );
        assert_eq!(
            to_ask_resolution(&Resolution::Timeout, Answer::Deny),
            AskResolution::Refuse
        );
        assert_eq!(
            to_ask_resolution(&Resolution::Cancelled { by: "y".into() }, Answer::Deny),
            AskResolution::Refuse
        );
        assert_eq!(
            to_ask_resolution(&Resolution::Denied { by: "y".into() }, Answer::Deny),
            AskResolution::Refuse
        );
    }

    #[test]
    fn adapter_constructs_over_a_shared_mediator() {
        let mediator =
            super::super::testkit::mediator(super::super::MediationPolicy::FirstResponder);
        let _policy: MediatedAskPolicy = MediatedAskPolicy::new(mediator);
    }
}
