//! The on-demand model-catalog search tool (kilo agent_manager_models,
//! research SS16): the MODEL itself may search the catalog for models
//! worth routing to — but the full catalog is NEVER injected into
//! context. Results are bounded, sorted, structured; empty queries
//! hydrate nothing.

use crate::catalog::{Catalog, EffortValue, Family};
use crate::dialect::ProtocolDialect;

/// The tool name registered for the model's own use.
pub const MODEL_CATALOG_SEARCH_TOOL: &str = "model_catalog_search";

/// Maximum matches one search ever hydrates.
pub const MATCH_LIMIT: usize = 5;

/// One structured catalog match (what the observation shows the model).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogMatch {
    /// Model id.
    pub model: String,
    /// Provider family name.
    pub family: String,
    /// Explicit protocol dialect.
    pub dialect: ProtocolDialect,
    /// Documented default effort.
    pub default_effort: EffortValue,
    /// The model's supported effort grid.
    pub supported_efforts: Vec<EffortValue>,
}

fn family_name(family: Family) -> &'static str {
    match family {
        Family::Anthropic => "anthropic",
        Family::OpenAi => "openai",
        Family::Google => "google",
        Family::Xai => "xai",
        Family::DeepSeek => "deepseek",
    }
}

/// Searches the catalog by case-insensitive substring over model id and
/// family name. Empty or whitespace queries return NOTHING (never the
/// full catalog); results are model-id-sorted and capped at
/// [`MATCH_LIMIT`].
#[must_use]
pub fn search(catalog: &Catalog, query: &str) -> Vec<CatalogMatch> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    catalog
        .entries()
        .filter(|entry| {
            entry.model.to_lowercase().contains(&needle)
                || family_name(entry.family).contains(needle.as_str())
        })
        .take(MATCH_LIMIT)
        .map(|entry| CatalogMatch {
            model: entry.model.clone(),
            family: family_name(entry.family).to_owned(),
            dialect: entry.dialect,
            default_effort: entry.default_effort,
            supported_efforts: entry.supported_efforts.clone(),
        })
        .collect()
}

/// Renders the observation the model sees: one bounded line per match.
#[must_use]
pub fn render_observation(matches: &[CatalogMatch]) -> String {
    if matches.is_empty() {
        return "no models matched".to_owned();
    }
    matches
        .iter()
        .map(|m| {
            format!(
                "{} (family {}, dialect {:?}, default effort {}, supports {})",
                m.model,
                m.family,
                m.dialect,
                m.default_effort,
                m.supported_efforts
                    .iter()
                    .map(EffortValue::to_string)
                    .collect::<Vec<_>>()
                    .join("/")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
