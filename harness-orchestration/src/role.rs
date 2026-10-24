//! Subagent role definitions (the Devin SS22 subagent-definitions fold):
//! named agents with a capability tier, a default model bracket, a tool
//! allowlist, a summary budget, and a command-execution policy. Custom
//! roles extend the builtin three-role split (Droid): explorer = read-only
//! light tier, worker = read-write medium tier, plus our deep tier — and
//! the registry also carries the curated cheap-pool read-only agents
//! (todo 14: explore, librarian).

use crate::authorize::CommandExecutionPolicy;

/// The builtin capability tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// Read-only light tier (explorer).
    Explorer,
    /// Read-write medium tier (worker).
    Worker,
    /// Read-write heavy tier (deep).
    Deep,
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl Tier {
    /// Stable wire name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Explorer => "explorer",
            Self::Worker => "worker",
            Self::Deep => "deep",
        }
    }
}

/// Parses a tier name.
pub fn parse_tier(value: &str) -> Option<Tier> {
    match value {
        "explorer" => Some(Tier::Explorer),
        "worker" => Some(Tier::Worker),
        "deep" => Some(Tier::Deep),
        _ => None,
    }
}

/// One subagent definition.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleDef {
    /// Role name.
    pub name: String,
    /// Capability tier.
    pub tier: Tier,
    /// Default model bracket (id with optional bracket knobs).
    pub model: Option<String>,
    /// Execution tool allowlist (fork narrowing).
    pub tools: Vec<String>,
    /// Summary budget in bytes (the 1-2k band).
    pub summary_budget: usize,
    /// Command-execution policy for children in this role.
    pub policy: CommandExecutionPolicy,
}

impl RoleDef {
    /// The builtin definition for a tier.
    pub fn builtin(tier: Tier) -> Self {
        match tier {
            Tier::Explorer => Self {
                name: "explorer".into(),
                tier,
                model: Some("claude-sonnet-4-6".into()),
                tools: ["read", "grep", "glob"].map(String::from).to_vec(),
                summary_budget: 1024,
                policy: CommandExecutionPolicy::Off,
            },
            Tier::Worker => Self {
                name: "worker".into(),
                tier,
                model: Some("gpt-5-5".into()),
                tools: ["read", "edit", "write", "grep", "glob", "test"]
                    .map(String::from)
                    .to_vec(),
                summary_budget: 2048,
                policy: CommandExecutionPolicy::Auto,
            },
            Tier::Deep => Self {
                name: "deep".into(),
                tier,
                model: Some("claude-opus-4-6".into()),
                tools: ["read", "edit", "write", "grep", "glob", "test", "run"]
                    .map(String::from)
                    .to_vec(),
                summary_budget: 2048,
                policy: CommandExecutionPolicy::Sandbox,
            },
        }
    }

    /// The curated cheap-pool read-only agent by reserved name (todo
    /// 14), if `name` is one of [`CURATED_AGENTS`]. Tool names are the
    /// daemon registry surface the agent's child sessions serve:
    /// read/query/search only — never `edit_lines`,
    /// `execute_command`, or the delegation `task` tool.
    pub fn curated(name: &str) -> Option<Self> {
        let tools: &[&str] = match name {
            "explore" => &["view_window", "lsp_query", "search_sessions", "tool_search"],
            "librarian" => &["view_window", "search_sessions", "tool_search"],
            _ => return None,
        };
        Some(Self {
            name: name.to_owned(),
            tier: Tier::Explorer,
            model: Some("claude-haiku-5-5".into()),
            tools: tools.iter().map(|tool| (*tool).to_owned()).collect(),
            summary_budget: 1024,
            policy: CommandExecutionPolicy::Off,
        })
    }
}

/// The curated cheap-pool read-only agents (todo 14): reserved
/// delegation names riding the task-tool path. Their models are
/// pinned by the routing table's utility-role cheap-pool validation
/// (fail-closed at build); their tools by the read-only allowlist.
pub const CURATED_AGENTS: &[&str] = &["explore", "librarian"];

/// Duplicate role registration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("duplicate role `{name}`")]
pub struct DuplicateRole {
    /// The duplicated name.
    pub name: String,
}

impl RoleRegistry {
    /// Merges spec-declared roles over a base registry (duplicate names
    /// are typed errors).
    pub fn with_declarations(
        base: &RoleRegistry,
        decls: &[crate::spec::RoleDecl],
    ) -> Result<RoleRegistry, crate::resolve::ResolveError> {
        let mut merged = base.clone();
        for decl in decls {
            let tier = decl.tier.unwrap_or(Tier::Worker);
            let builtin = RoleDef::builtin(tier);
            merged
                .register(RoleDef {
                    name: decl.name.clone(),
                    tier,
                    model: decl.model.clone(),
                    tools: if decl.tools.is_empty() {
                        builtin.tools
                    } else {
                        decl.tools.clone()
                    },
                    summary_budget: decl.summary.unwrap_or(builtin.summary_budget),
                    policy: builtin.policy,
                })
                .map_err(|_| crate::resolve::ResolveError::DuplicateRole(decl.name.clone()))?;
        }
        Ok(merged)
    }
}
/// The role registry: builtins plus spec-declared custom roles.
#[derive(Debug, Clone)]
pub struct RoleRegistry {
    roles: std::collections::BTreeMap<String, RoleDef>,
}

impl Default for RoleRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

impl RoleRegistry {
    /// The builtin three-role split plus the curated pair (todo 14).
    pub fn builtin() -> Self {
        let mut roles = std::collections::BTreeMap::new();
        for tier in [Tier::Explorer, Tier::Worker, Tier::Deep] {
            let def = RoleDef::builtin(tier);
            roles.insert(def.name.clone(), def);
        }
        for def in CURATED_AGENTS
            .iter()
            .filter_map(|name| RoleDef::curated(name))
        {
            roles.insert(def.name.clone(), def);
        }
        Self { roles }
    }

    /// Registers a custom role (duplicate names are typed errors).
    pub fn register(&mut self, def: RoleDef) -> Result<(), DuplicateRole> {
        if self.roles.contains_key(&def.name) {
            return Err(DuplicateRole {
                name: def.name.clone(),
            });
        }
        self.roles.insert(def.name.clone(), def);
        Ok(())
    }

    /// Looks a role up by name.
    pub fn get(&self, name: &str) -> Option<&RoleDef> {
        self.roles.get(name)
    }

    /// All registered role names.
    pub fn names(&self) -> Vec<String> {
        self.roles.keys().cloned().collect()
    }
}
