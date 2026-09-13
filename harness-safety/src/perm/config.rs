//! Engine configuration (plan todo 27): the one typed bundle the
//! unified permission engine runs on — grammar rules, command rules,
//! admin floor, effective profile, breaker, bypass stance, sandbox
//! policy, secret patterns.

use crate::perm::breaker::BreakerConfig;
use crate::perm::commands::CommandRules;
use crate::perm::floor::AdminFloor;
use crate::perm::grammar;
use crate::perm::profile::SecurityProfile;
use crate::policy::decl::SandboxPolicy;

/// Default high-risk path patterns (aligned with the todo-7 guard).
pub const DEFAULT_SECRET_PATTERNS: &[&str] = &[
    ".env*",
    "*id_rsa*",
    "*.pem",
    "*.p12",
    "*credentials*",
    ".npmrc",
    ".netrc",
    "*keychain*",
];

/// Bypass stance. Yolo escapes approval flows, NEVER the floors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BypassMode {
    /// Normal operation.
    #[default]
    Off,
    /// Auto-approve approval-asks; floors and ceilings survive.
    Yolo,
}

/// Full engine configuration.
#[derive(Clone)]
pub struct EngineConfig {
    /// Action(target) grammar rules.
    pub rules: Vec<grammar::Rule>,
    /// Command-prefix allow/deny lists.
    pub commands: CommandRules,
    /// Un-weakenable admin layer.
    pub admin: AdminFloor,
    /// Effective (already-intersected) security profile.
    pub profile: SecurityProfile,
    /// Circuit-breaker tuning.
    pub breaker: BreakerConfig,
    /// Bypass stance.
    pub bypass: BypassMode,
    /// T26 sandbox policy (protected paths, roots; egress is compiled
    /// from the rules above).
    pub sandbox: SandboxPolicy,
    /// Secret-path patterns for the read guard + audit masking.
    pub secret_patterns: Vec<String>,
}

impl EngineConfig {
    /// Defaults over `sandbox` (empty rules, unrestricted profile,
    /// disabled breaker, empty floor, T7 secret patterns).
    #[must_use]
    pub fn with_sandbox(sandbox: SandboxPolicy) -> Self {
        Self {
            rules: Vec::new(),
            commands: CommandRules::default(),
            admin: AdminFloor::empty(),
            profile: SecurityProfile::unrestricted(),
            breaker: BreakerConfig::disabled(),
            bypass: BypassMode::Off,
            secret_patterns: DEFAULT_SECRET_PATTERNS
                .iter()
                .map(ToString::to_string)
                .collect(),
            sandbox,
        }
    }
}

/// Engine construction failure.
#[derive(Debug, thiserror::Error)]
pub enum EngineBuildError {
    /// The rules could not compile into an enforceable egress policy.
    #[error(transparent)]
    EgressCompile(#[from] crate::perm::compile::CompileError),
}
