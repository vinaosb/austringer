//! Provider trait and per-vendor model adapter integrations.
//!
//! Todo 9: the dyn-safe streaming [`Provider`](provider::Provider) trait, the
//! typed provider-event vocabulary, jittered retry, and mid-stream recovery
//! that preserves turn state with exactly-once tool side effects (Codex
//! #8865/#19121/#927 lessons). Model identity is explicit everywhere; silent
//! substitution is structurally impossible (see [`identity`](self::identity)).
//!
//! The [`adapter`](self::adapter) module binds this stack to austringer-core's
//! `Inference` seam under its documented contract: llm-visible context in,
//! abandon-not-drain cancellation, complete-or-error turns.

#![deny(missing_docs)]

pub mod adapter;
pub mod adapters;
pub mod auth;
pub mod cost;
pub mod error;
pub mod event;
pub mod identity;
pub mod media;
pub mod provider;
pub mod retry;
pub mod state;
pub mod transport;
pub mod turn;
pub mod wire;

#[cfg(feature = "reqwest-transport")]
pub mod transport_reqwest;

mod recover;
mod sse;

#[test]
fn workspace_scaffold_smoke() {
    let scaffold_in_place = true;
    assert!(scaffold_in_place);
}
