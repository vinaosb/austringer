//! The submit-time projection resolve pipeline (plan M10).
//!
//! For one submission's [`DeliveryPlan::needed_projections`] the resolver
//! settles each `(media, kind)` in order, never computing twice and never
//! fabricating:
//!
//! 1. the M4 derived store under the canonical delivery alias (content-hash
//!    keyed cache hit — the zero-recompute pin);
//! 2. a deterministic [`ProjectionProvider`] from the registry (thin wraps
//!    over extract text projections; the shipped registry is empty until
//!    the adapter waves, and says so typed);
//! 3. MODEL-MEDIATED projection — the session model when its capability
//!    flags (the M7 data mirrored by [`DeliveryCapabilities`]) cover the
//!    media's modality, else the configured capable utility route (T42
//!    wiring at the daemon edge), else the typed `NoProjector` with a
//!    notice naming what is missing.
//!
//! Deterministic results store under `deterministic:{format}`; model
//! results under `model:{model-id}:{PROJECTION_PROMPT_VERSION}` — model id
//! plus generation params, the provenance contract. Every success ALSO
//! writes the `delivery` alias (equal to harness-providers'
//! `DELIVERY_PROJECTION_PRODUCER`) so the M8 delivery cache inlines the
//! projection into text-only prompts at request-build time.

use std::fmt;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use harness_core::agent_loop::Inference;
use harness_core::event::derived::DerivedStore;
use harness_core::event::media::{ContentHash, MediaStore};
use harness_core::event::{ContentBlock, MediaRef, ProjectionKind};
use harness_core::media_delivery::{self, DeliveryCapabilities, ProjectionCache};

use crate::error::MediaError;
use crate::projection::ProjectionRegistry;

mod deterministic;
mod mediate;

use deterministic::{Determination, deterministic};
use mediate::mediate;

/// Producer id of the delivery-facing cache alias. MUST equal
/// harness-providers' `DELIVERY_PROJECTION_PRODUCER` ("delivery"): the M8
/// delivery cache inlines exactly this key (equality pinned in the M10
/// tests; extend producer fallback lists HERE, never in providers).
pub const DELIVERY_CACHE_PRODUCER: &str = "delivery";

/// Version of the model-mediated projection prompt template. Part of the
/// derived-store producer id so a stored projection always names the
/// params that generated it.
pub const PROJECTION_PROMPT_VERSION: &str = "v1";

/// One model-mediated projection candidate: the model id (provenance),
/// its capability flags, a factory composing its inference, and the
/// wall-clock budget for one projection call.
pub struct MediatedProjection {
    /// Model id stored in the provenance producer id.
    pub model_id: String,
    /// The model's delivery capabilities (M7 flags mapped at the edge).
    pub caps: DeliveryCapabilities,
    /// Composes a fresh inference per projection call.
    pub factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>,
    /// Wall-clock budget; exceeding it is the typed timeout outcome.
    pub timeout: Duration,
}

/// The submission under resolution: the blocks being delivered and the
/// capabilities of the session's own model.
pub struct Submission<'a> {
    /// Content blocks of the submit (media blocks carry the references).
    pub blocks: &'a [ContentBlock],
    /// Session-model delivery capabilities (drives the planner's queue).
    pub caps: DeliveryCapabilities,
}

/// The model-mediated candidates in priority order: the session model
/// first, then the configured capable utility route (T42).
pub struct Mediation<'a> {
    /// The session's own model, if any.
    pub session: Option<&'a MediatedProjection>,
    /// The configured utility route, if any.
    pub utility: Option<&'a MediatedProjection>,
}

/// Why one projection could not be settled. Typed, never a silent miss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unprojectable {
    /// No deterministic projector, and no capable model anywhere.
    NoProjector,
    /// The deterministic projector refused (corrupt input et al).
    Deterministic(MediaError),
    /// The model call failed or returned nothing projectable.
    ModelMediation(String),
    /// The model call exceeded its wall-clock budget.
    Timeout,
    /// The media bytes are absent from the content-addressed store.
    MissingSource,
    /// The derived store refused the write.
    Store(String),
}

