//! The seeded catalog table (research SS33.1 control matrix, SS33.2
//! behavioral table, and `.omo/research/model-profiles.md` SS3). Every
//! row cites its source fact; the effort grids and trap lists are
//! PER-MODEL facts — fragmentation inside a family is the documented
//! reason this table exists (SS33.1 invariant 3).
//!
//! # Media capability citations (plan M7)
//!
//! Flags are conservative: `true` ONLY where a cited row documents
//! support; absent/unknown stays `false` (fail-closed). Provider-doc
//! citations below were verified as fetched sources in harness-research
//! SS33.8; the M8 wire table (media-formats plan) names each family's
//! native media encoding.
//!
//! | model(s) | vision | audio_input | video_input | cited source row |
//! |---|---|---|---|---|
//! | claude-opus-4-6/4-7/5, claude-sonnet-4-6, claude-fable-5-1 | true | false | false | platform.claude.com Messages API documents image input; no audio/video input blocks exist (SS33.8; M8 Anthropic native wire = image source only) |
//! | gpt-5-5, gpt-6-astra | true | false | false | developers.openai.com Responses API documents image input for the GPT-5/6 series (SS33.8). The OpenAI `input_audio` WIRE format exists (M8) but no cited row documents AUDIO acceptance for these models — conservative false until one does |
//! | gemini-2-5-pro, gemini-3-pro, gemini-3-1-pro | true | true | true | ai.google.dev Gemini input docs: Pro models accept image, audio, AND video input (SS33.8; M8 Google inline_data) |
//! | grok-4-6 | true | false | false | docs.x.ai documents grok-4 image input; M8 xAI native wire = image_url only; no audio/video input documented |
//! | deepseek-reasoner | false | false | false | DeepSeek API is text-only (no image input endpoint); `.omo/research/model-profiles.md` SS3 "DeepSeek" row is a text-editing profile |
//! | gpt-6-sol, gpt-6-1-sol, gpt-6-luna, gemini-4-argon, grok-4-7, claude-opus-5-5, claude-sonnet-5-5, claude-haiku-5-5 | false | false | false | 2026-10 wave (todo 10): no cited row yet — the SS33.8 fetches (2026-09-12) predate these models (Sol/Luna Sept 22, 6.1 Sol Sept 29, Grok 4.7 Sept 21, Argon Oct 19, 5.5 family Sept–Oct); conservative false + TODO markers in the entries until a verified row documents them |

