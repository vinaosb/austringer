//! The tool registry: versioned contracts, per-session pinning, and the
//! dispatch path (deferred loading + caps). Base-schema rendering lives in
//! [`super::render`]; catalog search lives in [`super::hydrate`].

use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use super::hydrate;
use super::provider::{ProviderError, RegistryError, ToolDispatchError, ToolProvider, ToolResult};
use super::schema::{CORE_TOOLS, TASK_COMPLETED, TOOL_SEARCH, ToolSchema, core_schema};
use super::spill::{IoCap, SpillStore};
use super::version::{DeprecationNotice, Lifecycle, PinnedToolSet, ToolVersion, VersionedBehavior};

/// The tool registry. One instance owns the versioned contracts and bound
/// executors for its session scope; every render, search, and dispatch is
/// resolved against a [`PinnedToolSet`] so upgrading the registry
/// mid-session cannot churn an existing session's schemas.
pub struct ToolRegistry {
    contracts: BTreeMap<String, Vec<VersionedBehavior>>,
    executors: BTreeMap<String, Arc<dyn ToolProvider>>,
    io_cap: IoCap,
    spill: SpillStore,
}

impl ToolRegistry {
    /// Opens the registry for a session directory (creating the spill
    /// store) with the six core contracts seeded at version 1, Active.
    pub fn open(session_dir: &Path, io_cap: IoCap) -> io::Result<Self> {
        let spill = SpillStore::open(session_dir)?;
        let mut contracts = BTreeMap::new();
        for name in CORE_TOOLS {
            contracts.insert(
                name.to_owned(),
                vec![VersionedBehavior {
                    version: ToolVersion(1),
                    schema: core_schema(name),
                    lifecycle: Lifecycle::Active,
                    replaced_by: None,
                }],
            );
        }
        Ok(Self {
            contracts,
            executors: BTreeMap::new(),
            io_cap,
            spill,
        })
    }

    /// Registers an auxiliary tool: contract v1 (Active) plus its
    /// executor. The tool appears in the aux table and is discoverable by
    /// `tool_search`; it never enters the core schema set.
    pub fn register(&mut self, provider: Arc<dyn ToolProvider>) -> Result<(), RegistryError> {
        let schema = provider.schema();
        let name = schema.name.clone();
        if self.contracts.contains_key(&name) {
            return Err(RegistryError::DuplicateTool(name));
        }
        self.contracts.insert(
            name.clone(),
            vec![VersionedBehavior {
                version: ToolVersion(1),
                schema,
                lifecycle: Lifecycle::Active,
                replaced_by: None,
            }],
        );
        self.executors.insert(name, provider);
        Ok(())
    }

    /// Binds (or rebinds) an executor to an existing contract — the path
    /// the core tools take when their real executors arrive (todos 16-19).
    pub fn bind_executor(
        &mut self,
        tool: &str,
        provider: Arc<dyn ToolProvider>,
    ) -> Result<(), RegistryError> {
        if !self.contracts.contains_key(tool) {
            return Err(RegistryError::UnknownTool(tool.to_owned()));
        }
        self.executors.insert(tool.to_owned(), provider);
        Ok(())
    }

    /// Publishes the next behavior version for a tool (Active). Existing
    /// pinned sessions keep their pinned version; new pins pick this one
    /// up.
    pub fn publish_version(
        &mut self,
        tool: &str,
        schema: ToolSchema,
    ) -> Result<ToolVersion, RegistryError> {
        let behaviors = self
            .contracts
            .get_mut(tool)
            .ok_or_else(|| RegistryError::UnknownTool(tool.to_owned()))?;
        let next = behaviors
            .iter()
            .fold(ToolVersion(0), |highest, behavior| {
                highest.max(behavior.version)
            })
            .0
            + 1;
        behaviors.push(VersionedBehavior {
            version: ToolVersion(next),
            schema,
            lifecycle: Lifecycle::Active,
            replaced_by: None,
        });
        Ok(ToolVersion(next))
    }

    /// Transitions a version's lifecycle (deprecation metadata only — the
    /// serialized schema for pinned sessions is unchanged).
    pub fn set_lifecycle(
        &mut self,
        tool: &str,
        version: ToolVersion,
        lifecycle: Lifecycle,
        replaced_by: Option<ToolVersion>,
    ) -> Result<(), RegistryError> {
        let behaviors = self
            .contracts
            .get_mut(tool)
            .ok_or_else(|| RegistryError::UnknownTool(tool.to_owned()))?;
        let behavior = behaviors
            .iter_mut()
            .find(|behavior| behavior.version == version)
            .ok_or_else(|| RegistryError::UnknownVersion {
                tool: tool.to_owned(),
                version,
            })?;
        behavior.lifecycle = lifecycle;
        behavior.replaced_by = replaced_by;
        Ok(())
    }

