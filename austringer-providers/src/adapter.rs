//! The binding to austringer-core's `Inference` seam (todo-5 contract).
//!
//! The adapter keeps the seam's three invariants:
//!
//! (a) **llm-visible projection** — the model-context input is rendered from
//!     the sanctioned [`austringer_core::event::llm_visible`] slice, with the
//!     focus block riding the volatile TAIL (the layout law: a todo
//!     transition must never invalidate the history prefix), in the same
//!     format the todo-5 test kit renders
//!     (so scripts and cassettes stay interchangeable);
//! (b) **cancel = abandon, not drain** — cancellation races the completion
//!     future via `select!`; losing drops the future, which drops the
//!     provider stream mid-flight;
//! (c) **complete-or-error** — a turn returns an envelope or a typed error
//!     rendered into [`InferenceError`]; silent partials are impossible
//!     because envelopes only exist post-`Finish`.
//!
//! Plan M8 adds capability-aware media delivery: block rendering goes
//! through the austringer-core delivery planner (capable models get native
//! payloads with bytes fetched from the session media store at
//! request-build time; incapable models get deterministic markers with
//! cached projections inlined and missing ones queued).
//!
//! The dependency direction is providers -> core, permanently: core owns the
//! seam, must never depend back on providers, and this binding lets the
//! seam's contract be TESTED against the resilient stack today.

mod render_cache;

use std::sync::Arc;

use async_trait::async_trait;
use austringer_core::agent_loop::{
    AssistantTurn, Inference, InferenceError, InferenceRequest, PendingCall, TurnUsage,
};
use austringer_core::event::RecordedEvent;
use austringer_core::event::content::{ContentBlock, MimeType};
use austringer_core::event::media::MediaRef;
use austringer_core::media_delivery::{
    self, DeliveryCapabilities, EmptyCache, ProjectionCache, WireBlock,
};

use crate::identity::CompletionRequest;
use crate::media::{MediaDelivery, NativeKind, NativeMedia, native_mimes};
use crate::provider::Provider;
use crate::retry::SeededJitter;
use crate::turn::{RecoveryConfig, ResumableTurn};

static EMPTY_CACHE: EmptyCache = EmptyCache;

/// Drives any [`Provider`] through austringer-core's `Inference` seam, with
/// retry, resume, and exactly-once recovery active underneath.
pub struct InferenceAdapter {
    provider: Arc<dyn Provider>,
    config: RecoveryConfig,
    jitter_seed: u64,
    caps: DeliveryCapabilities,
    media: Option<MediaDelivery>,
    prefix: Option<render_cache::PrefixCache>,
}

impl InferenceAdapter {
    /// Builds the adapter text-only (fail-closed): capabilities and the
    /// session media context attach explicitly. Every inference call
    /// seeds a fresh [`SeededJitter`] from `jitter_seed`, so backoff
    /// sequences are reproducible per call in tests.
    pub fn new(provider: Arc<dyn Provider>, config: RecoveryConfig, jitter_seed: u64) -> Self {
        Self {
            provider,
            config,
            jitter_seed,
            caps: DeliveryCapabilities::default(),
            media: None,
            prefix: None,
        }
    }

    /// Sets the delivery capabilities (mapped from the austringer-intel
    /// per-model flags at compose time, where overrides may only
    /// restrict).
    #[must_use]
    pub fn with_capabilities(mut self, caps: DeliveryCapabilities) -> Self {
        self.caps = caps;
        self
    }

    /// Attaches the session media context (byte source + derived
    /// projection cache). Without one the adapter plans text-only even
    /// for capable models — it cannot fetch bytes, so it does not
    /// promise them.
    #[must_use]
    pub fn with_media_delivery(mut self, media: MediaDelivery) -> Self {
        self.media = Some(media);
        self
    }
}

/// Everything block rendering needs: capabilities in force, the media
/// context (if any), and the provider family's native-MIME allowlist.
struct DeliveryContext<'a> {
    caps: DeliveryCapabilities,
    delivery: Option<&'a MediaDelivery>,
    allowlist: &'static [&'static str],
}

impl DeliveryContext<'_> {
    fn cache(&self) -> &dyn ProjectionCache {
        match self.delivery {
            Some(source) => source,
            None => &EMPTY_CACHE,
        }
    }

    /// Resolves a planned native block into wire payloads: dropped when
    /// the family cannot carry the MIME (the prompt already carries its
    /// marker), failed typed when the bytes are missing — never faked,
    /// never silently dropped after being promised.
    fn attach_native(
        &self,
        kind: NativeKind,
        mime: &MimeType,
        media: MediaRef,
        out: &mut Vec<NativeMedia>,
    ) -> Result<(), InferenceError> {
        if !self.allowlist.contains(&mime.as_str()) {
            return Ok(());
        }
        let Some(delivery) = self.delivery else {
            return Err(InferenceError::Failure(
                "adapter bug: native block planned without a media source".to_owned(),
            ));
        };
        match delivery.load_bytes(&media.hash) {
            Ok(Some(bytes)) => {
                out.push(NativeMedia {
                    kind,
                    mime: mime.as_str().to_owned(),
                    hash: media.hash,
                    bytes,
                });
                Ok(())
            }
            Ok(None) => Err(InferenceError::Failure(format!(
                "media {} missing from the content-addressed store ({} bytes declared): \
                 refusing to fake or silently drop it",
                media.hash.to_ref(),
                media.bytes
            ))),
            Err(error) => Err(InferenceError::Failure(format!(
                "media store read failed for {}: {error}",
                media.hash.to_ref()
            ))),
        }
    }
}

