//! The verified 2026-10 model wave (plan todo 10): GPT-6 Sol / GPT-6.1
//! Sol / GPT-6 Luna, Gemini 4 Argon, Grok 4.7, Claude Opus/Sonnet/Haiku
//! 5.5 — eight catalog entries added on top of the untouched twelve.
//! Every grid/trap decision below is receipt-cited (ecosystem sweep,
//! verified 2026-10-20, `.omo/research/harness-research.md` SS34-SS35
//! sources): GPT-6 Sol+Luna in Codex Sept 22; GPT-6.1 Sol Sept 29;
//! Grok 4.7 in Cursor/Grok Build/Copilot Sept 21; Gemini 4 Argon (Zed
//! v1.24.3, Oct 19); Opus 5.5 (Zed v1.21.0 Sept 23, Amp "powers
//! medium" Sept 28, Antigravity Oct 15); Sonnet 5.5 (Antigravity
//! v1.3.6 Oct 15); Haiku 5.5 (Junie Oct 12, Copilot Oct 9-week,
//! Devin Oct 7).
//!
//! Unverified axes are CONSERVATIVE + TODO-marked in the seed (per-model
//! grids on the OpenAI wave, media flags everywhere): fail-closed is the
//! documented posture (SS33.8 confidence section — unverified items are
//! carried as caveats, never assertions).

use austringer_intel::catalog::{
    Capabilities, Catalog, CatalogError, EffortValue as E, Family, Feature, ThinkingLevel,
    WireKnobs,
};
use austringer_intel::dialect::ProtocolDialect;

/// The eight wave models with the grid each entry must carry.
const WAVE: [(&str, Family, ProtocolDialect, &[E], E); 8] = [
    // GPT-6 generation follows Astra's documented shape (R2: the vendor
    // adapter keys on model-name generation; Astra is the only SS33.1-
    // documented GPT-6 member): no `none`, tops out at xhigh.
    (
        "gpt-6-sol",
        Family::OpenAi,
        ProtocolDialect::OpenAiResponses,
        &[E::Low, E::Medium, E::High, E::Xhigh],
        E::Medium,
    ),
    (
        "gpt-6-1-sol",
        Family::OpenAi,
        ProtocolDialect::OpenAiResponses,
        &[E::Low, E::Medium, E::High, E::Xhigh],
        E::Medium,
    ),
    (
        "gpt-6-luna",
        Family::OpenAi,
        ProtocolDialect::OpenAiResponses,
        &[E::Low, E::Medium, E::High, E::Xhigh],
        E::Medium,
    ),
    // Gemini 4 keeps the 3+ thinking_level generation (R2); Argon is
    // treated Pro-floor until a row says otherwise (conservative).
    (
        "gemini-4-argon",
        Family::Google,
        ProtocolDialect::GoogleGenAi,
        &[E::Low, E::Medium, E::High],
        E::High,
    ),
    // grok-4-7 shares the grok-4 generation dialect with 4.6 (R2).
    (
        "grok-4-7",
        Family::Xai,
        ProtocolDialect::Xai,
        &[E::Low, E::Medium, E::High, E::Xhigh],
        E::High,
    ),
    // Anthropic family ladder low->max, family default high (SS33.1).
    (
        "claude-opus-5-5",
        Family::Anthropic,
        ProtocolDialect::Anthropic,
        &[E::Low, E::Medium, E::High, E::Xhigh, E::Max],
        E::High,
    ),
    (
        "claude-sonnet-5-5",
        Family::Anthropic,
        ProtocolDialect::Anthropic,
        &[E::Low, E::Medium, E::High, E::Xhigh, E::Max],
        E::High,
    ),
    (
        "claude-haiku-5-5",
        Family::Anthropic,
        ProtocolDialect::Anthropic,
        &[E::Low, E::Medium, E::High, E::Xhigh, E::Max],
        E::High,
    ),
];

fn knobs() -> WireKnobs {
    WireKnobs::default()
}

