//! The provider seam: executors behind tool contracts, dispatch outcomes,
//! and the typed error vocabulary.

use async_trait::async_trait;
use serde_json::Value;

use super::schema::ToolSchema;
use super::spill::SpillPointer;
use super::version::DeprecationNotice;
use super::version::ToolVersion;

/// Why a provider rejected or failed a call.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    /// The arguments did not satisfy the tool's contract.
    #[error("invalid arguments: {message}")]
    InvalidArguments {
        /// What was wrong with the arguments.
        message: String,
    },
    /// The tool ran and failed.
    #[error("execution failed: {message}")]
    Execution {
        /// The failure cause.
        message: String,
    },
}

/// An executor behind a tool contract. The registry dispatches into this;
/// the real core-tool executors arrive in later todos (16, 18, 19).
#[async_trait]
pub trait ToolProvider: Send + Sync {
    /// The provider's own contract (used when registering a whole tool).
    fn schema(&self) -> ToolSchema;

    /// Execute the tool, returning the raw result bytes (capped by the
    /// registry before anything enters context).
    async fn execute(&self, arguments: &Value) -> Result<Vec<u8>, ProviderError>;
}

/// Why a registry dispatch failed. Every miss is typed and panic-free;
/// `UnknownTool` carries closest-match suggestions plus the aux table so
/// the caller can steer the model toward `tool_search`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToolDispatchError {
    /// No such tool in the session's pinned catalog.
    #[error(
        "unknown tool {name:?}; closest matches: {suggestions:?}; deferred tool catalog:\n{aux_table}"
    )]
    UnknownTool {
        /// The requested (unknown) name.
        name: String,
        /// Closest catalog names, bounded and sorted by distance.
        suggestions: Vec<String>,
        /// The aux table for steering the model to `tool_search`.
        aux_table: String,
    },
    /// The reserved completion tool was dispatched through the registry.
    #[error("tool {name:?} is loop-enforced and never registry-executed")]
    ReservedTool {
        /// The reserved tool's name.
        name: String,
    },
    /// The contract exists but no executor is bound (schema-only tool).
    #[error("no executor registered for tool {name:?}")]
    NoExecutor {
        /// The tool's name.
        name: String,
    },
    /// The arguments did not satisfy the tool's contract.
    #[error("invalid arguments for tool {tool:?}: {message}")]
    InvalidArguments {
        /// The tool's name.
        tool: String,
        /// What was wrong with the arguments.
        message: String,
    },
    /// The bound executor ran and failed.
    #[error("tool {tool:?} failed: {message}")]
    ExecutionFailed {
        /// The tool's name.
        tool: String,
        /// The failure cause.
        message: String,
    },
    /// The session's pinned version no longer exists in the registry.
    #[error("pinned version {version} of tool {tool:?} is gone from the registry")]
    PinnedVersionGone {
        /// The tool's name.
        tool: String,
        /// The pinned version.
        version: ToolVersion,
    },
    /// The spill store rejected the oversized result.
    #[error("spilling result of tool {tool:?} failed: {message}")]
    SpillIo {
        /// The tool's name.
        tool: String,
        /// The I/O failure cause.
        message: String,
    },
}

/// Why a registry mutation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// A tool with this name already has a contract.
    #[error("tool {0:?} is already registered")]
    DuplicateTool(String),
    /// No contract exists for this tool.
    #[error("unknown tool {0:?}")]
    UnknownTool(String),
    /// The contract exists but not this version of it.
    #[error("tool {tool:?} has no version {version}")]
    UnknownVersion {
        /// The tool's name.
        tool: String,
        /// The missing version.
        version: ToolVersion,
    },
}

/// One dispatched tool result: the model-facing observation plus spill and
/// deprecation metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    /// What the model sees: full content when inline, pointer + first-N
    /// bytes when spilled.
    pub observation: String,
    /// Set when the result spilled to disk.
    pub spill: Option<SpillPointer>,
    /// Set when the dispatched version is not [`super::Lifecycle::Active`].
    pub deprecation: Option<DeprecationNotice>,
}

/// A typed miss from the deferred-loading search: the reason, the closest
/// catalog matches, and the aux table for browsing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}\nclosest matches: {suggestions:?}\ndeferred tool catalog:\n{aux_table}")]
pub struct ToolSearchError {
    /// The query that missed.
    pub query: String,
    /// Why it missed (empty query, or no match).
    pub reason: String,
    /// Closest catalog names, bounded and sorted by distance.
    pub suggestions: Vec<String>,
    /// The aux table (name + one-line description of every non-core tool).
    pub aux_table: String,
}
