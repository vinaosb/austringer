//! Headless Language Server Protocol client for code navigation.
//!
//! Scope (plan todo 18): spawn/connect LSP servers with a per-language
//! command table (`rust-analyzer`, `gopls`, `typescript-language-server`,
//! `clangd`, `pyright-langserver` — availability is the user PATH's call);
//! stdio transport with bounded Content-Length framing; initialize /
//! shutdown lifecycle; captured server stderr; `goto_definition` /
//! `find_references` / `hover` tools returning compact, budgeted locations
//! (<25-token approximation, see [`render`]); multi-server routing by file
//! type with lazy spawn; an RSS-sampled always-on guard that kills and
//! restarts runaway servers (forge §26); and the IDE-as-toolbox provider
//! seam (JetBrains §24) where a future bridge substitutes for native
//! servers — bridge > native, no duplicate servers when the bridge covers
//! a language (tested in `tests/routing.rs`).
//!
//! # Layout
//! - [`config`] — server table, backoff, timeouts, guard knobs
//! - [`proto`] — bounded base-protocol framing
//! - [`uri`] / [`position`] — wire-level codecs
//! - [`provider`] — the `LspProvider` seam + typed answers
//! - [`render`] — compact <25-token-approximation rendering
//! - [`native`] / [`handshake`] — one server connection + lifecycle
//! - [`router`] — multi-server routing, recovery, events
//! - [`error`] — typed errors + degrade reasons

pub mod config;
pub mod error;
pub mod guard;
pub mod handshake;
pub mod native;
pub mod position;
pub mod proto;
pub mod provider;
pub mod render;
pub mod router;
pub mod uri;

mod ask;
mod docs;
mod events;
mod parse;
mod recovery;
mod stderr;
mod wire;

pub use config::{
    Backoff, Language, LspConfig, RssGuardConfig, ServerCommand, language_for_extension,
};
pub use error::{DegradeReason, LspError};
pub use native::{PositionEncoding, ServerHandle};
pub use proto::{FrameCaps, FrameError};
pub use provider::{
    Answer, DocumentPosition, HoverText, IdeBridgeStub, Location, LspProvider, SEARCH_FALLBACK_HINT,
};
pub use render::{LOCATION_BUDGET_CHARS, MAX_LOCATIONS, render_locations};
pub use router::{BoxRead, BoxWrite, LspRouter, RouterEvent, ServerFactory};

/// Locks a std [`std::sync::Mutex`] without panicking: a poisoned guard is
/// recovered via `into_inner` (crate critical sections leave consistent
/// state — map inserts/removes and flag writes).
pub(crate) fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
