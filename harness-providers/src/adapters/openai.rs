//! OpenAI adapter: chat completions and the Responses API over the same
//! injected transport and credential source.

use std::sync::Arc;

use async_trait::async_trait;

use super::SseProvider;
use super::chat::ChatCompletionsCodec;
use super::responses::ResponsesCodec;
use crate::auth::{AuthKind, CredentialSource};
use crate::error::ProviderStreamError;
use crate::identity::{CompletionRequest, ModelIdentity};
use crate::provider::{Provider, ProviderEventStream};
use crate::transport::Transport;

/// Adapter for OpenAI models.
///
/// Two stable wire surfaces, chosen at construction:
/// * [`OpenAiAdapter::chat`] — `/v1/chat/completions` (also the shape every
///   OpenAI-compatible endpoint speaks).
/// * [`OpenAiAdapter::responses`] — `/v1/responses` (the Responses API).
pub struct OpenAiAdapter(SseProvider);

impl OpenAiAdapter {
    /// Serves `model` over chat completions at the public endpoint.
    pub fn chat(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        model: &str,
    ) -> Self {
        Self::chat_at(transport, auth, model, "https://api.openai.com")
    }

    /// Chat completions against a custom base (proxies, Azure-style paths).
    #[must_use]
    pub fn chat_at(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        model: &str,
        base_url: &str,
    ) -> Self {
        let identity = ModelIdentity::new("openai", model);
        let codec = ChatCompletionsCodec {
            url: format!("{}/v1/chat/completions", base_url.trim_end_matches('/')),
        };
        Self(SseProvider::new(transport, auth, Box::new(codec), identity))
    }

    /// Serves `model` over the Responses API at the public endpoint.
    pub fn responses(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        model: &str,
    ) -> Self {
        Self::responses_at(transport, auth, model, "https://api.openai.com")
    }

    /// Responses API against a custom base.
    #[must_use]
    pub fn responses_at(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        model: &str,
        base_url: &str,
    ) -> Self {
        let identity = ModelIdentity::new("openai", model);
        let codec = ResponsesCodec {
            url: format!("{}/v1/responses", base_url.trim_end_matches('/')),
        };
        Self(SseProvider::new(transport, auth, Box::new(codec), identity))
    }

    /// The billing surface for the cost HUD.
    #[must_use]
    pub fn auth_kind(&self) -> AuthKind {
        self.0.auth_kind()
    }
}

#[async_trait]
impl Provider for OpenAiAdapter {
    fn identity(&self) -> &ModelIdentity {
        self.0.identity()
    }

    async fn stream_completion(
        &self,
        request: &CompletionRequest,
    ) -> Result<ProviderEventStream, ProviderStreamError> {
        self.0.stream_completion(request).await
    }
}
