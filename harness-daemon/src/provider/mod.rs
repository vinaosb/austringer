//! Config-driven provider composition for the session factory (M32).
//!
//! The daemon binary parses the provider seam from the environment
//! (`HARNESS_PROVIDER`, `HARNESS_MODEL`, `HARNESS_PROVIDER_KEY_ENV`,
//! `HARNESS_PROVIDER_BASE_URL`, `HARNESS_PROVIDER_TIMEOUT_SECS`) into a
//! [`ProviderSelection`], resolves the API key from the NAMED
//! environment variable (key values never appear in config or on disk),
//! and composes the cassette-tested harness-providers adapters behind
//! the shared [`Inference`] factory. No provider is enabled by default:
//! an unconfigured daemon keeps a typed, actionable inference error.
//!
//! The transport stays injected so the shipped binary passes the
//! reqwest transport while tests pass in-memory cassette transports —
//! the exact seam harness-providers was designed around.

mod compose;
mod selection;

pub use compose::ResolvedProvider;
pub use selection::{ProviderFamily, ProviderSelection, ProviderSetupError};

use std::sync::Arc;

use harness_core::agent_loop::Inference;
use harness_providers::transport::Transport;

/// Composes the daemon session factory from the provider seam.
///
/// `None` (the default, and the posture of an unconfigured daemon)
/// yields the typed unconfigured inference — never a panic, and never a
/// silently-enabled provider.
///
/// # Errors
/// The typed [`ProviderSetupError`] of the key resolution (an unset key
/// variable fails here, at factory-build time, not mid-turn).
pub fn compose_factory(
    selection: Option<&ProviderSelection>,
    transport: Arc<dyn Transport>,
) -> Result<Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>, ProviderSetupError> {
    match selection {
        Some(selection) => Ok(selection.resolve()?.compose_factory(transport)),
        None => Ok(compose::unconfigured_factory()),
    }
}
