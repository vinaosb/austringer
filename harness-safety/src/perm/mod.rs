//! Unified permission engine (plan todo 27): ONE decision layer for
//! every tool call — grammar rules (Action over named Target),
//! stored-grant seam, command-prefix rules with the substitution
//! guard, program-resolution blocklist, security-profile
//! intersection, admin floor, yolo-surviving hardline floor,
//! max-requests circuit breaker, audit sink, and the
//! permissions-derived sandbox compile. Lower layers (harness-core's
//! T7 chain, T8 store) adapt onto these seams; nothing re-implements
//! ad-hoc checks beside them.

pub mod audit;
pub mod breaker;
mod checks;
pub mod commands;
pub mod compile;
pub mod config;
pub mod decision;
pub mod engine;
pub mod floor;
pub mod grammar;
pub mod grants;
pub mod hardline;
pub mod profile;
pub mod reasons;
pub mod resolve;
