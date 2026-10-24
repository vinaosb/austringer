//! Usage-frame emission (plan todo 1): the session's inference stack
//! composed WITH its model identity, and the [`UsageInference`]
//! decorator broadcasting exactly one `ServerMsg::Usage` per
//! successful assistant turn carrying the provider-REPORTED counts
//! (never invented: a turn without reported usage emits no frame).
//! Costs come from the builtin integer price tables; unpriced models
//! report `cost_usd_micros: None`. v1 wires receive nothing (the
//! compat downgrade withholds the frame per connection).

use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;

use async_trait::async_trait;

use harness_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest,
};
use harness_providers::cost::{CostTable, Priced};
use harness_providers::identity::ModelIdentity;

use crate::attach::SessionHost;
use crate::protocol::ServerMsg;

/// What a session factory produces: the inference stack plus the
/// identity it serves. Identity rides WITH the stack (not beside it in
/// config) so a session cannot mislabel the model its Usage frames
/// cite. `None` = identity-less stacks (scripted test fakes, the
/// unconfigured posture) — no Usage emission.
pub struct ComposedInference {
    /// The per-session inference stack.
    pub inference: Box<dyn Inference>,
    /// The model identity the stack serves, when composed from a
    /// provider selection.
    pub identity: Option<ModelIdentity>,
}

impl ComposedInference {
    /// An identity-less stack (scripted tests; Usage frames off).
    #[must_use]
    pub fn plain(inference: Box<dyn Inference>) -> Self {
        Self {
            inference,
            identity: None,
        }
    }
}

/// The session-factory contract: each call yields one session's stack.
pub type InferenceFactory = Arc<dyn Fn() -> ComposedInference + Send + Sync>;

/// Everything the daemon state accepts as a session factory: the
/// identity-aware contract itself, or a legacy identity-less factory
/// (scripted test stacks — adapted, no Usage emission).
pub trait IntoSessionFactory {
    /// Convert into the factory contract.
    fn into_factory(self) -> InferenceFactory;
}

impl IntoSessionFactory for InferenceFactory {
    fn into_factory(self) -> InferenceFactory {
        self
    }
}

impl IntoSessionFactory for Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> {
    fn into_factory(self) -> InferenceFactory {
        Arc::new(move || ComposedInference::plain(self()))
    }
}

/// The builtin price tables, built once per process (shared by the
/// Usage-frame accounting and the session cost ledger — one table, one
/// money figure per turn).
pub(crate) fn builtin_prices() -> &'static CostTable {
    static TABLE: OnceLock<CostTable> = OnceLock::new();
    TABLE.get_or_init(CostTable::builtin)
}

/// Inference decorator emitting the per-turn Usage frame for the
/// composed identity. Failures of the inner inference emit nothing (an
/// aborted turn accounts for no tokens).
pub struct UsageInference {
    inner: Box<dyn Inference>,
    host: Arc<SessionHost>,
    identity: ModelIdentity,
    effort: Option<String>,
}

impl UsageInference {
    /// Wrap `inner` for `host`. The inputs are independent construction
    /// facts (session, identity, effort, inner inference); a grouping
    /// struct would only re-wrap them for this single caller.
    pub fn new(
        host: Arc<SessionHost>,
        identity: ModelIdentity,
        effort: Option<String>,
        inner: Box<dyn Inference>,
    ) -> Self {
        Self {
            inner,
            host,
            identity,
            effort,
        }
    }
}

#[async_trait]
impl Inference for UsageInference {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        let turn = self.inner.infer(request, cancel).await?;
        if let Some(usage) = turn.usage {
            let priced = builtin_prices().account(
                &self.identity,
                harness_providers::event::Usage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_read_tokens: usage.cache_read_tokens,
                    cache_write_tokens: usage.cache_write_tokens,
                },
            );
            let cost_usd_micros = match priced.priced {
                Priced::Priced(breakdown) => u64::try_from(breakdown.total_usd_micros).ok(),
                Priced::Unpriced => None,
            };
            self.host.broadcast(ServerMsg::Usage {
                // The driver increments the counter before the turn
                // runs, so its current value is the active turn number.
                turn: self.host.turn_counter.load(Ordering::SeqCst),
                model: self.identity.clone(),
                effort: self.effort.clone(),
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_read: usage.cache_read_tokens,
                cache_write: usage.cache_write_tokens,
                cost_usd_micros,
            });
        }
        Ok(turn)
    }
}