fn effort_knob(effort: E) -> WireKnobs {
    WireKnobs {
        effort: Some(effort),
        ..WireKnobs::default()
    }
}

#[test]
fn wave_entries_are_seeded_with_cited_grids() {
    // Given: the seeded catalog. When/Then: every wave model resolves
    // with the family, dialect, grid, and default its receipts support.
    let catalog = Catalog::seeded();
    for (model, family, dialect, grid, default) in WAVE {
        let entry = catalog
            .entry(model)
            .unwrap_or_else(|e| panic!("{model} must be seeded after todo 10: {e}"));
        assert_eq!(entry.family, family, "{model} family");
        assert_eq!(entry.dialect, dialect, "{model} dialect is explicit");
        assert_eq!(entry.supported_efforts, grid, "{model} effort grid");
        assert_eq!(entry.default_effort, default, "{model} default effort");
        // The default must itself be a legal rung of the grid.
        assert!(
            grid.contains(&default),
            "{model} default {default} outside its own grid"
        );
    }
}

#[test]
fn gpt6_wave_rejects_none_and_max_fail_closed() {
    // The GPT-6 generation posture (R2, keyed on Astra's documented
    // grid): `none` is the named family trap, `max` is outside the
    // grid, and the xhigh rung the bracket grammar routes
    // (`model[effort=xhigh]`) resolves.
    let catalog = Catalog::seeded();
    for model in ["gpt-6-sol", "gpt-6-1-sol", "gpt-6-luna"] {
        let err = catalog
            .validate(model, &effort_knob(E::None))
            .expect_err("none is not in the GPT-6 wave grid");
        match err {
            CatalogError::Trap(trap) => assert_eq!(trap.feature, Feature::EffortNone, "{model}"),
            other => panic!("{model}: expected EffortNone, got {other:?}"),
        }
        let err = catalog
            .validate(model, &effort_knob(E::Max))
            .expect_err("no OpenAI model documents max");
        match err {
            CatalogError::Trap(trap) => assert_eq!(
                trap.feature,
                Feature::EffortUnsupported { requested: E::Max },
                "{model}"
            ),
            other => panic!("{model}: expected EffortUnsupported, got {other:?}"),
        }
        for legal in [E::Low, E::Medium, E::High, E::Xhigh] {
            assert!(
                catalog.validate(model, &effort_knob(legal)).is_ok(),
                "{model} must accept {legal}"
            );
        }
    }
}

#[test]
fn grok_4_7_extends_the_grok4_dialect() {
    // grok-4-7 is the same generation as grok-4-6 (R2): the reasoning-
    // model parameter traps carry over, and the xhigh rung stays legal.
    let catalog = Catalog::seeded();

    let mut k = knobs();
    k.penalties = true;
    k.stop_sequences = true;
    match catalog.validate("grok-4-7", &k) {
        Err(CatalogError::Trap(trap)) => {
            assert_eq!(trap.feature, Feature::Penalties);
            assert_eq!(trap.model, "grok-4-7");
        }
        other => panic!("expected penalties trap, got {other:?}"),
    }

    let mut stop_only = knobs();
    stop_only.stop_sequences = true;
    match catalog.validate("grok-4-7", &stop_only) {
        Err(CatalogError::Trap(trap)) => assert_eq!(trap.feature, Feature::StopSequences),
        other => panic!("expected stop-sequences trap, got {other:?}"),
    }

    // Clean knobs and the bracket-grammar xhigh rung both pass.
    assert!(catalog.validate("grok-4-7", &knobs()).is_ok());
    assert!(catalog.validate("grok-4-7", &effort_knob(E::Xhigh)).is_ok());
}

