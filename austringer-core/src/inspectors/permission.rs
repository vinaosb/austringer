//! PermissionInspector: allow/ask/deny from glob rules over the tool name
//! and an extracted target (path or command), with grant memory.
//!
//! Target extraction convention: the first string-valued argument whose key
//! is in [`PATH_KEYS`] is the call's path target; failing that, the first in
//! [`COMMAND_KEYS`] is its command target. (Todo 6's tool registry aligns
//! its schemas with these key names; the convention is data, not code, so
//! it stays overridable per rule via [`TargetKind`].)
//!
//! Decision lattice (first match wins): **deny rule > stored grant > allow
//! rule > ask rule > no opinion** (a neutral pass — the chain's fallback
//! ask, not this inspector, covers the no-rule case, so more specific
//! guards such as the secret-file guard can raise their own ask). A stored
//! grant is what a user's `a` answer produced; it never overrides a deny
//! rule.

use std::sync::Arc;

use async_trait::async_trait;

use crate::agent_loop::PendingCall;
use crate::agent_loop::stages::StageError;
use crate::inspectors::glob::{Glob, InvalidGlob};
use crate::inspectors::secret_files::TranscriptRedactor;
use crate::inspectors::store::PermissionStore;
use crate::inspectors::verdict::{CallInspector, Verdict};

/// Argument keys treated as path targets, in precedence order.
pub const PATH_KEYS: &[&str] = &["path", "file_path", "file", "target"];

/// Argument keys treated as command targets, in precedence order.
pub const COMMAND_KEYS: &[&str] = &["command", "cmd", "shell"];

/// What kind of target a rule constrains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// A filesystem path argument.
    Path,
    /// A command string argument (prefix globs: `cargo *`).
    Command,
}

/// The target extracted from a pending call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRef {
    /// Which convention produced this target.
    pub kind: TargetKind,
    /// The normalized value (Windows `\` folded to `/`).
    pub value: String,
}

/// Extracts the call's target, if it carries one. Path keys take
/// precedence over command keys; empty strings are not targets.
#[must_use]
pub fn extract_target(call: &PendingCall) -> Option<TargetRef> {
    let object = call.arguments.as_object()?;
    for keys_and_kind in [
        (PATH_KEYS, TargetKind::Path),
        (COMMAND_KEYS, TargetKind::Command),
    ] {
        let (keys, kind) = keys_and_kind;
        for key in keys {
            if let Some(serde_json::Value::String(value)) = object.get(*key)
                && !value.is_empty()
            {
                return Some(TargetRef {
                    kind,
                    value: value.replace('\\', "/"),
                });
            }
        }
    }
    None
}

/// The effect a permission rule has on a matching call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Matching calls are explicitly allowed (short-circuits later
    /// ask-generators).
    Allow,
    /// Matching calls require user approval.
    Ask,
    /// Matching calls are refused with model feedback.
    Deny,
}

/// A target constraint attached to a rule.
#[derive(Debug, Clone)]
pub struct TargetRule {
    /// Which target convention this pattern applies to.
    pub kind: TargetKind,
    /// The glob the target value must match.
    pub pattern: Glob,
}

/// One permission rule: a tool glob plus an optional target glob.
#[derive(Debug, Clone)]
pub struct Rule {
    /// What happens when the rule matches.
    pub effect: Effect,
    /// Glob over the tool name.
    pub tool: Glob,
    /// Optional glob over the extracted target.
    pub target: Option<TargetRule>,
}

impl Rule {
    /// A tool-name-only rule.
    pub fn tool(effect: Effect, tool_pattern: &str) -> Result<Self, InvalidGlob> {
        Ok(Self {
            effect,
            tool: Glob::new(tool_pattern)?,
            target: None,
        })
    }