use super::catalog::{
    Capabilities, CatalogEntry, EffortValue as E, Family, Feature, ThinkingLevel,
};
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
            // claude image input (platform.claude.com, SS33.8); no
            // audio/video input blocks.
            capabilities: Capabilities {
                vision: true,
                audio_input: false,
                video_input: false,
            },
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
            // claude image input (platform.claude.com, SS33.8); no
            // audio/video input blocks.
            capabilities: Capabilities {
                vision: true,
                audio_input: false,
                video_input: false,
            },
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
            // claude image input (platform.claude.com, SS33.8); no
            // audio/video input blocks.
            capabilities: Capabilities {
                vision: true,
                audio_input: false,
                video_input: false,
            },
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
            // claude image input (platform.claude.com, SS33.8); no
            // audio/video input blocks.
            capabilities: Capabilities {
                vision: true,
                audio_input: false,
                video_input: false,
            },
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
            // claude image input (platform.claude.com, SS33.8); no
            // audio/video input blocks.
            capabilities: Capabilities {
                vision: true,
                audio_input: false,
                video_input: false,
            },
        },
        // Opus 5.5 (2026-10 wave: Zed BYOK v1.21.0 Sept 23; Amp ran it
        // at effort `medium` Sept 28; Antigravity v1.3.6 Oct 15): the
        // family ladder stands (SS33.1) and `medium` is a documented
        // legal rung. TODO(todo-10 wave): per-model traps, effort
        // beta, and media flags are UNVERIFIED (SS33.8 predates the
        // 5.5 line) — beta documented for Fable 5.1/Mythos 5.1/Opus 5
        // only; no 5.5 trap row exists, so none are carried.
        CatalogEntry {
            model: "claude-opus-5-5".into(),
            family: Family::Anthropic,
            dialect: ProtocolDialect::Anthropic,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            default_effort: E::High,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![],
            capabilities: Capabilities::default(),
        },
        // Sonnet 5.5 (Antigravity v1.3.6, Oct 15): same family
        // posture as Opus 5.5. TODO(todo-10 wave): per-model default,
        // traps, and media flags unverified — family ladder + family
        // default stand.
        CatalogEntry {
            model: "claude-sonnet-5-5".into(),
            family: Family::Anthropic,
            dialect: ProtocolDialect::Anthropic,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            default_effort: E::High,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![],
            capabilities: Capabilities::default(),
        },
        // Haiku 5.5 (Junie Oct 12; Copilot Oct 9-week; Devin Fusion
        // sidekick Oct 7): the cheap-tier 5.5. TODO(todo-10 wave):
        // per-model default, traps, and media flags unverified —
        // family ladder + family default stand.
        CatalogEntry {
            model: "claude-haiku-5-5".into(),
            family: Family::Anthropic,
            dialect: ProtocolDialect::Anthropic,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            default_effort: E::High,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![],
            capabilities: Capabilities::default(),
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
            // GPT-5.5 vision: Responses image input documented
            // (developers.openai.com, SS33.8); input_audio wire exists
            // family-wide but no row documents audio for THIS model.
            capabilities: Capabilities {
                vision: true,
                audio_input: false,
                video_input: false,
            },
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
            // GPT-6 Astra vision: Responses image input documented
            // (developers.openai.com, SS33.8); audio/video input not
            // documented for this model — conservative false.
            capabilities: Capabilities {
                vision: true,
                audio_input: false,
                video_input: false,
            },
        },
        // GPT-6 Sol (in Codex Sept 22; Zed BYOK v1.21.0 Sept 23; goose
        // v1.52.0 Sept 23): R2 keys the adapter on the GPT-6
        // generation, whose only SS33.1-documented member is Astra —
        // no `none`, tops out at xhigh. TODO(todo-10 wave): the
        // Sol-specific grid is UNVERIFIED (SS33.1 predates Sept 22);
        // Astra's sampling/effort-none traps are Astra-named and do
        // NOT transfer; the per-message effort beta is Astra-only.
        CatalogEntry {
            model: "gpt-6-sol".into(),
            family: Family::OpenAi,
            dialect: ProtocolDialect::OpenAiResponses,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh],
            default_effort: E::Medium,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![],
            capabilities: Capabilities::default(),
        },
        // GPT-6.1 Sol (in Codex Sept 29; Zed BYOK v1.22.0 Sept 30;
        // Copilot successor for GPT-5.5 on Oct 19): same GPT-6
        // generation posture as Sol. TODO(todo-10 wave): per-model
        // grid unverified.
        CatalogEntry {
            model: "gpt-6-1-sol".into(),
            family: Family::OpenAi,
            dialect: ProtocolDialect::OpenAiResponses,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh],
            default_effort: E::Medium,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![],
            capabilities: Capabilities::default(),
        },
        // GPT-6 Luna (dropped with Sol in Codex Sept 22; Copilot
        // successor Oct 19): same GPT-6 generation posture.
        // TODO(todo-10 wave): per-model grid unverified.
        CatalogEntry {
            model: "gpt-6-luna".into(),
            family: Family::OpenAi,
            dialect: ProtocolDialect::OpenAiResponses,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh],
            default_effort: E::Medium,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![],
            capabilities: Capabilities::default(),
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
            // Gemini Pro multimodal input: image, audio, AND video
            // documented (ai.google.dev, SS33.8; M8 inline_data).
            capabilities: Capabilities {
                vision: true,
                audio_input: true,
                video_input: true,
            },
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
            // Gemini Pro multimodal input: image, audio, AND video
            // documented (ai.google.dev, SS33.8; M8 inline_data).
            capabilities: Capabilities {
                vision: true,
                audio_input: true,
                video_input: true,
            },
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
            // Gemini Pro multimodal input: image, audio, AND video
            // documented (ai.google.dev, SS33.8; M8 inline_data).
            capabilities: Capabilities {
                vision: true,
                audio_input: true,
                video_input: true,
            },
        },
        // Gemini 4 Argon (Zed v1.24.3 Oct 19; Junie Oct 16): R2 — the
        // thinking_level parameter generation (3+) is the last
        // documented Google contract, so 4 conservatively keeps it:
        // level only, never alongside budget, Pro-style low floor.
        // TODO(todo-10 wave): Argon's tier, media flags, and grid are
        // UNVERIFIED (SS33.1 predates Oct 19).
        CatalogEntry {
            model: "gemini-4-argon".into(),
            family: Family::Google,
            dialect: ProtocolDialect::GoogleGenAi,
            supported_efforts: vec![E::Low, E::Medium, E::High],
            default_effort: E::High,
            thinking_floor: Some(ThinkingLevel::Low),
            per_message_effort_beta: false,
            traps: vec![Feature::ThinkingBudgetWithLevel],
            capabilities: Capabilities::default(),
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
            // grok-4 image input documented (docs.x.ai, SS33.8; M8 xAI
            // wire = image_url only); no audio/video input documented.
            capabilities: Capabilities {
                vision: true,
                audio_input: false,
                video_input: false,
            },
        },
        // Grok 4.7 (Cursor + Grok Build + Copilot all SKUs, Sept 21):
        // same grok-4 generation as 4.6 (R2) — the reasoning-model
        // parameter traps carry over and the family ladder (docs.x.ai,
        // SS33.1) stands. TODO(todo-10 wave): 4.7-specific media flags
        // and grid are UNVERIFIED (SS33.8 predates Sept 21).
        CatalogEntry {
            model: "grok-4-7".into(),
            family: Family::Xai,
            dialect: ProtocolDialect::Xai,
            supported_efforts: vec![E::Low, E::Medium, E::High, E::Xhigh],
            default_effort: E::High,
            thinking_floor: None,
            per_message_effort_beta: false,
            traps: vec![Feature::Penalties, Feature::StopSequences],
            capabilities: Capabilities::default(),
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
            // TEXT-ONLY: no image input endpoint in the DeepSeek API;
            // model-profiles.md SS3 "DeepSeek" row is text-only.
            capabilities: Capabilities {
                vision: false,
                audio_input: false,
                video_input: false,
            },
        },
    ]
}
