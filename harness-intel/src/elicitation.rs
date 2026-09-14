//! The SET/PROMPT/THROTTLE elicitation constitution (research SS33.6):
//! SET the deterministic knobs (API params) for depth/cost/latency; PROMPT
//! for each model's documented behavioral pathologies — gated on OUR
//! observed evals, never fired speculatively; THROTTLE structurally what
//! neither can bound. Never prompt for what a parameter controls; never
//! parameterize what only structure can bound.

use super::axes::BehaviorAxes;
use super::catalog::{CatalogEntry, EffortValue, Family, ThinkingLevel};
use super::model_profile::ModelProfile;

/// Workload class driving SET and THROTTLE (the SS33.3 scaling-rule
/// vocabulary: fact-find ~3-10 calls, comparisons 10-15, complex
/// watchdogged larger budgets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskClass {
    /// Cheap lookups; clamp expensive effort defaults down.
    FactFind,
    /// Multi-source comparisons.
    Comparison,
    /// Coding/agentic work; keep the model's documented default.
    Complex,
}

/// The sanctioned channels for a mid-conversation effort change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffortChannel {
    /// Wait for a completed compaction / session movement (cache-coupled
    /// families render effort INTO the prompt).
    EpochBoundary,
    /// The per-message effort beta (cache-preserving; exists on a
    /// documented subset of models).
    PerMessageBeta,
}

/// What the SET plan does about thinking, per family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingDirective {
    /// Effort carries depth; no separate thinking knob is set.
    EffortImplied,
    /// `thinking.type: adaptive` (Anthropic 4.6+).
    Adaptive,
    /// A `thinking_level` mapped from effort, clamped to the model floor.
    Level {
        /// The level to send.
        level: ThinkingLevel,
    },
}

/// The deterministic knobs (research SS33.6 "SET per family").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetPlan {
    /// The effort to send EXPLICITLY on every call — never the provider's
    /// implicit default.
    pub effort: EffortValue,
    /// The thinking directive.
    pub thinking: ThinkingDirective,
    /// Hold-effort-constant cache discipline: true on cache-coupled
    /// families where effort renders into the cached prefix.
    pub hold_effort_constant: bool,
    /// How a mid-run effort change may happen on this model.
    pub effort_change_channel: EffortChannel,
}

/// Counterweight identity (the documented pathologies, SS33.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterweightId {
    /// Opus-4.6-class over-exploration: commit to an approach.
    CommitToApproach,
    /// Fable-5.1-class under-parallelization: batch + report progress.
    BatchingAndProgress,
    /// GPT-5.5-class overthinking: explicit stopping conditions.
    StoppingConditions,
    /// Companion to stopping conditions: cap retrieval effort.
    RetrievalBudget,
    /// GPT-5.1-class premature concision: persist to completeness.
    Persistence,
    /// Weak-editor compensation: request exact line ranges.
    ExactRanges,
}

/// One gated prompt counterweight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptCounterweight {
    /// Which counterweight this is.
    pub id: CounterweightId,
    /// The prompt text to inject.
    pub text: &'static str,
}

/// Structural throttles (research SS33.3/SS33.6 "THROTTLE structurally").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrottlePlan {
    /// Soft per-class call budget injected into context.
    pub soft_call_budget: u32,
    /// Hard budget denying further calls with a structured error.
    pub hard_call_budget: u32,
    /// Concurrent subagent cap.
    pub max_concurrent_subagents: u32,
    /// Total subtask cap.
    pub max_total_subtasks: u32,
    /// Subagent depth — pinned to 1 (split caps doctrine).
    pub max_depth: u32,
}

/// One elicitation plan for (model, profile, task class).
#[derive(Debug, Clone, PartialEq)]
pub struct ElicitationPlan {
    /// The SET layer.
    pub set: SetPlan,
    /// The gated PROMPT layer (empty for a neutral profile).
    pub prompt: Vec<PromptCounterweight>,
    /// The THROTTLE layer.
    pub throttle: ThrottlePlan,
}

// Observed-bias gates for PROMPT counterweights (harness-side thresholds;
// the counterweights themselves are vendor-documented, the FIRING is ours).
const OVER_EXPLORATION_CALLS_PER_TURN: f32 = 12.0;
const UNDER_PARALLELIZER_CALLS_PER_TURN: f32 = 6.0;
const RETRY_STORM_RATE: f32 = 0.20;
const TERSE_OUTPUT_TOKENS_PER_TURN: f32 = 300.0;
const EDIT_REJECTION_RATE: f32 = 0.25;
const OVER_DELEGATION_RATIO: f32 = 2.0;

/// Elicits the SET/PROMPT/THROTTLE plan for one model under one observed
/// profile and one workload class.
#[must_use]
pub fn elicit(entry: &CatalogEntry, profile: &ModelProfile, class: TaskClass) -> ElicitationPlan {
    ElicitationPlan {
        set: set_plan(entry, class),
        prompt: counterweights_for(profile),
        throttle: throttle_plan(class, profile.axes.as_ref()),
    }
}

