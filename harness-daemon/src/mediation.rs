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
use harness_providers::identity::{FallbackRequest, ModelIdentity};

use crate::protocol::{
    Answer, AskPayload, FallbackAskPayload, PlanAskPayload, PlanDecision, PlanOutcome, Resolution,
    ServerMsg,
};

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
    /// The ask exists but is not a permission ask.
    #[error("ask {0} is not a permission ask")]
    NotAPermissionAsk(u64),
    /// The ask exists but is not a fallback ask.
    #[error("ask {0} is not a fallback ask")]
    NotAFallbackAsk(u64),
    /// The ask exists but is not a plan-approval ask.
    #[error("ask {0} is not a plan ask")]
    NotAPlanAsk(u64),
}

enum AskKind {
    Permission,
    Fallback {
        current: ModelIdentity,
        proposed: ModelIdentity,
    },
    /// The plan-approval lane (todo 16): votes, exactly-once guard,
    /// and waiter live beside the plan id so the permission machinery
    /// above stays untouched. Approve/reject/edit ride the SAME
    /// policy gate as permission answers (task 1's routing contract).
    Plan(PlanAsk),
}

struct PlanAsk {
    plan_id: u64,
    votes: Vec<(String, PlanDecision)>,
    resolved: bool,
    waiter: Option<oneshot::Sender<PlanOutcome>>,
}

struct PendingAsk {
    votes: Vec<(String, Answer)>,
    resolution: Option<Resolution>,
    waiter: Option<oneshot::Sender<Resolution>>,
    kind: AskKind,
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
        // The resolved frame follows the ask's kind so each ask type
        // observes its own vocabulary on the control bus; plan asks
        // resolve through `resolve_plan_once` (their waiter and frame
        // carry PlanOutcome) and can never land here — the kind checks
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

