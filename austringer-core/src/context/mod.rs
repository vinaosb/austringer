//! The Prompt Idempotency Manager (plan todo 11): a prefix-cache-stable
//! prompt builder, the context-epoch vocabulary, provider cache-control
//! plans, the cache-violation tracker, and the hold-effort-constant
//! discipline.
//!
//! # Layout law
//!
//! Every prompt composes seven fixed segments: `[0,L0)` system + the six
//! core tool schemas (byte-identical to the T6 base-schema render),
//! `[L0,L1)` rules + skills slots (typed slots fed by later todos; the T5
//! focus-chain render slots in here at epoch start — its `render()` API
//! fits as-is, reused not reimplemented), `[L1,L2)` repomap + history
//! (T4 LLM-visible events, append-only, never reordered), and the
//! volatile tail (the last user message, the ONLY mutable region). Same
//! logical state renders byte-identical bytes: slots sort by id, JSON
//! bodies re-serialize with sorted keys, and no timestamp survives the
//! volatile scan above L2 — injected volatile data is a typed
//! [`LayoutViolation`](violation::LayoutViolation), never a silent
//! prefix-churn.
//!
//! # Context epochs
//!
//! The stable span is a named, immutable provider-cache baseline
//! ([`EpochLedger`](epoch::EpochLedger)): frozen segment hashes +
//! started-at sequence, ending only at a completed compaction or session
//! movement. Durable mid-conversation updates append IN-HISTORY as
//! events; mutating a frozen segment mid-epoch is detected as typed
//! drift (Kilo CONTEXT.md doctrine, research SS11).
//!
//! # Cache-control engineering
//!
//! [`CachePlan`](cache_plan::CachePlan) places explicit breakpoints (max 4
//! for Anthropic) at segment boundaries with 5m/1h TTL spans as typed
//! metadata, exposes the provider-specific relocation hook (OpenRouter
//! part-level), and models OpenAI's automatic scheme as "no markers, the
//! stable layout rides the implicit cache". Post-turn background tasks
//! (review/curator forks) deliberately reuse this exact prefix so they
//! share the cache (hermes, research SS27).
//!
//! # Cache-violation tracker
//!
//! [`CacheViolationTracker`](tracker::CacheViolationTracker) keeps an
//! append-only blake3 hash chain of the expected stable prefix per turn
//! and compares it against the provider's reported cache hits: a miss
//! becomes an observable [`ViolationEvent`](tracker::ViolationEvent)
//! naming the diverged segment and byte offset (jcode, research SS32) —
//! never a silent cost spike.
//!
//! # Hold-effort-constant
//!
//! Effort renders into the frozen system segment (on cache-coupled
//! providers it IS prompt bytes), so it may change only at epoch
//! boundaries; mid-epoch changes are rejected typed with guidance
//! (wait-for-compaction or the per-message beta channel) (research
//! SS33).

mod builder;
mod cache_plan;
mod effort;
mod epoch;
mod history;
mod layout;
mod tracker;
mod violation;

// Todo 13's compaction renders compacted contexts through the same
// deterministic history renderer the prompt builder uses.
pub(crate) use history::render_history;

pub use builder::{ContextSpec, PromptBuilder, RenderedPrompt, SlotSegment};
pub use cache_plan::{
    CacheBreakpoint, CachePlan, CachePlanError, CachePolicy, CacheTtl, MAX_CACHE_BREAKPOINTS,
};
pub use effort::{EFFORT_REMEDIES, EffortChannel, EffortLevel, EffortSettings, EffortViolation};
pub use epoch::{
    ContextEpoch, EpochDrift, EpochError, EpochId, EpochLedger, EpochTransition,
    EpochTransitionRecord, StableBaseline,
};
pub use layout::{SegmentMap, SegmentName, SegmentSpan, Stability};
pub use tracker::{
    CacheReport, CacheReportFeed, CacheViolationTracker, ChainEntry, InMemoryFeed, TurnObservation,
    ViolationEvent, ViolationKind, common_prefix_len,
};
pub use violation::{BuildError, LayoutViolation, VolatileKind};
