//! Adapter composition and the inference wrappers: family-to-adapter
//! wiring, the unconfigured posture, and the wall-clock deadline that
//! keeps a hung provider typed instead of turn-fatal.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use austringer_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest,
};
use austringer_core::media_delivery::DeliveryCapabilities;
use austringer_providers::adapter::InferenceAdapter;
use austringer_providers::adapters::{
    AnthropicAdapter, CompatAdapter, GoogleAdapter, OpenAiAdapter, XaiAdapter,
};
use austringer_providers::auth::CredentialSource;
use austringer_providers::identity::ModelIdentity;
use austringer_providers::media::MediaDelivery;
use austringer_providers::provider::Provider;
use austringer_providers::retry::RetryPolicy;
use austringer_providers::transport::Transport;
use austringer_providers::turn::RecoveryConfig;

use super::selection::ProviderFamily;
use crate::usage::{ComposedInference, InferenceFactory};

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

    /// The identity of the model this provider serves — what Usage
    /// frames cite and the fallback contract switches between.
    #[must_use]
    pub fn identity(&self) -> ModelIdentity {
        ModelIdentity::new(self.family.canonical(), &self.model)
    }

    /// Builds the shared session factory: every call composes a fresh
    /// adapter + recovery stack over `transport`, yielded WITH the
    /// served identity so sessions can emit Usage frames.
    #[must_use]
    pub fn compose_factory(&self, transport: Arc<dyn Transport>) -> InferenceFactory {
        let provider = self.compose_provider(transport);
        let timeout = self.inference_timeout;
        let identity = self.identity();
        Arc::new(move || ComposedInference {
            inference: Box::new(DeadlineInference {
                inner: Box::new(InferenceAdapter::new(
                    Arc::clone(&provider),
                    daemon_recovery_config(),
                    0x5EED_0000_0032,
                )),
                deadline: timeout,
            }),
            identity: Some(identity.clone()),
        })
    }

    fn compose_provider(&self, transport: Arc<dyn Transport>) -> Arc<dyn Provider> {
        let model = &self.model;
        let auth = Arc::clone(&self.credentials);
        let base = self.base_url.as_deref();
        match self.family {
            ProviderFamily::Anthropic => match base {
                Some(base) => {
                    Arc::new(AnthropicAdapter::custom(transport, auth, model, base, 4096))
                }
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

    /// Composes the model-mediated projection factory (plan M10): the
    /// same adapter + deadline stack as [`Self::compose_factory`], with
    /// the PROJECTOR model's delivery capabilities and the session media
    /// context attached, so a queued projection rides the real family
    /// wire (native payload in, projection text out). `caps` must be the
    /// M7-mapped flags of `model_id`'s model — the resolver's capability
    /// gate is the caller's to enforce before building this.
    ///
    /// # Errors
    /// [`io::Error`] when the session media context cannot be opened.
    pub fn compose_projection_factory(
        &self,
        transport: Arc<dyn Transport>,
        caps: DeliveryCapabilities,
        session_dir: &Path,
    ) -> io::Result<Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>> {
        let provider = self.compose_provider(transport);
        let timeout = self.inference_timeout;
        let session_dir = session_dir.to_path_buf();
        Ok(Arc::new(move || {
            Box::new(SessionMediaInference {
                provider: Arc::clone(&provider),
                caps,
                session_dir: session_dir.clone(),
                timeout,
            })
        }))
    }
}

/// One projection-capable inference call: the adapter is composed per
/// call with a freshly opened session [`MediaDelivery`] (bytes fetch +
/// derived-cache inlining), because the delivery context is owned by the
/// adapter it is attached to. A session dir that stops opening mid-flight
/// is a typed inference failure — never a marker-less prompt.
struct SessionMediaInference {
    provider: Arc<dyn Provider>,
    caps: DeliveryCapabilities,
    session_dir: PathBuf,
    timeout: Duration,
}

#[async_trait]
impl Inference for SessionMediaInference {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        let delivery = MediaDelivery::open(&self.session_dir).map_err(|error| {
            InferenceError::Failure(format!(
                "projection session media context unavailable: {error}"
            ))
        })?;
        let mut inner = DeadlineInference {
            inner: Box::new(
                InferenceAdapter::new(
                    Arc::clone(&self.provider),
                    daemon_recovery_config(),
                    0x5EED_0000_0032,
                )
                .with_capabilities(self.caps)
                .with_media_delivery(delivery),
            ),
            deadline: self.timeout,
        };
        inner.infer(request, cancel).await
    }
}

pub(crate) fn unconfigured_factory() -> InferenceFactory {
    Arc::new(|| ComposedInference::plain(Box::new(UnconfiguredInference)))
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
            "no provider configured: set AUSTRINGER_PROVIDER (one of anthropic, openai, \
             google, xai, compat), AUSTRINGER_MODEL, and — for keyed families — \
             AUSTRINGER_PROVIDER_KEY_ENV naming the environment variable that holds \
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
                 (AUSTRINGER_PROVIDER_TIMEOUT_SECS adjusts it)",
                self.deadline.as_secs_f64()
            ))),
        }
    }
}
