//! Explicit model identity and the fallback-confirmation contract.
//!
//! Identity is carried explicitly on every request, event, and response — it
//! is NEVER inferred from an endpoint URL (an OpenAI-compatible base URL can
//! serve any model; the harness refuses to guess). A model substitution is
//! possible only through [`FallbackRequest::acknowledge`]: the explicit,
//! typed confirmation flow. There is no automatic provider swap anywhere.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The identity of a model: provider name plus model id.
///
/// Part of every typed provider event and of the completion envelope, so a
/// silent substitution is observable at every layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelIdentity {
    /// Provider name (`"anthropic"`, `"openai"`, ...). Configuration-owned;
    /// never derived from the wire.
    pub provider: String,
    /// Model id (`"claude-4-5"`, ...). Cross-checked against the wire when
    /// the provider reports the model it actually served.
    pub model: String,
}

impl ModelIdentity {
    /// Creates an explicit identity.
    pub fn new(provider: &str, model: &str) -> Self {
        Self {
            provider: provider.to_owned(),
            model: model.to_owned(),
        }
    }
}

/// A stable identity for one completion request: a content-derived id that is
/// identical across re-issues of the same request (resume paths re-issue the
/// request verbatim and must keep the same id).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnId(String);

impl TurnId {
    /// The id as a string (log and snapshot correlation).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A pending model switch that MUST be acknowledged before any request may
/// target the replacement identity. Constructing this type changes nothing;
/// only [`FallbackRequest::acknowledge`] produces the proof of consent.
#[derive(Debug, Clone, PartialEq)]
pub struct FallbackRequest {
    /// The identity being switched away from.
    pub from: ModelIdentity,
    /// The proposed replacement identity.
    pub to: ModelIdentity,
    /// Why the switch is proposed (surfaced to the user for the decision).
    pub reason: String,
}

impl FallbackRequest {
    /// Describes a proposed fallback. Inert until acknowledged.
    pub fn new(from: ModelIdentity, to: ModelIdentity, reason: &str) -> Self {
        Self {
            from,
            to,
            reason: reason.to_owned(),
        }
    }

    /// The explicit confirmation step. Consumes the proposal and yields the
    /// only constructor of [`AcknowledgedFallback`] outside deserialization.
    pub fn acknowledge(self) -> AcknowledgedFallback {
        AcknowledgedFallback {
            from: self.from,
            to: self.to,
            reason: self.reason,
        }
    }
}

/// Proof that a [`FallbackRequest`] was acknowledged. Attachable to a NEW
/// completion request via [`CompletionRequest::with_fallback`]; there is no
/// API to swap the model of a live stream or turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcknowledgedFallback {
    from: ModelIdentity,
    to: ModelIdentity,
    reason: String,
}

impl AcknowledgedFallback {
    /// The identity switched away from.
    pub fn from(&self) -> &ModelIdentity {
        &self.from
    }

    /// The acknowledged replacement identity.
    pub fn to(&self) -> &ModelIdentity {
        &self.to
    }

    /// Why the switch was proposed.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// One streaming completion request: explicit target identity, rendered
/// model-context prompt, transport headers, and an optional acknowledged
/// fallback audit trail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// The model this request targets (post-fallback target when a fallback
    /// was acknowledged and attached).
    pub identity: ModelIdentity,
    /// The rendered model-context input (llm-visible events + focus block).
    pub prompt: String,
    /// Extra request headers (auth material, per-provider options).
    pub headers: BTreeMap<String, String>,
    /// Audit trail present only when this request is an acknowledged
    /// fallback from another model.
    pub fallback: Option<AcknowledgedFallback>,
    turn_id: TurnId,
}

impl CompletionRequest {
    /// Creates a request for `identity` with `prompt`. The stable
    /// [`TurnId`] derives from the content, so a re-issued (resumed) request
    /// keeps the same id.
    pub fn new(identity: ModelIdentity, prompt: impl Into<String>) -> Self {
        let prompt = prompt.into();
        let mut hasher = blake3::Hasher::new();
        hasher.update(identity.provider.as_bytes());
        hasher.update(&[0]);
        hasher.update(identity.model.as_bytes());
        hasher.update(&[0]);
        hasher.update(prompt.as_bytes());
        Self {
            identity,
            prompt,
            headers: BTreeMap::new(),
            fallback: None,
            turn_id: TurnId(hasher.finalize().to_hex().to_string()),
        }
    }

    /// Attaches a header (builder style). Headers ride on the request but are
    /// excluded from the [`TurnId`] (auth material must not key identity).
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_owned(), value.to_owned());
        self
    }

    /// Retargets this request to the acknowledged fallback identity and
    /// records the audit trail. The ONLY way a request changes model.
    pub fn with_fallback(mut self, acknowledged: AcknowledgedFallback) -> Self {
        self.identity = acknowledged.to.clone();
        self.fallback = Some(acknowledged);
        self
    }

    /// The stable request identity (content-derived; re-issue-stable).
    pub fn turn_id(&self) -> &TurnId {
        &self.turn_id
    }
}