#[test]
fn gemini_4_argon_keeps_the_level_generation_dialect() {
    // Gemini 4 is unverified in-repo, so it conservatively keeps the
    // 3+ generation contract (R2): thinking_level only, never alongside
    // thinking_budget, with the Pro-tier floor.
    let catalog = Catalog::seeded();

    let mut both = knobs();
    both.thinking_budget = Some(2048);
    both.thinking_level = Some(ThinkingLevel::High);
    match catalog.validate("gemini-4-argon", &both) {
        Err(CatalogError::Trap(trap)) => {
            assert_eq!(trap.feature, Feature::ThinkingBudgetWithLevel)
        }
        other => panic!("expected budget+level trap, got {other:?}"),
    }

    let mut minimal = knobs();
    minimal.thinking_level = Some(ThinkingLevel::Minimal);
    match catalog.validate("gemini-4-argon", &minimal) {
        Err(CatalogError::Trap(trap)) => assert_eq!(trap.feature, Feature::ThinkingBelowFloor),
        other => panic!("expected below-floor trap, got {other:?}"),
    }

    let mut low = knobs();
    low.thinking_level = Some(ThinkingLevel::Low);
    assert!(catalog.validate("gemini-4-argon", &low).is_ok());
}

#[test]
fn claude_5_5_family_carries_the_full_ladder() {
    // The Anthropic family ladder low->max applies (SS33.1); Amp ran
    // Opus 5.5 at `medium` (Sept 28) — a documented legal rung. No
    // generation trap is documented for 5.5, so (mirroring the seeded
    // Opus 5 posture) thinking budget stays legal.
    let catalog = Catalog::seeded();
    for model in ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"] {
        for legal in [E::Low, E::Medium, E::High, E::Xhigh, E::Max] {
            assert!(
                catalog.validate(model, &effort_knob(legal)).is_ok(),
                "{model} must accept {legal}"
            );
        }
        let mut budget = knobs();
        budget.thinking_budget = Some(1024);
        assert!(
            catalog.validate(model, &budget).is_ok(),
            "{model} has no documented budget trap"
        );
        // The per-message effort beta is documented for Fable 5.1 /
        // Mythos 5.1 / Opus 5 only — NOT the 5.5 wave.
        let entry = catalog.entry(model).expect("seeded above");
        assert!(
            !entry.per_message_effort_beta,
            "{model} effort beta must stay false until documented"
        );
    }
}

#[test]
fn wave_media_capabilities_stay_fail_closed() {
    // No cited row documents media input for any wave model (SS33.8
    // predates them) — all three flags stay false until one does.
    let catalog = Catalog::seeded();
    for (model, ..) in WAVE {
        assert_eq!(
            catalog.capabilities(model),
            Ok(Capabilities::default()),
            "{model} media flags must be conservative false"
        );
    }
}

#[test]
fn daemon_catalog_seam_resolves_every_wave_model() {
    // The seam bin/austringer-daemon.rs reads (`Catalog::seeded()
    // .capabilities(model)`) must resolve every wave model — an unknown
    // model would silently downgrade to no caps; a seeded one never
    // does. This is the doctor-adjacent unknown-model guard.
    let catalog = Catalog::seeded();
    for (model, ..) in WAVE {
        assert!(
            catalog.capabilities(model).is_ok(),
            "{model} must resolve through the daemon caps seam"
        );
    }
}

/// One legacy entry's full field pin: (model, family, dialect, grid,
/// default, floor, beta, traps, caps).
type EntryPin<'a> = (
    &'a str,
    Family,
    ProtocolDialect,
    &'a [E],
    E,
    Option<ThinkingLevel>,
    bool,
    &'a [Feature],
    Capabilities,
);

