//! Cache-preserving forks (Qwen SS18) and parallelism normalization
//! (SS33): a fork inherits the parent's EXACT system prompt and tool
//! declarations verbatim — protecting the shared provider prefix — while
//! execution is narrowed through an allowlist; tool results batch into
//! ONE message; mutating tools are flagged serial. Also home of the
//! depth guard (flat depth-1 default, opt-in nesting, Zed
//! MAX_SUBAGENT_DEPTH).

/// One tool declaration exactly as the model sees it (never rewritten by
/// the fork machinery).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDecl {
    /// Tool name.
    pub name: String,
    /// The model-visible declaration text.
    pub declaration: String,
}

impl ToolDecl {
    /// Builds a declaration.
    pub fn new(name: &str, declaration: &str) -> Self {
        Self {
            name: name.to_string(),
            declaration: declaration.to_string(),
        }
    }
}

/// The parent's tool surface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolSurface {
    /// Declarations the model sees (verbatim through forks).
    pub declarations: Vec<ToolDecl>,
    /// Whether the batch meta-tool is declared.
    pub batch_tool: bool,
    /// Mutating tools that must not run in parallel.
    pub serial_tools: Vec<String>,
}

impl ToolSurface {
    /// Declaration names, in order.
    pub fn declaration_names(&self) -> Vec<String> {
        self.declarations.iter().map(|d| d.name.clone()).collect()
    }

    /// Whether a tool is flagged serial (mutating).
    pub fn is_serial(&self, tool: &str) -> bool {
        self.serial_tools.iter().any(|s| s == tool)
    }
}

/// Flat depth-1 default for subagent nesting.
pub const DEFAULT_MAX_DEPTH: u32 = 1;

/// Nesting beyond the cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("subagent depth {depth} exceeds max {max}")]
pub struct DepthExceeded {
    /// Requested depth.
    pub depth: u32,
    /// The cap.
    pub max: u32,
}

/// Enforces the nesting cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepthGuard {
    /// The cap.
    pub max: u32,
}

impl Default for DepthGuard {
    fn default() -> Self {
        Self {
            max: DEFAULT_MAX_DEPTH,
        }
    }
}

impl DepthGuard {
    /// A guard with a custom cap (opt-in nesting).
    pub fn new(max: u32) -> Self {
        Self { max }
    }

    /// Checks one child's depth.
    pub fn check(&self, depth: u32) -> Result<(), DepthExceeded> {
        if depth > self.max {
            return Err(DepthExceeded {
                depth,
                max: self.max,
            });
        }
        Ok(())
    }
}

/// What a forked child inherits: the parent's exact prompt, the untouched
/// declarations, and the execution allowlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkPlan {
    /// The parent's system prompt, verbatim.
    pub system_prompt: String,
    /// The surface (declarations unchanged).
    pub surface: ToolSurface,
    /// The fork_tools execution allowlist.
    pub fork_tools: Vec<String>,
}

impl ForkPlan {
    /// Whether the forked child may execute a tool at runtime.
    pub fn execution_allowed(&self, tool: &str) -> bool {
        self.fork_tools.iter().any(|t| t == tool)
    }
}

/// Builds a cache-preserving fork: verbatim prompt + verbatim
/// declarations, narrowed execution.
pub fn fork_child(parent_prompt: &str, parent_surface: &ToolSurface, allow: &[String]) -> ForkPlan {
    ForkPlan {
        system_prompt: parent_prompt.to_string(),
        surface: parent_surface.clone(),
        fork_tools: allow.to_vec(),
    }
}

/// One finished tool call.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ToolResultRecord {
    /// Call id.
    pub call_id: String,
    /// Tool name.
    pub tool: String,
    /// Whether the call succeeded.
    pub ok: bool,
    /// Result size.
    pub bytes: usize,
}

/// Tool results batched into ONE message (parallelism normalization: one
/// result per message silently teaches models to serialize).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchedResults {
    /// The batched records.
    pub results: Vec<ToolResultRecord>,
}

impl BatchedResults {
    /// Renders the batch as a single line — one message by construction.
    pub fn single_message(&self) -> String {
        serde_json::to_string(&self.results).unwrap_or_else(|_| "[]".to_string())
    }
}

/// Batches results.
pub fn batch(results: Vec<ToolResultRecord>) -> BatchedResults {
    BatchedResults { results }
}
