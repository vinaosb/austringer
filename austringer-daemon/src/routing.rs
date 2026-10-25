//! The dispatcher turn path (plan todo 9, ecosystem alignment): session
//! turns dispatch through austringer-intel's [`Dispatcher`] — stickiness
//! pins, role routes from routing.json, rank-verbatim fallback chains —
//! with EVERY model change flowing through the mediated ask
//! ([`SessionHost::mediate_fallback`]: `FallbackAsk` to v2 wires, the
//! v1-decodable `Notification` pair to v1 wires). No code path swaps
//! models silently: an unacknowledged (denied, cancelled, timed-out)
//! ask retries the SAME model, always.
//!
//! Two honest seam limitations, documented here rather than papered
//! over: (a) the daemon `Inference` seam carries no finish reason, so a
//! COMPLETED turn observes as `StopReason::EndTurn` — adapter-level
//! refusal/length truncations do not cross the seam and cannot trigger
//! refusal-retries from this layer; (b) failures surface as
//! `InferenceError::Failure(String)`, so rate-limit classification is a
//! bounded best-effort match on the message text (`429` / rate limit),
//! everything else observes as an outage.
//!
//! The Wave-R→Wave-U interim (asks emitted before all frontends can
//! answer) is a DOCUMENTED degraded mode: when an ask goes
//! unacknowledged the session publishes the verbatim
//! [`DEGRADED_FALLBACK_RETRY_NOTICE`] notification and retries the same
//! model (plan todo 9; carried into todo 31's README honesty pass).

use std::sync::Arc;

use async_trait::async_trait;
use austringer_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest,
};
use austringer_intel::routing::dispatch::Dispatcher;
use austringer_intel::routing::resolve::ResolutionError;
use austringer_intel::routing::{
    DispatchFailure, FallbackDecision, Role, RouteRequest, RoutingTable, TaskId, TurnDispatch,
    TurnOutcome,
};
use austringer_providers::identity::ModelIdentity;

use crate::attach::SessionHost;
use crate::cost_ledger::SessionLedger;
use crate::protocol::SessionEvent;

/// Builds one provider-level inference stack for a model id. `None` is a
/// typed refusal (the model is not composable on this daemon's provider
/// seam) — the dispatcher fails the turn, never guesses.
pub type ModelSource = Arc<dyn Fn(&str) -> Option<Box<dyn Inference>> + Send + Sync>;

/// The daemon-wide dispatcher intake (todo 9): the FULL routing table
/// (roles, tiers, chains, benchmarks, cheap pool, wait-out, retries,
/// sidekick — routing.json stops being a summarizer-only consumer) plus
/// the per-model provider-stack source routed identities compose on.
pub struct RoutingIntake {
    /// The validated routing table.
    pub table: RoutingTable,
    /// Per-model provider stack source.
    pub models: ModelSource,
}

/// The Wave-R→Wave-U degraded-mode sentence (plan todo 9): verbatim.
pub const DEGRADED_FALLBACK_RETRY_NOTICE: &str =
    "model fallback asked; no interactive client — retrying same model";

/// The notification kind every fallback-lane notification carries (the
/// ask/resolved pair from [`SessionHost::mediate_fallback`] and the
/// degraded-mode retry notice below).
const FALLBACK_KIND: &str = "fallback";

/// Unacknowledged asks tolerated per inference call: one ask may time
/// out (or be denied) and the SAME model is retried; a further
/// unacknowledged ask surfaces the attempt instead of looping — the
/// ask/wait/retry cycle is bounded, and it never substitutes.
const MAX_UNACKNOWLEDGED_ASKS: u32 = 1;

/// The session-role request every turn dispatches under (Zed's default
/// conversational role; Plandex vocabulary — research SS20/SS23).
const SESSION_ROLE: RouteRequest = RouteRequest::Role(Role::Default);

/// What one turn resolved to.
enum ActiveDispatch {
    /// A routed dispatch: the routing.json role entry's identity plus
    /// its explicit effort (or the acknowledged hop's revalidated one).
    Routed(Box<TurnDispatch>),
    /// The session's CONFIGURED model serving explicitly — the role has
    /// no routing entry, and an absent route is never an implicit hop.
    Configured(ModelIdentity),
}

impl ActiveDispatch {
    fn identity(&self) -> &ModelIdentity {
        match self {
            Self::Routed(dispatch) => &dispatch.model,
            Self::Configured(identity) => identity,
        }
    }

