//! The failover engine (plan todo 42): refusal/truncation retries with
//! an attempt counter, rate-limit wait-out windows (bounded at 75s)
//! before any failover, and rank-verbatim chain walks where every hop is
//! an EXPLICIT confirmed fallback — the router asks, never substitutes
//! (the T9 identity contract). The outcome/decision vocabulary lives in
//! [`super::outcome`].

use std::collections::BTreeMap;
use std::time::Duration;

use austringer_providers::identity::{AcknowledgedFallback, FallbackRequest, ModelIdentity};

// The vocabulary lives in `outcome`; re-exported here so the historic
// `routing::fallback::*` paths stay stable for embedders.
pub use super::outcome::{
    DispatchFailure, FallbackDecision, FallbackRejected, RoutingEvent, RoutingFailure, StopReason,
    TurnOutcome,
};

use super::resolve::TaskId;
use super::table::{RoutingTable, WAIT_OUT_CAP_SECS};

#[derive(Debug)]
struct FailState {
    origin: String,
    hops_acknowledged: usize,
    retries: u32,
    wait_outs: u32,
    pending: Option<(ModelIdentity, ModelIdentity)>,
}

impl FailState {
    fn fresh(identity: &ModelIdentity) -> Self {
        Self {
            origin: identity.model.clone(),
            hops_acknowledged: 0,
            retries: 0,
            wait_outs: 0,
            pending: None,
        }
    }
}

/// The failover engine: per-task verbatim-chain walks with confirmation
/// gating, retry counters, and the surfaced audit trail.
#[derive(Debug, Default)]
pub struct Failover {
    states: BTreeMap<TaskId, FailState>,
    audit: Vec<RoutingEvent>,
}

impl Failover {
    /// An idle engine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Observes a dispatched turn's outcome and decides the next move.
    pub fn observe(
        &mut self,
        table: &RoutingTable,
        task: &TaskId,
        identity: &ModelIdentity,
        outcome: TurnOutcome,
    ) -> FallbackDecision {
        // Take the per-task state out so the decision helpers can borrow
        // the audit trail freely; reinserted unchanged below either way.
        let mut state = match self.states.remove(task) {
            Some(existing) => existing,
            None => FailState::fresh(identity),
        };
        let decision = self.decide(table, task, identity, &mut state, outcome);
        self.states.insert(task.clone(), state);
        decision
    }

    fn decide(
        &mut self,
        table: &RoutingTable,
        task: &TaskId,
        identity: &ModelIdentity,
        state: &mut FailState,
        outcome: TurnOutcome,
    ) -> FallbackDecision {
        match outcome {
            TurnOutcome::Completed {
                stop: StopReason::EndTurn,
            } => {
                state.retries = 0;
                state.wait_outs = 0;
                FallbackDecision::Proceed
            }
            TurnOutcome::Completed {
                stop: StopReason::RefusalTruncation,
            } => self.retry_or_ask(table, task, identity, state, "refusal/truncation"),
            TurnOutcome::Completed {
                stop: StopReason::MaxTokens,
            } => self.retry_or_ask(table, task, identity, state, "max-tokens truncation"),
            TurnOutcome::Failed {
                failure: DispatchFailure::RateLimit { retry_after },
            } => {
                state.wait_outs += 1;
                if state.wait_outs > table.retry_attempts() {
                    return self.propose_hop(
                        table,
                        task,
                        identity,
                        state,
                        "rate limits persisted past the wait-out budget",
                    );
                }
                let configured = table.wait_out();
                let window = retry_after
                    .unwrap_or(configured)
                    .min(configured)
                    .min(Duration::from_secs(WAIT_OUT_CAP_SECS));
                self.audit.push(RoutingEvent::WaitOut {
                    task: task.as_str().to_owned(),
                    model: identity.model.clone(),
                    window_secs: window.as_secs(),
                });
                FallbackDecision::WaitOut {
                    window,
                    attempt: state.wait_outs,
                }
            }
            TurnOutcome::Failed {
                failure: DispatchFailure::Outage,
            } => self.propose_hop(table, task, identity, state, "outage"),
        }
    }

