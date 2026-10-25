//! OpenAI-compatible base-URL adapter: one chat-completions implementation
//! pointed at any endpoint that speaks the shape — Ollama, vLLM, OpenRouter,
//! Groq, LiteLLM, ...
//!
//! The identity's provider string is `openai-compat`, which is also the
//! cost-table key: arbitrary endpoints are UNPRICED by default (tokens-only
//! accounting) until the user supplies price overrides at runtime.

use std::sync::Arc;

use async_trait::async_trait;

use super::SseProvider;
use super::chat::ChatCompletionsCodec;
use crate::auth::{AuthKind, CredentialSource};
use crate::error::ProviderStreamError;
use crate::identity::{CompletionRequest, ModelIdentity};
use crate::provider::{Provider, ProviderEventStream};
use crate::transport::Transport;

/// Adapter for OpenAI-compatible endpoints. `base_url` is everything up to
/// (but excluding) `/chat/completions` — e.g. `http://localhost:11434/v1`
/// for Ollama or `https://openrouter.ai/api/v1` for OpenRouter. Auth may be
/// [`crate::auth::NoCredentials`] for local servers.
pub struct CompatAdapter(SseProvider);

impl CompatAdapter {
    /// Serves `model` at `{base_url}/chat/completions`.
    pub fn new(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        base_url: &str,
        model: &str,
    ) -> Self {
        let identity = ModelIdentity::new("openai-compat", model);
        let codec = ChatCompletionsCodec {
            url: format!("{}/chat/completions", base_url.trim_end_matches('/')),
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
impl Provider for CompatAdapter {
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
