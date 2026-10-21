//! Agent Client Protocol (ACP) agent-side surface (todo 29 amendment).
//! Wire shapes per ACP v1 (`agentclientprotocol/agent-client-protocol`,
//! schema v1); see [`agent`] for the plan-shorthand → spec name mapping
//! and the v2 disclosure.

pub mod agent;
pub mod channel;
pub mod types;

pub use agent::{AcpAgent, AcpEngine, AcpError, AcpPrompt, AcpWire, IdeChannel};
pub use types::{
    AcpContentBlock, PermissionOption, PermissionOptionKind, PermissionOutcome,
    PROTOCOL_VERSION, SessionUpdate, StopReason, ToolCallContent,
};
