//! Child-job vocabulary: artifact/report/failure/job types, the executor
//! seam, and the launcher configuration. A job carries resolved knobs,
//! the cache-preserving fork, the write-set contract, the policy ladder,
//! and the bubbling sink — everything an isolated child session needs,
//! and none of the parent's wholesale context.

use std::path::PathBuf;
use std::sync::Arc;

use harness_safety::perm::grammar::Rule;

pub use crate::authorize::ChildCapabilities;
use crate::authorize::{EscalationSink, ParentEscalations, SpawnAuthzClass};
use crate::budget::Pacer;
use crate::fork::{ForkPlan, ToolSurface};
use crate::knobs::ChildKnobs;
use crate::role::Tier;
pub use crate::telemetry::{ChildFailureClass, ChildTelemetry};

/// One artifact a child produced (winner-diff application moves the
/// `source` bytes into the shared scope).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Artifact path (scope-relative).
    pub path: String,
    /// blake3 content hash.
    pub content_hash: String,
    /// The file in the child's worktree holding the bytes.
    pub source: PathBuf,
}

/// A child's success result.
#[derive(Debug, Clone, PartialEq)]
pub struct ChildReport {
    /// Child id.
    pub child_id: String,
    /// Summary (clamped into the 1-2k band).
    pub summary: String,
    /// Artifacts produced.
    pub artifacts: Vec<Artifact>,
    /// Telemetry.
    pub telemetry: ChildTelemetry,
    /// File touches published.
    pub touches: Vec<crate::write_set::FileTouch>,
}

/// A child's failure.
#[derive(Debug, Clone, PartialEq)]
pub struct ChildFailure {
    /// Child id.
    pub child_id: String,
    /// Failure class.
    pub class: ChildFailureClass,
    /// Detail.
    pub detail: String,
    /// Telemetry, if any was measured.
    pub telemetry: Option<ChildTelemetry>,
}

/// One subagent job: resolved knobs, the cache-preserving fork, the
/// write-set contract, the policy ladder, and the bubbling sink.
#[derive(Clone)]
pub struct ChildJob {
    /// Child id (`{launch}-child-{task}`; resumable).
    pub child_id: String,
    /// Role name.
    pub role: String,
    /// Capability tier.
    pub tier: Tier,
    /// Resolved model knobs.
    pub knobs: ChildKnobs,
    /// Task objective.
    pub objective: String,
    /// The fork (verbatim parent prompt + declarations, narrowed tools).
    pub fork: ForkPlan,
    /// Declared write set.
    pub writes: crate::write_set::WriteSet,
    /// Command-execution policy.
    pub policy: crate::authorize::CommandExecutionPolicy,
    /// Capability gates.
    pub capabilities: ChildCapabilities,
    /// Deny-only inherited rules.
    pub deny_rules: Vec<Rule>,
    /// Spawn-authorization class.
    pub authz: SpawnAuthzClass,
    /// Escalation sink (bubbling through the parent).
    pub escalation: Arc<dyn EscalationSink>,
    /// Summary budget.
    pub summary_budget: usize,
    /// Family-switch warning (loud context-loss surface).
    pub context_warning: Option<String>,
}

/// Executes one child in an isolated session.
#[async_trait::async_trait]
pub trait SubagentExecutor: Send + Sync {
    /// Runs the job to a report or a typed failure.
    async fn run(&self, job: ChildJob) -> Result<ChildReport, ChildFailure>;
}

impl std::fmt::Debug for ChildJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChildJob")
            .field("child_id", &self.child_id)
            .field("role", &self.role)
            .field("tier", &self.tier)
            .field("model", &self.knobs.model.model)
            .field("effort", &self.knobs.effort.to_string())
            .field("writes", &self.writes.globs)
            .field("authz", &self.authz)
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

/// Launcher configuration.
#[derive(Clone)]
pub struct LaunchConfig {
    /// Default child cap.
    pub budget_children: u32,
    /// Default concurrency cap.
    pub budget_parallel: u32,
    /// Default token budget.
    pub budget_tokens: u64,
    /// Spawn pacing (state moved into the launcher at construction).
    pub pacer: Pacer,
    /// Nesting cap.
    pub max_depth: u32,
    /// Child capability gates.
    pub child_capabilities: ChildCapabilities,
    /// /boost phase decoupling opt-in.
    pub phases: bool,
    /// Mission validators opt-out.
    pub validators: bool,
    /// Wall clock (ms).
    pub clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    /// Parent permission rules (deny-only inheritance source).
    pub parent_rules: Vec<Rule>,
    /// Escalation sink.
    pub escalation: Arc<dyn EscalationSink>,
    /// Parent system prompt (fork cache-preserving source).
    pub parent_prompt: String,
    /// Parent tool surface (declarations inherited verbatim).
    pub tool_surface: ToolSurface,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            budget_children: 8,
            budget_parallel: 2,
            budget_tokens: 64_000,
            pacer: Pacer::amp(),
            max_depth: crate::fork::DEFAULT_MAX_DEPTH,
            child_capabilities: ChildCapabilities::default(),
            phases: false,
            validators: true,
            clock: Arc::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0)
            }),
            parent_rules: Vec::new(),
            escalation: Arc::new(ParentEscalations::new()),
            parent_prompt: String::new(),
            tool_surface: ToolSurface::default(),
        }
    }
}
