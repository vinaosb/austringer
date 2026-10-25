//! The dyn-safe streaming `Provider` trait (goose pattern).
//!
//! Object-safe and stream-oriented: one method opens a typed event stream;
//! failures before the first byte surface as the `Err` of the open call,
//! failures during the stream surface through the stream itself. Dropping
//! the returned [`ProviderEventStream`] ABANDONS the request (the todo-5
//! seam's cancel semantics: abandon, never drain).

use async_trait::async_trait;

use crate::error::ProviderStreamError;
use crate::event::ProviderEvent;
use crate::identity::{CompletionRequest, ModelIdentity};

/// Pull-based source of typed provider events. Adapters for real transports
/// implement this; the public surface is [`ProviderEventStream`].
#[async_trait]
pub trait EventSource: Send {
    /// Yields the next event, or `None` when the wire stream ended.
    /// Dropped-between-calls sources are simply never polled again.
    async fn next(&mut self) -> Option<Result<ProviderEvent, ProviderStreamError>>;
}

/// A typed provider-event stream. Dropping it cancels the underlying
/// request — nothing drains in the background.
pub struct ProviderEventStream {
    source: Box<dyn EventSource>,
}

impl ProviderEventStream {
    /// Wraps an adapter-supplied source.
    pub fn new(source: Box<dyn EventSource>) -> Self {
        Self { source }
    }

    /// Yields the next typed event, or `None` at end of stream.
    pub async fn next(&mut self) -> Option<Result<ProviderEvent, ProviderStreamError>> {
        self.source.next().await
    }
}

/// A streaming completion provider. Dyn-safe by construction: async methods
/// are desugared through `async_trait`, so `Arc<dyn Provider>` drives the
/// whole austringer.
///
/// Identity is explicit and configuration-owned: [`Provider::identity`] is
/// the identity this provider serves, and the request names its target. They
/// are cross-checked against the wire (see [`crate::wire`]) — a model is
/// never inferred from an endpoint, and never substituted silently.
#[async_trait]
pub trait Provider: Send + Sync {
    /// The identity this provider serves (explicit; never inferred).
    fn identity(&self) -> &ModelIdentity;

    /// Opens a streaming completion for `request`. Retrying and resuming are
    /// NOT this method's concern — the recovery layer ([`crate::turn`]) owns
    /// them; adapters stay honest one-request-one-stream.
    async fn stream_completion(
        &self,
        request: &CompletionRequest,
    ) -> Result<ProviderEventStream, ProviderStreamError>;
}
