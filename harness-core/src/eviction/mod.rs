//! Tiered eviction, dedup folding, and the single volatile error slot
//! (plan todo 12).
//!
//! The doctrine, in one paragraph: eviction triggers at 75% of the typed
//! [`ContextBudget`](budget::ContextBudget) window. Tier0 (system core,
//! active plan) is structurally eviction-immune — targeting it is a typed
//! rejection. The protected head (first ~3) and tail (last ~20) messages
//! are immune regardless of kind. The condensable middle walks a named,
//! ordered ladder — masking → distillation → compression → truncation,
//! each stage with an explicit trigger and budget — until the target
//! budget is met. Tool results fold to parseable
//! `<folded_tool_output>` markers carrying blake3 content pointers and
//! the call's return code (never dropped); stale tool results walk the
//! Grok pruning ladder (soft-trim head/tail excerpts → hard-clear to
//! pointer). Salient facts flush to a memory hook N tokens BEFORE the
//! compact threshold. The latest error REPLACES the previous in the
//! LLM-visible projection — errors never stack. Call budgets are
//! two-tier and per-class, never one global cap. The engine itself is
//! pluggable behind one trait; the eviction SIM drives it across
//! synthetic sessions deterministically.
//!
//! The output of [`engine::EvictionEngine::evict`] is a projected
//! history (`Vec<RecordedEvent>`) that feeds T11's `[L1,L2)` history
//! segment unchanged — the prompt builder is extended, never mutated.

mod budget;
mod engine;
mod fold;
mod governor;
mod ladder;
mod sim;
mod tiers;
mod walk;

pub use budget::{ContextBudget, DegenerateInput, EvictionInput, estimate_tokens, events_tokens};
pub use engine::{
    DefaultEvictionEngine, EvictionDeps, EvictionEngine, EvictionOutcome, MinimalEngine,
};
pub use fold::{
    CondensateBlock, CondenseError, CondenseRequest, Condenser, ErrorObservation, ErrorSlot,
    FoldParseError, FoldedToolOutput, MemoryFlush, MemoryHook, ReturnCode, SalienceFilter,
    ToolReturnTable, VecMemoryHook, fold_history,
};
pub use governor::{
    CallDenial, CallGovernor, ClassBudget, GovernorConfig, GovernorNotice, GovernorVerdict,
    KeywordClassifier, TaskClass, TaskClassifier,
};
pub use ladder::{
    LADDER_ORDER, LadderConfig, LadderStage, PruningConfig, StageConfig, StageDecision, soft_trim,
};
pub use sim::{SimReport, SimRng, TurnReport, simulate_session};
pub use tiers::{ActivePlan, Projection, ProtectionConfig, Tier};

/// Why an eviction turn failed, typed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EvictionError {
    /// Degenerate input (zero window, empty session).
    #[error("degenerate eviction input: {0}")]
    Degenerate(#[from] DegenerateInput),
    /// A decision targeted Tier0 content (system core or the active
    /// plan) — structurally forbidden.
    #[error("tier0 content is eviction-immune: {target}")]
    Tier0Protected {
        /// What was targeted ("tier0", "active_plan").
        target: String,
    },
    /// Folding would drop a tool return code — refused.
    #[error("missing return code for call {call_id}: folding refuses")]
    MissingReturnCode {
        /// The call whose authoritative code is absent.
        call_id: String,
    },
    /// A structural invariant broke post-walk (protected event evicted,
    /// marker unparseable, code mismatch).
    #[error("eviction invariant broken: {detail}")]
    InvariantBroken {
        /// What broke.
        detail: String,
    },
    /// The full ladder cannot bring usage under the target budget
    /// (protected zones alone exceed it, or conversation mass needs the
    /// real compactor — todo 13).
    #[error("eviction budget unmet: usage {usage} > target {target}")]
    BudgetUnmet {
        /// Post-walk usage.
        usage: u64,
        /// The target budget.
        target: u64,
    },
    /// The condensation seam failed.
    #[error("{0}")]
    Condense(CondenseError),
}

impl EvictionError {
    /// The typed rejection for targeting the active plan.
    #[must_use]
    pub fn tier0_plan_rejection() -> Self {
        Self::Tier0Protected {
            target: "active_plan".to_owned(),
        }
    }
}

/// The full eviction policy in one typed value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvictionConfig {
    /// The context-window budget (window, trigger, target, flush lead).
    pub budget: ContextBudget,
    /// Protected head/tail message counts.
    pub protection: ProtectionConfig,
    /// The ladder: named stages with explicit triggers and budgets.
    pub ladder: LadderConfig,
    /// Tool-result pruning budgets (soft-trim excerpt sizes).
    pub pruning: PruningConfig,
}

impl Default for EvictionConfig {
    fn default() -> Self {
        Self {
            budget: ContextBudget::for_window(200_000),
            protection: ProtectionConfig::default(),
            ladder: LadderConfig::default(),
            pruning: PruningConfig::default(),
        }
    }
}

impl EvictionConfig {
    /// The config with a different window size (SIM and tests).
    #[must_use]
    pub fn with_window(mut self, window_tokens: u64) -> Self {
        self.budget.window_tokens = window_tokens;
        self
    }
}
