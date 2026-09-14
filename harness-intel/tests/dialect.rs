//! Dialect semantics: DISJOINT token accounting (the DeepSeek pattern —
//! wire `prompt_tokens` is cache-hit-inclusive, so it must be split into
//! cache-read + cache-miss before it enters the harness ledger),
//! trailing-usage-chunk expectations, gateway null-identity stamping, and
//! the finish-reason vocabulary — per the dsh translate.ts precedents
//! (research SS6).

use harness_intel::dialect::{
    ProtocolDialect, WireUsage, normalize_usage, stamped_identity, usage_spec,
};
use harness_providers::event::FinishReason;
use harness_providers::identity::ModelIdentity;

#[test]
fn deepseek_usage_normalizes_to_disjoint_accounting() {
    // Given: DeepSeek-style wire usage where prompt_tokens (1000) INCLUDES
    // the 800 cache-hit tokens.
    let wire = WireUsage {
        prompt_tokens: 1000,
        completion_tokens: 200,
        cache_hit_tokens: Some(800),
        cache_write_tokens: None,
    };

    // When: normalized under the DeepSeek dialect.
    let usage = normalize_usage(&usage_spec(ProtocolDialect::DeepSeek), wire);

    // Then: the ledger fields are DISJOINT — input counts the 200 cache-MISS
    // tokens only, cache_read carries the 800 separately, never both.
    assert_eq!(usage.input_tokens, 200);
    assert_eq!(usage.output_tokens, 200);
    assert_eq!(usage.cache_read_tokens, 800);
    assert_eq!(usage.cache_write_tokens, 0);
    assert_eq!(usage.input_tokens + usage.cache_read_tokens, 1000);
}

#[test]
fn exclusive_semantics_pass_through_unchanged() {
    // Given: Anthropic-style wire usage where input_tokens (1000) already
    // EXCLUDES the 800 cache-read tokens.
    let wire = WireUsage {
        prompt_tokens: 1000,
        completion_tokens: 200,
        cache_hit_tokens: Some(800),
        cache_write_tokens: Some(50),
    };

    let usage = normalize_usage(&usage_spec(ProtocolDialect::Anthropic), wire);

    // Then: no double subtraction — the disjoint invariant holds as reported.
    assert_eq!(usage.input_tokens, 1000);
    assert_eq!(usage.cache_read_tokens, 800);
    assert_eq!(usage.cache_write_tokens, 50);
    assert_eq!(usage.output_tokens, 200);
}

#[test]
fn missing_cache_detail_yields_zero_cache_fields() {
    let wire = WireUsage {
        prompt_tokens: 500,
        completion_tokens: 50,
        cache_hit_tokens: None,
        cache_write_tokens: None,
    };
    let usage = normalize_usage(&usage_spec(ProtocolDialect::OpenAiResponses), wire);
    assert_eq!(usage.input_tokens, 500);
    assert_eq!(usage.cache_read_tokens, 0);
    assert_eq!(usage.cache_write_tokens, 0);
}

#[test]
fn trailing_usage_chunk_expectation_is_explicit_per_dialect() {
    // The dsh translate.ts precedent: OpenAI-compatible streams deliver the
    // authoritative usage in a TRAILING chunk; treating an earlier/absent
    // chunk as final mis-counts. The dialect table records the expectation
    // so the wire layer knows what to wait for.
    assert!(usage_spec(ProtocolDialect::OpenAiChat).usage_in_trailing_chunk);
    assert!(!usage_spec(ProtocolDialect::Anthropic).usage_in_trailing_chunk);
}

#[test]
fn gateway_null_identity_keeps_the_configured_identity() {
    // Gateways (OpenRouter-style) frequently OMIT the model on usage chunks.
    // The dialect rule: null/absent wire identity => keep the configured
    // identity; a DIFFERENT reported identity wins (substitution stays
    // observable — the T9 identity contract).
    let configured = ModelIdentity::new("openai", "gpt-5-5");

    let absent = stamped_identity(None, &configured);
    assert_eq!(absent, configured);

    let served_same = stamped_identity(Some("gpt-5-5"), &configured);
    assert_eq!(served_same, configured);

    let served_other = stamped_identity(Some("gpt-6-astra"), &configured);
    assert_eq!(served_other.model, "gpt-6-astra");
    assert_eq!(served_other.provider, "openai");
}

#[test]
fn finish_vocabulary_maps_per_dialect_and_unknown_is_none() {
    // Same wire word, different dialects, correct typed reason — and an
    // unknown word NEVER guesses.
    let openai = ProtocolDialect::OpenAiChat;
    let anthropic = ProtocolDialect::Anthropic;
    let google = ProtocolDialect::GoogleGenAi;

    assert_eq!(
        harness_intel::dialect::finish_reason(openai, "stop"),
        Some(FinishReason::EndTurn)
    );
    assert_eq!(
        harness_intel::dialect::finish_reason(anthropic, "end_turn"),
        Some(FinishReason::EndTurn)
    );
    assert_eq!(
        harness_intel::dialect::finish_reason(anthropic, "max_tokens"),
        Some(FinishReason::Length)
    );
    assert_eq!(
        harness_intel::dialect::finish_reason(google, "MAX_TOKENS"),
        Some(FinishReason::Length)
    );
    assert_eq!(
        harness_intel::dialect::finish_reason(google, "STOP"),
        Some(FinishReason::EndTurn)
    );

    // Cross-dialect: a word from one vocabulary is unknown in another.
    assert_eq!(
        harness_intel::dialect::finish_reason(openai, "end_turn"),
        None
    );
    // And nonsense is unknown everywhere — never a guess.
    for d in [
        ProtocolDialect::Anthropic,
        ProtocolDialect::OpenAiChat,
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::GoogleGenAi,
        ProtocolDialect::Xai,
        ProtocolDialect::DeepSeek,
    ] {
        assert_eq!(harness_intel::dialect::finish_reason(d, "banana"), None);
    }
}