fn set_plan(entry: &CatalogEntry, class: TaskClass) -> SetPlan {
    // Fact-find clamps expensive defaults down (SS33.2: "SET effort
    // explicitly even for cheap calls"); other classes keep the model's
    // documented default.
    let effort = match class {
        TaskClass::FactFind => entry.default_effort.min(EffortValue::Medium),
        _ => entry.default_effort,
    };
    let thinking = match entry.family {
        Family::Anthropic => ThinkingDirective::Adaptive,
        Family::Google => ThinkingDirective::Level {
            level: google_level(effort, entry.thinking_floor),
        },
        Family::OpenAi | Family::Xai | Family::DeepSeek => ThinkingDirective::EffortImplied,
    };
    SetPlan {
        effort,
        thinking,
        hold_effort_constant: entry.family == Family::Anthropic,
        effort_change_channel: if entry.per_message_effort_beta {
            EffortChannel::PerMessageBeta
        } else {
            EffortChannel::EpochBoundary
        },
    }
}

fn google_level(effort: EffortValue, floor: Option<ThinkingLevel>) -> ThinkingLevel {
    let mapped = match effort {
        EffortValue::None => ThinkingLevel::Minimal,
        EffortValue::Low => ThinkingLevel::Low,
        EffortValue::Medium => ThinkingLevel::Medium,
        EffortValue::High | EffortValue::Xhigh | EffortValue::Max => ThinkingLevel::High,
    };
    floor.map_or(mapped, |floor| mapped.max(floor))
}

fn counterweights_for(profile: &ModelProfile) -> Vec<PromptCounterweight> {
    let Some(axes) = profile.axes.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if axes.tool_calls_per_turn > OVER_EXPLORATION_CALLS_PER_TURN {
        out.push(weight(CounterweightId::CommitToApproach));
    }
    if axes.delegation_ratio == 0.0 && axes.tool_calls_per_turn > UNDER_PARALLELIZER_CALLS_PER_TURN
    {
        out.push(weight(CounterweightId::BatchingAndProgress));
    }
    let storms = profile
        .taxonomy
        .as_ref()
        .is_some_and(|t| t.retry_storm_rate > RETRY_STORM_RATE);
    if storms {
        out.push(weight(CounterweightId::StoppingConditions));
        out.push(weight(CounterweightId::RetrievalBudget));
    }
    if axes.verbosity_tokens_per_turn < TERSE_OUTPUT_TOKENS_PER_TURN {
        out.push(weight(CounterweightId::Persistence));
    }
    let weak_editor = profile
        .taxonomy
        .as_ref()
        .is_some_and(|t| t.edit_rejection_rate > EDIT_REJECTION_RATE);
    if weak_editor {
        out.push(weight(CounterweightId::ExactRanges));
    }
    out
}

fn weight(id: CounterweightId) -> PromptCounterweight {
    let text = match id {
        CounterweightId::CommitToApproach => {
            "Choose an approach and commit to it. Do not revisit the decision \
             unless new evidence contradicts it."
        }
        CounterweightId::BatchingAndProgress => {
            "Issue independent reads in one batched tool block, and end each \
             round with a one-sentence progress update."
        }
        CounterweightId::StoppingConditions => {
            "State the stopping conditions before you start, and stop when \
             they are met."
        }
        CounterweightId::RetrievalBudget => {
            "Cap retrieval at the budgeted number of searches; answer from \
             evidence already gathered when it suffices."
        }
        CounterweightId::Persistence => {
            "Persist to completeness: finish the whole task before \
             summarizing, and verify the result end to end."
        }
        CounterweightId::ExactRanges => {
            "Request exact line ranges when editing, and re-read the target \
             region before proposing a change."
        }
    };
    PromptCounterweight { id, text }
}

fn throttle_plan(class: TaskClass, axes: Option<&BehaviorAxes>) -> ThrottlePlan {
    // Base two-tier budgets keyed on task class (SS33.3 scaling rules +
    // Anthropic orchestration caps MAX_CONCURRENT 4 / MAX_TOTAL_SUBTASKS
    // 200 for complex work).
    let mut plan = match class {
        TaskClass::FactFind => ThrottlePlan {
            soft_call_budget: 10,
            hard_call_budget: 15,
            max_concurrent_subagents: 1,
            max_total_subtasks: 3,
            max_depth: 1,
        },
        TaskClass::Comparison => ThrottlePlan {
            soft_call_budget: 15,
            hard_call_budget: 25,
            max_concurrent_subagents: 4,
            max_total_subtasks: 15,
            max_depth: 1,
        },
        TaskClass::Complex => ThrottlePlan {
            soft_call_budget: 40,
            hard_call_budget: 60,
            max_concurrent_subagents: 4,
            max_total_subtasks: 200,
            max_depth: 1,
        },
    };
    // Observed over-delegation tightens the STRUCTURAL caps (A6): never a
    // prompt plea for what a cap can bound.
    if axes.is_some_and(|a| a.delegation_ratio > OVER_DELEGATION_RATIO) {
        plan.max_concurrent_subagents = plan.max_concurrent_subagents.min(2);
        plan.max_total_subtasks /= 2;
    }
    plan
}
