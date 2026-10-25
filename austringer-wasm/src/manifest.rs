//! The plugin manifest: the typed declaration every plugin embeds in its
//! memory (see [`crate::abi`] for where it lives). The manifest declares the
//! plugin's identity, its interface version, its capability demands, its
//! preopen-scope requirements, and the tools it registers.

use serde::Deserialize;

/// A capability a plugin may declare (Zed model, research §23). Declaring
/// one without an explicit host grant fails the load - there is no silent
/// escalation path. Interface 1 exposes no host functions behind these;
/// they are a declaration contract the host validates today and future
/// interfaces bind to real surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// Spawn host processes.
    ProcessExec,
    /// Download content from the network.
    Download,
    /// Install host packages.
    PackageInstall,
    /// Act as a media format adapter (interface 2, plan M6). The
    /// interface-2 host functions (`read_media` / `write_media`) are SPEC
    /// ONLY in v0.2 - declared and validated here, never linked - so a
    /// plugin importing them cannot instantiate today (the documented
    /// v0.2 boundary).
    MediaAdapter,
}

impl Capability {
    /// The manifest string for this capability.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProcessExec => "process-exec",
            Self::Download => "download",
            Self::PackageInstall => "package-install",
            Self::MediaAdapter => "media-adapter",
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Access mode of a preopen scope. The manifest states what the plugin
/// REQUIRES; the host grant states what it is WILLING to give. A read-write
/// requirement over a read-only grant fails the load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreopenMode {
    /// Read-only access.
    ReadOnly,
    /// Read and write access.
    ReadWrite,
}

/// A preopen scope the plugin wants visible under a logical name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreopenRequest {
    /// Logical scope name used in virtual guest paths (`name/relative`).
    pub name: String,
    /// Access the plugin requires.
    pub mode: PreopenMode,
}

/// One tool the plugin registers into the T6 registry.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDecl {
    /// Tool name (must be unique within the plugin and not collide with an
    /// already-registered tool - collisions fail registration typed).
    pub name: String,
    /// Human-facing description.
    pub description: String,
    /// JSON-schema-shaped object describing the arguments.
    pub input_schema: serde_json::Value,
}

/// The full manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginManifest {
    /// Plugin identity, used in errors and reload events.
    pub name: String,
    /// Interface version this plugin was built against.
    pub interface: u8,
    /// Capabilities the plugin demands.
    pub capabilities: Vec<Capability>,
    /// Preopen scopes the plugin requires.
    pub preopens: Vec<PreopenRequest>,
    /// Tools the plugin registers.
    pub tools: Vec<ToolDecl>,
}

impl PluginManifest {
    /// Manifest index of a tool by name (the ABI's tool id).
    pub fn tool_id(&self, name: &str) -> Option<u32> {
        self.tools
            .iter()
            .position(|t| t.name == name)
            .map(|i| i as u32)
    }
}

/// Everything wrong a manifest can be, typed.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// The manifest bytes are not UTF-8.
    #[error("manifest is not valid UTF-8")]
    NotUtf8,
    /// The manifest is not parseable JSON.
    #[error("manifest is not valid JSON: {detail}")]
    InvalidJson {
        /// Parser detail.
        detail: String,
    },
    /// The manifest region has no NUL terminator within its capacity.
    #[error("manifest exceeds its {capacity}-byte region")]
    TooLarge {
        /// Region capacity that was exhausted.
        capacity: usize,
    },
    /// A capability string outside the known enum.
    #[error(
        "unknown capability `{raw}`; known: process-exec, download, package-install, media-adapter"
    )]
    UnknownCapability {
        /// The unknown string.
        raw: String,
    },
    /// A mode string other than `ro`/`rw`.
    #[error("unknown preopen mode `{raw}`; expected `ro` or `rw`")]
    UnknownMode {
        /// The unknown string.
        raw: String,
    },
    /// Empty plugin name.
    #[error("plugin name must not be empty")]
    EmptyName,
    /// No tools declared.
    #[error("plugin declares no tools; a plugin must register at least one")]
    NoTools,
    /// Empty tool name.
    #[error("tool names must not be empty")]
    EmptyToolName,
    /// Duplicate tool name within the plugin.
    #[error("duplicate tool name `{tool}`")]
    DuplicateTool {
        /// The duplicated name.
        tool: String,
    },
    /// input_schema is not a JSON object.
    #[error("tool `{tool}` input_schema must be a JSON object")]
    SchemaNotObject {
        /// The offending tool.
        tool: String,
    },
    /// Empty preopen scope name.
    #[error("preopen scope names must not be empty")]
    EmptyPreopenName,
    /// Duplicate preopen scope.
    #[error("duplicate preopen scope `{name}`")]
    DuplicatePreopen {
        /// The duplicated scope.
        name: String,
    },
}

#[derive(Deserialize)]
struct RawPreopen {
    name: String,
    mode: String,
}

#[derive(Deserialize)]
struct RawTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Deserialize)]
struct RawManifest {
    name: String,
    interface: u8,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    preopens: Vec<RawPreopen>,
    tools: Vec<RawTool>,
}

/// Parse and validate manifest bytes (the NUL-terminated region payload).
pub fn parse(bytes: &[u8]) -> Result<PluginManifest, ManifestError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ManifestError::NotUtf8)?;
    let raw: RawManifest = serde_json::from_str(text).map_err(|e| ManifestError::InvalidJson {
        detail: e.to_string(),
    })?;

    let mut capabilities = Vec::new();
    for c in &raw.capabilities {
        capabilities.push(match c.as_str() {
            "process-exec" => Capability::ProcessExec,
            "download" => Capability::Download,
            "package-install" => Capability::PackageInstall,
            "media-adapter" => Capability::MediaAdapter,
            other => return Err(ManifestError::UnknownCapability { raw: other.into() }),
        });
    }

    let mut preopens = Vec::new();
    for p in &raw.preopens {
        if p.name.is_empty() {
            return Err(ManifestError::EmptyPreopenName);
        }
        if preopens.iter().any(|e: &PreopenRequest| e.name == p.name) {
            return Err(ManifestError::DuplicatePreopen {
                name: p.name.clone(),
            });
        }
        preopens.push(PreopenRequest {
            name: p.name.clone(),
            mode: match p.mode.as_str() {
                "ro" => PreopenMode::ReadOnly,
                "rw" => PreopenMode::ReadWrite,
                other => return Err(ManifestError::UnknownMode { raw: other.into() }),
            },
        });
    }

    if raw.name.is_empty() {
        return Err(ManifestError::EmptyName);
    }
    if raw.tools.is_empty() {
        return Err(ManifestError::NoTools);
    }
    let mut tools = Vec::new();
    for t in &raw.tools {
        if t.name.is_empty() {
            return Err(ManifestError::EmptyToolName);
        }
        if tools.iter().any(|e: &ToolDecl| e.name == t.name) {
            return Err(ManifestError::DuplicateTool {
                tool: t.name.clone(),
            });
        }
        if !t.input_schema.is_object() {
            return Err(ManifestError::SchemaNotObject {
                tool: t.name.clone(),
            });
        }
        tools.push(ToolDecl {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: t.input_schema.clone(),
        });
    }

    Ok(PluginManifest {
        name: raw.name,
        interface: raw.interface,
        capabilities,
        preopens,
        tools,
    })
}
