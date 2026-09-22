//! Adapter composition and the inference wrappers: family-to-adapter
//! wiring, the unconfigured posture, and the wall-clock deadline that
//! keeps a hung provider typed instead of turn-fatal.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use harness_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest,
};
use harness_providers::adapter::InferenceAdapter;
use harness_providers::adapters::{
    AnthropicAdapter, CompatAdapter, GoogleAdapter, OpenAiAdapter, XaiAdapter,
};
use harness_providers::auth::CredentialSource;
use harness_providers::provider::Provider;
use harness_providers::retry::RetryPolicy;
use harness_providers::transport::Transport;
use harness_providers::turn::RecoveryConfig;

use super::selection::ProviderFamily;

/// A validated selection with the credential source resolved: ready to
/// compose adapters over any [`Transport`].
#[derive(Clone)]
pub struct ResolvedProvider {
    family: ProviderFamily,
    model: String,
    credentials: Arc<dyn CredentialSource>,
    base_url: Option<String>,
    inference_timeout: Duration,
}

impl ResolvedProvider {
    pub(crate) fn new(
        family: ProviderFamily,
        model: String,
        credentials: Arc<dyn CredentialSource>,
        base_url: Option<String>,
        inference_timeout: Duration,
    ) -> Self {
        Self {
            family,
            model,
            credentials,
            base_url,
            inference_timeout,
        }
    }

    /// Builds the shared session factory: every call composes a fresh
    /// adapter + recovery stack over `transport`.
    #[must_use]
    pub fn compose_factory(
        &self,
        transport: Arc<dyn Transport>,
    ) -> Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> {
        let provider = self.compose_provider(transport);
        let timeout = self.inference_timeout;
        Arc::new(move || {
            Box::new(DeadlineInference {
                inner: Box::new(InferenceAdapter::new(
                    Arc::clone(&provider),
                    daemon_recovery_config(),
                    0x5EED_0000_0032,
                )),
                deadline: timeout,
            })
        })
    }

    fn compose_provider(&self, transport: Arc<dyn Transport>) -> Arc<dyn Provider> {
        let model = &self.model;
        let auth = Arc::clone(&self.credentials);
        let base = self.base_url.as_deref();
        match self.family {
            ProviderFamily::Anthropic => match base {
                Some(base) => Arc::new(AnthropicAdapter::custom(
                    transport, auth, model, base, 4096,
                )),
                None => Arc::new(AnthropicAdapter::new(transport, auth, model)),
            },
            ProviderFamily::OpenAi => match base {
                Some(base) => Arc::new(OpenAiAdapter::chat_at(transport, auth, model, base)),
                None => Arc::new(OpenAiAdapter::chat(transport, auth, model)),
            },
            ProviderFamily::Google => match base {
                Some(base) => Arc::new(GoogleAdapter::at_base(transport, auth, model, base)),
                None => Arc::new(GoogleAdapter::new(transport, auth, model)),
            },
            ProviderFamily::Xai => match base {
                Some(base) => Arc::new(XaiAdapter::at_base(transport, auth, model, base)),
                None => Arc::new(XaiAdapter::new(transport, auth, model)),
            },
            ProviderFamily::Compat => {
                let base = base.unwrap_or("http://127.0.0.1:11434/v1");
                Arc::new(CompatAdapter::new(transport, auth, base, model))
            }
        }
    }
}

pub(crate) fn unconfigured_factory() -> Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> {
    Arc::new(|| Box::new(UnconfiguredInference))
}

/// Recovery knobs for daemon-composed turns: bounded retries with
/// short backoffs, the T9 resume budget, and the 60 s per-event idle
/// ceiling on top of the wall-clock [`DeadlineInference`] budget.
fn daemon_recovery_config() -> RecoveryConfig {
    RecoveryConfig {
        retry: RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(8),
            multiplier: 2.0,
        },
        max_resumes: 4,
        idle_timeout: Duration::from_secs(60),
    }
}

/// The unconfigured posture: every inference call fails typed, with the
/// exact config keys to set.
struct UnconfiguredInference;

#[async_trait]
impl Inference for UnconfiguredInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Err(InferenceError::Failure(
            "no provider configured: set HARNESS_PROVIDER (one of anthropic, openai, \
             google, xai, compat), HARNESS_MODEL, and — for keyed families — \
             HARNESS_PROVIDER_KEY_ENV naming the environment variable that holds \
             the API key, then restart the daemon"
                .to_owned(),
        ))
    }
}

/// Wall-clock budget around one inference call: a provider whose
/// transport never answers surfaces a typed timeout and the in-flight
/// future (with its stream) is dropped — abandon, never drain.
struct DeadlineInference {
    inner: Box<dyn Inference>,
    deadline: Duration,
}

#[async_trait]
impl Inference for DeadlineInference {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        match tokio::time::timeout(self.deadline, self.inner.infer(request, cancel)).await {
            Ok(outcome) => outcome,
            Err(_elapsed) => Err(InferenceError::Failure(format!(
                "provider timeout: inference exceeded the {}s budget \
                 (HARNESS_PROVIDER_TIMEOUT_SECS adjusts it)",
                self.deadline.as_secs_f64()
            ))),
        }
    }
}
