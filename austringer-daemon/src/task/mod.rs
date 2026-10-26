//! The `task(category)` delegation surface (todo 13, ecosystem
//! alignment): the model-facing tool and its spawn path. Children are
//! REAL child sessions on the task-8 launch pattern — a fresh
//! [`SessionHost`] per child in the hidden `-child-` namespace, one
//! engine turn over the prompt, Child* team events on the parent hub —
//! with the child's turns dispatched through the todo-9
//! [`crate::routing::SessionRouter`] under
//! [`RouteRequest::Category`], so the category's model + effort (and
//! the ask-first fallback contract) follow the child exactly as roles
//! follow a session. The curated cheap-pool read-only agents (todo
//! 14: `explore`, `librarian`) ride the same path as RESERVED
//! category names — they dispatch under their
//! [`austringer_intel::routing::Role`] utility routes (cheap-pool
//! validated fail-closed at table build; user categories cannot
//! shadow them) and run under [`ReadOnlyApproval`] narrowing.
//!
//! Storm guards (T10/A6, model-profiles.md: hard caps plus explicit
//! escalation criteria): the fan-out cap (default 4, escalation is a
//! per-call explicit flag, the absolute ceiling stays hard) and the
//! token budget aggregate PER PARENT SESSION across task() calls —
//! [`DelegationBudget`] is the nameable aggregate todo 15 surfaces
//! (BudgetGuard stays per-launch and is not a cost guard). Every spawn
//! carries FRESH grant state: the child builds its own host, mediator,
//! approval chain, and registry — nothing session-scoped crosses the
//! spawn boundary. The fork.rs DepthGuard (default depth 1) is enforced
//! on the task-tool path: a session whose id marks it as a child cannot
//! spawn grandchildren.

mod budget;
mod child;
mod readonly;
mod runner;
mod tool;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use austringer_orchestration::budget::{ABSOLUTE_MAX_TOKENS, BudgetBreach};
use austringer_orchestration::fork::DepthExceeded;
use austringer_orchestration::journal::is_child_session;

use crate::attach::SessionHost;
use crate::engine::HostResolver;
use crate::launch::LaunchServices;
use crate::routing::RoutingIntake;

pub use budget::{
    DEFAULT_FAN_OUT_CAP, DelegationBudget, DelegationSnapshot, ESCALATED_FAN_OUT_CAP,
    FanOutExceeded,
};
pub use child::{ChildObserved, await_child_turn};
pub use readonly::ReadOnlyApproval;
pub use tool::TaskTool;

/// Default per-parent token budget across task() calls: the absolute
/// per-launch ceiling made an explicit per-session aggregate.
pub const DEFAULT_TASK_TOKEN_BUDGET: u64 = ABSOLUTE_MAX_TOKENS;
/// Wall-clock bound on one child's engine turn. Wider than the
/// launch-path timeout: a mid-task fallback ask may time out twice
/// (MAX_UNACKNOWLEDGED_ASKS retries the same model) before the turn
/// surfaces.
const CHILD_TURN_TIMEOUT: Duration = Duration::from_secs(120);
/// Child-session hub capacities (a delegation turn is a handful of
/// frames plus ask notifications; headroom for the clamp test's long
/// transcripts).
const CHILD_HUB_CAPACITY: usize = 64;
/// The delegation role children run under — the source of the summary
/// budget the tool result clamps to (worker: read-write medium tier,
/// 2048 bytes).
const TASK_ROLE: &str = "worker";

/// The delegation role's summary budget (the clamp ceiling for every
/// task-tool result).
fn summary_budget() -> usize {
    austringer_orchestration::role::RoleRegistry::builtin()
        .get(TASK_ROLE)
        .map_or(austringer_orchestration::telemetry::SUMMARY_MAX, |role| {
            role.summary_budget
        })
}

/// The clamp ceiling for one task() dispatch: the curated agent's own
/// budget (todo 14) or the worker-role default.
fn summary_budget_for(category: &str) -> usize {
    austringer_orchestration::role::RoleDef::curated(category)
        .map_or_else(summary_budget, |def| def.summary_budget)
}

/// One curated cheap-pool read-only agent dispatch (todo 14): the
/// reserved task-tool name, the utility role its turns route under
/// (cheap-pool validated fail-closed at routing-table build), and the
/// read-only tool allowlist enforced at the child's approval stage.
struct CuratedAgent {
    /// Curated name (also the ChildStarted role label).
    name: String,
    /// The utility role the child's turns dispatch under.
    role: austringer_intel::routing::Role,
    /// Read-only tool allowlist.
    tools: Vec<String>,
}

/// Resolves the curated agent for a task() category, if the name is
/// one of the reserved curated keys. Curated names take precedence
/// over user categories of the same name — the cheap-pool and
/// read-only guarantees are the point of the curated agents, and a
/// category must not smuggle one onto a frontier model.
fn curated(category: &str) -> Option<CuratedAgent> {
    let def = austringer_orchestration::role::RoleDef::curated(category)?;
    Some(CuratedAgent {
        name: def.name.clone(),
        role: austringer_intel::routing::Role::from_key(category)?,
        tools: def.tools.clone(),
    })
}