impl fmt::Display for Unprojectable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProjector => {
                formatter.write_str("no projector registered and no capable model")
            }
            Self::Deterministic(error) => {
                write!(formatter, "deterministic projector failed: {error}")
            }
            Self::ModelMediation(detail) => {
                write!(formatter, "model-mediated projection failed: {detail}")
            }
            Self::Timeout => formatter.write_str("model-mediated projection timed out"),
            Self::MissingSource => formatter.write_str("media bytes missing from the store"),
            Self::Store(detail) => write!(formatter, "derived store write failed: {detail}"),
        }
    }
}

/// The settled state of one queued projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionOutcome {
    /// Stored (or found) derived, with the producer id that made it.
    Resolved {
        /// Source media hash.
        source: ContentHash,
        /// The projection kind.
        kind: ProjectionKind,
        /// Winning producer id (cache alias, deterministic, or model).
        producer: String,
    },
    /// Not projected: the block keeps its marker and `notice` names the
    /// gap for the submit path's system event.
    Unprojectable {
        /// Source media hash.
        source: ContentHash,
        /// The projection kind.
        kind: ProjectionKind,
        /// The media's MIME type.
        mime: String,
        /// The typed why.
        reason: Unprojectable,
    },
}

impl ProjectionOutcome {
    /// The system notice naming what is missing (`None` when resolved) —
    /// the daemon submit path appends it as a `Event::System`.
    #[must_use]
    pub fn notice(&self) -> Option<String> {
        match self {
            Self::Resolved { .. } => None,
            Self::Unprojectable {
                mime, kind, reason, ..
            } => Some(format!(
                "media projection unavailable: {mime} needs {} but was not projected ({reason}); \
                 the block keeps its deterministic marker",
                kind.as_slug()
            )),
        }
    }
}

/// Resolves one submission's queued projections against a session's
/// derived store, the deterministic registry, and the mediation
/// candidates. Pure function of (store state, inputs) — cache hits
/// short-circuit every later arm.
pub struct ProjectionResolver<'a> {
    session_dir: &'a Path,
    registry: &'a ProjectionRegistry,
}

/// The session's two media stores, opened once per resolve pass.
struct SessionStores {
    media: MediaStore,
    derived: DerivedStore,
}

/// One queued projection under resolution: the media reference, the
/// needed kind, and the originating block (mime and modality derive
/// from it — never stored twice).
struct Pending<'b> {
    reference: MediaRef,
    kind: ProjectionKind,
    block: &'b ContentBlock,
}

impl Pending<'_> {
    fn source(&self) -> ContentHash {
        self.reference.hash
    }

    fn unprojectable(&self, reason: Unprojectable) -> ProjectionOutcome {
        ProjectionOutcome::Unprojectable {
            source: self.source(),
            kind: self.kind,
            mime: block_mime(self.block),
            reason,
        }
    }

    /// Cache probe: a corrupt entry counts as a miss so the resolve
    /// pass re-projects and the store write repairs it.
    fn cached(&self, derived: &DerivedStore, producer: &str) -> bool {
        matches!(
            derived.load_derived(&self.source(), producer, self.kind),
            Ok(Some(_))
        )
    }

    /// Stores the projection under its provenance producer and the
    /// delivery alias (content-addressed artifacts dedup; the alias is
    /// what the request-build cache inlines).
    fn store(&self, derived: &DerivedStore, producer: &str, text: &str) -> ProjectionOutcome {
        let bytes = text.as_bytes();
        let stored = derived
            .store_derived(&self.source(), producer, self.kind, bytes)
            .and_then(|_| {
                derived.store_derived(&self.source(), DELIVERY_CACHE_PRODUCER, self.kind, bytes)
            });
        match stored {
            Ok(_) => ProjectionOutcome::Resolved {
                source: self.source(),
                kind: self.kind,
                producer: producer.to_owned(),
            },
            Err(error) => self.unprojectable(Unprojectable::Store(error.to_string())),
        }
    }
}

