//! Session cost governance (todo 33): the EXISTING austringer-core money
//! budget wired into the daemon engine loop. One [`SessionLedger`] per
//! session books the priced share of every Usage-accounted turn into a
//! core [`SpendLedger`] over a Session-period micro-USD cap; the
//! [`CostLedgerInference`] decorator is the wiring point — the ledger's
//! preflight gate refuses inference BEFORE the provider call once the
//! cap is reached, and threshold signals become typed
//! [`SessionEvent::CostWarning`] / [`SessionEvent::CostStop`] frames on
//! the session stream (v1 wires get the `cost_warning` / `cost_stop`
//! notification downgrade).
//!
//! Contracts this module exists to keep:
//!
//! - A Stop is a TYPED terminal event; further Submit attempts get a
//!   typed [`CommandRejected::CostCapReached`](crate::attach::CommandRejected::CostCapReached)
//!   naming the cap. A cost cap NEVER switches models — the ask-first
//!   path resolves (same-model retry on deny/timeout) inside the
//!   inference call whose spend crosses the cap, strictly BEFORE the
//!   next preflight can refuse.
//! - Unpriced turns book zero money: a cap cannot trip on unknowns
//!   (the token ceilings in austringer-orchestration remain the token
//!   contract; this ledger is the COST contract).
//! - Delegation children record into the PARENT session's ledger (the
//!   launch path passes the parent host); every session's halt is its
//!   own — other sessions keep running.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;

use austringer_core::agent_loop::stages::{ErrorFlow, ExitOnError, StageCtx, StageError};
use austringer_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest,
};
use austringer_core::budget::{
    BudgetCap, BudgetPeriod, BudgetSignal, BudgetSignalSink, BudgetSpec, Cost, Dimension,
    MONEY_STOP_PREFIX, MicroUsd, Spend, SpendLedger, StopReason, TOKEN_STOP_PREFIX, TokenCount,
};
use austringer_providers::cost::Priced;
use austringer_providers::identity::ModelIdentity;

use crate::attach::SessionHost;
use crate::protocol::SessionEvent;
use crate::usage::builtin_prices;

/// The default session cost cap: $20 (20_000_000 micro-USD). Opus-class
/// heavy turns (~$3-4 each) reach the 80% warning within ~5 turns;
/// mid-tier sessions run dozens of turns untouched — the same order of
/// magnitude the 20M-token per-launch guard implies at list prices.
/// User-global override: `AUSTRINGER_SESSION_COST_CAP_USD` (decimal
/// dollars, parsed by
/// [`parse_session_cost_cap_usd`](crate::config::parse_session_cost_cap_usd)).
pub const DEFAULT_SESSION_COST_CAP_MICROS: u128 = 20_000_000;

/// The HUD badge state (todos 19/20 render this; todo 33 only exposes
/// it). `warned`/`stopped` flip monotonically — consumption never
/// rewinds within a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostSnapshot {
    /// The session cost cap, micro-USD.
    pub cap_micros: u128,
    /// Cumulative priced spend, micro-USD.
    pub consumed_micros: u128,
    /// The 80% warning has fired.
    pub warned: bool,
    /// The 100% stop has fired.
    pub stopped: bool,
}

struct LedgerState {
    ledger: SpendLedger,
    consumed_micros: u128,
    warned: bool,
    stopped: bool,
}

/// Per-session money ledger: a core [`SpendLedger`] over a
/// Session-period micro-USD cap, shared by the session's own inference
/// decorator, its delegation children, and the submit intake gate.
/// Locking is brief and leaf-level (record/preflight/snapshot); signal
/// PUBLISHING happens outside the lock on the owning host's hub.
pub struct SessionLedger {
    cap_micros: u128,
    state: Mutex<LedgerState>,
}

