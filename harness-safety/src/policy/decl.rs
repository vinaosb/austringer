//! Sandbox policy declaration (plan todo 26): one typed policy, many
//! backends (Copilot MXC pattern). The policy is the single source of
//! truth; every backend compiles it into its OS-native enforcement.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::egress::EgressPolicy;
use crate::scrub::EnvScrub;

pub use crate::policy::glob::{DenyGlob, GlobError};

/// Sandbox tier (Zed tier details): which workspace surface the agent
/// may modify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxTier {
    /// Worktree roots auto-granted writable; VCS metadata protected.
    Worktree,
    /// No writable roots at all (read-only review mode).
    Readonly,
    /// Full process isolation via a container backend.
    Container,
}

/// The single declared sandbox policy consumed by every backend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SandboxPolicy {
    /// Enforcement tier for this session.
    pub tier: SandboxTier,
    /// Absolute paths the sandboxed process may write (and read).
    pub writable_roots: Vec<PathBuf>,
    /// Absolute paths the sandboxed process may read (no write).
    pub readable_roots: Vec<PathBuf>,
    /// Absolute paths denied write even when inside a writable root
    /// (`.git`, `.harness`, `.agents`, the settings file).
    pub protected_paths: Vec<PathBuf>,
    /// Globs that must resolve to guardable paths or enforcement refuses.
    pub deny_globs: Vec<DenyGlob>,
    /// Network egress allowlist (starts empty — no vendor escapes).
    pub egress: EgressPolicy,
    /// Shell-env scrubbing applied to every spawned child.
    pub env_scrub: EnvScrub,
}

impl SandboxPolicy {
    /// The standard worktree policy: root writable, VCS/harness metadata
    /// and the sandbox's own settings file protected, empty egress list.
    ///
    /// This is the settings-file write-protection seam: the sandbox always
    /// guards its own config.
    #[must_use]
    pub fn for_worktree(root: &Path, settings_path: &Path) -> Self {
        let mut protected: Vec<PathBuf> = [".git", ".harness", ".agents"]
            .iter()
            .map(|rel| root.join(rel))
            .collect();
        protected.push(settings_path.to_path_buf());
        Self {
            tier: SandboxTier::Worktree,
            writable_roots: vec![root.to_path_buf()],
            readable_roots: Vec::new(),
            protected_paths: protected,
            deny_globs: Vec::new(),
            egress: EgressPolicy::none(),
            env_scrub: EnvScrub::default(),
        }
    }

    /// Fail-closed validation: roots must be absolute and every deny-glob
    /// must resolve to at least one existing guarded path under a root.
    ///
    /// # Errors
    /// [`PolicyError`] naming the first violation; unresolvable globs are
    /// a refusal, never an allow-by-failure.
    pub fn validate(&self) -> Result<(), PolicyError> {
        for root in self.writable_roots.iter().chain(self.readable_roots.iter()) {
            if !root.is_absolute() {
                return Err(PolicyError::RelativeRoot { path: root.clone() });
            }
        }
        for glob in &self.deny_globs {
            let resolved = self
                .writable_roots
                .iter()
                .find_map(|root| glob.resolve(root).ok());
            if resolved.is_none() {
                return Err(PolicyError::Glob(GlobError::Unresolvable {
                    pattern: glob.pattern().to_string(),
                }));
            }
        }
        Ok(())
    }
}

/// Typed policy validation failure.
#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    /// A writable/readable root was not absolute.
    #[error("policy root must be absolute: {path:?}")]
    RelativeRoot {
        /// The offending path.
        path: PathBuf,
    },
    /// A deny-glob could not be resolved to an enforceable target set.
    #[error(transparent)]
    Glob(#[from] GlobError),
}

/// A profile fixed for the lifetime of one session (Grok session-pinned
/// profiles): serialized on checkpoint, restored on resume, and any
/// mid-session loosening is a typed rejection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PinnedProfile {
    /// Owning session id.
    pub session_id: String,
    /// The frozen policy.
    pub policy: SandboxPolicy,
}

impl PinnedProfile {
    /// Pin `policy` for `session_id`.
    #[must_use]
    pub fn pin(session_id: &str, policy: SandboxPolicy) -> Self {
        Self {
            session_id: session_id.to_string(),
            policy,
        }
    }
}

/// What the sandboxed process tried to do that the policy had to surface.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ViolationEvent {
    /// Classification of the attempt.
    pub kind: ViolationKind,
    /// Path or host the attempt touched.
    pub detail: String,
}

impl ViolationEvent {
    /// Build a violation event for the UI decision seam.
    #[must_use]
    pub fn new(kind: ViolationKind, detail: &str) -> Self {
        Self {
            kind,
            detail: detail.to_string(),
        }
    }

    /// The default decision when no interactive consumer is wired yet:
    /// deny. Fail-closed by construction.
    #[must_use]
    pub fn default_decision(&self) -> ViolationDecision {
        ViolationDecision::Deny
    }
}

/// Violation taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationKind {
    /// Write outside every writable root.
    WriteOutsideRoot,
    /// Write to a protected path.
    ProtectedPathWrite,
    /// Egress to a host outside the allowlist.
    EgressDenied,
    /// A deny-glob that could not be resolved.
    UnresolvedGlob,
}

/// The interactive `sandbox.violations` decision seam (allow-once /
/// always / deny). The UI wires this later; the sandbox never defaults
/// to allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationDecision {
    /// Permit this one attempt only.
    AllowOnce,
    /// Permit this and future identical attempts.
    AllowAlways,
    /// Deny (default).
    Deny,
}

/// Typed once-per-thread approval gate for unsandboxed escapes (Zed):
/// leaving the sandbox needs an explicit approval per thread id.
#[derive(Debug, Default)]
pub struct EscapeGate {
    approved: HashSet<u64>,
}

impl EscapeGate {
    /// A gate with no approvals.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// True when this thread id already holds an escape approval.
    #[must_use]
    pub fn may_escape_unsandboxed(&self, thread_id: u64) -> bool {
        self.approved.contains(&thread_id)
    }

    /// Record the once-per-thread approval.
    pub fn approve_thread(&mut self, thread_id: u64) {
        self.approved.insert(thread_id);
    }
}
