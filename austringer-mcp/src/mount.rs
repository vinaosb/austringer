//! Mounting MCP server tools into the T6 tool registry (todo 29): each
//! remote tool becomes a `mcp__<server>__<tool>` `ToolProvider`, mounted
//! only after the org allowlist passes AND the entry's fingerprint is an
//! approved trust decision (no auto-trust; the daemon records approvals
//! through the T7 ask flow). Tool descriptions are UNTRUSTED server text
//! and are bounded before entering any schema surface.

use std::sync::Arc;

use async_trait::async_trait;
use austringer_core::tools::{ProviderError, ToolProvider, ToolRegistry, ToolSchema};
use serde_json::Value;

use crate::allowlist::{OrgAllowlist, OrgVerdict};
use crate::client::{McpClient, ToolDescriptor};
use crate::config::ServerEntry;
use crate::trust::{TrustDecision, TrustStore};

/// Hard cap on a mounted tool's description (untrusted server text).
pub const MAX_DESCRIPTION_CHARS: usize = 200;

/// The mounted name for one remote tool (`mcp__<server>__<tool>`).
#[must_use]
pub fn mounted_tool_name(server: &str, tool: &str) -> String {
    format!("mcp__{server}__{tool}")
}

/// Why a mount refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MountError {
    /// The entry is not trusted under its CURRENT fingerprint (first use
    /// or the entry changed since approval — the TOCTOU re-ask).
    #[error("mcp server {server} is not trusted: {reason}")]
    Untrusted { server: String, reason: String },
    /// The org layer denied the entry; user config cannot override.
    #[error("mcp server {server} denied by org allowlist: {rule}")]
    OrgDenied { server: String, rule: String },
}

/// One mounted remote tool behind the T6 `ToolProvider` seam.
struct MountedProvider {
    server: String,
    tool: String,
    description: String,
    input_schema: Value,
    client: Arc<McpClient>,
}

#[async_trait]
impl ToolProvider for MountedProvider {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: mounted_tool_name(&self.server, &self.tool),
            description: self.description.clone(),
            parameters: self.input_schema.clone(),
        }
    }

    async fn execute(&self, arguments: &Value) -> Result<Vec<u8>, ProviderError> {
        self.client
            .call_tool(&self.tool, arguments)
            .await
            .map_err(|error| ProviderError::Execution {
                message: error.to_string(),
            })
    }
}

/// The shared gates every mount path re-runs: org first (unoverridable),
/// then the fingerprint trust check. No registration path skips them.
fn check_gates(
    entry: &ServerEntry,
    trust: &dyn TrustStore,
    org: &OrgAllowlist,
) -> Result<(), MountError> {
    if let OrgVerdict::Denied { rule } = org.check(entry) {
        return Err(MountError::OrgDenied {
            server: entry.name.clone(),
            rule,
        });
    }
    match entry.trust_decision(trust) {
        TrustDecision::Trusted => Ok(()),
        TrustDecision::NeedsApproval { .. } => Err(MountError::Untrusted {
            server: entry.name.clone(),
            reason: "first use requires approval".to_owned(),
        }),
        TrustDecision::ReApproval { .. } => Err(MountError::Untrusted {
            server: entry.name.clone(),
            reason:
                "server entry changed since approval (fingerprint mismatch) — re-approval required"
                    .to_owned(),
        }),
    }
}

/// Registers one cached descriptor as a `MountedProvider` over the
/// SHARED client.
fn register_mounted(
    registry: &mut ToolRegistry,
    entry: &ServerEntry,
    client: &Arc<McpClient>,
    tool: &ToolDescriptor,
) -> Result<(), MountError> {
    let provider = MountedProvider {
        server: entry.name.clone(),
        tool: tool.name.clone(),
        description: bound_description(&tool.description),
        input_schema: tool.input_schema.clone(),
        client: Arc::clone(client),
    };
    registry
        .register(Arc::new(provider))
        .map_err(|error| MountError::Untrusted {
            server: entry.name.clone(),
            reason: format!("registry rejected tool: {error}"),
        })
}

/// Mounts one connected server's tools: org check first (unoverridable),
/// then the fingerprint trust check (first use / changed entry refuse
/// typed — the T7 permission seam in the daemon performs the actual ask
/// and records approval against the SAME fingerprint), then `tools/list`
/// and registration. Returns the mounted descriptors.
pub async fn mount(
    registry: &mut ToolRegistry,
    entry: ServerEntry,
    client: McpClient,
    trust: &dyn TrustStore,
    org: &OrgAllowlist,
) -> Result<Vec<ToolDescriptor>, MountError> {
    check_gates(&entry, trust, org)?;
    let tools = client
        .list_tools()
        .await
        .map_err(|error| MountError::Untrusted {
            server: entry.name.clone(),
            reason: format!("tools/list failed: {error}"),
        })?;
    let client = Arc::new(client);
    for tool in &tools {
        register_mounted(registry, &entry, &client, tool)?;
    }
    Ok(tools)
}

/// Mounts one ALREADY-CONNECTED, ALREADY-LISTED server into a registry:
/// the same org + fingerprint gates re-run, then each cached descriptor
/// registers over the SHARED (pooled) client. This is the daemon
/// session-factory path (todo 12): connect and `tools/list` happened
/// once at daemon startup over the shared pool, so per-session mounting
/// is synchronous — no round-trip on the session-creation path. Returns
/// the number of tools registered.
///
/// # Errors
/// [`MountError`] — the gates refuse exactly like [`mount`].
pub fn mount_shared(
    registry: &mut ToolRegistry,
    entry: &ServerEntry,
    client: Arc<McpClient>,
    tools: &[ToolDescriptor],
    trust: &dyn TrustStore,
    org: &OrgAllowlist,
) -> Result<usize, MountError> {
    check_gates(entry, trust, org)?;
    let mut mounted = 0_usize;
    for tool in tools {
        register_mounted(registry, entry, &client, tool)?;
        mounted += 1;
    }
    Ok(mounted)
}

fn bound_description(untrusted: &str) -> String {
    if untrusted.chars().count() <= MAX_DESCRIPTION_CHARS {
        return untrusted.to_owned();
    }
    let mut bounded: String = untrusted.chars().take(MAX_DESCRIPTION_CHARS).collect();
    bounded.push_str("…(truncated)");
    bounded
}