#[test]
fn legacy_twelve_entries_are_untouched_by_the_wave() {
    // Full-field pin of the pre-wave twelve: the wave is additive, so
    // every legacy entry must stay byte-identical (grids, defaults,
    // floors, beta flags, traps, media caps).
    let catalog = Catalog::seeded();
    let vision_only = Capabilities {
        vision: true,
        audio_input: false,
        video_input: false,
    };
    let av = Capabilities {
        vision: true,
        audio_input: true,
        video_input: true,
    };
    let legacy: Vec<EntryPin> = vec![
        (
            "claude-opus-4-6",
            Family::Anthropic,
            ProtocolDialect::Anthropic,
            &[E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            E::High,
            None,
            false,
            &[Feature::Prefill],
            vision_only,
        ),
        (
            "claude-opus-4-7",
            Family::Anthropic,
            ProtocolDialect::Anthropic,
            &[E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            E::Xhigh,
            None,
            false,
            &[Feature::ThinkingBudget, Feature::Prefill],
            vision_only,
        ),
        (
            "claude-sonnet-4-6",
            Family::Anthropic,
            ProtocolDialect::Anthropic,
            &[E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            E::Medium,
            None,
            false,
            &[Feature::Prefill],
            vision_only,
        ),
        (
            "claude-fable-5-1",
            Family::Anthropic,
            ProtocolDialect::Anthropic,
            &[E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            E::Medium,
            None,
            true,
            &[Feature::ForcedToolChoice],
            vision_only,
        ),
        (
            "claude-opus-5",
            Family::Anthropic,
            ProtocolDialect::Anthropic,
            &[E::Low, E::Medium, E::High, E::Xhigh, E::Max],
            E::High,
            None,
            true,
            &[],
            vision_only,
        ),
        (
            "gpt-5-5",
            Family::OpenAi,
            ProtocolDialect::OpenAiResponses,
            &[E::None, E::Low, E::Medium, E::High, E::Xhigh],
            E::Medium,
            None,
            false,
            &[],
            vision_only,
        ),
        (
            "gpt-6-astra",
            Family::OpenAi,
            ProtocolDialect::OpenAiResponses,
            &[E::Low, E::Medium, E::High, E::Xhigh],
            E::Medium,
            None,
            true,
            &[Feature::Sampling],
            vision_only,
        ),
        (
            "gemini-2-5-pro",
            Family::Google,
            ProtocolDialect::GoogleGenAi,
            &[E::Low, E::Medium, E::High],
            E::High,
            Some(ThinkingLevel::Low),
            false,
            &[Feature::ThinkingBudgetWithLevel],
            av,
        ),
        (
            "gemini-3-pro",
            Family::Google,
            ProtocolDialect::GoogleGenAi,
            &[E::Low, E::Medium, E::High],
            E::High,
            Some(ThinkingLevel::Low),
            false,
            &[Feature::ThinkingBudgetWithLevel],
            av,
        ),
        (
            "gemini-3-1-pro",
            Family::Google,
            ProtocolDialect::GoogleGenAi,
            &[E::Low, E::Medium, E::High],
            E::High,
            Some(ThinkingLevel::Low),
            false,
            &[Feature::ThinkingBudgetWithLevel],
            av,
        ),
        (
            "grok-4-6",
            Family::Xai,
            ProtocolDialect::Xai,
            &[E::Low, E::Medium, E::High, E::Xhigh],
            E::High,
            None,
            false,
            &[Feature::Penalties, Feature::StopSequences],
            vision_only,
        ),
        (
            "deepseek-reasoner",
            Family::DeepSeek,
            ProtocolDialect::DeepSeek,
            &[E::Low, E::Medium, E::High],
            E::Medium,
            None,
            false,
            &[],
            Capabilities::default(),
        ),
    ];
    for (model, family, dialect, grid, default, floor, beta, traps, caps) in legacy {
        let entry = catalog
            .entry(model)
            .unwrap_or_else(|e| panic!("legacy {model} must stay seeded: {e}"));
        assert_eq!(entry.family, family, "{model} family drifted");
        assert_eq!(entry.dialect, dialect, "{model} dialect drifted");
        assert_eq!(entry.supported_efforts, grid, "{model} grid drifted");
        assert_eq!(entry.default_effort, default, "{model} default drifted");
        assert_eq!(entry.thinking_floor, floor, "{model} floor drifted");
        assert_eq!(entry.per_message_effort_beta, beta, "{model} beta drifted");
        assert_eq!(entry.traps, traps, "{model} traps drifted");
        assert_eq!(entry.capabilities, caps, "{model} caps drifted");
    }
}
