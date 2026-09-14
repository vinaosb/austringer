//! Spawn authorization classes, the commandExecutionPolicy ladder
//! (Antigravity SS21), permission bubbling (children escalate THROUGH the
//! parent, never self-approve), deny-only permission inheritance, and
//! task/todowrite capability gating for children.

use std::sync::Mutex;

use harness_safety::perm::grammar::{Action, Rule, Target};

use crate::role::{RoleDef, Tier};
use crate::write_set::WriteSet;

/// The spawn-authorization classes (SS33): standing authorization for
/// read-only/validation/exploration, fresh authorization for
/// edits/commits/deploys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpawnAuthzClass {
    /// Pre-authorized (read-only / validation / exploration).
    Standing,
    /// Requires a fresh, explicit grant (writes anything).
    Fresh,
}

/// Classifies a spawn: read-only explorer work is standing; anything that
/// writes — or any non-explorer tier — is fresh.
pub fn classify_spawn(role: &RoleDef, writes: &WriteSet) -> SpawnAuthzClass {
    if writes.is_empty() && role.tier == Tier::Explorer {
        SpawnAuthzClass::Standing
    } else {
        SpawnAuthzClass::Fresh
    }
}

/// The commandExecutionPolicy ladder: `off < auto < eager`, with
/// `sandbox` as the forced-sandboxed rung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CommandExecutionPolicy {
    /// Children cannot execute commands.
    Off,
    /// Commands escalate through the parent (bubbling).
    #[default]
    Auto,
    /// Auto-allowed within stored grants.
    Eager,
    /// Forced sandboxed execution.
    Sandbox,
}

impl CommandExecutionPolicy {
    /// The full ladder, weakest to strongest.
    pub fn ladder() -> [Self; 4] {
        [Self::Off, Self::Auto, Self::Eager, Self::Sandbox]
    }

    fn rank(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Auto => 1,
            Self::Eager => 2,
            Self::Sandbox => 3,
        }
    }

    /// Whether `self` is weak enough to be served under `other`.
    pub fn permits(&self, other: &Self) -> bool {
        self.rank() <= other.rank()
    }

    /// Strict parse.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "auto" => Some(Self::Auto),
            "eager" => Some(Self::Eager),
            "sandbox" => Some(Self::Sandbox),
            _ => None,
        }
    }
}

/// A child's escalation request, routed THROUGH the parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationRequest {
    /// Requesting child id.
    pub child_id: String,
    /// The action needing escalation.
    pub action: Action,
    /// The target needing escalation.
    pub target: Target,
    /// Why the child needs it.
    pub justification: String,
}

/// The parent's decision on one escalation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationDecision {
    /// The request being decided.
    pub request: EscalationRequest,
    /// Granted?
    pub granted: bool,
    /// Who decided (always the parent; children never self-approve).
    pub decided_by: String,
}

/// The parent-side escalation sink (permission bubbling). The default
/// records and refuses (fail closed); [`ParentEscalations::approving`]
/// models an explicitly approving parent.
pub struct ParentEscalations {
    requests: Mutex<Vec<EscalationRequest>>,
    approving: bool,
}

impl std::fmt::Debug for ParentEscalations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParentEscalations")
            .field("approving", &self.approving)
            .field(
                "recorded",
                &self.requests.lock().map(|r| r.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl Default for ParentEscalations {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentEscalations {
    /// Record-and-refuse (fail closed).
    pub fn new() -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            approving: false,
        }
    }

    /// Record-and-approve (explicit parent configuration).
    pub fn approving() -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            approving: true,
        }
    }

    /// Handle one escalation: record it, then decide.
    pub fn escalate(&self, request: EscalationRequest) -> EscalationDecision {
        let granted = self.approving;
        if let Ok(mut queue) = self.requests.lock() {
            queue.push(request.clone());
        }
        EscalationDecision {
            request,
            granted,
            decided_by: "parent".to_string(),
        }
    }

    /// All recorded requests.
    pub fn requests(&self) -> Vec<EscalationRequest> {
        self.requests.lock().map(|q| q.clone()).unwrap_or_default()
    }
}

/// The escalation sink: children bubble permission requests THROUGH the
/// parent (or a parent-configured mediator); there is no child-side
/// self-approval path.
pub trait EscalationSink: Send + Sync {
    /// Decide one escalation request.
    fn escalate(&self, request: EscalationRequest) -> EscalationDecision;
}

impl EscalationSink for ParentEscalations {
    fn escalate(&self, request: EscalationRequest) -> EscalationDecision {
        Self::escalate(self, request)
    }
}

/// DENY-ONLY permission inheritance: children inherit exactly the parent's
/// deny rules — allows and asks are never inherited.
pub fn inherit_denies(parent: &[Rule]) -> Vec<Rule> {
    parent
        .iter()
        .filter(|rule| rule.effect == harness_safety::perm::grammar::Effect::Deny)
        .cloned()
        .collect()
}

/// One explicit fresh-authorization grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshGrant {
    /// Grant scope (child id).
    pub scope: String,
    /// Who granted it.
    pub granted_by: String,
}

/// Ledger of fresh grants the parent minted.
#[derive(Debug, Default)]
pub struct AuthorizationLedger {
    grants: std::collections::BTreeMap<String, FreshGrant>,
}

impl AuthorizationLedger {
    /// Empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mints a grant for a scope (child id).
    pub fn mint(&mut self, scope: &str, granted_by: &str) -> FreshGrant {
        let grant = FreshGrant {
            scope: scope.to_string(),
            granted_by: granted_by.to_string(),
        };
        self.grants.insert(grant.scope.clone(), grant.clone());
        grant
    }

    /// Whether a scope holds a valid grant.
    pub fn has(&self, scope: &str) -> bool {
        self.grants.contains_key(scope)
    }
}

/// A loud, structured spawn refusal — the launcher NEVER silently falls
/// back to running the task in the parent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "fresh authorization required for role `{role}` writing {:?}: mint a grant or drop the writes",
    writes
)]
pub struct SpawnRefusal {
    /// The refused role.
    pub role: String,
    /// The writes that made it fresh-class.
    pub writes: Vec<String>,
    /// The child id scope a grant must name.
    pub scope: String,
}

/// Capability gates for children (task/todowrite are NOT granted by
/// default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChildCapabilities {
    /// May the child use the task tool?
    pub task: bool,
    /// May the child use todowrite?
    pub todo: bool,
}
