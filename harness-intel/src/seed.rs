//! The seeded catalog table (research SS33.1 control matrix, SS33.2
//! behavioral table, and `.omo/research/model-profiles.md` SS3). Every
//! row cites its source fact; the effort grids and trap lists are
//! PER-MODEL facts — fragmentation inside a family is the documented
//! reason this table exists (SS33.1 invariant 3).

use super::catalog::{CatalogEntry, EffortValue as E, Family, Feature, ThinkingLevel};
use super::dialect::ProtocolDialect;

/// All seeded entries.
pub fn entries() -> Vec<CatalogEntry> {
    vec![
        // Anthropic: effort low->max, default high; budget_tokens
        // deprecated on 4.6, 400 on 4.7+; prefill 400 on 4.6+ (SS33.1).
        CatalogEntry {
            model: "claude-opus-4-6".into(),
            family: Family::Anthropic,
            dialect: ProtocolDialect::Anthropic,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            default_effort: E::High,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![Feature::Prefill],
        },
        // 4.7 respects effort more strictly; start xhigh for coding
        // (SS33.2); budget_tokens now 400s.
        CatalogEntry {
            model: "claude-opus-4-7".into(),
            family: Family::Anthropic,
            dialect: ProtocolDialect::Anthropic,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            default_effort: E::Xhigh,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![Feature::ThinkingBudget, Feature::Prefill],
        },
        // Sonnet 4.6: medium is the documented balance (SS33.2).
        CatalogEntry {
            model: "claude-sonnet-4-6".into(),
            family: Family::Anthropic,
            dialect: ProtocolDialect::Anthropic,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            default_effort: E::Medium,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![Feature::Prefill],
        },
        // Fable 5.1: thinking always-on; forced tool_choice 400s; the
        // per-message effort beta exists here (SS33.1).
        CatalogEntry {
            model: "claude-fable-5-1".into(),
            family: Family::Anthropic,
            dialect: ProtocolDialect::Anthropic,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            default_effort: E::Medium,
            thinking_floor: None,
            per_message_effort_beta: true,
            traps: vec![Feature::ForcedToolChoice],
        },
        // Opus 5: thinking on, effort beta available (SS33.1).
        CatalogEntry {
            model: "claude-opus-5".into(),
            family: Family::Anthropic,
            dialect: ProtocolDialect::Anthropic,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            default_effort: E::High,
            thinking_floor: None,
            per_message_effort_beta: true,
            traps: vec![],
        },
        // GPT-5.5: none..xhigh (no max), default medium; verbosity is the
        // length knob, orthogonal to effort (SS33.1).
        CatalogEntry {
            model: "gpt-5-5".into(),
            family: Family::OpenAi,
            dialect: ProtocolDialect::OpenAiResponses,
            supported_efforts: vec![E::None, E::Low, E::Medium, E::High, E::Xhigh],
            default_effort: E::Medium,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![],
        },
        // GPT-6 Astra: no `none` (400), floor low; sampling params 400
        // (SS33.1).
        CatalogEntry {
            model: "gpt-6-astra".into(),
            family: Family::OpenAi,
            dialect: ProtocolDialect::OpenAiResponses,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh],
            default_effort: E::Medium,
            thinking_floor: None,
            per_message_effort_beta: true,
            traps: vec![Feature::Sampling],
        },
        // Gemini 2.5 Pro: thinking_budget 128..=32768 (0 invalid);
        // budget+level together 400; cannot disable thinking (SS33.1).
        CatalogEntry {
            model: "gemini-2-5-pro".into(),
            family: Family::Google,
            dialect: ProtocolDialect::GoogleGenAi,
            supported_efforts: vec![E::Low, E::Medium, E::High],
            default_effort: E::High,
            thinking_floor: Some(ThinkingLevel::Low),
            per_message_effort_beta: false,
            traps: vec![Feature::ThinkingBudgetWithLevel],
        },
        // Gemini 3 Pro: thinking_level replaces budget across the
        // generation boundary; Pro floor is low (SS33.1).
        CatalogEntry {
            model: "gemini-3-pro".into(),
            family: Family::Google,
            dialect: ProtocolDialect::GoogleGenAi,
            supported_efforts: vec![E::Low, E::Medium, E::High],
            default_effort: E::High,
            thinking_floor: Some(ThinkingLevel::Low),
            per_message_effort_beta: false,
            traps: vec![Feature::ThinkingBudgetWithLevel],
        },
        CatalogEntry {
            model: "gemini-3-1-pro".into(),
            family: Family::Google,
            dialect: ProtocolDialect::GoogleGenAi,
            supported_efforts: vec![E::Low, E::Medium, E::High],
            default_effort: E::High,
            thinking_floor: Some(ThinkingLevel::Low),
            per_message_effort_beta: false,
            traps: vec![Feature::ThinkingBudgetWithLevel],
        },
        // Grok: always-on reasoning at default high — the most expensive
        // default of the four families; penalties/stop error on reasoning
        // models (SS33.1).
        CatalogEntry {
            model: "grok-4-6".into(),
            family: Family::Xai,
            dialect: ProtocolDialect::Xai,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh],
            default_effort: E::High,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![Feature::Penalties, Feature::StopSequences],
        },
        // DeepSeek reasoner: strongest architect-mode editor; wire usage
        // is cache-hit-inclusive (research SS6 DeepSeek pattern).
        CatalogEntry {
            model: "deepseek-reasoner".into(),
            family: Family::DeepSeek,
            dialect: ProtocolDialect::DeepSeek,
            supported_efforts: vec![E::Low, E::Medium, E::High],
            default_effort: E::Medium,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![],
        },
    ]
}
