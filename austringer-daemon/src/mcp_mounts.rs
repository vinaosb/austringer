//! Daemon-side MCP session mounting (todo 12, ecosystem alignment):
//! servers declared ENABLED — `trusted_autostart: true` in the scanned
//! manifest, the only writers of which are explicit user edits (pack
//! installs land the field `false` whatever they claim) — and approved
//! under their full-entry fingerprint mount their tools into every
//! session's tool registry as `mcp__<server>__<tool>` providers.
//!
//! Lifecycle: the mcp crate's [`TransportPool`] is the sharing
//! primitive (connections keyed by name+fingerprint, shared across
//! sessions), so the pool is daemon-wide. [`McpMounts::start`] runs the
//! async half ONCE at daemon startup — connect + `tools/list` per
//! enabled server — and holds the pool handle for the daemon's lifetime
//! (`AutostartDecision::StartNow` means start now). Sessions then mount
//! SYNCHRONOUSLY from the cached descriptors inside the executor
//! factory (the factory closure cannot await); each session's registry
//! holds Arc clones of the shared client, so host eviction drops that
//! session's providers while the daemon-owned connection keeps serving
//! other sessions. Everything — pool entry included — tears down with
//! the daemon.
//!
//! Refusals are typed and loud but never fatal: an org-denied,
//! unapproved, or unreachable server stays a DISABLED declaration with
//! zero mounted tools, and the daemon serves on.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use austringer_core::tools::ToolRegistry;
use austringer_mcp::allowlist::{OrgAllowlist, OrgVerdict};
use austringer_mcp::client::ToolDescriptor;
use austringer_mcp::config::ServerEntry;
use austringer_mcp::manager::{AutostartDecision, ScanReport, scan_directories};
use austringer_mcp::mount::mount_shared;
use austringer_mcp::pool::{PoolConfig, PoolHandle, TransportPool};
use austringer_mcp::trust::{TrustDecision, TrustStore};

/// The daemon pool budget: one live connection per enabled server, so
/// a ceiling of 16 covers every realistic mount set with headroom; the
/// warn threshold leaves four slots of margin and the hysteresis gap
/// keeps a full pool from flapping between allow and reject.
#[must_use]
pub fn default_pool_config() -> PoolConfig {
    PoolConfig {
        max_live: 16,
        warn_at: 12,
        hysteresis_gap: 4,
        idle_drain_after: Duration::from_secs(60),
    }
}

/// One started server: its entry, the daemon-held pool handle, and the
/// startup-fetched tool descriptors the session mounts register from.
struct Started {
    entry: ServerEntry,
    handle: PoolHandle,
    tools: Vec<ToolDescriptor>,
}

/// The daemon-wide MCP mount surface: started once in an async context
/// (binary startup, test setup), bound into
/// [`crate::tools::CoreExecutors`] before any session opens.
pub struct McpMounts {
    pool: Arc<TransportPool>,
    trust: Arc<dyn TrustStore>,
    org: OrgAllowlist,
    servers: Vec<Started>,
}

impl McpMounts {
    /// Scans the daemon data dir, connects every ENABLED entry that
    /// passes the org and fingerprint gates, and caches its tool
    /// descriptors. Disabled declarations (every pack install) are
    /// skipped without a word — DISABLED is their contract. Gate and
    /// connect failures log typed and move on: the daemon serves on
    /// with fewer mounted tools (fail-closed per server, never for
    /// availability).
    pub async fn start(
        data_dir: &Path,
        trust: Arc<dyn TrustStore>,
        org: OrgAllowlist,
        pool: Arc<TransportPool>,
    ) -> Self {
        let scan = scan_directories(&[data_dir.to_path_buf()]);
        let mut servers = Vec::new();
        for installation in scan.installations {
            let entry = installation.entry;
            if ScanReport::autostart_for(&entry) != AutostartDecision::StartNow {
                continue;
            }
            if let OrgVerdict::Denied { rule } = org.check(&entry) {
                eprintln!(
                    "austringer-daemon: mcp server {} stays disabled (org allowlist): {}",
                    entry.name, rule
                );
                continue;
            }
            match entry.trust_decision(trust.as_ref()) {
                TrustDecision::Trusted => {}
                decision => {
                    eprintln!(
                        "austringer-daemon: mcp server {} stays disabled (trust gate): {:?}",
                        entry.name, decision
                    );
                    continue;
                }
            }
            let handle = match pool.acquire(&entry).await {
                Ok(handle) => handle,
                Err(error) => {
                    eprintln!(
                        "austringer-daemon: mcp server {} stays disabled (connect): {error}",
                        entry.name
                    );
                    continue;
                }
            };
            match handle.client().list_tools().await {
                Ok(tools) => servers.push(Started {
                    entry,
                    handle,
                    tools,
                }),
                Err(error) => {
                    eprintln!(
                        "austringer-daemon: mcp server {} stays disabled (tools/list): {error}",
                        entry.name
                    );
                }
            }
        }
        Self {
            pool,
            trust,
            org,
            servers,
        }
    }

    /// Mounts every started server's cached tools into one session's
    /// registry — the session-factory call, synchronous by design (the
    /// async connect/list happened once at startup). The gates re-run
    /// per registration ([`austringer_mcp::mount::mount_shared`]
    /// enforces them); a per-server refusal logs and leaves that
    /// server's tools out of the session.
    pub fn mount_into(&self, registry: &mut ToolRegistry) {
        for server in &self.servers {
            match mount_shared(
                registry,
                &server.entry,
                server.handle.client_arc(),
                &server.tools,
                self.trust.as_ref(),
                &self.org,
            ) {
                Ok(count) => tracing::info!(
                    server = %server.entry.name,
                    tools = count,
                    "mcp tools mounted into session registry"
                ),
                Err(error) => {
                    tracing::warn!("mcp session mount refused: {error}");
                }
            }
        }
    }

    /// Live pooled-connection count (observability: budgets, tests).
    #[must_use]
    pub fn pool_live_count(&self) -> usize {
        self.pool.live_count()
    }

    /// The mounted server names (observability; tests).
    #[must_use]
    pub fn server_names(&self) -> Vec<&str> {
        self.servers.iter().map(|s| s.entry.name.as_str()).collect()
    }
}