    fn vote_counts(policy: &MediationPolicy, client: &str) -> bool {
        match policy {
            MediationPolicy::FirstResponder | MediationPolicy::ConsensusQuorum { .. } => true,
            MediationPolicy::Designated { client_id }
            | MediationPolicy::LocalOnly { client_id } => client_id == client,
        }
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
    /// deny (the current model is kept — ask, never substitute). The
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

    /// Record one connection's decision on a plan-approval ask. The
    /// same policy gate, vote recording, and exactly-once resolution
    /// as permission answers apply (task 1's routing contract):
    /// FirstResponder resolves on the first counting decision;
    /// Designated/LocalOnly ignore everyone else; ConsensusQuorum
    /// requires `quorum` approves while ANY reject fails closed — and
    /// an edit under quorum records without resolving (it is not an
    /// approval). Timeout is resolved by [`Self::mediate_plan`] and is
    /// NEVER an approval.
    pub fn answer_plan(
        &self,
        ask_id: u64,
        client_id: &str,
        decision: PlanDecision,
    ) -> Result<Option<PlanOutcome>, MediationError> {
        let outcome = {
            let mut state = self.lock();
            let ask = state
                .asks
                .get_mut(&ask_id)
                .ok_or(MediationError::UnknownAsk(ask_id))?;
            let AskKind::Plan(plan) = &mut ask.kind else {
                return Err(MediationError::NotAPlanAsk(ask_id));
            };
            if plan.resolved {
                return Ok(None);
            }
            plan.votes.push((client_id.to_owned(), decision.clone()));
            if !Self::vote_counts(&self.policy, client_id) {
                return Ok(None);
            }
            match (&self.policy, decision) {
                (
                    MediationPolicy::ConsensusQuorum { quorum },
                    PlanDecision::Approve,
                ) => {
                    let allows = plan
                        .votes
                        .iter()
                        .filter(|(_, decision)| !matches!(decision, PlanDecision::Reject))
                        .count();
                    (allows >= *quorum).then(|| PlanOutcome::Approved {
                        by: client_id.to_owned(),
                    })
                }
                (_, PlanDecision::Reject) => Some(PlanOutcome::Rejected {
                    by: client_id.to_owned(),
                }),
                (MediationPolicy::ConsensusQuorum { .. }, PlanDecision::Edit { .. }) => None,
                (_, PlanDecision::Edit { artifact }) => Some(PlanOutcome::Edited {
                    by: client_id.to_owned(),
                    artifact,
                }),
                (_, PlanDecision::Approve) => Some(PlanOutcome::Approved {
                    by: client_id.to_owned(),
                }),
            }
        };
        let mut state = self.lock();
        match outcome {
            Some(outcome) => Ok(self.resolve_plan_once(&mut state, ask_id, outcome)),
            None => Ok(None),
        }
    }

    /// Resolve one plan ask exactly once: wake the waiter, broadcast
    /// the typed [`ServerMsg::PlanResolved`] frame, reap the ask.
    fn resolve_plan_once(
        &self,
        state: &mut MediatorState,
        ask_id: u64,
        outcome: PlanOutcome,
    ) -> Option<PlanOutcome> {
        let ask = state.asks.get_mut(&ask_id)?;
        let AskKind::Plan(plan) = &mut ask.kind else {
            return None;
        };
        if plan.resolved {
            return None;
        }
        plan.resolved = true;
        if let Some(waiter) = plan.waiter.take() {
            let _ = waiter.send(outcome.clone());
        }
        let plan_id = plan.plan_id;
        let _ = self.control.send(ServerMsg::PlanResolved {
            ask_id,
            plan_id,
            outcome: outcome.clone(),
        });
        state.asks.remove(&ask_id);
        Some(outcome)
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
                    PlanOutcome::Cancelled {
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
    /// proposed hop when acknowledged, the current model otherwise —
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

    /// Mediate one plan-approval ask (todo 16): broadcast a
    /// [`ServerMsg::PlanAsk`] carrying the artifact revision, wait up
    /// to the synchronous deadline, resolve exactly once under the
    /// session's policy. The deadline NEVER approves — it resolves
    /// [`PlanOutcome::Timeout`] and the plan stays pending (the
    /// Antigravity v1.3.2 skip-permissions receipt: no surface
    /// auto-approves a plan).
    pub async fn mediate_plan(
        &self,
        plan_id: u64,
        revision: u64,
        artifact: &str,
    ) -> PlanOutcome {
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
                    waiter: None,
                    kind: AskKind::Plan(PlanAsk {
                        plan_id,
                        votes: Vec::new(),
                        resolved: false,
                        waiter: Some(tx),
                    }),
                },
            );
            let payload = PlanAskPayload {
                ask_id,
                plan_id,
                revision,
                artifact: artifact.to_owned(),
                deadline_ms: u64::try_from(self.ask_timeout.as_millis()).unwrap_or(u64::MAX),
            };
            (ask_id, payload, rx)
        };
        let _ = self.control.send(ServerMsg::PlanAsk { ask: payload });
        match tokio::time::timeout(self.ask_timeout, rx).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(_)) => PlanOutcome::Timeout,
            Err(_) => {
                // Deadline won the race; if a decision landed
                // concurrently the N1 gate already resolved and the
                // winner's outcome is returned, never Timeout.
                let mut state = self.lock();
                self.resolve_plan_once(&mut state, ask_id, PlanOutcome::Timeout)
                    .unwrap_or(PlanOutcome::Timeout)
            }
        }
    }
}

