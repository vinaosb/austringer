//! The dial effort-bundle grammar (Amp, research SS17): named user-facing
//! levels bundle model + effort + prompt addenda + tools per position.
//! Profiles make each dial position CONCRETE — a resolved identity and an
//! EXPLICIT effort (the catalog's documented default when the spec omits
//! one, never the provider's implicit default) — instead of exposing raw
//! model pickers.

use harness_providers::identity::ModelIdentity;

use super::catalog::{Catalog, CatalogError, EffortValue, WireKnobs};

/// One dial position as declared by the user surface.
#[derive(Debug, Clone, PartialEq)]
pub struct DialSpec {
    /// User-facing position name ("quick", "deep", ...).
    pub name: String,
    /// Model id for the position.
    pub model: String,
    /// Explicit effort; `None` = use the model's catalog default.
    pub effort: Option<EffortValue>,
    /// Prompt addenda applied when the dial sits here.
    pub prompt_addenda: Vec<String>,
    /// Tool surface for the position.
    pub tools: Vec<String>,
}

/// One concrete dial position: everything resolved, nothing implicit.
#[derive(Debug, Clone, PartialEq)]
pub struct DialPosition {
    /// User-facing position name.
    pub name: String,
    /// Resolved model identity.
    pub model: ModelIdentity,
    /// The explicit effort this position sends.
    pub effort: EffortValue,
    /// Prompt addenda for the position.
    pub prompt_addenda: Vec<String>,
    /// Tool surface for the position.
    pub tools: Vec<String>,
}

/// Builds the dial: resolves every spec against the catalog — unknown
/// models are typed errors, and each position's effort is validated
/// against the model's grid exactly like a direct wire request (fail
/// closed).
pub fn build_dial(
    provider: &str,
    specs: &[DialSpec],
    catalog: &Catalog,
) -> Result<Vec<DialPosition>, CatalogError> {
    specs
        .iter()
        .map(|spec| {
            let entry = catalog.entry(&spec.model)?;
            let effort = spec.effort.unwrap_or(entry.default_effort);
            let knobs = WireKnobs {
                effort: Some(effort),
                ..WireKnobs::default()
            };
            catalog.validate(&spec.model, &knobs)?;
            Ok(DialPosition {
                name: spec.name.clone(),
                model: ModelIdentity::new(provider, &spec.model),
                effort,
                prompt_addenda: spec.prompt_addenda.clone(),
                tools: spec.tools.clone(),
            })
        })
        .collect()
}
