//! Explicit protocol dialects (plan todo 34): the wire vocabulary each
//! model actually speaks — reasoning-content format, usage semantics,
//! tool-call identity rules, finish-reason vocabulary.
//!
//! A dialect is a CATALOG FACT keyed on the model, never inferred from an
//! endpoint URL: the dsh thinkingFormat bug guessed a dialect from the URL
//! and produced 72k-char reasoning bloat against a 6k tuned baseline
//! (research SS6). Usage normalization enforces DISJOINT token accounting:
//! the harness ledger's `input_tokens` counts cache-MISS tokens only, with
//! cache reads/writes carried separately — the DeepSeek wire reports
//! `prompt_tokens` cache-hit-inclusive and must be split before it enters
//! any ledger (research SS6 translate.ts precedents).

use harness_providers::event::{FinishReason, Usage};
use harness_providers::identity::ModelIdentity;
use serde::{Deserialize, Serialize};

/// The named wire dialects. Keyed explicitly per model in the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolDialect {
    /// Anthropic Messages API (thinking blocks, block-id tool calls).
    Anthropic,
    /// OpenAI Responses API (reasoning items, call ids).
    OpenAiResponses,
    /// OpenAI Chat Completions (index-based tool calls, trailing usage).
    OpenAiChat,
    /// Google GenAI (thought signatures, STOP/MAX_TOKENS vocabulary).
    GoogleGenAi,
    /// xAI Grok API (OpenAI-chat-shaped, always-on reasoning).
    Xai,
    /// DeepSeek (OpenAI-chat-shaped, cache-hit-inclusive prompt tokens).
    DeepSeek,
}

/// How reasoning content reaches the wire (the thinkingFormat contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningFormat {
    /// Opaque `thinking` content blocks (Anthropic).
    ThinkingBlocks,
    /// A `reasoning_content` field on message/delta objects.
    ReasoningContentField,
    /// Thought signatures round-tripped verbatim (Google stateless FC).
    ThoughtSignatures,
    /// The dialect reports no reasoning content at all.
    NoneReported,
}

/// How tool calls carry identity across the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolIdentity {
    /// Provider-assigned stable call ids survive the whole call.
    StableCallIds,
    /// Calls are identified by index only (chat-completions shape).
    IndexedCalls,
    /// Gateway-synthesized ids; identity may be absent on some chunks
    /// (OpenRouter-style null-identity) — the configured identity wins.
    GatewaySynthesized,
}

/// Usage-reporting semantics for one dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageSpec {
    /// True when the wire `prompt_tokens` INCLUDES cache-hit tokens (the
    /// DeepSeek pattern: prompt_tokens = cache_hit + cache_miss) and must
    /// be split before entering the disjoint harness ledger.
    pub prompt_tokens_includes_cache_hit: bool,
    /// True when the authoritative usage arrives in a trailing chunk that
    /// the wire layer must wait for (dsh translate.ts precedent) rather
    /// than trusting an earlier or absent report.
    pub usage_in_trailing_chunk: bool,
}

/// The full dialect specification for one named dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialectSpec {
    /// Reasoning-content format.
    pub reasoning_format: ReasoningFormat,
    /// Usage semantics.
    pub usage: UsageSpec,
    /// Tool-call identity rules.
    pub tool_identity: ToolIdentity,
}

