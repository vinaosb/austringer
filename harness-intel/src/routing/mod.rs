//! Per-role model routing (plan todo 42): the role -> model resolution
//! layer over the T34 catalog. Roles (Plandex's architect/editor/
//! reviewer/hydrator/summarizer plus the utility set, merged with Zed's
//! default/subagent/inline/commit-message/summary/compaction) route to
//! configured models with EXPLICIT efforts; utility roles are pinned to
//! the cheap pool; complexity tiers (Droid) and benchmark-driven live
//! routing (Kilo) compose; per-task stickiness kills silent model drift;
//! every failover hop is an explicit confirmed fallback (never silent);
//! rate limits wait out a bounded (<= 75s) window first; and the
//! model-catalog search tool hydrates bounded matches on demand — the
//! full catalog is never injected into context.

pub mod config;
pub mod dispatch;
pub mod fallback;
pub mod outcome;
pub mod resolve;
pub mod search;
pub mod sidekick;
pub mod table;

pub use dispatch::{Dispatcher, TurnDispatch};
pub use fallback::{
    DispatchFailure, FallbackDecision, FallbackRejected, RoutingEvent, RoutingFailure, StopReason,
    TurnOutcome,
};
pub use resolve::{
    PinConflict, PinnedProfile, ResolutionError, ResolutionSource, RouteResolution,
    RouteSuggestion, Stickiness, TaskId, UnknownTask, suggest_replacement,
};
pub use sidekick::{SidekickRoute, SwitchRejected, gate_model_switch};
pub use table::{RoutingConfigError, RoutingTable, WAIT_OUT_CAP_SECS};

// The confirmation contract is the T9 identity layer's; re-exported so
// routing consumers never import around it.
pub use harness_providers::identity::FallbackRequest;

/// One routed agent role (Plandex + Zed union, research SS20/SS23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Plans and decomposes (lead tier).
    Architect,
    /// Applies code changes (lead tier).
    Editor,
    /// Critiques output (lead tier).
    Reviewer,
    /// Cheap-model context hydration (utility tier).
    Hydrator,
    /// Cheap-model summarization (utility tier).
    Summarizer,
    /// Zed: the default conversational role (lead tier).
    Default,
    /// Zed: spawned subagent (lead tier).
    Subagent,
    /// Zed: inline micro-completions (lead tier).
    Inline,
    /// Utility: commit messages on cheap models.
    CommitMessage,
    /// Utility: session titles on cheap models.
    SessionTitle,
    /// Utility: auto-continue continuation on cheap models.
    AutoContinue,
    /// Utility: dedup folding on cheap models.
    DedupFolding,
    /// Zed utility: summaries on cheap models.
    Summary,
    /// Zed utility: compaction condensation on cheap models.
    Compaction,
    /// Utility: curated read-only codebase-search agent (todo 14).
    /// Cheap pool only — the table's utility validation enforces it
    /// fail-closed at build.
    Explore,
    /// Utility: curated read-only docs/OSS-search agent (todo 14).
    /// Cheap pool only — the table's utility validation enforces it
    /// fail-closed at build.
    Librarian,
}

impl Role {
    /// Parses a config key. Unknown keys are `None` — never a guess.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "architect" => Self::Architect,
            "editor" => Self::Editor,
            "reviewer" => Self::Reviewer,
            "hydrator" => Self::Hydrator,
            "summarizer" => Self::Summarizer,
            "default" => Self::Default,
            "subagent" => Self::Subagent,
            "inline" => Self::Inline,
            "commit-message" => Self::CommitMessage,
            "session-title" => Self::SessionTitle,
            "auto-continue" => Self::AutoContinue,
            "dedup-folding" => Self::DedupFolding,
            "summary" => Self::Summary,
            "compaction" => Self::Compaction,
            "explore" => Self::Explore,
            "librarian" => Self::Librarian,
            _ => return None,
        })
    }

    /// The config key for this role.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Architect => "architect",
            Self::Editor => "editor",
            Self::Reviewer => "reviewer",
            Self::Hydrator => "hydrator",
            Self::Summarizer => "summarizer",
            Self::Default => "default",
            Self::Subagent => "subagent",
            Self::Inline => "inline",
            Self::CommitMessage => "commit-message",
            Self::SessionTitle => "session-title",
            Self::AutoContinue => "auto-continue",
            Self::DedupFolding => "dedup-folding",
            Self::Summary => "summary",
            Self::Compaction => "compaction",
            Self::Explore => "explore",
            Self::Librarian => "librarian",
        }
    }

    /// The role's tier: lead roles may target frontier models; utility
    /// roles MUST stay on the cheap pool (validated fail-closed).
    #[must_use]
    pub fn class(self) -> RoleClass {
        match self {
            Self::Hydrator
            | Self::Summarizer
            | Self::CommitMessage
            | Self::SessionTitle
            | Self::AutoContinue
            | Self::DedupFolding
            | Self::Summary
            | Self::Compaction
            | Self::Explore
            | Self::Librarian => RoleClass::Utility,
            _ => RoleClass::Lead,
        }
    }
}

/// Whether a role is lead-tier or utility-tier work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleClass {
    /// Frontier work.
    Lead,
    /// Cheap-pool-only work.
    Utility,
}

/// A complexity tier (Droid, research SS16): light/medium/heavy task
/// classes with per-tier model + effort defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Quick lookups and micro-tasks.
    Light,
    /// Multi-step interactive work.
    Medium,
    /// Long-running agentic work.
    Heavy,
}

impl Tier {
    /// Parses a config key. Unknown tiers are `None`.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "light" => Self::Light,
            "medium" => Self::Medium,
            "heavy" => Self::Heavy,
            _ => return None,
        })
    }

    /// The config key for this tier.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Medium => "medium",
            Self::Heavy => "heavy",
        }
    }
}

/// What the caller asks the router for. `Tier`/`Benchmarked` requests are
/// AUTO-ROUTED (router's choice — priced with the routing discount);
/// `Role`/`Category` requests follow the user's own config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteRequest {
    /// Resolve one role's configured route.
    Role(Role),
    /// Resolve a complexity tier's default model + effort.
    Tier(Tier),
    /// Benchmark-driven live routing: the cheapest benchmark-proven
    /// model meeting the tier's score floor, at the tier's effort.
    Benchmarked(Tier),
    /// Resolve a delegation category's configured route (todo 13): the
    /// model-facing `task(category)` tool dispatches child sessions
    /// under this request. Category keys are user-authored config, so
    /// the variant carries the key.
    Category(String),
}

impl RouteRequest {
    /// True when the router (not the user's config) picked the model.
    #[must_use]
    pub fn auto_routed(&self) -> bool {
        matches!(self, Self::Tier(_) | Self::Benchmarked(_))
    }
}
