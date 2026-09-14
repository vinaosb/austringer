//! The model catalog: per-model capability/legality records carrying the
//! EXPLICIT protocol dialect, the per-model effort grid, and the 400-trap
//! feature-detection table (research SS33.1). Legality is feature-detected
//! per MODEL, never per provider — effort-support grids are fragmented
//! model-by-model inside a single family, and every family has a trap
//! list.
//!
//! [`Catalog::validate`] is the fail-closed gate callers run BEFORE any
//! wire request is built: a knob the model rejects with HTTP 400 is
//! rejected here with a typed [`TrapViolation`] carrying the remedy.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::dialect::ProtocolDialect;
use super::seed;

/// Provider family (drives SET semantics in the elicitation layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// Anthropic Claude family.
    Anthropic,
    /// OpenAI GPT family.
    OpenAi,
    /// Google Gemini family.
    Google,
    /// xAI Grok family.
    Xai,
    /// DeepSeek family.
    DeepSeek,
}

/// Unified effort ladder. Declaration order is ascending — `Low < Medium <
/// High < Xhigh < Max` — so ladders clamp by `min`/`max`. Which rungs a
/// model supports is per-model catalog data, never family data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffortValue {
    /// Effort off (legal on exactly three surfaces, research SS33.1).
    None,
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
    /// Extra-high.
    Xhigh,
    /// Maximum.
    Max,
}

impl std::fmt::Display for EffortValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let word = match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        };
        f.write_str(word)
    }
}

/// Google-style thinking level (mutually exclusive with budgets by
/// generation; declaration order ascending).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingLevel {
    /// Minimal — advisory "off", not guaranteed, and ILLEGAL on Pro tiers.
    Minimal,
    /// Low — the floor for Pro-tier models.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
}

/// A wire-request knob set under validation. Only knobs a caller actually
/// intends to send are set; every set knob is checked against the model's
/// trap table and effort grid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WireKnobs {
    /// Thinking budget in tokens (`budget_tokens` / `thinking_budget`).
    pub thinking_budget: Option<u32>,
    /// Thinking level (`thinking_level`).
    pub thinking_level: Option<ThinkingLevel>,
    /// Reasoning effort.
    pub effort: Option<EffortValue>,
    /// Forced tool choice (the Fable 5.1 trap).
    pub forced_tool_choice: bool,
    /// Assistant prefill (the Anthropic 4.6+ trap).
    pub prefill: bool,
    /// Sampling parameters (temperature/top_p).
    pub sampling: bool,
    /// Presence/frequency penalties (the Grok reasoning-model trap).
    pub penalties: bool,
    /// Stop sequences (the Grok reasoning-model trap).
    pub stop_sequences: bool,
}

/// A feature the model rejects — the named 400-trap taxonomy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    /// The `budget_tokens` knob (400 on Anthropic 4.7+).
    #[error("thinking budget (budget_tokens) — removed on this generation")]
    ThinkingBudget,
    /// Assistant prefill (400 on Anthropic 4.6+).
    #[error("prefill — removed on this generation")]
    Prefill,
    /// Forced `tool_choice` (400 on Fable 5.1; use strict:true + instruction).
    #[error("forced tool_choice")]
    ForcedToolChoice,
    /// `thinking_budget` and `thinking_level` together (400 on Gemini).
    #[error("thinking_budget together with thinking_level")]
    ThinkingBudgetWithLevel,
    /// Effort `none` (400 on GPT-6 Astra; the floor is `low`).
    #[error("effort none — this model's floor is low")]
    EffortNone,
    /// Sampling parameters (rejected alongside effort-none on Astra).
    #[error("sampling parameters")]
    Sampling,
    /// Penalties on a reasoning model (error on Grok).
    #[error("presence/frequency penalties on a reasoning model")]
    Penalties,
    /// Stop sequences on a reasoning model (error on Grok).
    #[error("stop sequences on a reasoning model")]
    StopSequences,
    /// A thinking level below the model's floor (Cline bug 7735 guard).
    #[error("thinking level below this model's floor")]
    ThinkingBelowFloor,
    /// An effort rung outside this model's grid (grids are per-model).
    #[error("effort {requested} is outside this model's grid")]
    EffortUnsupported {
        /// The requested rung.
        requested: EffortValue,
    },
}

/// A typed fail-closed violation: the model rejects this combination with
/// a 400-class error, so the harness rejects it before the wire.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{model} rejects this request: {feature} — see the catalog trap table")]
pub struct TrapViolation {
    /// The model that would reject the request.
    pub model: String,
    /// The offending feature.
    pub feature: Feature,
    /// The documented remedy.
    pub remedy: &'static str,
}

/// The model is not in the catalog. Never guessed around: an unknown model
/// has no dialect, no effort grid, and no trap table, so no request may be
/// built for it.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("unknown model {model:?}: not in the catalog — add it before routing requests")]
pub struct UnknownModel {
    /// The model id that missed.
    pub model: String,
}