    fn effort(&self) -> Option<String> {
        match self {
            Self::Routed(dispatch) => Some(dispatch.effort.to_string()),
            Self::Configured(_) => None,
        }
    }
}

/// The outermost inference of a routed session: each call resolves the
/// turn's model through the dispatcher, composes the dispatched model's
/// full stack (provider → Usage → CostLedger — so identity, Usage
/// frames, and spend follow the DISPATCH, and an acknowledged hop
/// re-cites the new model), runs it, and observes the outcome through
/// the failover engine. Retry, wait-out, and ask decisions replay
/// inside the call; the loop is bounded by the table's retry budget,
/// the wait-out cap, the chain length, and [`MAX_UNACKNOWLEDGED_ASKS`].
pub struct SessionRouter {
    intake: Arc<RoutingIntake>,
    task: TaskId,
    host: Arc<SessionHost>,
    ledger: Arc<SessionLedger>,
    /// The session's configured identity — the explicit route when the
    /// routing table has no entry for the session role.
    configured: ModelIdentity,
    dispatcher: Dispatcher,
    /// The request every turn dispatches under (the session role by
    /// default; task-tool children dispatch under their delegation
    /// category, todo 13).
    request: RouteRequest,
    /// The live per-model stack, rebuilt only when the dispatched
    /// identity (or effort) changes.
    stack: Option<(ModelIdentity, Option<String>, Box<dyn Inference>)>,
}

impl SessionRouter {
    /// Build the router for one session. The inputs are independent
    /// construction facts (intake, session id, owning host + its ledger,
    /// the configured fallback identity); a grouping struct would only
    /// re-wrap them for this single caller — same shape as
    /// [`crate::usage::UsageInference::new`] /
    /// [`crate::cost_ledger::CostLedgerInference::new`].
    #[must_use]
    pub fn new(
        intake: Arc<RoutingIntake>,
        session_id: &str,
        host: Arc<SessionHost>,
        ledger: Arc<SessionLedger>,
        configured: ModelIdentity,
    ) -> Self {
        Self {
            dispatcher: Dispatcher::new(intake.table.clone()),
            task: TaskId::new(session_id),
            intake,
            host,
            ledger,
            configured,
            request: SESSION_ROLE,
            stack: None,
        }
    }

    /// Dispatches every turn under `request` instead of the session
    /// role (the `task(category)` child path; the request is pinned for
    /// the router's lifetime, so stickiness keys on it from the first
    /// dispatch).
    #[must_use]
    pub fn under_request(mut self, request: RouteRequest) -> Self {
        self.request = request;
        self
    }

    /// Resolves this turn's dispatch: the pinned (or hop-overridden)
    /// route under the session's request, or the session's configured
    /// model when the request is unrouted (explicit fallback, never a
    /// hop).
    fn resolve_dispatch(&mut self) -> Result<ActiveDispatch, InferenceError> {
        match self
            .dispatcher
            .dispatch_turn(&self.task, self.request.clone())
        {
            Ok(dispatch) => Ok(ActiveDispatch::Routed(Box::new(dispatch))),
            Err(ResolutionError::UnconfiguredRole { .. }) => {
                Ok(ActiveDispatch::Configured(self.configured.clone()))
            }
            Err(ResolutionError::UnconfiguredCategory { .. }) => Err(InferenceError::Failure(
                "routing: task category unrouted mid-session — refusing to guess".to_owned(),
            )),
            Err(other) => Err(InferenceError::Failure(format!(
                "route resolution failed: {other}"
            ))),
        }
    }

    /// Ensures the cached stack serves `identity` at `effort`, building
    /// a fresh provider → Usage → CostLedger wrap on change.
    fn ensure_stack(
        &mut self,
        identity: &ModelIdentity,
        effort: Option<&str>,
    ) -> Result<(), InferenceError> {
        let cached =
            matches!(&self.stack, Some((id, eff, _)) if id == identity && eff.as_deref() == effort);
        if cached {
            return Ok(());
        }
        let inner = (self.intake.models)(identity.model.as_str()).ok_or_else(|| {
            InferenceError::Failure(format!(
                "routing: no provider stack composes for model {} — refusing to guess",
                identity.model
            ))
        })?;
        let usage = crate::usage::UsageInference::new(
            Arc::clone(&self.host),
            identity.clone(),
            effort.map(str::to_owned),
            inner,
        );
        let costed = crate::cost_ledger::CostLedgerInference::new(
            Arc::clone(&self.host),
            Arc::clone(&self.ledger),
            identity.clone(),
            Box::new(usage),
        );
        self.stack = Some((
            identity.clone(),
            effort.map(str::to_owned),
            Box::new(costed) as Box<dyn Inference>,
        ));
        Ok(())
    }
}

