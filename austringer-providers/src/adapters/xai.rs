//! xAI (Grok) adapter: the chat-completions wire at `api.x.ai`.

use std::sync::Arc;

use async_trait::async_trait;

use super::SseProvider;
use super::chat::ChatCompletionsCodec;
use crate::auth::{AuthKind, CredentialSource};
use crate::error::ProviderStreamError;
use crate::identity::{CompletionRequest, ModelIdentity};
use crate::provider::{Provider, ProviderEventStream};
use crate::transport::Transport;

/// Adapter for xAI Grok models over `POST /v1/chat/completions`.
pub struct XaiAdapter(SseProvider);

impl XaiAdapter {
    /// Serves `model` at the public xAI endpoint.
    pub fn new(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        model: &str,
    ) -> Self {
        Self::at_base(transport, auth, model, "https://api.x.ai")
    }

    /// Custom base (proxies).
    #[must_use]
    pub fn at_base(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        model: &str,
        base_url: &str,
    ) -> Self {
        let identity = ModelIdentity::new("xai", model);
        let codec = ChatCompletionsCodec {
            url: format!("{}/v1/chat/completions", base_url.trim_end_matches('/')),
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
impl Provider for XaiAdapter {
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
