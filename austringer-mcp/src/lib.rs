//! Model Context Protocol integration (plan todo 29): the rmcp-backed MCP
//! client (stdio + Streamable HTTP, roots confinement, non-stalling
//! timeouts), the austringer-as-MCP-server surface, the ACP agent-side core,
//! the transport pool, org allowlists, entry-hash trust, the file-based
//! multi-provider manager, and the schema disk cache.
//!
//! Trust model: approval binds to the blake3 fingerprint of the ENTIRE
//! server config entry ([`config::ServerEntry::fingerprint`]); identity
//! changes re-ask ([`trust`]). The org layer ([`allowlist`]) sits above
//! user trust and cannot be overridden. Nothing auto-trusts.

pub mod acp;
pub mod allowlist;
pub mod client;
pub mod config;
pub mod error;
pub mod http_backend;
pub mod manager;
pub mod mount;
pub mod pool;
pub mod provider_formats;
pub mod reference_server;
pub mod roots;
pub mod schema_cache;
pub mod secret;
pub mod server;
pub mod session_surface;
pub mod stderr_bridge;
pub mod trust;