/// Every way a task spawn is refused before a child exists.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TaskRefusal {
    /// The fork depth guard refused the spawn (fork.rs vocabulary).
    #[error(transparent)]
    Depth(#[from] DepthExceeded),
    /// The per-parent fan-out cap without explicit escalation.
    #[error(transparent)]
    FanOut(#[from] FanOutExceeded),
    /// The per-parent token budget is exhausted.
    #[error(transparent)]
    Tokens(#[from] BudgetBreach),
    /// The dispatch (category or curated role route) has no route —
    /// category errors list the valid ones; curated agents name the
    /// missing role route.
    #[error(transparent)]
    Category(#[from] austringer_intel::routing::ResolutionError),
    /// The parent session host is no longer resolvable.
    #[error("parent session host unavailable — task spawns require a live parent")]
    ParentGone,
    /// The child session could not be spawned or driven.
    #[error("child spawn failed: {0}")]
    Spawn(String),
}

/// One `task()` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRequest {
    /// Delegation category (routing.json `categories[]` key).
    pub category: String,
    /// The child's objective.
    pub prompt: String,
    /// Explicit fan-out escalation (T10: the call must say so).
    pub escalate_fan_out: bool,
}

/// What one finished child surfaced (the child host rides along so
/// later waves can surface its stream — todo 15's blocked-child asks).
pub struct TaskChildOutcome {
    /// Child session id (`{parent}-child-task{ordinal}`).
    pub child_id: String,
    /// The child's TurnFinished outcome text.
    pub outcome: String,
    /// The child's last assistant text (empty when it produced none).
    pub summary: String,
    /// Tokens the child's Usage frames reported (the budget aggregate).
    pub tokens: u64,
    /// One `(provider, model, effort)` row per child Usage frame — the
    /// dispatch-level proof of what served.
    pub usage: Vec<(String, String, Option<String>)>,
    /// The child session host (snapshot retained for surfacing).
    pub child: Arc<SessionHost>,
}

impl TaskChildOutcome {
    /// Whether the child's turn completed successfully.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.outcome == "task_completed" || self.outcome == "turn_complete"
    }
}

/// The daemon-wide delegation services: the routing intake categories
/// resolve through, the launch services children spawn under, and the
/// parent-host resolver. Per-parent ledgers are created lazily from the
/// current caps (set caps before the first spawn).
pub struct TaskServices {
    intake: Arc<RoutingIntake>,
    launch: Arc<LaunchServices>,
    resolve_parent: HostResolver,
    caps: Mutex<(u32, u64)>,
    ledgers: Mutex<HashMap<String, Arc<Mutex<DelegationBudget>>>>,
}

impl TaskServices {
    /// Services over an intake (categories must be routable), launch
    /// services, and a live-parent resolver.
    #[must_use]
    pub fn new(
        intake: Arc<RoutingIntake>,
        launch: Arc<LaunchServices>,
        resolve_parent: HostResolver,
    ) -> Self {
        Self {
            intake,
            launch,
            resolve_parent,
            caps: Mutex::new((DEFAULT_FAN_OUT_CAP, DEFAULT_TASK_TOKEN_BUDGET)),
            ledgers: Mutex::new(HashMap::new()),
        }
    }

    /// Overrides the fan-out cap for ledgers created after this call
    /// (the config seam; call before the first task spawn).
    pub fn set_fan_out_cap(&self, cap: u32) {
        if let Ok(mut caps) = self.caps.lock() {
            caps.0 = cap;
        }
    }

    /// Overrides the per-parent token budget for ledgers created after
    /// this call (the config seam; call before the first task spawn).
    pub fn set_token_budget(&self, tokens: u64) {
        if let Ok(mut caps) = self.caps.lock() {
            caps.1 = tokens;
        }
    }

    fn ledger_for(&self, parent_id: &str) -> Arc<Mutex<DelegationBudget>> {
        let caps = self
            .caps
            .lock()
            .map_or((DEFAULT_FAN_OUT_CAP, DEFAULT_TASK_TOKEN_BUDGET), |caps| {
                *caps
            });
        if let Ok(ledgers) = self.ledgers.lock()
            && let Some(existing) = ledgers.get(parent_id)
        {
            return Arc::clone(existing);
        }
        let fresh = Arc::new(Mutex::new(DelegationBudget::new(caps.0, caps.1)));
        if let Ok(mut ledgers) = self.ledgers.lock() {
            ledgers
                .entry(parent_id.to_owned())
                .or_insert_with(|| Arc::clone(&fresh));
        }
        fresh
    }

    /// One parent session's delegation aggregate, if it has one (todo
    /// 15's budget surfacing).
    #[must_use]
    pub fn budget_of(&self, parent_id: &str) -> Option<DelegationSnapshot> {
        let ledgers = self.ledgers.lock().ok()?;
        let ledger = ledgers.get(parent_id)?;
        let budget = ledger.lock().ok()?;
        Some(budget.snapshot())
    }

    /// The parent-session host resolver (the tool's parent lookup).
    #[must_use]
    pub fn parent_resolver(&self) -> &HostResolver {
        &self.resolve_parent
    }
}

/// A session's delegation depth: ids in the `-child-` namespace are
/// depth-1 children (the launch path's own namespace contract).
fn depth_of(session_id: &str) -> u32 {
    u32::from(is_child_session(session_id))
}

/// Resolves a session id from its session dir (the registry's naming
/// contract; empty when the dir has no file name).
pub(crate) fn session_id_of(session_dir: &Path) -> String {
    session_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