impl SessionLedger {
    /// A ledger over a Session-period money cap (`cap_micros`
    /// micro-USD). The token dimension stays uncapped here — per-launch
    /// token ceilings remain austringer-orchestration's contract.
    #[must_use]
    pub fn new(cap_micros: u128) -> Self {
        let spec = BudgetSpec::from_caps(BTreeMap::from([(
            BudgetPeriod::Session,
            BudgetCap {
                money: Some(MicroUsd(cap_micros)),
                tokens: None,
            },
        )]));
        let ledger = SpendLedger::new(spec).unwrap_or_else(|error| {
            unreachable!("a single Session money cap is always a valid spec: {error:?}")
        });
        Self {
            cap_micros,
            state: Mutex::new(LedgerState {
                ledger,
                consumed_micros: 0,
                warned: false,
                stopped: false,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, LedgerState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Books one spend, returning the threshold signals it fired (in
    /// canonical order: warning before stop). Overflow-refused records
    /// (u128 sums past realistic magnitudes) book nothing and signal
    /// nothing — the ledger state is unchanged, mirroring the core
    /// ledger's contract.
    pub fn record(&self, spend: Spend) -> Vec<BudgetSignal> {
        let mut state = self.lock();
        let mut sink = Sink(Vec::new());
        if state.ledger.record(spend, &mut sink).is_err() {
            return Vec::new();
        }
        state.consumed_micros = state.consumed_micros.saturating_add(spend.money().0);
        state.warned |= sink.0.iter().any(
            |signal| matches!(signal, BudgetSignal::Warning(w) if w.dimension == Dimension::Money),
        );
        state.stopped |= sink
            .0
            .iter()
            .any(|signal| matches!(signal, BudgetSignal::Stop(r) if r.exhaustion().dimension == Dimension::Money));
        sink.0
    }

    /// The gate the engine loop faces before inference: the typed stop
    /// reason once any capped dimension is at or over its cap.
    pub fn preflight(&self) -> Result<(), StopReason> {
        self.lock().ledger.preflight()
    }

    /// The badge state for HUD surfaces.
    #[must_use]
    pub fn snapshot(&self) -> CostSnapshot {
        let state = self.lock();
        CostSnapshot {
            cap_micros: self.cap_micros,
            consumed_micros: state.consumed_micros,
            warned: state.warned,
            stopped: state.stopped,
        }
    }
}

/// Collecting sink for one record's signals.
struct Sink(Vec<BudgetSignal>);

impl BudgetSignalSink for Sink {
    fn on_signal(&mut self, signal: BudgetSignal) {
        self.0.push(signal);
    }
}

/// Wire-side micros: u64 (the internally-tagged event frames cannot
/// carry u128 — see [`SessionEvent::CostWarning`]). Saturation is the
/// documented contract: a spend past u64::MAX micro-USD (~18 trillion
/// USD) is a budget that already exploded, and the event still fires.
fn wire_micros(micros: u128) -> u64 {
    u64::try_from(micros).unwrap_or(u64::MAX)
}

fn cost_event(signal: BudgetSignal) -> SessionEvent {
    match signal {
        BudgetSignal::Warning(w) => SessionEvent::CostWarning {
            consumed_micros: wire_micros(w.consumed),
            cap_micros: wire_micros(w.cap),
        },
        BudgetSignal::Stop(reason) => SessionEvent::CostStop {
            consumed_micros: wire_micros(reason.exhaustion().consumed),
            cap_micros: wire_micros(reason.exhaustion().cap),
        },
    }
}

/// The engine-loop wiring: the outermost inference decorator. Before
/// the provider call the ledger's preflight gate refuses typed once the
/// cap is reached; after the turn returns, the provider-REPORTED usage
/// (priced against the same builtin table the Usage frame cites) books
/// into the ledger and threshold signals publish on the owning host's
/// stream. `host` is the ledger's OWNING session — a delegation child's
/// decorator carries the PARENT host, so child spend aggregates into
/// the parent ledger and the parent stream carries the badge events.
pub struct CostLedgerInference {
    host: Arc<SessionHost>,
    ledger: Arc<SessionLedger>,
    identity: ModelIdentity,
    inner: Box<dyn Inference>,
}

impl CostLedgerInference {
    /// Wrap `inner`. The inputs are independent construction facts
    /// (owning host, ledger, identity, inner stack); a grouping struct
    /// would only re-wrap them for this single caller — same shape as
    /// [`crate::usage::UsageInference::new`].
    #[must_use]
    pub fn new(
        host: Arc<SessionHost>,
        ledger: Arc<SessionLedger>,
        identity: ModelIdentity,
        inner: Box<dyn Inference>,
    ) -> Self {
        Self {
            host,
            ledger,
            identity,
            inner,
        }
    }
}

#[async_trait]
impl Inference for CostLedgerInference {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        if let Err(reason) = self.ledger.preflight() {
            // The cap is spent: no provider call. The refusal rides the
            // inference seam; CostStopExit maps it onto the loop's quiet
            // EndTurn flow. The Stop EVENT already landed when the
            // crossing record booked; any pending fallback ask resolved
            // inside that record's call.
            return Err(InferenceError::Failure(reason.to_string()));
        }
        let turn = self.inner.infer(request, cancel).await?;
        if let Some(usage) = turn.usage {
            let priced = builtin_prices().account(
                &self.identity,
                austringer_providers::event::Usage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_read_tokens: usage.cache_read_tokens,
                    cache_write_tokens: usage.cache_write_tokens,
                },
            );
            let cost = match priced.priced {
                Priced::Priced(breakdown) => Cost::Priced(MicroUsd(breakdown.total_usd_micros)),
                Priced::Unpriced => Cost::Unpriced,
            };
            let spend = Spend {
                tokens: TokenCount(
                    usage
                        .input_tokens
                        .saturating_add(usage.output_tokens)
                        .saturating_add(usage.cache_read_tokens)
                        .saturating_add(usage.cache_write_tokens),
                ),
                cost,
            };
            for signal in self.ledger.record(spend) {
                self.host.publish(cost_event(signal));
            }
        }
        Ok(turn)
    }
}

/// The ExitOnError stage piece of the cost contract: provider-seam budget
/// refusals end the turn QUIETLY (the CostStop event on the session
/// stream is the failure surface — signal.rs's graceful-stop), while
/// every other inference error keeps the wrapped policy (fail-fast).
pub struct CostStopExit {
    inner: Box<dyn ExitOnError>,
}

impl CostStopExit {
    /// Wraps `inner` (the shipped composition passes the core default).
    #[must_use]
    pub fn new(inner: Box<dyn ExitOnError>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl ExitOnError for CostStopExit {
    async fn on_error(
        &mut self,
        ctx: &mut StageCtx<'_>,
        error: &InferenceError,
    ) -> Result<ErrorFlow, StageError> {
        let message = error.message();
        if message.starts_with(MONEY_STOP_PREFIX) || message.starts_with(TOKEN_STOP_PREFIX) {
            return Ok(ErrorFlow::EndTurn);
        }
        self.inner.on_error(ctx, error).await
    }
}
