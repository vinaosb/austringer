//! The deterministic arm of the resolve pipeline: probe the bytes, then
//! the per-format projector registry. Unregistered formats and missing
//! kinds fall through (the registry's typed `NoProjector` is the plan's
//! honest posture until the adapter waves land); any other registry
//! failure is a typed refusal the resolver surfaces.

use austringer_core::event::ProjectionKind;

use crate::error::MediaError;
use crate::extract::ExtractBudgets;
use crate::probe::{ProbeResult, probe};
use crate::projection::{ProjectionContext, ProjectionRegistry};

pub(super) enum Determination {
    Projected { producer: String, text: String },
    Refused(MediaError),
    FallThrough,
}

pub(super) fn deterministic(
    registry: &ProjectionRegistry,
    bytes: &[u8],
    kind: ProjectionKind,
) -> Determination {
    let ProbeResult::Identified { format, .. } = probe(bytes, None) else {
        return Determination::FallThrough;
    };
    let context = ProjectionContext {
        budgets: ExtractBudgets::default(),
        name_hint: None,
    };
    match registry.project(&format, kind, bytes, &context) {
        Ok(text) => Determination::Projected {
            producer: format!("deterministic:{format}"),
            text,
        },
        Err(MediaError::NoProjector | MediaError::UnsupportedFormat) => Determination::FallThrough,
        Err(error) => Determination::Refused(error),
    }
}