/// Informational mapping of a plan outcome onto the shared resolution
/// vocabulary (cancel-vote callers log it; the typed truth rides
/// [`ServerMsg::PlanResolved`]).
fn resolution_of_plan(outcome: &PlanOutcome) -> Resolution {
    match outcome {
        PlanOutcome::Approved { by } | PlanOutcome::Edited { by, .. } => Resolution::Allowed {
            by: by.clone(),
        },
        PlanOutcome::Rejected { by } => Resolution::Denied { by: by.clone() },
        PlanOutcome::Cancelled { by } => Resolution::Cancelled { by: by.clone() },
        PlanOutcome::Timeout => Resolution::Timeout,
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
                kind: AskKind::Permission,
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

    async fn begin_plan(
        mediator: &PermissionMediator,
        plan_id: u64,
    ) -> (u64, oneshot::Receiver<PlanOutcome>) {
        let mut state = mediator.lock();
        let ask_id = state.next_ask_id.saturating_add(1);
        state.next_ask_id = ask_id;
        let (tx, rx) = oneshot::channel();
        state.asks.insert(
            ask_id,
            PendingAsk {
                votes: Vec::new(),
                resolution: None,
                waiter: None,
                kind: AskKind::Plan(PlanAsk {
                    plan_id,
                    votes: Vec::new(),
                    resolved: false,
                    waiter: Some(tx),
                }),
            },
        );
        (ask_id, rx)
    }

    #[tokio::test]
    async fn plan_first_responder_approve_resolves_exactly_once() {
        let m = mediator(MediationPolicy::FirstResponder);
        let mut watch = m.subscribe();
        let (ask_id, rx) = begin_plan(&m, 1).await;
        let resolved = m
            .answer_plan(ask_id, "tui", PlanDecision::Approve)
            .unwrap_or(None);
        assert_eq!(
            resolved,
            Some(PlanOutcome::Approved {
                by: "tui".into()
            })
        );
        assert_eq!(
            m.answer_plan(ask_id, "web", PlanDecision::Reject)
                .unwrap_or(None),
            None
        );
        assert!(matches!(
            m.answer_plan(ask_id, "web", PlanDecision::Approve),
            Err(MediationError::UnknownAsk(_))
        ));
        assert_eq!(
            rx.await.unwrap_or(PlanOutcome::Timeout),
            PlanOutcome::Approved {
                by: "tui".into()
            }
        );
        let mut resolved_frames = 0;
        while let Ok(msg) = watch.try_recv() {
            if matches!(msg, ServerMsg::PlanResolved { .. }) {
                resolved_frames += 1;
            }
        }
        assert_eq!(resolved_frames, 1, "exactly one PlanResolved frame");
    }

    #[tokio::test]
    async fn plan_local_only_ignores_remote_approve() {
        let m = mediator(MediationPolicy::LocalOnly {
            client_id: "local".into(),
        });
        let (ask_id, rx) = begin_plan(&m, 1).await;
        assert_eq!(
            m.answer_plan(ask_id, "remote-attacker", PlanDecision::Approve)
                .unwrap_or(None),
            None
        );
        let res = m
            .answer_plan(ask_id, "local", PlanDecision::Approve)
            .unwrap_or(None);
        assert_eq!(
            res,
            Some(PlanOutcome::Approved {
                by: "local".into()
            })
        );
        assert_eq!(
            rx.await.unwrap_or(PlanOutcome::Timeout),
            PlanOutcome::Approved {
                by: "local".into()
            }
        );
    }

    #[tokio::test]
    async fn plan_edit_resolves_with_the_edited_artifact() {
        let m = mediator(MediationPolicy::FirstResponder);
        let (ask_id, rx) = begin_plan(&m, 7).await;
        let artifact = "# Revised plan\n- task one".to_owned();
        let res = m
            .answer_plan(ask_id, "web", PlanDecision::Edit {
                artifact: artifact.clone(),
            })
            .unwrap_or(None);
        assert_eq!(
            res,
            Some(PlanOutcome::Edited {
                by: "web".into(),
                artifact,
            })
        );
        match rx.await.unwrap_or(PlanOutcome::Timeout) {
            PlanOutcome::Edited { artifact, .. } => {
                assert!(artifact.contains("Revised plan"));
            }
            other => panic!("expected edited outcome, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn plan_reject_resolves_rejected() {
        let m = mediator(MediationPolicy::FirstResponder);
        let (ask_id, rx) = begin_plan(&m, 2).await;
        let res = m
            .answer_plan(ask_id, "web", PlanDecision::Reject)
            .unwrap_or(None);
        assert_eq!(
            res,
            Some(PlanOutcome::Rejected {
                by: "web".into()
            })
        );
        assert_eq!(
            rx.await.unwrap_or(PlanOutcome::Timeout),
            PlanOutcome::Rejected {
                by: "web".into()
            }
        );
    }

    #[tokio::test]
    async fn plan_cancel_withdraws_without_deciding() {
        let m = mediator(MediationPolicy::FirstResponder);
        let (ask_id, rx) = begin_plan(&m, 3).await;
        let res = m.cancel_vote(ask_id, "web").unwrap_or(None);
        assert_eq!(res, Some(Resolution::Cancelled { by: "web".into() }));
        match rx.await.unwrap_or(PlanOutcome::Timeout) {
            PlanOutcome::Cancelled { by } => assert_eq!(by, "web"),
            other => panic!("expected cancelled outcome, got {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn plan_timeout_never_approves() {
        let m = Arc::new(PermissionMediator::new(
            MediationPolicy::FirstResponder,
            Duration::from_secs(5),
            control_bus(16),
        ));
        let mediating = tokio::spawn({
            let m = Arc::clone(&m);
            async move { m.mediate_plan(1, 1, "# draft").await }
        });
        tokio::time::advance(Duration::from_secs(6)).await;
        let outcome = mediating.await.unwrap_or(PlanOutcome::Timeout);
        assert_eq!(outcome, PlanOutcome::Timeout);
        let state = m.lock();
        assert!(state.asks.is_empty(), "timeout reaps the ask (N1)");
    }

    #[tokio::test]
    async fn plan_quorum_requires_explicit_approves_and_reject_fails_closed() {
        let m = mediator(MediationPolicy::ConsensusQuorum { quorum: 2 });
        let (ask_id, rx) = begin_plan(&m, 4).await;
        assert_eq!(
            m.answer_plan(ask_id, "a", PlanDecision::Approve)
                .unwrap_or(None),
            None,
            "one approve is below quorum"
        );
        assert_eq!(
            m.answer_plan(ask_id, "b", PlanDecision::Edit {
                artifact: "x".into()
            })
            .unwrap_or(None),
            None,
            "an edit under quorum records without resolving"
        );
        let res = m
            .answer_plan(ask_id, "c", PlanDecision::Approve)
            .unwrap_or(None);
        assert_eq!(
            res,
            Some(PlanOutcome::Approved {
                by: "c".into()
            })
        );
        drop(rx.await);

        let (ask_id2, _rx2) = begin_plan(&m, 5).await;
        m.answer_plan(ask_id2, "a", PlanDecision::Approve).ok();
        let res = m
            .answer_plan(ask_id2, "b", PlanDecision::Reject)
            .unwrap_or(None);
        assert_eq!(
            res,
            Some(PlanOutcome::Rejected {
                by: "b".into()
            }),
            "any reject fails closed even below quorum"
        );
    }

    #[tokio::test]
    async fn mediate_plan_broadcasts_ask_and_kind_checks_are_typed() {
        let m = mediator(MediationPolicy::FirstResponder);
        let mut watch = m.subscribe();
        let mediating = tokio::spawn({
            let m = Arc::clone(&m);
            async move { m.mediate_plan(9, 2, "# draft v2").await }
        });
        let ask_id = loop {
            match watch.try_recv() {
                Ok(ServerMsg::PlanAsk { ask }) => {
                    assert_eq!(ask.plan_id, 9);
                    assert_eq!(ask.revision, 2);
                    assert_eq!(ask.artifact, "# draft v2");
                    break ask.ask_id;
                }
                _ => tokio::task::yield_now().await,
            }
        };
        // Permission answers cannot resolve a plan ask (and vice
        // versa): the kind checks are typed.
        assert!(matches!(
            m.answer(ask_id, "web", Answer::AllowOnce),
            Err(MediationError::NotAPermissionAsk(_))
        ));
        assert_eq!(
            m.answer_plan(ask_id, "web", PlanDecision::Approve)
                .unwrap_or(None),
            Some(PlanOutcome::Approved {
                by: "web".into()
            })
        );
        assert_eq!(
            mediating.await.unwrap_or(PlanOutcome::Timeout),
            PlanOutcome::Approved {
                by: "web".into()
            }
        );
    }
}
