//! Multi-client permission mediation (Qwen §18 pattern): every ask
//! broadcasts to all attached clients; exactly one resolution ever lands
//! (answer XOR cancel XOR timeout — the N1 invariant); four policies
//! decide whose answer counts. Adapts onto harness-core's
//! [`harness_core::inspectors::store::AskPolicy`] seam.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{broadcast, oneshot};

use harness_core::inspectors::store::{AskPolicy, AskRequest, AskResolution, GrantScope};

use crate::protocol::{Answer, AskPayload, Resolution, ServerMsg};

/// Which attached clients may resolve an ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediationPolicy {
    /// First valid answer from any client resolves (crush pattern: one
    /// answer applies to all).
    FirstResponder,
    /// Only the designated client's answer counts.
    Designated {
        /// The client allowed to answer.
        client_id: String,
    },
    /// `quorum` agreeing allows resolve; any deny fails closed; deadline
    /// denies.
    ConsensusQuorum {
        /// How many allows are required.
        quorum: usize,
    },
    /// Only the local owner may answer; remote clients observe the ask
    /// but can never grant permissions.
    LocalOnly {
        /// The owning (local) client id.
        client_id: String,
    },
}

/// Mediation-layer failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediationError {
    /// No such ask (unknown, resolved-and-reaped, or garbage id).
    #[error("unknown ask {0}")]
    UnknownAsk(u64),
}

struct PendingAsk {
    votes: Vec<(String, Answer)>,
    resolution: Option<Resolution>,
    waiter: Option<oneshot::Sender<Resolution>>,
}

#[derive(Default)]
struct MediatorState {
    next_ask_id: u64,
    asks: HashMap<u64, PendingAsk>,
}

/// Shared permission mediator. Clones share state; every transport
/// forwards answers/cancels here, and every ask resolves at most once.
pub struct PermissionMediator {
    state: Mutex<MediatorState>,
    policy: MediationPolicy,
    ask_timeout: Duration,
    control: broadcast::Sender<ServerMsg>,
}

impl PermissionMediator {
    /// New mediator with the given policy, synchronous ask deadline, and
    /// the control bus asks are broadcast on (the same bus transports
    /// push [`ServerMsg::PermissionAsk`] / [`ServerMsg::PermissionResolved`]).
    #[must_use]
    pub fn new(
        policy: MediationPolicy,
        ask_timeout: Duration,
        control: broadcast::Sender<ServerMsg>,
    ) -> Self {
        Self {
            state: Mutex::new(MediatorState::default()),
            policy,
            ask_timeout,
            control,
        }
    }

