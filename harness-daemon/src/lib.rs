//! Long-running daemon hosting the agent engine behind all frontends.
//!
//! Todo 35 composes the todo 4-29 engine surface into a long-running
//! process: one versioned client protocol spoken over WebSocket, SSE, and
//! local-socket IPC; multi-client attach with a shared permission queue;
//! hardened loopback auth; a PID-keyed session registry; a cron scheduler
//! and notification emitter; an optional durable turn-workflow journal;
//! remote-worker split-brain tool execution with claim-based
//! reconciliation; zero-downtime reload; and optional OTel export.

pub mod activity;
pub mod attach;
pub mod auth;
pub mod config;
pub mod durable;
pub mod engine;
pub mod events;
pub mod handlers;
pub mod ipc;
pub mod journal;
pub mod mediation;
pub mod notify;
pub mod otel;
pub mod outposts;
pub mod protocol;
pub mod registry;
pub mod reload;
pub mod scheduler;
pub mod server;
pub mod stages;
pub mod transport;
pub mod web;
pub mod worker;
pub mod ws;

/// Placeholder retained until the first real module asserted it in its
/// own test suite (todo 35 keeps the scaffold smoke test green throughout).
#[test]
fn workspace_scaffold_smoke() {
    let scaffold_in_place = true;
    assert!(scaffold_in_place);
}
