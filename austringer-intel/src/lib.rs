//! Model-profiles subsystem (plan todo 34): per-model failure taxonomies,
//! behavioral axes, the seeded model catalog with per-model 400-trap
//! feature detection, explicit protocol dialects, the SET/PROMPT/THROTTLE
//! elicitation constitution (research SS33), the dial effort-bundle
//! grammar, and A/B skill-variant selection reporting through the Skill
//! Forge gate.
//!
//! Constitution (research SS33.6): SET the deterministic knobs, PROMPT for
//! documented behavioral pathologies gated on OUR observed evals, THROTTLE
//! structurally what neither can bound. Protocol dialects are EXPLICIT per
//! model in the catalog and never inferred from endpoint URLs (the dsh
//! thinkingFormat lesson: a guessed dialect produced 72k-char reasoning
//! bloat vs 6k tuned). Feature legality (the 400-traps) is detected per
//! MODEL, never per provider — effort-support grids are fragmented
//! model-by-model inside a single family.
//!
//! This crate has no network-capable dependency and performs no I/O beyond
//! the local profile store: profile data never leaves the machine.

#![deny(missing_docs)]

pub mod ab;
pub mod axes;
pub mod capabilities;
pub mod catalog;
pub mod dial;
pub mod dialect;
pub mod elicitation;
pub mod model_profile;
pub mod routing;
pub mod seed;
pub mod store;
pub mod taxonomy;
