//! Multi-client permission mediation (Qwen §18 pattern): every ask
//! broadcasts to all attached clients; exactly one resolution ever lands
//! (answer XOR cancel XOR timeout — the N1 invariant); four policies
//! decide whose answer counts. Adapts onto austringer-core's
//! [`austringer_core::inspectors::store::AskPolicy`] seam.
//!
//! Layout: [`PermissionMediator`]'s ask bookkeeping lives here (shared
//! across lanes); the policy vocabulary in [`policies`], the
//! permission/fallback lanes and the core adapter in [`permission`],
//! the plan-approval lane in [`plan`].

mod adapter;
mod permission;
mod plan;
mod policies;

pub use adapter::{MediatedAskPolicy, to_ask_resolution};
pub use policies::MediationPolicy;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::{broadcast, oneshot};

use austringer_providers::identity::ModelIdentity;

use crate::protocol::{Answer, PlanDecision, PlanOutcome, Resolution, ServerMsg};

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
}

/// Convenience constructor for the shared control bus.
#[must_use]
pub fn control_bus(capacity: usize) -> broadcast::Sender<ServerMsg> {
    broadcast::channel(capacity.max(1)).0
}

#[cfg(test)]
pub(crate) mod testkit {
    //! Cross-lane test constructors (each lane's tests build asks
    //! directly to keep assertions off the broadcast path).
    use super::{AskKind, PendingAsk, PermissionMediator, PlanAsk};
    use crate::protocol::{PlanOutcome, Resolution};
    use tokio::sync::oneshot;

    pub(crate) fn mediator(policy: super::MediationPolicy) -> std::sync::Arc<PermissionMediator> {
        std::sync::Arc::new(PermissionMediator::new(
            policy,
            std::time::Duration::from_secs(30),
            super::control_bus(16),
        ))
    }

    pub(crate) async fn begin_permission(
        mediator: &PermissionMediator,
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

    pub(crate) async fn begin_plan(
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
}
