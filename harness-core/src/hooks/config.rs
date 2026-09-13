//! Unified hook configuration: the spec every format maps onto, the four
//! execution kinds, fail policies with their per-event defaults, timeout
//! caps, and content hashing for trust pinning.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::HookEvent;

/// Default per-hook timeout (seconds). Small by design: hooks ride the
/// hot path of the agent loop.
pub const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 10;

/// Hard ceiling for any configured per-hook timeout (seconds). Hung
/// hooks must never wedge the loop beyond this.
pub const HOOK_TIMEOUT_CEILING_SECS: u64 = 60;

/// How a hook failure (error or timeout) resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailPolicy {
    /// The gated action proceeds without the hook's opinion; the failure
    /// is journaled as non-fatal.
    FailOpen,
    /// The gated action is denied. The required default for hooks gating
    /// safety decisions (Grok ships fail-open — AVOID).
    FailClosed,
}

/// The per-event default fail policy: `PreToolUse` hooks gate tool
/// execution — a safety decision — so they default fail-closed; every
/// other event defaults fail-open. Stop-hook failures are fail-open but
/// always surfaced as transcript notices regardless of policy (the
/// bounded-iteration guarantee forbids blocking the exit on failure).
#[must_use]
pub fn default_fail_policy(events: &[HookEvent]) -> FailPolicy {
    if events.contains(&HookEvent::PreToolUse) {
        FailPolicy::FailClosed
    } else {
        FailPolicy::FailOpen
    }
}

/// How a hook executes. The kinds are seams: production adapters live
/// outside this crate (shell → `harness-exec`'s PTY runner, MCP →
/// `harness-mcp`'s rmcp client in todo 29); tests inject mocks and the
/// tree keeps zero new dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookKind {
    /// Runs a local command through the [`ShellRunner`](super::ShellRunner)
    /// seam; the deterministic payload is handed to the command and its
    /// stdout is parsed as a [`HookResponse`](super::HookResponse).
    Shell {
        /// The command line to run.
        command: String,
    },
    /// POSTs the payload JSON to a URL through the
    /// [`HttpTransport`](super::HttpTransport) seam (no live network in
    /// tests; the transport is injected).
    Http {
        /// Request method (GET or POST; default POST).
        method: super::HttpMethod,
        /// The URL to call.
        url: String,
    },
    /// Invokes an MCP tool through the [`McpToolInvoker`](super::McpToolInvoker)
    /// seam; wiring lands with todo 29.
    Mcp {
        /// MCP server name.
        server: String,
        /// Tool name on that server.
        tool: String,
    },
    /// LLM-evaluated policy hook (Cursor pattern): the injected
    /// [`PromptEvaluator`](super::PromptEvaluator) seam judges the policy
    /// against the payload and returns a typed verdict. Stop-event
    /// prompt hooks drive the bounded iterate-until-goal loop.
    Prompt {
        /// The policy text evaluated against each payload.
        policy: String,
    },
}

/// One hook configuration: the unified model every foreign format maps
/// onto. The canonical serialization (sorted-key JSON via
/// [`serde_json::to_value`]) is what [`spec_hash`] pins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookSpec {
    /// Hook identity (unique within a config set; the trust key).
    pub name: String,
    /// Which trigger points fire this hook.
    pub events: Vec<HookEvent>,
    /// Optional `|`-separated glob list over tool names; only meaningful
    /// for the tool events, ignored elsewhere.
    pub tool_matcher: Option<String>,
    /// How the hook executes.
    pub kind: HookKind,
    /// Per-hook timeout in seconds, capped at
    /// [`HOOK_TIMEOUT_CEILING_SECS`] by the compat readers.
    pub timeout_secs: u64,
    /// Failure resolution (see [`default_fail_policy`]).
    pub fail_policy: FailPolicy,
}

impl HookSpec {
    /// A spec with defaults: default timeout and the per-event default
    /// fail policy.
    #[must_use]
    pub fn new(name: &str, events: &[HookEvent], kind: HookKind) -> Self {
        Self {
            name: name.to_owned(),
            events: events.to_vec(),
            tool_matcher: None,
            kind,
            timeout_secs: DEFAULT_HOOK_TIMEOUT_SECS,
            fail_policy: default_fail_policy(events),
        }
    }

    /// Overrides the fail policy.
    #[must_use]
    pub fn with_fail_policy(mut self, policy: FailPolicy) -> Self {
        self.fail_policy = policy;
        self
    }

    /// Overrides the timeout (seconds).
    #[must_use]
    pub fn with_timeout_secs(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Sets the `|`-separated tool-name matcher.
    #[must_use]
    pub fn with_tool_matcher(mut self, matcher: &str) -> Self {
        self.tool_matcher = Some(matcher.to_owned());
        self
    }

    /// Whether this spec triggers for `tool` on a tool event. A matcher
    /// segment that fails to compile matches nothing (fail closed).
    #[must_use]
    pub fn matches_tool(&self, tool: &str) -> bool {
        let Some(matcher) = &self.tool_matcher else {
            return true;
        };
        matcher.split('|').any(|pattern| {
            crate::inspectors::glob::Glob::new(pattern).is_ok_and(|glob| glob.matches(tool))
        })
    }
}

/// The content hash of a spec: blake3 over its canonical sorted-key JSON
/// serialization. Approval pins this; any config change moves it and
/// re-inerts the hook until re-review.
#[must_use]
pub fn spec_hash(spec: &HookSpec) -> String {
    let canonical = serde_json::to_value(spec).unwrap_or(Value::Null);
    blake3::hash(
        serde_json::to_string(&canonical)
            .unwrap_or_default()
            .as_bytes(),
    )
    .to_hex()
    .to_string()
}