    /// Subscribe to the control bus (asks + resolutions).
    pub fn subscribe(&self) -> broadcast::Receiver<ServerMsg> {
        self.control.subscribe()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, MediatorState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn resolve_once(
        &self,
        state: &mut MediatorState,
        ask_id: u64,
        resolution: Resolution,
    ) -> Option<Resolution> {
        let ask = state.asks.get_mut(&ask_id)?;
        if ask.resolution.is_some() {
            return None;
        }
        ask.resolution = Some(resolution.clone());
        if let Some(waiter) = ask.waiter.take() {
            let _ = waiter.send(resolution.clone());
        }
        let _ = self.control.send(ServerMsg::PermissionResolved {
            ask_id,
            resolution: resolution.clone(),
        });
        state.asks.remove(&ask_id);
        Some(resolution)
    }

    fn vote_counts(policy: &MediationPolicy, client: &str) -> bool {
        match policy {
            MediationPolicy::FirstResponder | MediationPolicy::ConsensusQuorum { .. } => true,
            MediationPolicy::Designated { client_id }
            | MediationPolicy::LocalOnly { client_id } => client_id == client,
        }
    }

    /// Record one client's answer. Returns the resolution when this call
    /// produced it (exactly once per ask), `Ok(None)` when the vote was
    /// recorded (or ignored per policy) without resolving, and
    /// [`MediationError::UnknownAsk`] for garbage ids.
    pub fn answer(
        &self,
        ask_id: u64,
        client_id: &str,
        answer: Answer,
    ) -> Result<Option<Resolution>, MediationError> {
        let resolution = {
            let mut state = self.lock();
            let ask = state
                .asks
                .get_mut(&ask_id)
                .ok_or(MediationError::UnknownAsk(ask_id))?;
            ask.votes.push((client_id.to_owned(), answer));
            if !Self::vote_counts(&self.policy, client_id) {
                return Ok(None);
            }
            match (&self.policy, answer) {
                (
                    MediationPolicy::ConsensusQuorum { quorum },
                    Answer::AllowOnce | Answer::AllowSession,
                ) => {
                    let allows = ask
                        .votes
                        .iter()
                        .filter(|(_, a)| !matches!(a, Answer::Deny))
                        .count();
                    (allows >= *quorum).then(|| Resolution::Allowed {
                        by: client_id.to_owned(),
                    })
                }
                (_, Answer::Deny) => Some(Resolution::Denied {
                    by: client_id.to_owned(),
                }),
                (_, Answer::AllowOnce | Answer::AllowSession) => Some(Resolution::Allowed {
                    by: client_id.to_owned(),
                }),
            }
        };
        let mut state = self.lock();
        match resolution {
            Some(res) => Ok(self.resolve_once(&mut state, ask_id, res)),
            None => Ok(None),
        }
    }

    /// Cancel-vote sentinel: any attached client withdraws a pending
    /// ask. Cancelling an already-resolved ask is a no-op (N1).
    pub fn cancel_vote(
        &self,
        ask_id: u64,
        client_id: &str,
    ) -> Result<Option<Resolution>, MediationError> {
        let mut state = self.lock();
        if !state.asks.contains_key(&ask_id) {
            return Err(MediationError::UnknownAsk(ask_id));
        }
        Ok(self.resolve_once(
            &mut state,
            ask_id,
            Resolution::Cancelled {
                by: client_id.to_owned(),
            },
        ))
    }

    /// Mediate one [`AskRequest`]: broadcast, wait up to the synchronous
    /// deadline, resolve exactly once (timeout resolves deny-side).
    pub async fn mediate(&self, request: AskRequest) -> Resolution {
        let (ask_id, payload, rx) = {
            let mut state = self.lock();
            let ask_id = state.next_ask_id.saturating_add(1);
            state.next_ask_id = ask_id;
            let (tx, rx) = oneshot::channel();
            state.asks.insert(
                ask_id,
                PendingAsk {
                    votes: Vec::new(),
                    resolution: None,
                    waiter: Some(tx),
                },
            );
            let payload = AskPayload {
                ask_id,
                tool: request.tool,
                target: request.target,
                reason: request.reason,
                deadline_ms: u64::try_from(self.ask_timeout.as_millis()).unwrap_or(u64::MAX),
            };
            (ask_id, payload, rx)
        };
        let _ = self.control.send(ServerMsg::PermissionAsk { ask: payload });
        match tokio::time::timeout(self.ask_timeout, rx).await {
            Ok(Ok(resolution)) => resolution,
            Ok(Err(_)) => Resolution::Timeout,
            Err(_) => {
                // Deadline won the race; if an answer landed concurrently
                // the N1 gate already resolved and `resolve_once` yields
                // the winner's value instead of Timeout.
                let mut state = self.lock();
                self.resolve_once(&mut state, ask_id, Resolution::Timeout)
                    .unwrap_or(Resolution::Timeout)
            }
        }
    }
}

/// Maps a mediation resolution onto the harness-core ask vocabulary.
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

/// Convenience constructor for the shared control bus.
#[must_use]
pub fn control_bus(capacity: usize) -> broadcast::Sender<ServerMsg> {
    broadcast::channel(capacity.max(1)).0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(tool: &str) -> AskRequest {
        AskRequest {
            tool: tool.into(),
            target: Some("src/main.rs".into()),
            reason: "edit file".into(),
        }
    }

    async fn begin(
        mediator: &PermissionMediator,
        _tool: &str,
    ) -> (u64, oneshot::Receiver<Resolution>) {
        let mut state = mediator.lock();
        let ask_id = state.next_ask_id.saturating_add(1);
        state.next_ask_id = ask_id;
        let (tx, rx) = oneshot::channel();
        state.asks.insert(
            ask_id,
            PendingAsk {
                votes: Vec::new(),
                resolution: None,
                waiter: Some(tx),
            },
        );
        (ask_id, rx)
    }

    fn mediator(policy: MediationPolicy) -> Arc<PermissionMediator> {
        Arc::new(PermissionMediator::new(
            policy,
            Duration::from_secs(30),
            control_bus(16),
        ))
    }

    #[tokio::test]
    async fn first_answer_resolves_exactly_once() {
        let m = mediator(MediationPolicy::FirstResponder);
        let (ask_id, rx) = begin(&m, "write").await;
        let resolved = m.answer(ask_id, "tui", Answer::AllowOnce).unwrap_or(None);
        assert_eq!(resolved, Some(Resolution::Allowed { by: "tui".into() }));
        let second = m.answer(ask_id, "web", Answer::Deny).unwrap_or(None);
        assert_eq!(second, None);
        assert!(matches!(
            m.answer(ask_id, "web", Answer::Deny),
            Err(MediationError::UnknownAsk(_))
        ));
        let via_waiter = rx.await.unwrap_or(Resolution::Timeout);
        assert_eq!(via_waiter, Resolution::Allowed { by: "tui".into() });
    }

    #[tokio::test]
    async fn exactly_one_resolution_broadcast_lands() {
        let m = mediator(MediationPolicy::FirstResponder);
        let mut watch = m.subscribe();
        let (ask_id, _rx) = begin(&m, "write").await;
        m.answer(ask_id, "a", Answer::AllowOnce).unwrap_or(None);
        m.answer(ask_id, "b", Answer::Deny).unwrap_or(None);
        m.cancel_vote(ask_id, "c").unwrap_or(None);
        let mut resolved_count = 0;
        while let Ok(msg) = watch.try_recv() {
            if matches!(msg, ServerMsg::PermissionResolved { .. }) {
                resolved_count += 1;
            }
        }
        assert_eq!(resolved_count, 1);
    }

    #[tokio::test]
    async fn cancel_withdraws_pending_ask() {
        let m = mediator(MediationPolicy::FirstResponder);
        let (ask_id, rx) = begin(&m, "bash").await;
        let res = m.cancel_vote(ask_id, "web").unwrap_or(None);
        assert_eq!(res, Some(Resolution::Cancelled { by: "web".into() }));
        assert_eq!(
            rx.await.unwrap_or(Resolution::Timeout),
            Resolution::Cancelled { by: "web".into() }
        );
    }

    #[tokio::test]
    async fn designated_ignores_non_designated_answers() {
        let m = mediator(MediationPolicy::Designated {
            client_id: "tui".into(),
        });
        let (ask_id, rx) = begin(&m, "edit").await;
        assert_eq!(
            m.answer(ask_id, "web", Answer::AllowOnce).unwrap_or(None),
            None
        );
        let res = m.answer(ask_id, "tui", Answer::AllowOnce).unwrap_or(None);
        assert_eq!(res, Some(Resolution::Allowed { by: "tui".into() }));
        assert_eq!(
            rx.await.unwrap_or(Resolution::Timeout),
            Resolution::Allowed { by: "tui".into() }
        );
    }

    #[tokio::test]
    async fn local_only_blocks_remote_grants() {
        let m = mediator(MediationPolicy::LocalOnly {
            client_id: "local".into(),
        });
        let (ask_id, _rx) = begin(&m, "edit").await;
        assert_eq!(
            m.answer(ask_id, "remote-attacker", Answer::AllowSession)
                .unwrap_or(None),
            None
        );
        let res = m
            .answer(ask_id, "local", Answer::AllowSession)
            .unwrap_or(None);
        assert_eq!(res, Some(Resolution::Allowed { by: "local".into() }));
    }

    #[tokio::test]
    async fn consensus_requires_quorum_of_allows() {
        let m = mediator(MediationPolicy::ConsensusQuorum { quorum: 3 });
        let (ask_id, rx) = begin(&m, "edit").await;
        assert_eq!(
            m.answer(ask_id, "a", Answer::AllowOnce).unwrap_or(None),
            None
        );
        assert_eq!(
            m.answer(ask_id, "b", Answer::AllowOnce).unwrap_or(None),
            None
        );
        let res = m.answer(ask_id, "c", Answer::AllowOnce).unwrap_or(None);
        assert_eq!(res, Some(Resolution::Allowed { by: "c".into() }));
        assert_eq!(
            rx.await.unwrap_or(Resolution::Timeout),
            Resolution::Allowed { by: "c".into() }
        );
    }

    #[tokio::test]
    async fn consensus_any_deny_fails_closed() {
        let m = mediator(MediationPolicy::ConsensusQuorum { quorum: 3 });
        let (ask_id, rx) = begin(&m, "edit").await;
        m.answer(ask_id, "a", Answer::AllowOnce).unwrap_or(None);
        let res = m.answer(ask_id, "b", Answer::Deny).unwrap_or(None);
        assert_eq!(res, Some(Resolution::Denied { by: "b".into() }));
        assert_eq!(
            rx.await.unwrap_or(Resolution::Timeout),
            Resolution::Denied { by: "b".into() }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn synchronous_timeout_resolves_once() {
        let m = Arc::new(PermissionMediator::new(
            MediationPolicy::FirstResponder,
            Duration::from_secs(5),
            control_bus(16),
        ));
        let handle = tokio::spawn({
            let m = Arc::clone(&m);
            async move { m.mediate(request("slow")).await }
        });
        tokio::time::advance(Duration::from_secs(6)).await;
        let res = handle.await.unwrap_or(Resolution::Timeout);
        assert_eq!(res, Resolution::Timeout);
        let state = m.lock();
        assert!(state.asks.is_empty());
    }

    #[tokio::test]
    async fn ask_policy_adapter_maps_resolutions() {
        use crate::protocol::ServerMsg;
        let m = mediator(MediationPolicy::FirstResponder);
        let mut watch = m.subscribe();
        let mediating = tokio::spawn({
            let m = Arc::clone(&m);
            async move { m.mediate(request("edit")).await }
        });
        let ask_id = loop {
            match watch.try_recv() {
                Ok(ServerMsg::PermissionAsk { ask }) => break ask.ask_id,
                _ => tokio::task::yield_now().await,
            }
        };
        assert_eq!(
            m.answer(ask_id, "web", Answer::AllowSession)
                .unwrap_or(None),
            Some(Resolution::Allowed { by: "web".into() })
        );
        let res = mediating.await.unwrap_or(Resolution::Timeout);
        assert_eq!(res, Resolution::Allowed { by: "web".into() });
    }

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

    #[tokio::test]
    async fn unknown_ask_is_typed_error() {
        let m = mediator(MediationPolicy::FirstResponder);
        assert_eq!(
            m.answer(999, "x", Answer::AllowOnce),
            Err(MediationError::UnknownAsk(999))
        );
    }
}
