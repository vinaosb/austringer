//! The plan-approval lane (todo 16): votes and exactly-once resolution
//! beside the plan id, sharing the permission machinery's policy gate
//! (task 1's routing contract). The deadline NEVER approves.

use tokio::sync::oneshot;

use super::policies::vote_counts;
use super::{AskKind, MediationError, MediatorState, PendingAsk, PermissionMediator, PlanAsk};
use crate::protocol::{PlanAskPayload, PlanDecision, PlanOutcome, Resolution, ServerMsg};

impl PermissionMediator {
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
            if !vote_counts(&self.policy, client_id) {
                return Ok(None);
            }
            match (&self.policy, decision) {
                (super::MediationPolicy::ConsensusQuorum { quorum }, PlanDecision::Approve) => {
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
                (super::MediationPolicy::ConsensusQuorum { .. }, PlanDecision::Edit { .. }) => None,
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
    pub(super) fn resolve_plan_once(
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

    /// Mediate one plan-approval ask (todo 16): broadcast a
    /// [`ServerMsg::PlanAsk`] carrying the artifact revision, wait up
    /// to the synchronous deadline, resolve exactly once under the
    /// session's policy. The deadline NEVER approves — it resolves
    /// [`PlanOutcome::Timeout`] and the plan stays pending (the
    /// Antigravity v1.3.2 skip-permissions receipt: no surface
    /// auto-approves a plan).
    pub async fn mediate_plan(&self, plan_id: u64, revision: u64, artifact: &str) -> PlanOutcome {
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
pub(super) fn resolution_of_plan(outcome: &PlanOutcome) -> Resolution {
    match outcome {
        PlanOutcome::Approved { by } | PlanOutcome::Edited { by, .. } => {
            Resolution::Allowed { by: by.clone() }
        }
        PlanOutcome::Rejected { by } => Resolution::Denied { by: by.clone() },
        PlanOutcome::Cancelled { by } => Resolution::Cancelled { by: by.clone() },
        PlanOutcome::Timeout => Resolution::Timeout,
    }
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{begin_plan, mediator};
    use super::super::{MediationError, MediationPolicy, PermissionMediator};
    use crate::protocol::Resolution;
    use std::sync::Arc;
    use std::time::Duration;

    use crate::protocol::{Answer, PlanDecision, PlanOutcome, ServerMsg};

    #[tokio::test]
    async fn plan_first_responder_approve_resolves_exactly_once() {
        let m = mediator(MediationPolicy::FirstResponder);
        let mut watch = m.subscribe();
        let (ask_id, rx) = begin_plan(&m, 1).await;
        let resolved = m
            .answer_plan(ask_id, "tui", PlanDecision::Approve)
            .unwrap_or(None);
        assert_eq!(resolved, Some(PlanOutcome::Approved { by: "tui".into() }));
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
            PlanOutcome::Approved { by: "tui".into() }
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
        assert_eq!(res, Some(PlanOutcome::Approved { by: "local".into() }));
        assert_eq!(
            rx.await.unwrap_or(PlanOutcome::Timeout),
            PlanOutcome::Approved { by: "local".into() }
        );
    }

    #[tokio::test]
    async fn plan_edit_resolves_with_the_edited_artifact() {
        let m = mediator(MediationPolicy::FirstResponder);
        let (ask_id, rx) = begin_plan(&m, 7).await;
        let artifact = "# Revised plan\n- task one".to_owned();
        let res = m
            .answer_plan(
                ask_id,
                "web",
                PlanDecision::Edit {
                    artifact: artifact.clone(),
                },
            )
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
        assert_eq!(res, Some(PlanOutcome::Rejected { by: "web".into() }));
        assert_eq!(
            rx.await.unwrap_or(PlanOutcome::Timeout),
            PlanOutcome::Rejected { by: "web".into() }
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
            super::super::control_bus(16),
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
            m.answer_plan(
                ask_id,
                "b",
                PlanDecision::Edit {
                    artifact: "x".into()
                }
            )
            .unwrap_or(None),
            None,
            "an edit under quorum records without resolving"
        );
        let res = m
            .answer_plan(ask_id, "c", PlanDecision::Approve)
            .unwrap_or(None);
        assert_eq!(res, Some(PlanOutcome::Approved { by: "c".into() }));
        drop(rx.await);

        let (ask_id2, _rx2) = begin_plan(&m, 5).await;
        m.answer_plan(ask_id2, "a", PlanDecision::Approve).ok();
        let res = m
            .answer_plan(ask_id2, "b", PlanDecision::Reject)
            .unwrap_or(None);
        assert_eq!(
            res,
            Some(PlanOutcome::Rejected { by: "b".into() }),
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
            Some(PlanOutcome::Approved { by: "web".into() })
        );
        assert_eq!(
            mediating.await.unwrap_or(PlanOutcome::Timeout),
            PlanOutcome::Approved { by: "web".into() }
        );
    }
}