/// The static spec for a named dialect.
#[must_use]
pub fn spec(dialect: ProtocolDialect) -> DialectSpec {
    use ProtocolDialect as D;
    use ReasoningFormat as R;
    use ToolIdentity as T;
    let exclusive = UsageSpec {
        prompt_tokens_includes_cache_hit: false,
        usage_in_trailing_chunk: true,
    };
    match dialect {
        D::Anthropic => DialectSpec {
            reasoning_format: R::ThinkingBlocks,
            usage: UsageSpec {
                prompt_tokens_includes_cache_hit: false,
                usage_in_trailing_chunk: false,
            },
            tool_identity: T::StableCallIds,
        },
        D::OpenAiResponses => DialectSpec {
            reasoning_format: R::ReasoningContentField,
            usage: exclusive,
            tool_identity: T::StableCallIds,
        },
        D::OpenAiChat => DialectSpec {
            reasoning_format: R::NoneReported,
            usage: exclusive,
            tool_identity: T::IndexedCalls,
        },
        D::GoogleGenAi => DialectSpec {
            reasoning_format: R::ThoughtSignatures,
            usage: UsageSpec {
                prompt_tokens_includes_cache_hit: false,
                usage_in_trailing_chunk: false,
            },
            tool_identity: T::StableCallIds,
        },
        D::Xai => DialectSpec {
            reasoning_format: R::ReasoningContentField,
            usage: exclusive,
            tool_identity: T::IndexedCalls,
        },
        D::DeepSeek => DialectSpec {
            reasoning_format: R::ReasoningContentField,
            usage: UsageSpec {
                prompt_tokens_includes_cache_hit: true,
                usage_in_trailing_chunk: true,
            },
            tool_identity: T::GatewaySynthesized,
        },
    }
}

/// The usage semantics for a named dialect.
#[must_use]
pub fn usage_spec(dialect: ProtocolDialect) -> UsageSpec {
    spec(dialect).usage
}

/// Wire-reported usage before normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireUsage {
    /// Prompt-side tokens as the wire reported them (semantics per
    /// [`UsageSpec::prompt_tokens_includes_cache_hit`]).
    pub prompt_tokens: u64,
    /// Completion-side tokens.
    pub completion_tokens: u64,
    /// Cache-hit tokens, when the wire reported them.
    pub cache_hit_tokens: Option<u64>,
    /// Cache-write tokens, when the wire reported them.
    pub cache_write_tokens: Option<u64>,
}

/// Normalizes wire usage into the harness's DISJOINT ledger shape:
/// `input_tokens` counts cache-MISS tokens only; cache reads and writes
/// ride their own fields. Inclusive dialects are split, exclusive dialects
/// pass through unchanged.
#[must_use]
pub fn normalize_usage(spec: &UsageSpec, wire: WireUsage) -> Usage {
    let hit = wire.cache_hit_tokens.unwrap_or(0);
    let input = if spec.prompt_tokens_includes_cache_hit {
        wire.prompt_tokens.saturating_sub(hit)
    } else {
        wire.prompt_tokens
    };
    Usage::with_cache(
        input,
        wire.completion_tokens,
        hit,
        wire.cache_write_tokens.unwrap_or(0),
    )
}

/// Identity stamping under gateway null-identity (dsh translate.ts): an
/// absent or null wire model keeps the CONFIGURED identity; a DIFFERENT
/// reported model wins, because substitution must stay observable (the T9
/// identity contract).
#[must_use]
pub fn stamped_identity(wire_model: Option<&str>, configured: &ModelIdentity) -> ModelIdentity {
    match wire_model {
        Some(served) if served != configured.model => ModelIdentity {
            provider: configured.provider.clone(),
            model: served.to_owned(),
        },
        _ => configured.clone(),
    }
}

/// Maps a wire finish-reason word through the dialect's vocabulary. Words
/// outside the vocabulary return `None` — unknown is surfaced, never
/// guessed into a typed reason.
#[must_use]
pub fn finish_reason(dialect: ProtocolDialect, wire: &str) -> Option<FinishReason> {
    use FinishReason as F;
    use ProtocolDialect as D;
    let table: &[(&str, F)] = match dialect {
        D::Anthropic => &[
            ("end_turn", F::EndTurn),
            ("max_tokens", F::Length),
            ("tool_use", F::ToolCall),
            ("refusal", F::Refusal),
        ],
        D::OpenAiResponses | D::Xai | D::DeepSeek | D::OpenAiChat => &[
            ("stop", F::EndTurn),
            ("length", F::Length),
            ("tool_calls", F::ToolCall),
            ("content_filter", F::Refusal),
        ],
        D::GoogleGenAi => &[
            ("STOP", F::EndTurn),
            ("MAX_TOKENS", F::Length),
            ("SAFETY", F::Refusal),
        ],
    };
    table
        .iter()
        .find(|(word, _)| *word == wire)
        .map(|(_, reason)| *reason)
}