#[async_trait]
impl Inference for InferenceAdapter {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        cancel: &austringer_core::agent_loop::CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        let family = self.provider.identity().provider.clone();
        let context = DeliveryContext {
            caps: if self.media.is_some() {
                self.caps
            } else {
                DeliveryCapabilities::default()
            },
            delivery: self.media.as_ref(),
            allowlist: native_mimes(&family),
        };
        let (rendered, media_blocks) = if context.delivery.is_none() {
            // Pure render of the events (default delivery context): the prefix cache extends.
            render_cache::render(request.events, &context, &mut self.prefix)?
        } else {
            // Renders consult live stores here — the cache is dropped, never consulted.
            self.prefix = None;
            render_context(request.events, &context)?
        };
        // Layout law (context/builder.rs): the volatile focus block rides the
        // TAIL — its `status:` line churns on every todo transition, and head
        // placement would invalidate the prefix cache for the whole history.
        let (prompt, volatile_tail_bytes) = push_volatile_tail(rendered, request.focus_block);
        let completion = CompletionRequest::new(self.provider.identity().clone(), prompt)
            .with_native_media(media_blocks)
            .with_system(request.system)
            .with_tools(request.tools.to_vec())
            .with_volatile_tail(volatile_tail_bytes);
        let mut turn = ResumableTurn::start(
            self.provider.clone(),
            completion,
            self.config.clone(),
            Box::new(SeededJitter::new(self.jitter_seed)),
        );
        let completed = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                // Abandon: the in-flight future (and its provider stream) is
                // dropped at this branch; nothing is drained.
                return Err(InferenceError::Failure(
                    "cancelled: stream abandoned".to_owned(),
                ));
            }
            outcome = turn.complete() => outcome,
        }
        .map_err(|err| InferenceError::Failure(err.to_string()))?;
        let blocks = if completed.text.is_empty() {
            Vec::new()
        } else {
            vec![ContentBlock::Text {
                text: completed.text,
            }]
        };
        let tool_calls = completed
            .tool_calls
            .into_iter()
            .map(|call| PendingCall {
                call_id: call.call_id,
                tool: call.name,
                arguments: call.arguments,
            })
            .collect();
        // Refusing to silently drop it (adapter ethos): the provider's
        // usage counts ride the turn; absence stays absence.
        let usage = completed.usage.map(|usage| TurnUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
        });
        Ok(AssistantTurn {
            blocks,
            tool_calls,
            usage,
        })
    }
}

/// Renders the llm-visible history in the adapter's wire line format,
/// fail-closed (text-only): the exact bytes a media-less adapter puts on
/// the wire. The measurement surface — the perf gate's wire scenario and
/// [`InferenceAdapter::infer`] share [`render_context`], so the gated
/// bytes are the production bytes (native delivery is an infer-time
/// overlay that never alters this rendering: default capabilities plan
/// no native blocks).
pub fn render_wire_history(events: &[RecordedEvent]) -> Result<String, InferenceError> {
    let context = DeliveryContext {
        caps: DeliveryCapabilities::default(),
        delivery: None,
        allowlist: &[],
    };
    Ok(render_context(events, &context)?.0)
}

/// Assembles the provider-wire prompt (the layout law): the rendered
/// history is the stable head, the focus block rides the volatile tail,
/// joined by one newline. Returns the prompt and the volatile-tail byte
/// length (the cache boundary every completion request surfaces).
pub fn assemble_wire_prompt(rendered: &str, focus_block: &str) -> (String, usize) {
    push_volatile_tail(rendered.to_owned(), focus_block)
}

/// The one implementation of the head/tail join: appends the volatile
/// focus block in place onto an owned rendered history (the adapter's
/// hot path; the pub seam above pays the one copy it always paid).
fn push_volatile_tail(mut head: String, focus_block: &str) -> (String, usize) {
    head.reserve(1 + focus_block.len());
    head.push('\n');
    head.push_str(focus_block);
    (head, focus_block.len())
}

/// Renders the llm-visible events through the delivery planner — one
/// line per event, deterministic, no internal kinds (the
/// `Condensation`/`Snapshot` arms are unreachable through the filter and
/// kept only for match exhaustiveness). Native payloads collected along
/// the way ride the completion request, not the text.
fn render_context(
    events: &[RecordedEvent],
    context: &DeliveryContext<'_>,
) -> Result<(String, Vec<NativeMedia>), InferenceError> {
    let mut native = Vec::new();
    let lines = austringer_core::event::llm_visible(events)
        .map(|event| render_cache::render_event_line(event, context, &mut native))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((lines.join("\n"), native))
}

/// Plan-driven block rendering (replaces the M3-era image drop): every
/// block contributes its deterministic text (verbatim text, markers,
/// inlined cached projections) and capable-model native blocks
/// additionally resolve into wire payloads.
fn render_blocks(
    blocks: &[ContentBlock],
    context: &DeliveryContext<'_>,
    native: &mut Vec<NativeMedia>,
) -> Result<String, InferenceError> {
    let delivery = media_delivery::plan(blocks, context.caps, context.cache());
    if !delivery.needed_projections.is_empty() {
        tracing::debug!(
            queued = delivery.needed_projections.len(),
            "media projections queued (the M10 pipeline resolves them)"
        );
    }
    let mut parts = Vec::with_capacity(delivery.wire.len());
    for block in &delivery.wire {
        match block {
            WireBlock::NativeImage { mime, media } => {
                parts.push(block.text());
                context.attach_native(NativeKind::Image, mime, *media, native)?;
            }
            WireBlock::NativeAudio { mime, media } => {
                parts.push(block.text());
                context.attach_native(NativeKind::Audio, mime, *media, native)?;
            }
            WireBlock::Text(_) | WireBlock::Marker(_) => parts.push(block.text()),
        }
    }
    Ok(parts.join(" "))
}
