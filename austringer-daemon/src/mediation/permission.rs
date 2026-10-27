//! The permission and fallback ask lanes: vote recording under the
//! policy gate, exactly-once resolution, and the synchronous mediate
//! deadlines. (The core-seam adapter lives in [`super::adapter`].)

use tokio::sync::oneshot;

use austringer_core::inspectors::store::AskRequest;
use austringer_providers::identity::{FallbackRequest, ModelIdentity};

use super::plan::resolution_of_plan;
use super::policies::vote_counts;
use super::{AskKind, MediationError, MediatorState, PendingAsk, PermissionMediator};
use crate::protocol::{Answer, AskPayload, FallbackAskPayload, Resolution, ServerMsg};

impl PermissionMediator {
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
        // The resolved frame follows the ask's kind so each ask type
        // observes its own vocabulary on the control bus; plan asks
        // resolve through `resolve_plan_once` (their waiter and frame
        // carry PlanOutcome) and can never land here â€” the kind checks
        // at every entry point route them away first.
        let resolved = match &ask.kind {
            AskKind::Permission => ServerMsg::PermissionResolved {
                ask_id,
                resolution: resolution.clone(),
            },
            AskKind::Fallback { current, proposed } => {
                let chosen = match resolution {
                    Resolution::Allowed { .. } => proposed.clone(),
                    _ => current.clone(),
                };
                ServerMsg::FallbackResolved { ask_id, chosen }
            }
            AskKind::Plan(_) => return None,
        };
        let _ = self.control.send(resolved);
        state.asks.remove(&ask_id);
        Some(resolution)
    }

    /// Record one client's answer to a permission ask. Returns the
    /// resolution when this call produced it (exactly once per ask),
    /// `Ok(None)` when the vote was recorded (or ignored per policy)
    /// without resolving, and a typed error for garbage or mismatched
    /// ids.
    pub fn answer(
        &self,
        ask_id: u64,
        client_id: &str,
        answer: Answer,
    ) -> Result<Option<Resolution>, MediationError> {
        {
            let state = self.lock();
            match state.asks.get(&ask_id) {
                None => return Err(MediationError::UnknownAsk(ask_id)),
                Some(ask) if !matches!(ask.kind, AskKind::Permission) => {
                    return Err(MediationError::NotAPermissionAsk(ask_id));
                }
                Some(_) => {}
            }
        }
        self.record_answer(ask_id, client_id, answer)
    }

    /// Record one connection's answer to a fallback ask. `chosen` maps
    /// onto the permission vocabulary under the ask's own proposal:
    /// an exact match with `proposed` is an allow, anything else is a
    /// deny (the current model is kept â€” ask, never substitute). The
    /// same policy gate and exactly-once resolution as permission
    /// answers apply.
    pub fn answer_fallback(
        &self,
        ask_id: u64,
        client_id: &str,
        chosen: ModelIdentity,
    ) -> Result<Option<Resolution>, MediationError> {
        let answer = {
            let state = self.lock();
            let Some(ask) = state.asks.get(&ask_id) else {
                return Err(MediationError::UnknownAsk(ask_id));
            };
            let AskKind::Fallback { proposed, .. } = &ask.kind else {
                return Err(MediationError::NotAFallbackAsk(ask_id));
            };
            if chosen == *proposed {
                Answer::AllowOnce
            } else {
                Answer::Deny
            }
        };
        self.record_answer(ask_id, client_id, answer)
    }

    /// Shared vote/resolve tail of [`Self::answer`] and
    /// [`Self::answer_fallback`] (kind checks happen before entry).
    fn record_answer(
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
            if !vote_counts(&self.policy, client_id) {
                return Ok(None);
            }
            match (&self.policy, answer) {
                (
                    super::MediationPolicy::ConsensusQuorum { quorum },
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
    /// Cancelling a plan ask withdraws it WITHOUT deciding the plan
    /// (the plan stays pending; only approval re-arms writes).
    pub fn cancel_vote(
        &self,
        ask_id: u64,
        client_id: &str,
    ) -> Result<Option<Resolution>, MediationError> {
        let mut state = self.lock();
        if !state.asks.contains_key(&ask_id) {
            return Err(MediationError::UnknownAsk(ask_id));
        }
        if state
            .asks
            .get(&ask_id)
            .is_some_and(|ask| matches!(ask.kind, AskKind::Plan(_)))
        {
            return Ok(self
                .resolve_plan_once(
                    &mut state,
                    ask_id,
                    crate::protocol::PlanOutcome::Cancelled {
                        by: client_id.to_owned(),
                    },
                )
                .as_ref()
                .map(resolution_of_plan));
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
                    kind: AskKind::Permission,
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

    /// Mediate one model-fallback ask: broadcast a
    /// [`ServerMsg::FallbackAsk`], wait up to the synchronous deadline,
    /// resolve exactly once under the same policy as permission asks.
    /// The returned identity is what the NEXT request targets: the
    /// proposed hop when acknowledged, the current model otherwise â€”
    /// deny, cancel, and timeout all retry the SAME model (the T9
    /// identity contract: no silent substitution exists).
    pub async fn mediate_fallback(&self, request: FallbackRequest) -> ModelIdentity {
        let FallbackRequest { from, to, reason } = request;
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
                    kind: AskKind::Fallback {
                        current: from.clone(),
                        proposed: to.clone(),
                    },
                },
            );
            let payload = FallbackAskPayload {
                ask_id,
                current: from.clone(),
                proposed: to.clone(),
                reason,
                deadline_ms: u64::try_from(self.ask_timeout.as_millis()).unwrap_or(u64::MAX),
            };
            (ask_id, payload, rx)
        };
        let _ = self.control.send(ServerMsg::FallbackAsk { ask: payload });
        let resolution = match tokio::time::timeout(self.ask_timeout, rx).await {
            Ok(Ok(resolution)) => resolution,
            Ok(Err(_)) => Resolution::Timeout,
            Err(_) => {
                let mut state = self.lock();
                self.resolve_once(&mut state, ask_id, Resolution::Timeout)
                    .unwrap_or(Resolution::Timeout)
            }
        };
        match resolution {
            Resolution::Allowed { .. } => to,
            _ => from,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{begin_permission, mediator};
    use super::super::{AskKind, MediationError, MediationPolicy, PermissionMediator};
    use austringer_core::inspectors::store::AskRequest;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::oneshot;

    use crate::protocol::{Answer, Resolution, ServerMsg};

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
        begin_permission(mediator).await
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
            super::super::control_bus(16),
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

    #[tokio::test]
    async fn unknown_ask_is_typed_error() {
        let m = mediator(MediationPolicy::FirstResponder);
        assert_eq!(
            m.answer(999, "x", Answer::AllowOnce),
            Err(MediationError::UnknownAsk(999))
        );
    }

    #[test]
    fn fallback_kinds_stay_shared_across_the_split() {
        let kind = AskKind::Permission;
        assert!(!matches!(kind, AskKind::Fallback { .. } | AskKind::Plan(_)));
    }
}