#[async_trait]
impl Inference for SessionRouter {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        let mut unacknowledged: u32 = 0;
        loop {
            let dispatch = self.resolve_dispatch()?;
            let identity = dispatch.identity().clone();
            let effort = dispatch.effort();
            self.ensure_stack(&identity, effort.as_deref())?;
            let attempt = match self.stack.as_mut() {
                Some((_, _, stack)) => stack.infer(request, cancel).await,
                // ensure_stack guarantees the entry; typed, never a panic.
                None => {
                    return Err(InferenceError::Failure(
                        "routing stack cache invariant broken".to_owned(),
                    ));
                }
            };
            let outcome = match &attempt {
                // No finish reason crosses this seam: a returned turn is
                // an end-turn (see module docs).
                Ok(_) => TurnOutcome::Completed {
                    stop: austringer_intel::routing::StopReason::EndTurn,
                },
                Err(error) => TurnOutcome::Failed {
                    failure: dispatch_failure(error),
                },
            };
            let decision = self.dispatcher.observe_turn(&self.task, &identity, outcome);
            match decision {
                FallbackDecision::Proceed => return attempt,
                FallbackDecision::Retry { .. } => continue,
                FallbackDecision::WaitOut { window, .. } => {
                    tokio::time::sleep(window).await;
                    continue;
                }
                FallbackDecision::AskFallback(proposal) => {
                    let to = proposal.to.clone();
                    // The ask machinery is todo 1's: FallbackAsk on v2
                    // wires, the v1 Notification pair, exactly-once
                    // resolution under the session's mediation policy.
                    let chosen = self.host.mediate_fallback(proposal.clone()).await;
                    if chosen == to {
                        match self
                            .dispatcher
                            .acknowledge_fallback(&self.task, proposal.acknowledge())
                        {
                            Ok(()) => continue, // next dispatch carries the hop
                            Err(rejected) => {
                                return Err(InferenceError::Failure(format!(
                                    "fallback acknowledgement rejected: {rejected}"
                                )));
                            }
                        }
                    }
                    // Unacknowledged (deny / cancel / timeout): the SAME
                    // model retries — the T9 identity contract. The
                    // degraded-mode notice makes the retry v1-visible.
                    unacknowledged += 1;
                    self.host.publish(SessionEvent::Notification {
                        kind: FALLBACK_KIND.to_owned(),
                        message: DEGRADED_FALLBACK_RETRY_NOTICE.to_owned(),
                    });
                    if unacknowledged > MAX_UNACKNOWLEDGED_ASKS {
                        return attempt;
                    }
                    continue;
                }
                FallbackDecision::Fail(failure) => {
                    return Err(InferenceError::Failure(failure.to_string()));
                }
            }
        }
    }
}

/// Classifies an inference failure for the failover engine: rate-limit
/// text (HTTP 429 / "rate limit") waits out, everything else is an
/// outage. Best-effort by necessity — the seam carries strings (see
/// module docs).
fn dispatch_failure(error: &InferenceError) -> DispatchFailure {
    let InferenceError::Failure(message) = error;
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("429") || lowered.contains("rate limit") || lowered.contains("rate_limit") {
        DispatchFailure::RateLimit { retry_after: None }
    } else {
        DispatchFailure::Outage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_text_classifies_as_rate_limit() {
        assert_eq!(
            dispatch_failure(&InferenceError::Failure(
                "HTTP 429 too many requests".to_owned()
            )),
            DispatchFailure::RateLimit { retry_after: None }
        );
        assert_eq!(
            dispatch_failure(&InferenceError::Failure("Rate Limit exceeded".to_owned())),
            DispatchFailure::RateLimit { retry_after: None }
        );
    }

    #[test]
    fn everything_else_classifies_as_outage() {
        assert_eq!(
            dispatch_failure(&InferenceError::Failure(
                "provider unreachable: connection refused".to_owned()
            )),
            DispatchFailure::Outage
        );
        assert_eq!(
            dispatch_failure(&InferenceError::Failure(
                "provider timeout: inference exceeded the 300s budget".to_owned()
            )),
            DispatchFailure::Outage
        );
    }
}
