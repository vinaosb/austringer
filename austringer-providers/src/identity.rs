//! Explicit model identity and the fallback-confirmation contract.
//!
//! Identity is carried explicitly on every request, event, and response — it
//! is NEVER inferred from an endpoint URL (an OpenAI-compatible base URL can
//! serve any model; the austringer refuses to guess). A model substitution is
//! possible only through [`FallbackRequest::acknowledge`]: the explicit,
//! typed confirmation flow. There is no automatic provider swap anywhere.

use std::collections::BTreeMap;

use austringer_core::tools::ToolSchema;
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
/// model-context prompt, the session-stable wire surfaces (system prompt
/// and declared tool schemas), native media payloads for capable models,
/// transport headers, and an optional acknowledged fallback audit trail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// The model this request targets (post-fallback target when a fallback
    /// was acknowledged and attached).
    pub identity: ModelIdentity,
    /// The rendered model-context input (llm-visible events + focus block).
    pub prompt: String,
    /// The stable system prompt for the provider's native system surface
    /// (research §33.1). `None` = the legacy shape: no system field rides
    /// the wire.
    #[serde(default)]
    pub system: Option<String>,
    /// The declared tool surface for the provider's native tools shape.
    /// Empty = no tools field rides the wire (a provider cannot emit
    /// tool calls without declarations — research §33.1).
    #[serde(default)]
    pub tools: Vec<ToolSchema>,
    /// Bytes at the END of `prompt` that mutate turn-to-turn (the
    /// volatile focus tail; the stable-first/volatile-last layout law,
    /// research §24.6). Native-cache adapters split the stable prefix on
    /// this boundary; `0` = the whole prompt is stable.
    #[serde(default)]
    pub volatile_tail_bytes: usize,
    /// Native media payloads planned for a capable model (plan M8). The
    /// payloads carry content-addressed references plus bytes resolved at
    /// request-build time; bytes are serde-skipped and Debug-redacted so
    /// they exist only inside the rendered wire request.
    #[serde(default)]
    pub media: Vec<crate::media::NativeMedia>,
    /// Extra request headers (auth material, per-provider options).
    pub headers: BTreeMap<String, String>,
    /// Audit trail present only when this request is an acknowledged
    /// fallback from another model.
    pub fallback: Option<AcknowledgedFallback>,
    turn_id: TurnId,
}

impl CompletionRequest {
    /// Creates a request for `identity` with `prompt`. The stable
    /// [`TurnId`] derives from the content (identity, prompt, and wire
    /// surfaces), so a re-issued (resumed) request keeps the same id.
    pub fn new(identity: ModelIdentity, prompt: impl Into<String>) -> Self {
        let mut request = Self {
            identity,
            prompt: prompt.into(),
            system: None,
            tools: Vec::new(),
            volatile_tail_bytes: 0,
            media: Vec::new(),
            headers: BTreeMap::new(),
            fallback: None,
            turn_id: TurnId(String::new()),
        };
        request.rehash();
        request
    }

    /// Attaches the stable system prompt. An empty string attaches
    /// nothing (the legacy shape).
    #[must_use]
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        let system = system.into();
        self.system = (!system.is_empty()).then_some(system);
        self.rehash();
        self
    }

    /// Attaches the declared tool surface.
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<ToolSchema>) -> Self {
        self.tools = tools;
        self.rehash();
        self
    }

    /// Declares how many trailing prompt bytes are volatile (the focus
    /// tail). Does not change the [`TurnId`]: the id keys content, and
    /// the tail bytes are already inside `prompt`.
    #[must_use]
    pub fn with_volatile_tail(mut self, bytes: usize) -> Self {
        self.volatile_tail_bytes = bytes;
        self
    }

    /// Attaches natively-delivered media (plan M8). The [`TurnId`] is
    /// unchanged: it derives from the prompt, which already embeds the
    /// content-hash-keyed marker of every planned block, so the same
    /// media yields the same id with or without the payload list.
    pub fn with_native_media(mut self, media: Vec<crate::media::NativeMedia>) -> Self {
        self.media = media;
        self
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

    /// Recomputes the content-derived id over identity + prompt + wire
    /// surfaces. The tail-byte count is layout metadata, not content,
    /// and stays out of the hash; the media payloads stay out too (see
    /// [`Self::with_native_media`]).
    fn rehash(&mut self) {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.identity.provider.as_bytes());
        hasher.update(&[0]);
        hasher.update(self.identity.model.as_bytes());
        hasher.update(&[0]);
        hasher.update(self.prompt.as_bytes());
        hasher.update(&[0]);
        hasher.update(self.system.as_deref().unwrap_or_default().as_bytes());
        hasher.update(&[0]);
        for tool in &self.tools {
            // Byte-stable by type: sorted-key maps, declaration-order fields.
            hasher.update(serde_json::to_string(tool).unwrap_or_default().as_bytes());
            hasher.update(&[0]);
        }
        self.turn_id = TurnId(hasher.finalize().to_hex().to_string());
    }
}
