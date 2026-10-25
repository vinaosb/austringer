//! The fusion sidekick route (Devin, research SS22): a cheap sidekick
//! with its OWN persistent cached context runs parallel to the frontier
//! lead. Lead-model switches execute ONLY at compaction boundaries —
//! the cache miss happens anyway, so the switch is free there and
//! cost-bearing everywhere else.

use austringer_providers::identity::ModelIdentity;

/// The routed pair: frontier lead + cheap sidekick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidekickRoute {
    /// The frontier lead model.
    pub lead: ModelIdentity,
    /// The cheap sidekick (own persistent cached context).
    pub sidekick: ModelIdentity,
}

/// A lead-model switch attempted away from a compaction boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "model switch {current:?} -> {target:?} rejected: not at a compaction boundary — the switch would bust the cache mid-run"
)]
pub struct SwitchRejected {
    /// The model switched away from.
    pub current: String,
    /// The refused target.
    pub target: String,
}

/// Gates a lead-model switch: allowed exactly at compaction boundaries,
/// rejected typed everywhere else.
pub fn gate_model_switch(
    current: &ModelIdentity,
    target: &ModelIdentity,
    at_compaction_boundary: bool,
) -> Result<ModelIdentity, SwitchRejected> {
    if at_compaction_boundary {
        Ok(target.clone())
    } else {
        Err(SwitchRejected {
            current: current.model.clone(),
            target: target.model.clone(),
        })
    }
}
