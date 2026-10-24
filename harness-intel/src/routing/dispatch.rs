//! The turn dispatcher (plan todo 42): composes the routing table,
//! per-task stickiness, and the failover engine into the per-turn
//! surface. Every dispatch carries the routed identity, the EXPLICIT
//! knobs, and the acknowledged-fallback audit trail; an outage proposal
//! never changes a dispatch until the user confirms it. Also hosts the
//! routing-incentive pricing (auto-routed usage priced below manual
//! picks so user economics favor the router).

use std::collections::BTreeMap;

use harness_providers::identity::AcknowledgedFallback;

use super::RouteRequest;
use super::fallback::{
    Failover, FallbackDecision, FallbackRejected, RoutingEvent, RoutingFailure, TurnOutcome,
};
use super::resolve::{
    PinnedProfile, ResolutionError, ResolutionSource, Stickiness, TaskId, UnknownTask,
};
use super::sidekick::SidekickRoute;
use super::table::RoutingTable;
use crate::catalog::{CatalogError, EffortValue, WireKnobs};

/// The routing-incentive discount on auto-routed usage (Copilot's ~10%
/// pattern, research SS20): user economics favor the router.
pub const AUTO_ROUTE_DISCOUNT: f64 = 0.10;

/// One dispatched turn: everything a wire request must carry.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnDispatch {
    /// The task the turn belongs to.
    pub task: TaskId,
    /// The routed identity (post-fallback target when a hop was
    /// acknowledged and attached).
    pub model: harness_providers::identity::ModelIdentity,
    /// The explicit effort rung.
    pub effort: EffortValue,
    /// The knobs to set (validated against the catalog).
    pub knobs: WireKnobs,
    /// Why this route.
    pub source: ResolutionSource,
    /// Whether the router (vs the user's config) picked the model.
    pub auto_routed: bool,
    /// Present only when this dispatch targets an acknowledged fallback
    /// hop — the surfaced audit trail on the request itself.
    pub fallback: Option<AcknowledgedFallback>,
}

/// The per-turn routing surface over a validated table.
pub struct Dispatcher {
    table: RoutingTable,
    sticky: Stickiness,
    failover: Failover,
    overrides: BTreeMap<TaskId, AcknowledgedFallback>,
}

impl Dispatcher {
    /// A dispatcher over a validated routing table.
    #[must_use]
    pub fn new(table: RoutingTable) -> Self {
        Self {
            table,
            sticky: Stickiness::new(),
            failover: Failover::new(),
            overrides: BTreeMap::new(),
        }
    }

    /// Dispatches one turn under the task's effective route. The FIRST
    /// dispatch pins the launch profile (stickiness): later dispatches —
    /// including resumes and divergent requests — keep the pin. An
    /// acknowledged fallback override retargets the pin's model with the
    /// effort revalidated for the hop (the hop model's DOCUMENTED default
    /// when the pinned rung is off its grid).
    pub fn dispatch_turn(
        &mut self,
        task: &TaskId,
        request: RouteRequest,
    ) -> Result<TurnDispatch, ResolutionError> {
        let profile = match self.sticky.resolve(task) {
            Some(pinned) => pinned.clone(),
            None => {
                let resolution = self.table.resolve(request.clone())?;
                self.sticky.launch(
                    task.clone(),
                    PinnedProfile {
                        request,
                        resolution,
                    },
                )
            }
        };
        let pinned = &profile.resolution;
        let (model, effort, fallback) = match self.overrides.get(task) {
            Some(ack) => {
                let target = ack.to().model.clone();
                let entry = self
                    .table
                    .catalog
                    .entry(&target)
                    .map_err(CatalogError::from)?;
                let effort = if entry.supported_efforts.contains(&pinned.effort) {
                    pinned.effort
                } else {
                    entry.default_effort
                };
                (
                    harness_providers::identity::ModelIdentity::new(
                        &pinned.model.provider,
                        &target,
                    ),
                    effort,
                    Some(ack.clone()),
                )
            }
            None => (pinned.model.clone(), pinned.effort, None),
        };
        let knobs = WireKnobs {
            effort: Some(effort),
            ..WireKnobs::default()
        };
        self.table.catalog.validate(&model.model, &knobs)?;
        Ok(TurnDispatch {
            task: task.clone(),
            model,
            effort,
            knobs,
            source: pinned.source.clone(),
            auto_routed: pinned.auto_routed,
            fallback,
        })
    }

    /// Observes a completed turn's outcome (identity = the model that
    /// served it) and decides retries, wait-outs, or fallback proposals.
    pub fn observe_turn(
        &mut self,
        task: &TaskId,
        identity: &harness_providers::identity::ModelIdentity,
        outcome: TurnOutcome,
    ) -> FallbackDecision {
        self.failover.observe(&self.table, task, identity, outcome)
    }

    /// Confirms a proposed hop: the next dispatch carries the hop model
    /// plus the audit trail. Mismatched confirmations are typed
    /// rejections (fail closed).
    pub fn acknowledge_fallback(
        &mut self,
        task: &TaskId,
        ack: AcknowledgedFallback,
    ) -> Result<(), FallbackRejected> {
        self.failover.acknowledge(task, &ack)?;
        self.overrides.insert(task.clone(), ack);
        Ok(())
    }

    /// Declines the pending proposal: the original route stays and the
    /// refusal surfaces typed.
    pub fn decline_fallback(&mut self, task: &TaskId) -> RoutingFailure {
        self.failover.decline(task)
    }

    /// A subtask inherits its parent's launch profile (typed miss when
    /// the parent is unknown).
    pub fn spawn_subtask(&mut self, parent: &TaskId, child: TaskId) -> Result<(), UnknownTask> {
        self.sticky.spawn_subtask(parent, child)
    }

    /// Removes the task's pin (the deliberate re-route path).
    pub fn unpin(&mut self, task: &TaskId) -> bool {
        self.sticky.unpin(task)
    }

    /// The surfaced hop audit trail, in order.
    #[must_use]
    pub fn audit(&self) -> &[RoutingEvent] {
        self.failover.audit()
    }

    /// The fusion sidekick pair, when configured.
    #[must_use]
    pub fn sidekick(&self) -> Option<&SidekickRoute> {
        self.table.sidekick()
    }

    /// The underlying table (suggestions, chains, catalog).
    #[must_use]
    pub fn table(&self) -> &RoutingTable {
        &self.table
    }
}

/// A priced usage: list cost, effective cost, and the surfaced savings
/// estimate (feeds the HUD's routing-savings readout).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PricedUsage {
    /// List price of the usage.
    pub list_cost: f64,
    /// Effective cost after the routing discount (when auto-routed).
    pub effective_cost: f64,
    /// List minus effective — the user-visible savings.
    pub savings: f64,
}

/// Prices one usage: auto-routed usage carries the
/// [`AUTO_ROUTE_DISCOUNT`] discount; manual picks pay list.
#[must_use]
pub fn price_usage(list_cost: f64, auto_routed: bool) -> PricedUsage {
    let effective_cost = if auto_routed {
        list_cost * (1.0 - AUTO_ROUTE_DISCOUNT)
    } else {
        list_cost
    };
    PricedUsage {
        list_cost,
        effective_cost,
        savings: list_cost - effective_cost,
    }
}
