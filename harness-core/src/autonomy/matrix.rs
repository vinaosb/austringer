//! The autonomy matrix (plan todo 8): four autonomy LEVELS expressed as
//! per-capability toggle presets — not a bare dial (Plandex 5-level ×
//! 10-toggle pattern).
//!
//! # The ten capability columns
//!
//! Each column is backed by a seam that exists today:
//!
//! | Column | Seam that consumes it |
//! |---|---|
//! | [`Capability::Read`] | `view_window` calls (auto-read) |
//! | [`Capability::Edit`] | `edit_lines` calls (auto-edit) |
//! | [`Capability::Exec`] | `execute_command` calls (auto-exec) |
//! | [`Capability::Compact`] | the `Compactor` pipeline stage (todo 13) |
//! | [`Capability::Context`] | `lsp_query` semantic navigation (auto-context) |
//! | [`Capability::Search`] | `tool_search` deferred hydration |
//! | [`Capability::Completion`] | `task_completed` reporting (always on: metadata) |
//! | [`Capability::SecretRead`] | the secret-file guard (default off everywhere) |
//! | [`Capability::PairFold`] | the `ToolPairCompaction` stage (todo 12) |
//! | [`Capability::GrantMemory`] | `a` answers persisting to the store |
//!
//! The matrix is the SINGLE SOURCE both the inspector chain and the loop
//! consult: [`crate::autonomy::gate::AutonomyGate`] reads it per call, and
//! `/mode` (see [`crate::autonomy::mode_command`]) writes it through the
//! shared [`AutonomyHandle`]. Mode changes never touch the serialized tool
//! schema — they gate dispatch and approval only.

use std::sync::{Arc, RwLock};

/// The autonomy levels. Levels are toggle PRESETS: a mode fixes a default
/// toggle vector, a fallback strategy for off-capabilities, and a review
/// policy; individual toggles remain independently flippable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutonomyMode {
    /// Converse only: every capability asks (the headless default).
    Chat,
    /// Auto-approve reads/context, ask for writes and commands.
    Approve,
    /// Auto-approve benign work, ask on destructive — the classifier
    /// decides every off-capability call.
    SmartApprove,
    /// Full auto: edits run free; commands still classify; the
    /// destructive floor and softDeny remain armed.
    Auto,
}

impl AutonomyMode {
    /// Every mode, in level order.
    pub const ALL: [AutonomyMode; 4] = [Self::Chat, Self::Approve, Self::SmartApprove, Self::Auto];

    /// Canonical lowercase name (`/mode` words).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Approve => "approve",
            Self::SmartApprove => "smartapprove",
            Self::Auto => "auto",
        }
    }

    /// Parses a `/mode` word (case-insensitive; `smart-approve` accepted).
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        let normalized = word.trim().to_lowercase().replace(['-', '_'], "");
        Self::ALL.into_iter().find(|mode| normalized == mode.name())
    }
}

/// One capability column of the matrix (see the module docs for the
/// backing seams).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// `view_window` file reads (auto-read).
    Read,
    /// `edit_lines` file edits (auto-edit).
    Edit,
    /// `execute_command` shell commands (auto-exec).
    Exec,
    /// History compaction (auto-compact; consumed by todo 13's stage).
    Compact,
    /// `lsp_query` semantic context acquisition (auto-context).
    Context,
    /// `tool_search` deferred hydration.
    Search,
    /// `task_completed` reporting — metadata, on in every preset.
    Completion,
    /// Reads of secret-pattern files; off in every preset by default.
    SecretRead,
    /// Tool-pair folding (consumed by todo 12's stage).
    PairFold,
    /// Whether `a` answers persist (off downgrades GrantAlways to once).
    GrantMemory,
}

impl Capability {
    /// All ten columns, in declaration order.
    pub const ALL: [Capability; 10] = [
        Self::Read,
        Self::Edit,
        Self::Exec,
        Self::Compact,
        Self::Context,
        Self::Search,
        Self::Completion,
        Self::SecretRead,
        Self::PairFold,
        Self::GrantMemory,
    ];

    fn index(self) -> usize {
        self as usize
    }
}

/// Artifact-gated approval (Antigravity, research §21): the autonomy-mode
/// dimension deciding whether task-level completion requires an artifact
/// review verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewPolicy {
    /// Task completion is hard-blocked until a review verdict clears the
    /// plan gate (`PlanGate::clear`).
    RequestReview,
    /// Completion proceeds without a review verdict.
    AlwaysProceed,
}

