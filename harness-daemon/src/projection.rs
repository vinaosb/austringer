//! Submit-time projection wiring at the daemon edge (plan M10).
//!
//! The resolver core lives in harness-media; this module composes it
//! with what only the daemon knows — the provider seam's session model
//! (M32) and the configured capable utility route from the T42 routing
//! table (M7 caps gating both) — and turns outcomes into the system
//! notices the submit path appends as `Event::System`. An unconfigured
//! daemon wires the intake with NO mediation candidates: every media
//! submit stays marker-only with an honest notice, never a fabricated
//! projection.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use harness_core::event::ContentBlock;
use harness_core::media_delivery::DeliveryCapabilities;
use harness_intel::catalog::{Capabilities, Catalog, UnknownModel};
use harness_intel::routing::resolve::ResolutionError;
use harness_intel::routing::{Role, RouteRequest, RoutingConfigError, RoutingTable};
use harness_media::pipeline::{
    MediatedProjection, Mediation, ProjectionOutcome, ProjectionResolver, Submission,
};
use harness_media::projection::ProjectionRegistry;
use harness_providers::transport::Transport;

use crate::attach::SteerQueue;
use crate::provider::ResolvedProvider;

/// One mediation candidate resolved at startup: the daemon provider,
/// its model id (the provenance producer), its M7-mapped delivery caps,
/// and the wall-clock budget for one projection call.
pub struct ResolvedMediation {
    /// Composes the projection inference over the family wire.
    pub resolved: ResolvedProvider,
    /// Model id stored in the provenance producer id.
    pub model_id: String,
    /// The model's delivery caps (the capability gate's data).
    pub caps: DeliveryCapabilities,
    /// Wall-clock budget for one projection call.
    pub timeout: Duration,
}

/// Per-session intake: owns the mediation candidates and the notice
/// queue the engine's steer stage drains into `Event::System` events.
pub struct ProjectionIntake {
    session_dir: PathBuf,
    registry: ProjectionRegistry,
    session: Option<MediatedProjection>,
    utility: Option<MediatedProjection>,
    notices: SteerQueue,
}

impl ProjectionIntake {
    /// Intake over one session dir. The planner caps derive from the
    /// session candidate (all-false when absent — the text-only path).
    #[must_use]
    pub fn new(
        session_dir: PathBuf,
        registry: ProjectionRegistry,
        session: Option<MediatedProjection>,
        utility: Option<MediatedProjection>,
    ) -> Self {
        Self {
            session_dir,
            registry,
            session,
            utility,
            notices: SteerQueue::default(),
        }
    }

    /// The notice queue drained as `Event::System` events.
    #[must_use]
    pub fn notice_queue(&self) -> SteerQueue {
        Arc::clone(&self.notices)
    }

    /// Resolves the submission's queued projections and queues one
    /// system notice per unprojectable media. Cache hits and model
    /// results land in the derived store (the M8 delivery cache inlines
    /// them at request-build time); a store that will not open is an
    /// honest notice — a submit is never crashed by a projection.
    pub async fn enqueue_notices(&self, blocks: &[ContentBlock]) {
        let resolver = ProjectionResolver::new(&self.session_dir, &self.registry);
        let submission = Submission {
            blocks,
            caps: self
                .session
                .as_ref()
                .map_or_else(DeliveryCapabilities::default, |model| model.caps),
        };
        let mediation = Mediation {
            session: self.session.as_ref(),
            utility: self.utility.as_ref(),
        };
        let notices = match resolver.resolve(submission, mediation).await {
            Ok(outcomes) => outcomes
                .iter()
                .filter_map(ProjectionOutcome::notice)
                .collect::<VecDeque<_>>(),
            Err(failure) => VecDeque::from([format!(
                "media projection unavailable: the session stores did not open ({failure})"
            )]),
        };
        if let Ok(mut queue) = self.notices.lock() {
            queue.extend(notices);
        }
    }
}

/// Builds one intake per new session dir; `None` leaves that session's
/// submit path marker-only with no notices (the pre-M10 posture).
pub type IntakeFactory = Arc<dyn Fn(&Path) -> Option<Arc<ProjectionIntake>> + Send + Sync>;

/// The disabled factory (the daemon default; tests needing the
/// pre-M10 posture).
#[must_use]
pub fn disabled_factory() -> IntakeFactory {
    Arc::new(|_| None)
}

/// Composes the intake factory over the daemon transport: every new
/// session gets an intake carrying fresh projection factories over its
/// own session dir. An intake exists even with NO candidates — the
/// unconfigured daemon still names what it could not project.
#[must_use]
pub fn compose_intake_factory(
    transport: Arc<dyn Transport>,
    session: Option<ResolvedMediation>,
    utility: Option<ResolvedMediation>,
) -> IntakeFactory {
    Arc::new(move |session_dir| {
        let candidate = |mediation: &ResolvedMediation| {
            mediation
                .resolved
                .compose_projection_factory(
                    Arc::clone(&transport),
                    mediation.caps,
                    session_dir,
                )
                .ok()
                .map(|factory| MediatedProjection {
                    model_id: mediation.model_id.clone(),
                    caps: mediation.caps,
                    factory,
                    timeout: mediation.timeout,
                })
        };
        Some(Arc::new(ProjectionIntake::new(
            session_dir.to_path_buf(),
            ProjectionRegistry::new(),
            session.as_ref().and_then(candidate),
            utility.as_ref().and_then(candidate),
        )))
    })
}

/// Maps M7 intel caps onto the M8 delivery mirror. `video_input` has no
/// delivery surface (the planner queues no native video blocks) —
/// dropped, never fabricated.
#[must_use]
pub fn delivery_caps(caps: Capabilities) -> DeliveryCapabilities {
    DeliveryCapabilities {
        vision: caps.vision,
        audio_input: caps.audio_input,
    }
}

/// The configured capable utility route (plan M10: T42).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtilityRoute {
    /// The routed model id (provenance + wire model).
    pub model: String,
    /// The routed model's delivery caps.
    pub caps: DeliveryCapabilities,
}

/// Utility-route lookup failures (typed, never a silent miss).
#[derive(Debug, thiserror::Error)]
pub enum UtilityRouteError {
    /// The routing document failed table construction.
    #[error("routing table rejected: {0}")]
    Table(#[from] RoutingConfigError),
    /// The utility role has no configured route.
    #[error("utility route unresolved: {0}")]
    Unresolved(#[from] ResolutionError),
    /// The routed model is absent from the catalog (unreachable for a
    /// validated table; typed anyway).
    #[error("routed model unknown: {0}")]
    Unknown(#[from] UnknownModel),
}

/// Looks up the projection utility route through the T42 routing
/// table: the `summarizer` utility role (cheap-pool summarization —
/// scene descriptions and transcripts are summarization-class work).
/// The capability gate stays in the resolver: a routed model without
/// the needed modality answers `NoProjector`, never a model call.
///
/// # Errors
/// Typed [`UtilityRouteError`]; the caller stays fail-open (an
/// optional route never takes inference down).
pub fn utility_route(json: &str, catalog: &Catalog) -> Result<UtilityRoute, UtilityRouteError> {
    let table = RoutingTable::from_json(json, catalog)?;
    let resolution = table.resolve(RouteRequest::Role(Role::Summarizer))?;
    let caps = table.catalog().capabilities(&resolution.model.model)?;
    Ok(UtilityRoute {
        model: resolution.model.model,
        caps: delivery_caps(caps),
    })
}
