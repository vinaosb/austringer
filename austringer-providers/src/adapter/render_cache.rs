//! Incremental history rendering for the adapter's prefix cache.
//!
//! The wire prompt's history region is a pure function of the
//! llm-visible events — the layout law keeps the volatile focus block
//! on the tail, and a media-less adapter renders under the fail-closed
//! default delivery context (no projections, no native payloads) — so
//! the region can be rendered once as a cached prefix plus a per-call
//! suffix instead of re-rendering the whole session on every inference
//! call (the O(events^2) cumulative cost across a tool-looping session;
//! the prefix is byte-stable by design, research §27/§32).
//!
//! Byte identity with [`super::render_context`] is the contract: both
//! paths share one per-event line renderer, and the cache only ever
//! appends, so the incremental result equals the full render.
//!
//! Invalidation is explicit and conservative:
//!
//! * a SHORTER event list (rewind, compaction shrink) drops the cache;
//! * a boundary mismatch — the event at `through - 1` no longer carries
//!   the sequence the cache last rendered, i.e. a same-length rewrite
//!   such as a compaction-retained list — drops the cache;
//! * media delivery NEVER caches (the adapter drops the cache before
//!   rendering): renders consult the live projection and byte stores,
//!   so the same events can render differently once a projection lands
//!   (see `tests/media_prefix_identity.rs`).
//!
//! What does NOT invalidate: appended events (including invisible ones
//! — the filter is per-event), focus-block changes (the cache sits
//! below the focus join), and same-length lists whose boundary event
//! still carries the cached sequence.

use austringer_core::agent_loop::InferenceError;
use austringer_core::event::llm_visible;
use austringer_core::event::{Event, RecordedEvent, Sequence};

use super::{DeliveryContext, NativeMedia, render_blocks};

/// The cached render of `events[..through]`: joined wire lines, native
/// payloads resolved along the way, and the boundary identity (`through`
/// plus the sequence of the event at `through - 1`) that validates reuse.
pub(super) struct PrefixCache {
    through: usize,
    through_seq: Sequence,
    text: String,
    native: Vec<NativeMedia>,
}

impl PrefixCache {
    fn empty() -> Self {
        Self {
            through: 0,
            through_seq: Sequence::default(),
            text: String::new(),
            native: Vec::new(),
        }
    }
}

/// True when the cache can be extended with `events`'s tail: the list is
/// at least as long as the rendered watermark AND the boundary event is
/// still the event whose rendering the cache holds.
fn still_valid(events: &[RecordedEvent], cache: &PrefixCache) -> bool {
    events.len() >= cache.through
        && (cache.through == 0 || events[cache.through - 1].seq == cache.through_seq)
}

/// Renders the llm-visible history through the delivery planner — one
/// line per event, deterministic, no internal kinds (the
/// `Condensation`/`Snapshot` arms are unreachable through the filter and
/// kept only for match exhaustiveness). Shared by the full render and
/// the prefix cache so their bytes cannot drift.
pub(super) fn render_event_line(
    event: &Event,
    context: &DeliveryContext<'_>,
    native: &mut Vec<NativeMedia>,
) -> Result<String, InferenceError> {
    Ok(match event {
        Event::User { blocks } => format!("user: {}", render_blocks(blocks, context, native)?),
        Event::Assistant { blocks } => {
            format!("assistant: {}", render_blocks(blocks, context, native)?)
        }
        Event::ToolCall { call_id, tool, .. } => format!("tool-call {call_id} {tool}"),
        Event::ToolResult { call_id, .. } => format!("tool-result {call_id}"),
        Event::System { text } => format!("system: {text}"),
        Event::Condensation { .. } | Event::Snapshot { .. } => String::new(),
    })
}

/// Renders `events` as cached-prefix + new-suffix, extending `cache` in
/// place. Returns the full history text and native payloads (clones —
/// the wire request owns its copy; the cache keeps theirs). The suffix
/// renders into a scratch buffer first and is committed to the cache
/// only when every line rendered, so a mid-render error leaves the
/// cache consistent with its watermark.
pub(super) fn render(
    events: &[RecordedEvent],
    context: &DeliveryContext<'_>,
    cache: &mut Option<PrefixCache>,
) -> Result<(String, Vec<NativeMedia>), InferenceError> {
    if !cache
        .as_ref()
        .is_some_and(|cached| still_valid(events, cached))
    {
        *cache = None;
    }
    let prefix = cache.get_or_insert_with(PrefixCache::empty);
    if events.len() > prefix.through {
        let mut suffix = String::new();
        let mut native = Vec::new();
        for event in llm_visible(&events[prefix.through..]) {
            let line = render_event_line(event, context, &mut native)?;
            // join semantics: every visible line is non-empty (each arm
            // carries a literal prefix), so buffer emptiness means "no
            // lines yet" and the separator decision matches `join("\n")`.
            if !suffix.is_empty() {
                suffix.push('\n');
            }
            suffix.push_str(&line);
        }
        if !suffix.is_empty() && !prefix.text.is_empty() {
            prefix.text.push('\n');
        }
        prefix.text.push_str(&suffix);
        prefix.native.append(&mut native);
        prefix.through = events.len();
        prefix.through_seq = events.last().map(|event| event.seq).unwrap_or_default();
    }
    Ok((prefix.text.clone(), prefix.native.clone()))
}