/// How an off-capability call is resolved when no toggle, grant, or floor
/// answered it first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    /// Route to the ask policy (headless default: refuse).
    Ask,
    /// Route to the two-stage classifier (Auto/SmartApprove).
    Classify,
}

/// The matrix: one mode level, ten independent toggles, a plan-mode flag,
/// and the review dimension. Cloneable by value; share via
/// [`AutonomyHandle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutonomyMatrix {
    mode: AutonomyMode,
    plan: bool,
    toggles: [bool; 10],
    review: ReviewPolicy,
}

impl AutonomyMatrix {
    /// The level preset for `mode`.
    #[must_use]
    pub fn for_mode(mode: AutonomyMode) -> Self {
        let toggles = match mode {
            AutonomyMode::Chat => [
                false, false, false, false, false, false, true, false, false, false,
            ],
            AutonomyMode::Approve | AutonomyMode::SmartApprove => [
                true, false, false, true, true, true, true, false, true, true,
            ],
            AutonomyMode::Auto => [true, true, false, true, true, true, true, false, true, true],
        };
        Self {
            mode,
            plan: false,
            toggles,
            review: if mode == AutonomyMode::Auto {
                ReviewPolicy::AlwaysProceed
            } else {
                ReviewPolicy::RequestReview
            },
        }
    }

    /// The current mode level.
    #[must_use]
    pub fn mode(&self) -> AutonomyMode {
        self.mode
    }

    /// Switches the level: toggles and review reset to the preset and the
    /// plan pairing clears (re-enter with [`Self::enter_plan`]).
    pub fn set_mode(&mut self, mode: AutonomyMode) {
        *self = Self::for_mode(mode);
    }

    /// Whether plan mode (read-only pairing) is active.
    #[must_use]
    pub fn is_plan(&self) -> bool {
        self.plan
    }

    /// Enters plan mode: write tools freeze at the dispatch gate.
    pub fn enter_plan(&mut self) {
        self.plan = true;
    }

    /// Flips one capability toggle independently of the others.
    pub fn toggle(&mut self, capability: Capability, on: bool) {
        self.toggles[capability.index()] = on;
    }

    /// Whether `capability` is auto-approved in the current state.
    #[must_use]
    pub fn allows(&self, capability: Capability) -> bool {
        self.toggles[capability.index()]
    }

    /// The artifact-review dimension.
    #[must_use]
    pub fn review(&self) -> ReviewPolicy {
        self.review
    }

    /// Overrides the review dimension (an independent matrix dimension).
    pub fn set_review(&mut self, review: ReviewPolicy) {
        self.review = review;
    }

    /// The fallback strategy for off-capability calls in this mode.
    #[must_use]
    pub fn fallback(&self) -> Fallback {
        match self.mode {
            AutonomyMode::Chat | AutonomyMode::Approve => Fallback::Ask,
            AutonomyMode::SmartApprove | AutonomyMode::Auto => Fallback::Classify,
        }
    }
}

impl Default for AutonomyMatrix {
    /// The headless default: Chat — everything asks, and an unresolved
    /// ask is a refusal ([`crate::inspectors::store::RefuseAsks`]).
    fn default() -> Self {
        Self::for_mode(AutonomyMode::Chat)
    }
}

/// A shared, cloneable matrix handle: `/mode` writes it, the autonomy
/// gate and ask wrapper read it. Lock poisoning recovers the data
/// (`into_inner`) instead of panicking — a half-written mode switch is
/// observable, never fatal.
#[derive(Debug, Clone)]
pub struct AutonomyHandle(Arc<RwLock<AutonomyMatrix>>);

impl AutonomyHandle {
    /// Shares `matrix`.
    #[must_use]
    pub fn new(matrix: AutonomyMatrix) -> Self {
        Self(Arc::new(RwLock::new(matrix)))
    }

    /// Another handle over the same matrix.
    #[must_use]
    pub fn shared(&self) -> Self {
        Self(Arc::clone(&self.0))
    }

    /// Reads the matrix under a closure.
    pub fn read<T>(&self, reader: impl FnOnce(&AutonomyMatrix) -> T) -> T {
        let guard = self
            .0
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reader(&guard)
    }

    /// Writes the matrix under a closure.
    pub fn write<T>(&self, writer: impl FnOnce(&mut AutonomyMatrix) -> T) -> T {
        let mut guard = self
            .0
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        writer(&mut guard)
    }
}