impl<'a> ProjectionResolver<'a> {
    /// Resolver over one session directory and one projector registry.
    #[must_use]
    pub fn new(session_dir: &'a Path, registry: &'a ProjectionRegistry) -> Self {
        Self {
            session_dir,
            registry,
        }
    }

    /// Settles every queued projection of the planned submission.
    ///
    /// # Errors
    /// [`io::Error`] only when the session stores cannot be opened; every
    /// per-projection failure is a typed [`Unprojectable`] outcome (a
    /// submit is never crashed by a projection).
    pub async fn resolve(
        &self,
        submission: Submission<'_>,
        mediation: Mediation<'_>,
    ) -> io::Result<Vec<ProjectionOutcome>> {
        let stores = SessionStores {
            media: MediaStore::open(self.session_dir)?,
            derived: DerivedStore::open(self.session_dir)?,
        };
        let cache = DeliveryAlias {
            derived: &stores.derived,
        };
        let plan = media_delivery::plan(submission.blocks, submission.caps, &cache);
        let mut outcomes = Vec::with_capacity(plan.needed_projections.len());
        for (reference, kind) in plan.needed_projections {
            let outcome = match block_of(submission.blocks, &reference.hash) {
                Some(block) => {
                    let pending = Pending {
                        reference,
                        kind,
                        block,
                    };
                    self.resolve_pending(&stores, &pending, &mediation).await
                }
                None => ProjectionOutcome::Unprojectable {
                    source: reference.hash,
                    kind,
                    mime: "application/octet-stream".to_owned(),
                    reason: Unprojectable::MissingSource,
                },
            };
            outcomes.push(outcome);
        }
        Ok(outcomes)
    }

    async fn resolve_pending(
        &self,
        stores: &SessionStores,
        pending: &Pending<'_>,
        mediation: &Mediation<'_>,
    ) -> ProjectionOutcome {
        if pending.cached(&stores.derived, DELIVERY_CACHE_PRODUCER) {
            return ProjectionOutcome::Resolved {
                source: pending.source(),
                kind: pending.kind,
                producer: DELIVERY_CACHE_PRODUCER.to_owned(),
            };
        }
        let bytes = match stores.media.load(&pending.reference.hash) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return pending.unprojectable(Unprojectable::MissingSource),
            Err(error) => return pending.unprojectable(Unprojectable::Store(error.to_string())),
        };
        match deterministic(self.registry, &bytes, pending.kind) {
            Determination::Projected { producer, text } => {
                return pending.store(&stores.derived, &producer, &text);
            }
            Determination::Refused(error) => {
                return pending.unprojectable(Unprojectable::Deterministic(error));
            }
            Determination::FallThrough => {}
        }
        mediate(stores, pending, mediation).await
    }
}

fn block_of<'b>(blocks: &'b [ContentBlock], source: &ContentHash) -> Option<&'b ContentBlock> {
    blocks.iter().find(|block| match block {
        ContentBlock::Image(image) => &image.media.hash == source,
        ContentBlock::Media(media) => &media.media.hash == source,
        ContentBlock::Text { .. } => false,
    })
}

fn block_mime(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Image(image) => image.mime.as_str().to_owned(),
        ContentBlock::Media(media) => media.mime.as_str().to_owned(),
        ContentBlock::Text { .. } => "text/plain".to_owned(),
    }
}

struct DeliveryAlias<'a> {
    derived: &'a DerivedStore,
}

impl ProjectionCache for DeliveryAlias<'_> {
    fn cached_projection(&self, source: &ContentHash, kind: ProjectionKind) -> Option<String> {
        match self
            .derived
            .load_derived(source, DELIVERY_CACHE_PRODUCER, kind)
        {
            Ok(Some(bytes)) => String::from_utf8(bytes).ok(),
            Ok(None) | Err(_) => None,
        }
    }
}
