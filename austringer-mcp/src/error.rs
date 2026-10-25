//! The typed error vocabulary for every MCP surface in this crate: client
//! transport failures, trust refusals, org-allowlist denials, roots
//! confinement, pool guardrails, and manager scan diagnostics. Nothing in
//! this crate panics or unwraps; every failure path returns one of these.

/// Failures from the MCP client surface (connect, list, call, read).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum McpClientError {
    /// The server did not answer within the per-server connect timeout.
    #[error("mcp server connect to {server} timed out after {timeout_ms} ms")]
    ConnectTimeout {
        /// The server entry name.
        server: String,
        /// The elapsed budget in milliseconds.
        timeout_ms: u64,
    },
    /// An in-flight request exceeded its call deadline; the server is
    /// considered unresponsive. The connection is torn down; the next
    /// acquire reconnects (never a hung await).
    #[error("mcp server {server} is unresponsive: no reply within {timeout_ms} ms")]
    Unresponsive {
        /// The server entry name.
        server: String,
        /// The per-call deadline in milliseconds.
        timeout_ms: u64,
    },
    /// Too many concurrent in-flight operations for one server (bounded
    /// concurrency guard); retry with fewer.
    #[error("mcp server {server} in-flight limit reached ({limit})")]
    InFlightLimit {
        /// The server entry name.
        server: String,
        /// The bounded concurrency ceiling.
        limit: usize,
    },
    /// The tool arguments tried to touch a path outside every declared
    /// root (client-side roots confinement).
    #[error("tool {tool} arguments escape all declared roots: {path}")]
    RootsEscape {
        /// The tool that was about to be called.
        tool: String,
        /// The offending path, redacted to its textual form.
        path: String,
    },
    /// The trust seam refused this server (first use or changed entry).
    #[error("mcp server {server} is not trusted: {reason}")]
    Untrusted {
        /// The server entry name.
        server: String,
        /// Why trust was refused (approval needed, or entry hash changed).
        reason: String,
    },
    /// The org-managed allowlist denied this server; user config cannot
    /// override it.
    #[error("mcp server {server} denied by org allowlist: {rule}")]
    OrgDenied {
        /// The server entry name.
        server: String,
        /// The matched org rule.
        rule: String,
    },
    /// A protocol-level failure reported by the transport/service layer.
    #[error("mcp server {server} protocol failure: {message}")]
    Protocol {
        /// The server entry name.
        server: String,
        /// The underlying failure, rendered.
        message: String,
    },
}

/// Failures from the austringer-as-MCP-server surface.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum McpServerError {
    /// The session-control seam rejected the request.
    #[error("session surface failure: {0}")]
    Session(String),
}