    fn retry_or_ask(
        &mut self,
        table: &RoutingTable,
        task: &TaskId,
        identity: &ModelIdentity,
        state: &mut FailState,
        reason: &str,
    ) -> FallbackDecision {
        state.retries += 1;
        if state.retries <= table.retry_attempts() {
            return FallbackDecision::Retry {
                attempt: state.retries,
            };
        }
        self.propose_hop(table, task, identity, state, reason)
    }

    /// Proposes the next verbatim hop of the ORIGIN model's chain. The
    /// chain belongs to the launch model (rank-verbatim: position 0 is
    /// always the first configured hop, never a re-ranking).
    fn propose_hop(
        &mut self,
        table: &RoutingTable,
        task: &TaskId,
        identity: &ModelIdentity,
        state: &mut FailState,
        reason: &str,
    ) -> FallbackDecision {
        let chain = table.fallback_chain(&state.origin);
        let Some(target) = chain.get(state.hops_acknowledged) else {
            if chain.is_empty() {
                return FallbackDecision::Fail(RoutingFailure::NoFallbackConfigured {
                    model: identity.model.clone(),
                });
            }
            self.audit.push(RoutingEvent::ChainExhausted {
                task: task.as_str().to_owned(),
                origin: state.origin.clone(),
            });
            return FallbackDecision::Fail(RoutingFailure::ChainExhausted {
                origin: state.origin.clone(),
                chain: chain.to_vec(),
            });
        };
        let to = ModelIdentity::new(&identity.provider, target);
        state.pending = Some((identity.clone(), to.clone()));
        self.audit.push(RoutingEvent::HopProposed {
            task: task.as_str().to_owned(),
            from: identity.model.clone(),
            to: target.clone(),
        });
        FallbackDecision::AskFallback(FallbackRequest::new(
            identity.clone(),
            to,
            &format!(
                "{reason}; verbatim hop {} of {}",
                state.hops_acknowledged + 1,
                chain.len()
            ),
        ))
    }

    /// Confirms the pending proposal. Mismatched acknowledgements are
    /// typed rejections — only the exact proposed hop may be confirmed.
    pub fn acknowledge(
        &mut self,
        task: &TaskId,
        ack: &AcknowledgedFallback,
    ) -> Result<(), FallbackRejected> {
        let Some(state) = self.states.get_mut(task) else {
            return Err(FallbackRejected {
                expected: String::new(),
                got: ack.to().model.clone(),
            });
        };
        let Some((from, to)) = state.pending.clone() else {
            return Err(FallbackRejected {
                expected: String::new(),
                got: ack.to().model.clone(),
            });
        };
        if ack.to() != &to {
            return Err(FallbackRejected {
                expected: to.model.clone(),
                got: ack.to().model.clone(),
            });
        }
        state.pending = None;
        state.hops_acknowledged += 1;
        state.retries = 0;
        state.wait_outs = 0;
        self.audit.push(RoutingEvent::HopAcknowledged {
            task: task.as_str().to_owned(),
            from: from.model.clone(),
            to: to.model.clone(),
        });
        Ok(())
    }

    /// Declines the pending proposal (typed failure; the original route
    /// stays).
    pub fn decline(&mut self, task: &TaskId) -> RoutingFailure {
        let declined = self
            .states
            .get_mut(task)
            .and_then(|state| state.pending.take());
        let (from, to) = declined.map_or_else(
            || (String::new(), String::new()),
            |(from, to)| (from.model.clone(), to.model.clone()),
        );
        self.audit.push(RoutingEvent::HopDeclined {
            task: task.as_str().to_owned(),
            from: from.clone(),
            to: to.clone(),
        });
        RoutingFailure::Declined { from, to }
    }

    /// The surfaced audit trail, in order.
    #[must_use]
    pub fn audit(&self) -> &[RoutingEvent] {
        &self.audit
    }
}
