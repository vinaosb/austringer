//! The model profile: catalog entry + observed taxonomy + observed axes
//! for one model, the per-model tips rendering (the todo-34 happy QA:
//! "profile renders per-model tips + correct dialect"), and the typed feed
//! into the T30/T31 per-model skill-guidance surface.

use austringer_providers::identity::ModelIdentity;
use austringer_skills::skill::ModelGuidance;

use super::axes::{self, BehaviorAxes};
use super::catalog::{Catalog, CatalogEntry, EffortValue, UnknownModel};
use super::taxonomy::{self, FailureTaxonomy, TurnObservation};

/// One model's profile: what the catalog knows plus what OUR transcripts
/// observed. `None` taxonomy/axes = insufficient data = neutral.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelProfile {
    /// The model this profile describes.
    pub model: ModelIdentity,
    /// Observed failure taxonomy (`None` under the data floor).
    pub taxonomy: Option<FailureTaxonomy>,
    /// Observed behavioral axes (`None` under the data floor).
    pub axes: Option<BehaviorAxes>,
}

impl ModelProfile {
    /// A neutral profile: catalog facts only, no observations claimed.
    #[must_use]
    pub fn neutral(model: ModelIdentity) -> Self {
        Self {
            model,
            taxonomy: None,
            axes: None,
        }
    }

    /// True when no observations back this profile (insufficient data).
    #[must_use]
    pub fn is_neutral(&self) -> bool {
        self.taxonomy.is_none() && self.axes.is_none()
    }
}

/// Builds a profile from the catalog and an observation window. Unknown
/// models are typed errors — there is no profile without a dialect, an
/// effort grid, and a trap table.
pub fn build_profile(
    catalog: &Catalog,
    identity: &ModelIdentity,
    observations: &[TurnObservation],
) -> Result<ModelProfile, UnknownModel> {
    catalog.entry(&identity.model)?;
    Ok(ModelProfile {
        model: identity.clone(),
        taxonomy: taxonomy::compute_taxonomy(observations, identity),
        axes: axes::compute_axes(observations, identity),
    })
}

/// Renders the per-model tips: identity, dialect, explicit-effort guidance,
/// the 400-trap list, and — only when data suffices — the observed
/// weakness section.
#[must_use]
pub fn render_tips(entry: &CatalogEntry, profile: &ModelProfile) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "model {} ({})\n",
        entry.model, profile.model.provider
    ));
    out.push_str(&format!("dialect: {:?}\n", entry.dialect).to_lowercase());
    out.push_str(&format!(
        "effort: default {} (supported: {}) — set explicitly, never trust provider defaults\n",
        entry.default_effort,
        entry
            .supported_efforts
            .iter()
            .map(EffortValue::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if !entry.traps.is_empty() {
        let traps = entry
            .traps
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        out.push_str(&format!("400-traps: {traps}\n"));
    }
    if let (Some(taxonomy), Some(axes)) = (&profile.taxonomy, &profile.axes) {
        out.push_str("observed:\n");
        let mut lines: Vec<String> = Vec::new();
        let mut rate = |label: &str, value: f32| {
            if value > 0.0 {
                lines.push(format!("  {label} rate {value:.2}"));
            }
        };
        rate("syntax-error", taxonomy.syntax_error_rate);
        rate("tool-misuse", taxonomy.tool_misuse_rate);
        rate("context-overflow", taxonomy.context_overflow_rate);
        rate("retry-storm", taxonomy.retry_storm_rate);
        rate("edit-rejection", taxonomy.edit_rejection_rate);
        lines.push(format!(
            "  {:.0} tokens/turn, {:.1} tool calls/turn, delegation ratio {:.2}",
            taxonomy.tokens_per_turn, axes.tool_calls_per_turn, axes.delegation_ratio
        ));
        out.push_str(&lines.join("\n"));
        out.push('\n');
    }
    out
}

/// Maps the tips into the T30/T31 per-model guidance vocabulary (skills
/// gain per-model guidance sections keyed on exactly this model id).
#[must_use]
pub fn to_model_guidance(entry: &CatalogEntry, profile: &ModelProfile) -> ModelGuidance {
    ModelGuidance {
        model: entry.model.clone(),
        guidance: render_tips(entry, profile),
    }
}
