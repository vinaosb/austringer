//! User lifecycle hooks (plan todo 28).
//!
//! One unified hook model behind four execution kinds and thirteen
//! lifecycle trigger points, with the trust posture the research file
//! demands:
//!
//! - **Trust review + hash pinning (Codex model)** — a hook config is
//!   inert until a human approves it; approval pins the config's content
//!   hash, and any later content change (hash mismatch) re-inerts the
//!   hook until re-review. See [`TrustRegistry`].
//! - **Fail-closed for safety gates** — hooks gating tool execution
//!   ([`HookEvent::PreToolUse`]) default to [`FailPolicy::FailClosed`]:
//!   a hook timeout or error denies the gated action. Non-safety hooks
//!   default fail-open with journaled non-fatal errors. Stop hooks are
//!   the documented exception: failures are fail-open but ALWAYS surfaced
//!   as transcript notices (never silently swallowed), because Stop gates
//!   continuation.
//! - **No permission bypass, structurally** — a hook's `modify` gate
//!   action never reaches execution directly: the modified call passes
//!   back through the T7 inspector chain ([`crate::inspectors`]) before
//!   anything runs, and a hook `allow` cannot short-circuit the chain
//!   either. See [`gates`].
//! - **Timeout caps** — every hook runs under a per-hook timeout
//!   (default 10 s, hard ceiling 60 s); a timeout follows the hook's
//!   fail policy.
//! - **Foreign formats never crash** — Claude-settings and Cursor/legacy
//!   flat hook configs map onto this model with unknown fields/events
//!   swallowed as typed warnings ([`compat`]).
//!
//! Hook outputs are untrusted: synthesized or replaced tool results pass
//! the T7 [`SafeOutputs`](crate::inspectors::safe_outputs::SafeOutputs)
//! threat-scan seam and output budget before entering context, and
//! prompt augmentations are budget-capped.

pub mod compat;
pub mod config;
pub mod decision;
pub mod executor;
pub mod gates;
pub mod manager;
pub mod seams;
pub mod shared;
pub mod stop;
pub mod trust;

pub use config::{
    DEFAULT_HOOK_TIMEOUT_SECS, FailPolicy, HOOK_TIMEOUT_CEILING_SECS, HookKind, HookSpec,
    default_fail_policy, spec_hash,
};
pub use decision::{
    GateAction, HookOpinion, HookParseError, HookResponse, PostToolAction, StopAction,
};
pub use executor::{HookDeps, HookError, run_hook};
pub use manager::{HookFiring, HookManager, HookOutcome};
pub use seams::{
    HookHttpRequest, HookHttpResponse, HookTransportError, HttpMethod, HttpTransport,
    McpToolInvoker, PolicyVerdict, PromptEvaluator, ShellOutput, ShellRunner,
};
pub use shared::{HookedEntry, SharedHooks};
pub use trust::{TrustCheck, TrustRegistry};

use serde::{Deserialize, Serialize};

/// One lifecycle trigger point. Serialized names are snake_case; the
/// [`HookEvent::parse`] reader also accepts CamelCase and Cursor-style
/// aliases so foreign configs map without crashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    /// A session's first pipeline iteration.
    SessionStart,
    /// Session close (embedder-fired; the loop itself has no end stage).
    SessionEnd,
    /// A user prompt entered the queue; may augment the prompt.
    UserPromptSubmit,
    /// The model's turn ended; block-until semantics (bounded).
    Stop,
    /// A tool call is about to be approved/executed; may gate it.
    PreToolUse,
    /// A tool call finished; may replace/redact the output.
    PostToolUse,
    /// A tool call failed at execution.
    PostToolUseFailure,
    /// The permission chain denied a tool call.
    PermissionDenied,
    /// An embedder notification (idle, attention requests).
    Notification,
    /// A subagent session began.
    SubagentStart,
    /// A subagent session ended.
    SubagentStop,
    /// Compaction is about to run (wraps the compactor stage; T13's
    /// transcript markers remain the durable record — these hooks are
    /// the user-triggerable surface around them).
    PreCompact,
    /// Compaction finished (see [`HookEvent::PreCompact`]).
    PostCompact,
}

impl HookEvent {
    /// Every trigger point, in canonical order.
    pub const ALL: &'static [Self] = &[
        Self::SessionStart,
        Self::SessionEnd,
        Self::UserPromptSubmit,
        Self::Stop,
        Self::PreToolUse,
        Self::PostToolUse,
        Self::PostToolUseFailure,
        Self::PermissionDenied,
        Self::Notification,
        Self::SubagentStart,
        Self::SubagentStop,
        Self::PreCompact,
        Self::PostCompact,
    ];

    /// The canonical snake_case name (also the serialized form).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "session_start",
            Self::SessionEnd => "session_end",
            Self::UserPromptSubmit => "user_prompt_submit",
            Self::Stop => "stop",
            Self::PreToolUse => "pre_tool_use",
            Self::PostToolUse => "post_tool_use",
            Self::PostToolUseFailure => "post_tool_use_failure",
            Self::PermissionDenied => "permission_denied",
            Self::Notification => "notification",
            Self::SubagentStart => "subagent_start",
            Self::SubagentStop => "subagent_stop",
            Self::PreCompact => "pre_compact",
            Self::PostCompact => "post_compact",
        }
    }

    /// Parses a foreign event name: canonical snake_case, CamelCase, and
    /// Cursor-style aliases (`before_tool_call`, `stop_hook`, `notify`, …).
    /// Unknown names return `None` (callers surface a typed warning).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        const ALIASES: &[(&str, HookEvent)] = &[
            ("before_tool_call", HookEvent::PreToolUse),
            ("after_tool_call", HookEvent::PostToolUse),
            ("stop_hook", HookEvent::Stop),
            ("tool_failure", HookEvent::PostToolUseFailure),
            ("notify", HookEvent::Notification),
            ("before_compact", HookEvent::PreCompact),
            ("after_compact", HookEvent::PostCompact),
        ];
        for (alias, event) in ALIASES {
            if name == *alias {
                return Some(*event);
            }
        }
        let snake = to_snake(name);
        Self::ALL
            .iter()
            .copied()
            .find(|event| event.as_str() == snake)
    }
}

fn to_snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}
