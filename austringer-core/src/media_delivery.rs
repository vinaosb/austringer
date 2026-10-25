//! Capability-aware media delivery planning (plan M8).
//!
//! The planner turns conversation [`ContentBlock`]s into a [`DeliveryPlan`]:
//! a wire-block sequence (text, native image/audio for capable models,
//! deterministic markers for incapable ones) plus the derived projections
//! the incapable path needs. Text-only rendering is the default quality
//! bar: markers are the M3-pinned deterministic forms and cached
//! projections inline right after their marker, keyed by content hash, so
//! consecutive-turn prompts stay byte-prefix stable (prefix-cache
//! discipline; see [`prefix_identity`]).
//!
//! The planner is pure: capabilities arrive as data ([`DeliveryCapabilities`]
//! mirrors the austringer-intel per-model flags — core must not depend on
//! intel), and the derived cache is a read-only [`ProjectionCache`] trait so
//! tests run on maps and production on the M4 derived store.

use crate::event::content::{ContentBlock, MimeType};
use crate::event::derived::ProjectionKind;
use crate::event::media::{ContentHash, MediaRef};

/// Which native inputs the target model can actually use (the planner's
/// slice of the M7 per-model capability flags; `video_input` has no v0.2
/// wire rule and is deliberately absent). Fail-closed: `Default` is
/// all-false, so an unconfigured adapter plans text-only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeliveryCapabilities {
    /// Accepts image input natively.
    pub vision: bool,
    /// Accepts audio input natively.
    pub audio_input: bool,
}

/// One planned wire block: what the request builder should emit for a
/// content block under the given capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireBlock {
    /// Verbatim text content.
    Text(String),
    /// An image delivered natively (capable models only).
    NativeImage {
        /// Image MIME type.
        mime: MimeType,
        /// Out-of-line reference; bytes are fetched at request-build time.
        media: MediaRef,
    },
    /// Audio delivered natively (capable models only).
    NativeAudio {
        /// Audio MIME type.
        mime: MimeType,
        /// Out-of-line reference; bytes are fetched at request-build time.
        media: MediaRef,
    },
    /// The deterministic text-only placeholder for undeliverable media.
    Marker(String),
}

impl WireBlock {
    /// Deterministic text rendering of this block (the prompt's view).
    /// Native blocks render a content-hash-keyed marker — never bytes —
    /// so the text stays stable across turns for capable models too.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Marker(text) => text.clone(),
            Self::NativeImage { mime, media } => {
                format!("[image: {} {}]", mime, media.hash.short_ref())
            }
            Self::NativeAudio { mime, media } => {
                format!("[audio: {} {}]", mime, media.hash.short_ref())
            }
        }
    }
}

/// The result of planning one block sequence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeliveryPlan {
    /// Planned wire blocks, in input order.
    pub wire: Vec<WireBlock>,
    /// Derived projections the incapable path needs (deduplicated,
    /// first-seen order). Stays populated even when a cached copy was
    /// inlined — the queue is the durable work list, the cache is an
    /// optimization.
    pub needed_projections: Vec<(MediaRef, ProjectionKind)>,
}

impl DeliveryPlan {
    /// Plans one block into this delivery (the M8 rules live in
    /// [`DeliveryPlan::emit_media`]).
    fn plan_block(
        &mut self,
        block: &ContentBlock,
        caps: DeliveryCapabilities,
        cache: &dyn ProjectionCache,
    ) {
        match block {
            ContentBlock::Text { text } => self.wire.push(WireBlock::Text(text.clone())),
            ContentBlock::Image(image) => {
                self.emit_media(&image.mime, image.media, &image.text_marker(), caps, cache);
            }
            ContentBlock::Media(media) => {
                self.emit_media(&media.mime, media.media, &media.text_marker(), caps, cache);
            }
        }
    }

    /// One media-typed block through the M8 rules: image+vision ->
    /// native; audio+audio_input -> native; everything else -> marker +
    /// queued projection, with a cached projection inlined as text when
    /// present. The inputs are the destructured media block (mime,
    /// reference, marker — the same triple `MediaStore::store` takes as
    /// its boundary) plus the two planning inputs.
    fn emit_media(
        &mut self,
        mime: &MimeType,
        media: MediaRef,
        marker: &str,
        caps: DeliveryCapabilities,
        cache: &dyn ProjectionCache,
    ) {
        let major = mime.as_str().split_once('/').map_or("", |(head, _)| head);
        let native = match major {
            "image" if caps.vision => Some(WireBlock::NativeImage {
                mime: mime.clone(),
                media,
            }),
            "audio" if caps.audio_input => Some(WireBlock::NativeAudio {
                mime: mime.clone(),
                media,
            }),
            _ => None,
        };
        if let Some(block) = native {
            self.wire.push(block);
            return;
        }
        let kind = match major {
            "image" => ProjectionKind::SceneDescription,
            "audio" => ProjectionKind::Transcript,
            _ => ProjectionKind::TextProjection,
        };
        self.wire.push(WireBlock::Marker(marker.to_owned()));
        if !self
            .needed_projections
            .iter()
            .any(|(queued, queued_kind)| *queued == media && *queued_kind == kind)
        {
            self.needed_projections.push((media, kind));
        }
        if let Some(text) = cache.cached_projection(&media.hash, kind) {
            self.wire.push(WireBlock::Text(text));
        }
    }
}

/// Read-only view of the derived-projection cache for planning.
pub trait ProjectionCache {
    /// Cached projection text for (source, kind), or `None` on a miss.
    fn cached_projection(&self, source: &ContentHash, kind: ProjectionKind) -> Option<String>;
}

/// The no-store cache: every lookup misses. The delivery default when no
/// session media context is attached to the adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyCache;

impl ProjectionCache for EmptyCache {
    fn cached_projection(&self, _source: &ContentHash, _kind: ProjectionKind) -> Option<String> {
        None
    }
}

/// Plans the delivery of `blocks` under `caps`, inlining cached
/// projections from `derived_cache` where the incapable path needs them.
#[must_use]
pub fn plan(
    blocks: &[ContentBlock],
    caps: DeliveryCapabilities,
    derived_cache: &dyn ProjectionCache,
) -> DeliveryPlan {
    let mut delivery = DeliveryPlan::default();
    for block in blocks {
        delivery.plan_block(block, caps, derived_cache);
    }
    delivery
}

/// Byte-prefix identity of consecutive-turn prompts: the length of the
/// common byte prefix divided by the previous prompt's length. A prompt
/// that extends its predecessor verbatim scores `1.0`; a projection
/// arriving mid-window (the single sanctioned divergence class) drops
/// exactly the pair it splits. An empty previous prompt is trivially
/// identical.
#[must_use]
pub fn prefix_identity(previous: &[u8], current: &[u8]) -> f64 {
    if previous.is_empty() {
        return 1.0;
    }
    let common = previous
        .iter()
        .zip(current.iter())
        .take_while(|(left, right)| left == right)
        .count();
    common as f64 / previous.len() as f64
}

/// Mean [`prefix_identity`] over every consecutive pair in a prompt
/// window. Plan M8 floors: `1.0` on the pre-media prefix, `>= 0.9`
/// across a 20-turn window. Degenerate windows (< 2 prompts) are `1.0`.
#[must_use]
pub fn window_prefix_identity(prompts: &[String]) -> f64 {
    if prompts.len() < 2 {
        return 1.0;
    }
    let mut sum = 0.0;
    let mut count = 0.0;
    for pair in prompts.windows(2) {
        sum += prefix_identity(pair[0].as_bytes(), pair[1].as_bytes());
        count += 1.0;
    }
    sum / count
}