    /// Pins the current version set: what a new session starts with.
    #[must_use]
    pub fn pin_current(&self) -> PinnedToolSet {
        let versions = self
            .contracts
            .iter()
            .map(|(name, behaviors)| {
                (
                    name.clone(),
                    behaviors.iter().fold(ToolVersion(0), |highest, behavior| {
                        highest.max(behavior.version)
                    }),
                )
            })
            .collect();
        PinnedToolSet::from_versions(versions)
    }

    /// The session's spill store (read-back by pointer).
    #[must_use]
    pub fn spill_store(&self) -> &SpillStore {
        &self.spill
    }

    pub(super) fn pinned_schema(&self, pinned: &PinnedToolSet, name: &str) -> Option<&ToolSchema> {
        let version = pinned.version_of(name)?;
        self.contracts
            .get(name)?
            .iter()
            .find(|behavior| behavior.version == version)
            .map(|behavior| &behavior.schema)
    }

    fn resolve(
        &self,
        pinned: &PinnedToolSet,
        tool: &str,
    ) -> Result<&VersionedBehavior, ToolDispatchError> {
        let version = pinned
            .version_of(tool)
            .ok_or_else(|| self.unknown_tool_error(pinned, tool))?;
        self.contracts
            .get(tool)
            .ok_or_else(|| self.unknown_tool_error(pinned, tool))?
            .iter()
            .find(|behavior| behavior.version == version)
            .ok_or_else(|| ToolDispatchError::PinnedVersionGone {
                tool: tool.to_owned(),
                version,
            })
    }

    fn unknown_tool_error(&self, pinned: &PinnedToolSet, name: &str) -> ToolDispatchError {
        let names: Vec<String> = pinned.tool_names().map(str::to_owned).collect();
        ToolDispatchError::UnknownTool {
            name: name.to_owned(),
            suggestions: hydrate::closest_names(&names, name),
            aux_table: self.aux_table(pinned),
        }
    }

    /// Deprecation notices for a pinned session (typed metadata; the
    /// serialized schema is never touched by them).
    #[must_use]
    pub fn deprecations(&self, pinned: &PinnedToolSet) -> Vec<DeprecationNotice> {
        pinned
            .tool_names()
            .filter_map(|name| {
                let behavior = self.resolve(pinned, name).ok()?;
                (behavior.lifecycle != Lifecycle::Active).then(|| DeprecationNotice {
                    tool: name.to_owned(),
                    version: behavior.version,
                    lifecycle: behavior.lifecycle,
                    replaced_by: behavior.replaced_by,
                })
            })
            .collect()
    }

    /// Dispatches one tool call for a pinned session: resolves the pinned
    /// contract, executes (or runs the internal deferred-loading search),
    /// applies the byte cap with transparent spill, and attaches
    /// deprecation metadata.
    pub async fn dispatch(
        &self,
        pinned: &PinnedToolSet,
        tool: &str,
        arguments: &Value,
    ) -> Result<ToolResult, ToolDispatchError> {
        if tool == TASK_COMPLETED {
            return Err(ToolDispatchError::ReservedTool {
                name: tool.to_owned(),
            });
        }
        let behavior = self.resolve(pinned, tool)?;
        let deprecation = (behavior.lifecycle != Lifecycle::Active).then(|| DeprecationNotice {
            tool: tool.to_owned(),
            version: behavior.version,
            lifecycle: behavior.lifecycle,
            replaced_by: behavior.replaced_by,
        });
        let raw = if tool == TOOL_SEARCH {
            self.run_tool_search(pinned, arguments)?
        } else {
            let provider =
                self.executors
                    .get(tool)
                    .ok_or_else(|| ToolDispatchError::NoExecutor {
                        name: tool.to_owned(),
                    })?;
            provider
                .execute(arguments)
                .await
                .map_err(|error| match error {
                    ProviderError::InvalidArguments { message } => {
                        ToolDispatchError::InvalidArguments {
                            tool: tool.to_owned(),
                            message,
                        }
                    }
                    ProviderError::Execution { message } => ToolDispatchError::ExecutionFailed {
                        tool: tool.to_owned(),
                        message,
                    },
                })?
        };
        let outcome =
            self.io_cap
                .apply(&raw, &self.spill)
                .map_err(|error| ToolDispatchError::SpillIo {
                    tool: tool.to_owned(),
                    message: error.to_string(),
                })?;
        Ok(ToolResult {
            observation: outcome.observation,
            spill: outcome.spill,
            deprecation,
        })
    }
}
