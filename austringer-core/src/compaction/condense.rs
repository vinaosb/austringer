//! The same-model condense seam (plan todo 13, Roo lesson).
//!
//! Compaction condenses with the SESSION'S OWN model by design: a
//! summary written in a different model's dialect degrades the very
//! context it is supposed to preserve. [`SessionCondenser`] therefore
//! carries the session's [`ModelId`] and rejects a request stamped with
//! a different model as a typed [`ModelMismatch`] — the cheap-model
//! utility roles of the model-profile todo operate at THEIR OWN
//! sessions and tasks, never by swapping this session's condenser.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;

use crate::event::RecordedEvent;
pub use crate::eviction::CondenseError;

/// Identity of the model a session runs on (newtype: one name, one
/// concept — never a bare string).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelId(String);

impl ModelId {
    /// Wraps a model identifier string.
    #[must_use]
    pub fn new(model: impl Into<String>) -> Self {
        Self(model.into())
    }

    /// The identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The default condense prompt handed to the session's own model.
pub const DEFAULT_CONDENSE_PROMPT: &str = "\
Summarize the preceding conversation for continued work. Preserve:
architectural decisions and their rationale, unresolved bugs with
symptoms, file paths under discussion, and any stated constraints or
facts. Omit tool-output bulk. Write in the session's own voice.";

/// The effective condense prompt: the user's override when supplied,
/// otherwise the doctrine default.
#[must_use]
pub fn effective_prompt(custom: Option<&str>) -> &str {
    custom.unwrap_or(DEFAULT_CONDENSE_PROMPT)
}

/// One doctrine condense request: the span to fold, the SESSION's
/// model identity, and the effective prompt.
pub struct DoctrineCondenseRequest<'a> {
    /// The events being folded into the summary.
    pub span: &'a [RecordedEvent],
    /// The identity of the session requesting the condense.
    pub session_model: &'a ModelId,
    /// The condense prompt (user override or doctrine default).
    pub prompt: &'a str,
}

/// The second-model condense seam (async by design; tests inject a
/// rule-based script, the daemon wires the real provider).
#[async_trait]
pub trait DoctrineCondenser: Send + Sync {
    /// Condenses `request.span` under `request.prompt`.
    async fn condense(&self, request: DoctrineCondenseRequest<'_>)
    -> Result<String, CondenseError>;
}

/// A condenser registered for ONE session's model. The typed rejection
/// for a different-model request — the anti-swap guard.
#[derive(Debug, thiserror::Error)]
pub struct ModelMismatch {
    /// The model this condenser is registered for (the session's own).
    pub session_model: ModelId,
    /// The model the incoming request was stamped with.
    pub requested_model: ModelId,
}

impl fmt::Display for ModelMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "same-model condensing violated: condenser registered for {} but \
             request carries {} — condensing with a different model breaks the \
             dialect of the preserved context (Roo lesson); cheap-model roles \
             run at their own sessions, never by swapping this session's \
             condenser",
            self.session_model, self.requested_model
        )
    }
}

/// Why a session condenser refused a request.
#[derive(Debug, thiserror::Error)]
pub enum CondenseRefusal {
    /// The request carries a different session's model identity.
    #[error("{0}")]
    ModelMismatch(#[from] ModelMismatch),
    /// The underlying seam failed.
    #[error(transparent)]
    Seam(#[from] CondenseError),
}

/// The session-scoped condenser: one session, one model, one seam.
pub struct SessionCondenser {
    session_model: ModelId,
    inner: Arc<dyn DoctrineCondenser>,
}

impl SessionCondenser {
    /// Registers `inner` for `session_model`.
    #[must_use]
    pub fn for_session(session_model: ModelId, inner: Arc<dyn DoctrineCondenser>) -> Self {
        Self {
            session_model,
            inner,
        }
    }

    /// The model this condenser serves.
    #[must_use]
    pub fn session_model(&self) -> &ModelId {
        &self.session_model
    }

    /// Condenses, refusing typed when the request's session model is
    /// not the model this condenser was registered for.
    pub async fn condense(
        &self,
        request: DoctrineCondenseRequest<'_>,
    ) -> Result<String, CondenseRefusal> {
        if request.session_model != &self.session_model {
            return Err(ModelMismatch {
                session_model: self.session_model.clone(),
                requested_model: request.session_model.clone(),
            }
            .into());
        }
        self.inner
            .condense(request)
            .await
            .map_err(CondenseRefusal::Seam)
    }
}