/// Everything catalog lookup and validation can fail with.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CatalogError {
    /// Unknown model.
    #[error(transparent)]
    UnknownModel(#[from] UnknownModel),
    /// A trap violation.
    #[error(transparent)]
    Trap(#[from] TrapViolation),
}

/// One catalog record: everything the harness must know about one model
/// before building a request for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogEntry {
    /// Model id (exact wire name).
    pub model: String,
    /// Provider family.
    pub family: Family,
    /// Explicit protocol dialect — never inferred from an endpoint URL.
    pub dialect: ProtocolDialect,
    /// The effort rungs this model accepts (per-model grid).
    pub supported_efforts: Vec<EffortValue>,
    /// The documented default effort — SET explicitly, never trusted from
    /// the provider.
    pub default_effort: EffortValue,
    /// The model's thinking floor, when it has one (Gemini Pro tiers).
    pub thinking_floor: Option<ThinkingLevel>,
    /// Whether the per-message effort beta channel exists on this model
    /// (cache-preserving mid-run effort changes).
    pub per_message_effort_beta: bool,
    /// Features this model rejects (the 400-trap table).
    pub traps: Vec<Feature>,
}

/// The catalog: model id -> entry, seeded from research SS33.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    entries: BTreeMap<String, CatalogEntry>,
}

impl Catalog {
    /// The seeded catalog (research SS33.1/SS33.2 + model-profiles.md SS3).
    #[must_use]
    pub fn seeded() -> Self {
        Self {
            entries: seed::entries()
                .into_iter()
                .map(|e| (e.model.clone(), e))
                .collect(),
        }
    }

    /// Looks one model up. Unknown models are typed errors.
    pub fn entry(&self, model: &str) -> Result<&CatalogEntry, UnknownModel> {
        self.entries.get(model).ok_or_else(|| UnknownModel {
            model: model.to_string(),
        })
    }

    /// Iterates every entry in model-id order (the todo-42 catalog
    /// search tool's bounded hydration source).
    pub fn entries(&self) -> impl Iterator<Item = &CatalogEntry> {
        self.entries.values()
    }

    /// Fail-closed validation of a knob set against the model's trap table
    /// and effort grid. `Ok(())` means every set knob is legal.
    pub fn validate(&self, model: &str, knobs: &WireKnobs) -> Result<(), CatalogError> {
        let entry = self.entry(model)?;
        validate_entry(entry, knobs).map_err(Into::into)
    }
}

/// Trap-table validation for one entry.
fn validate_entry(entry: &CatalogEntry, knobs: &WireKnobs) -> Result<(), TrapViolation> {
    use Family as Fm;
    let trap = |feature: Feature| TrapViolation {
        model: entry.model.clone(),
        remedy: remedy_for(&feature),
        feature,
    };
    if let Some(effort) = knobs.effort
        && !entry.supported_efforts.contains(&effort)
    {
        let feature = if effort == EffortValue::None && entry.family == Fm::OpenAi {
            // The named Astra trap: this family HAS `none` elsewhere;
            // this model's floor is `low`.
            Feature::EffortNone
        } else {
            Feature::EffortUnsupported { requested: effort }
        };
        return Err(trap(feature));
    }
    if knobs.thinking_budget == Some(0) {
        // Zero budgets are invalid on floor-bearing models (Cline 7735).
        return Err(trap(Feature::ThinkingBelowFloor));
    }
    let has = |probe: &Feature| {
        entry
            .traps
            .iter()
            .any(|t| std::mem::discriminant(t) == std::mem::discriminant(probe))
    };
    if knobs.thinking_budget.is_some() && has(&Feature::ThinkingBudget) {
        return Err(trap(Feature::ThinkingBudget));
    }
    if knobs.thinking_budget.is_some()
        && knobs.thinking_level.is_some()
        && has(&Feature::ThinkingBudgetWithLevel)
    {
        return Err(trap(Feature::ThinkingBudgetWithLevel));
    }
    if knobs.prefill && has(&Feature::Prefill) {
        return Err(trap(Feature::Prefill));
    }
    if knobs.forced_tool_choice && has(&Feature::ForcedToolChoice) {
        return Err(trap(Feature::ForcedToolChoice));
    }
    if knobs.sampling && has(&Feature::Sampling) {
        return Err(trap(Feature::Sampling));
    }
    if knobs.penalties && has(&Feature::Penalties) {
        return Err(trap(Feature::Penalties));
    }
    if knobs.stop_sequences && has(&Feature::StopSequences) {
        return Err(trap(Feature::StopSequences));
    }
    if knobs
        .thinking_level
        .is_some_and(|level| entry.thinking_floor.is_some_and(|floor| level < floor))
    {
        return Err(trap(Feature::ThinkingBelowFloor));
    }
    Ok(())
}

/// Remedy text per trap (cites the documented fix).
fn remedy_for(feature: &Feature) -> &'static str {
    match feature {
        Feature::ThinkingBudget => {
            "use output_config.effort; budget_tokens 400s on this generation"
        }
        Feature::Prefill => "drop the prefill block; this generation rejects it",
        Feature::ForcedToolChoice => {
            "use strict:true plus an instruction instead of forced tool_choice"
        }
        Feature::ThinkingBudgetWithLevel => {
            "send thinking_level only, never alongside thinking_budget"
        }
        Feature::EffortNone => "raise effort to low; this model's floor is low",
        Feature::Sampling => "drop temperature/top_p; not legal on this model",
        Feature::Penalties => "strip presence/frequency penalties from requests",
        Feature::StopSequences => "strip stop sequences from requests",
        Feature::ThinkingBelowFloor => {
            "raise the thinking level to the model's floor (or drop the zero budget)"
        }
        Feature::EffortUnsupported { .. } => "pick an effort rung from this model's supported grid",
    }
}
