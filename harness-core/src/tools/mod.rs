//! The tool registry (plan todo 6): the invariant six-tool base schema,
//! deferred loading via `tool_search`, versioned tool behaviors with
//! per-session pinning, and tool-I/O byte caps with transparent disk
//! spill.
//!
//! # Invariant base schema
//!
//! The registry serializes exactly [`CORE_TOOLS`] (six tools, canonical
//! order) into the base schema — the tool section of the system prefix —
//! plus a markdown aux table (name + one-line description) of every other
//! pinned tool. The core set never varies with autonomy mode, active
//! skills, or deferred-loaded tools: it is a pure function of the pinned
//! version set, byte-identical across turns (see
//! [`ToolRegistry::render_base_schema`]).
//!
//! # Deferred loading
//!
//! Every non-core tool is schema-less in the prompt. The model discovers
//! them through `tool_search(query)`: the full schema of each match
//! hydrates INTO the observation of the search call itself
//! ([`hydrate_schemas`]) — the system prefix is never touched.
//!
//! # Version pinning
//!
//! Each tool owns a stack of [`VersionedBehavior`]s with an
//! Active/Deprecated/RemovalCandidate lifecycle (Grok Build pattern): a
//! session pins the version set it started with
//! ([`ToolRegistry::pin_current`]), the registry resolves per session, so
//! upgrades mid-history cannot churn schemas for existing sessions.
//! Deprecation surfaces typed metadata ([`DeprecationNotice`]) without
//! changing the serialized shape.
//!
//! # Byte caps and spill
//!
//! Oversized tool results spill to a content-addressed store
//! (`<session>/spill/<blake3-hex>`); the context receives a pointer plus
//! the first N bytes ([`IoCap`], default 4 KiB) and the full content is
//! recoverable by pointer ([`SpillStore::read`]).

mod hydrate;
mod provider;
mod registry;
mod render;
mod schema;
mod spill;
mod version;

pub use provider::{
    ProviderError, RegistryError, ToolDispatchError, ToolProvider, ToolResult, ToolSearchError,
};
pub use registry::ToolRegistry;
pub use schema::{CORE_TOOLS, TASK_COMPLETED, TOOL_SEARCH, ToolSchema};
pub use spill::{IoCap, SpillOutcome, SpillPointer, SpillStore};
pub use version::{DeprecationNotice, Lifecycle, PinnedToolSet, ToolVersion, VersionedBehavior};