    /// A rule constraining the call's path target.
    pub fn path(
        effect: Effect,
        tool_pattern: &str,
        path_pattern: &str,
    ) -> Result<Self, InvalidGlob> {
        Ok(Self {
            effect,
            tool: Glob::new(tool_pattern)?,
            target: Some(TargetRule {
                kind: TargetKind::Path,
                pattern: Glob::new(path_pattern)?,
            }),
        })
    }

    /// A rule constraining the call's command target.
    pub fn command(
        effect: Effect,
        tool_pattern: &str,
        command_pattern: &str,
    ) -> Result<Self, InvalidGlob> {
        Ok(Self {
            effect,
            tool: Glob::new(tool_pattern)?,
            target: Some(TargetRule {
                kind: TargetKind::Command,
                pattern: Glob::new(command_pattern)?,
            }),
        })
    }

    fn matches(&self, call: &PendingCall) -> bool {
        if !self.tool.matches(&call.tool) {
            return false;
        }
        match &self.target {
            None => true,
            Some(constraint) => extract_target(call).is_some_and(|target| {
                target.kind == constraint.kind && constraint.pattern.matches(&target.value)
            }),
        }
    }

    fn describe(&self) -> &str {
        match &self.target {
            Some(constraint) => constraint.pattern.pattern(),
            None => self.tool.pattern(),
        }
    }
}

/// Permission rule set. Empty by default: every call asks.
#[derive(Debug, Clone, Default)]
pub struct PermissionConfig {
    /// Rules in configuration order.
    pub rules: Vec<Rule>,
}

/// The pattern-matching permission inspector.
#[derive(Clone)]
pub struct PermissionInspector {
    config: PermissionConfig,
    store: Arc<dyn PermissionStore>,
    redactor: TranscriptRedactor,
}

impl PermissionInspector {
    /// Builds the inspector with its rule set, grant store, and feedback
    /// redactor.
    #[must_use]
    pub fn new(
        config: PermissionConfig,
        store: Arc<dyn PermissionStore>,
        redactor: TranscriptRedactor,
    ) -> Self {
        Self {
            config,
            store,
            redactor,
        }
    }

    fn describe_target(&self, call: &PendingCall) -> String {
        match extract_target(call) {
            Some(target) => self.redactor.mask_path(&target.value),
            None => "(no target)".to_owned(),
        }
    }
}

#[async_trait]
impl CallInspector for PermissionInspector {
    fn name(&self) -> &'static str {
        "permission"
    }

    async fn inspect(&mut self, call: &PendingCall) -> Result<Verdict, StageError> {
        // Deny always wins, including over stored grants.
        for rule in &self.config.rules {
            if rule.effect == Effect::Deny && rule.matches(call) {
                return Ok(Verdict::Deny {
                    reason: format!(
                        "permission denied: tool '{}' target '{}' matched deny rule '{}'",
                        call.tool,
                        self.describe_target(call),
                        rule.describe()
                    ),
                });
            }
        }
        // A stored user grant (`a` answer) is an authoritative allow.
        let target = extract_target(call);
        let grant_key = target.as_ref().map(|target| target.value.as_str());
        if self.store.lookup(&call.tool, grant_key).is_some() {
            return Ok(Verdict::Allow {
                authoritative: true,
            });
        }
        // Explicit allow rules are authoritative allows.
        for rule in &self.config.rules {
            if rule.effect == Effect::Allow && rule.matches(call) {
                return Ok(Verdict::Allow {
                    authoritative: true,
                });
            }
        }
        for rule in &self.config.rules {
            if rule.effect == Effect::Ask && rule.matches(call) {
                return Ok(Verdict::Ask {
                    reason: format!(
                        "approval required: tool '{}' target '{}' matched ask rule '{}'",
                        call.tool,
                        self.describe_target(call),
                        rule.describe()
                    ),
                    notice_on_approval: None,
                });
            }
        }
        // Absence of policy is not a permission decision: pass neutrally
        // so more specific guards (secret files) can raise their own ask,
        // and let the chain's fallback ask cover the no-rule case.
        Ok(Verdict::Allow {
            authoritative: false,
        })
    }
}
