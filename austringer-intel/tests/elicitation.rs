//! The SET/PROMPT/THROTTLE elicitation constitution (research SS33.6):
//! SET the deterministic knobs explicitly (never trust provider defaults),
//! PROMPT per-model counterweights gated on OUR observed data, THROTTLE
//! structurally what neither can bound. Never prompt for what a parameter
//! controls; never parameterize what only structure can bound.

use austringer_intel::axes::BehaviorAxes;
use austringer_intel::catalog::{Catalog, EffortValue};
use austringer_intel::elicitation::{CounterweightId, EffortChannel, TaskClass, elicit};
use austringer_intel::model_profile::ModelProfile;
use austringer_intel::taxonomy::FailureTaxonomy;
use austringer_providers::identity::ModelIdentity;

fn profile_for(model: &str) -> ModelProfile {
    ModelProfile::neutral(ModelIdentity::new("anthropic", model))
}

fn biased_profile(
    taxonomy: FailureTaxonomy,
    axes: BehaviorAxes,
    provider: &str,
    model: &str,
) -> ModelProfile {
    let mut p = ModelProfile::neutral(ModelIdentity::new(provider, model));
    p.taxonomy = Some(taxonomy);
    p.axes = Some(axes);
    p
}

fn zero_taxonomy() -> FailureTaxonomy {
    FailureTaxonomy {
        turns: 50,
        syntax_error_rate: 0.0,
        tool_misuse_rate: 0.0,
        context_overflow_rate: 0.0,
        retry_storm_rate: 0.0,
        edit_rejection_rate: 0.0,
        tokens_per_turn: 1000.0,
    }
}

fn zero_axes() -> BehaviorAxes {
    BehaviorAxes {
        verbosity_tokens_per_turn: 1000.0,
        tool_calls_per_turn: 4.0,
        delegation_ratio: 0.5,
        tool_precision: 1.0,
        edit_competence: 1.0,
        drift_delta: 0.0,
        thinking_tokens_per_turn: None,
    }
}

#[test]
fn set_plan_is_explicit_and_holds_effort_constant_on_cache_coupled_families() {
    // Given: a neutral profile on a cache-coupled model.
    let catalog = Catalog::seeded();
    let entry = catalog.entry("claude-opus-4-7").expect("seeded");
    let profile = profile_for("claude-opus-4-7");

    // When: eliciting for a complex workload.
    let plan = elicit(entry, &profile, TaskClass::Complex);

    // Then: SET carries an EXPLICIT effort (the catalog's documented default
    // for the model — never the provider's implicit default), and the
    // hold-effort-constant cache discipline is on for Anthropic.
    assert_eq!(plan.set.effort, EffortValue::Xhigh);
    assert!(plan.set.hold_effort_constant);
    // And the per-message effort beta exists only where the family offers it
    // (Fable 5.1 / Opus 5); Sonnet-class changes wait for an epoch boundary.
    let fable = catalog.entry("claude-fable-5-1").expect("seeded");
    assert_eq!(
        elicit(fable, &profile_for("claude-fable-5-1"), TaskClass::Complex)
            .set
            .effort_change_channel,
        EffortChannel::PerMessageBeta
    );
    let sonnet = catalog.entry("claude-sonnet-4-6").expect("seeded");
    assert_eq!(
        elicit(
            sonnet,
            &profile_for("claude-sonnet-4-6"),
            TaskClass::Complex
        )
        .set
        .effort_change_channel,
        EffortChannel::EpochBoundary
    );
}

#[test]
fn set_fact_find_clamps_expensive_defaults() {
    // Given: grok-4-6 — reasoning always-on at default `high`, "the most
    // expensive default of the four; SET explicitly even for cheap calls".
    let catalog = Catalog::seeded();
    let entry = catalog.entry("grok-4-6").expect("seeded");

    // When: eliciting for a fact-find workload.
    let plan = elicit(entry, &profile_for("grok-4-6"), TaskClass::FactFind);

    // Then: the fact-find class clamps the default down to medium.
    assert_eq!(plan.set.effort, EffortValue::Medium);
    // And a complex workload keeps the model's documented default.
    assert_eq!(
        elicit(entry, &profile_for("grok-4-6"), TaskClass::Complex)
            .set
            .effort,
        EffortValue::High
    );
    // Not cache-coupled: effort may move without epoch discipline.
    assert!(!plan.set.hold_effort_constant);
}

#[test]
fn neutral_profile_gets_no_counterweights() {
    // Given: a neutral profile (insufficient observed data).
    let catalog = Catalog::seeded();
    let entry = catalog.entry("claude-opus-4-6").expect("seeded");
    let profile = profile_for("claude-opus-4-6");

    // When / Then: counterweights are gated on OUR evals — a neutral profile
    // ships none. (The research's per-model counterweights exist, but firing
    // them without observed bias is guessing.)
    let plan = elicit(entry, &profile, TaskClass::Complex);
    assert!(plan.prompt.is_empty());
}

#[test]
fn over_exploration_gets_commit_to_approach() {
    // Given: observed over-exploration — tool volume far above the
    // documented threshold (Opus 4.6's documented pathology).
    let catalog = Catalog::seeded();
    let entry = catalog.entry("claude-opus-4-6").expect("seeded");
    let mut axes = zero_axes();
    axes.tool_calls_per_turn = 18.0;
    let profile = biased_profile(zero_taxonomy(), axes, "anthropic", "claude-opus-4-6");

    let plan = elicit(entry, &profile, TaskClass::Complex);

    assert!(
        plan.prompt
            .iter()
            .any(|cw| cw.id == CounterweightId::CommitToApproach)
    );
    // The counterweight carries actual prompt text (asserted, not just typed).
    let cw = plan
        .prompt
        .iter()
        .find(|cw| cw.id == CounterweightId::CommitToApproach)
        .expect("present");
    assert!(cw.text.len() > 40, "counterweight must be real prompt text");
}

#[test]
fn under_parallelizer_gets_batching_and_progress() {
    // Given: an under-parallelizer — zero delegation despite heavy tool use
    // (Fable 5.1's documented parallel-call decay).
    let catalog = Catalog::seeded();
    let entry = catalog.entry("claude-fable-5-1").expect("seeded");
    let mut axes = zero_axes();
    axes.delegation_ratio = 0.0;
    axes.tool_calls_per_turn = 9.0;
    let profile = biased_profile(zero_taxonomy(), axes, "anthropic", "claude-fable-5-1");

    let plan = elicit(entry, &profile, TaskClass::Complex);

    assert!(
        plan.prompt
            .iter()
            .any(|cw| cw.id == CounterweightId::BatchingAndProgress)
    );
    // And NOT the commit-to-approach counterweight: tool volume is below the
    // over-exploration threshold, so the gate must not fire it too.
    assert!(
        !plan
            .prompt
            .iter()
            .any(|cw| cw.id == CounterweightId::CommitToApproach)
    );
}

#[test]
fn retry_storms_get_stopping_conditions_and_retrieval_budget() {
    // Given: GPT-5.5's documented overthinking shape — retry storms above
    // the threshold.
    let catalog = Catalog::seeded();
    let entry = catalog.entry("gpt-5-5").expect("seeded");
    let mut taxonomy = zero_taxonomy();
    taxonomy.retry_storm_rate = 0.3;
    let profile = biased_profile(taxonomy, zero_axes(), "openai", "gpt-5-5");

    let plan = elicit(entry, &profile, TaskClass::Complex);

    let ids: Vec<CounterweightId> = plan.prompt.iter().map(|cw| cw.id).collect();
    assert!(ids.contains(&CounterweightId::StoppingConditions));
    assert!(ids.contains(&CounterweightId::RetrievalBudget));
}

#[test]
fn terse_output_gets_persistence_and_edit_rejections_get_exact_ranges() {
    // Given: a concise-to-a-fault model AND a weak editor in the same
    // profile (GPT-5.1 persistence + aider's GPT-4 Turbo elision class).
    let catalog = Catalog::seeded();
    let entry = catalog.entry("gpt-5-5").expect("seeded");
    let mut axes = zero_axes();
    axes.verbosity_tokens_per_turn = 180.0;
    let mut taxonomy = zero_taxonomy();
    taxonomy.edit_rejection_rate = 0.4;
    let profile = biased_profile(taxonomy, axes, "openai", "gpt-5-5");

    let plan = elicit(entry, &profile, TaskClass::Comparison);

    let ids: Vec<CounterweightId> = plan.prompt.iter().map(|cw| cw.id).collect();
    assert!(ids.contains(&CounterweightId::Persistence));
    assert!(ids.contains(&CounterweightId::ExactRanges));
}

#[test]
fn throttle_budgets_key_on_task_class_with_split_subagent_caps() {
    // Given: neutral profiles across the three task classes (SS33.3
    // scaling rules: fact-find ~3-10 calls, comparisons 10-15, complex
    // watchdogged with MAX_CONCURRENT 4 / MAX_TOTAL_SUBTASKS 200).
    let catalog = Catalog::seeded();
    let entry = catalog.entry("claude-opus-5").expect("seeded");
    let profile = profile_for("claude-opus-5");

    let fact_find = elicit(entry, &profile, TaskClass::FactFind);
    let comparison = elicit(entry, &profile, TaskClass::Comparison);
    let complex = elicit(entry, &profile, TaskClass::Complex);

    // Then: soft budget injected in context < hard budget denying calls.
    assert!(fact_find.throttle.soft_call_budget < fact_find.throttle.hard_call_budget);
    assert!(comparison.throttle.soft_call_budget > fact_find.throttle.soft_call_budget);
    assert_eq!(complex.throttle.soft_call_budget, 40);
    assert_eq!(complex.throttle.hard_call_budget, 60);
    assert_eq!(complex.throttle.max_concurrent_subagents, 4);
    assert_eq!(complex.throttle.max_total_subtasks, 200);
    // Depth is pinned to 1 everywhere (split caps: concurrent x total x depth).
    assert_eq!(fact_find.throttle.max_depth, 1);
    assert_eq!(comparison.throttle.max_depth, 1);
    assert_eq!(complex.throttle.max_depth, 1);
}

#[test]
fn over_delegator_gets_tighter_structural_caps() {
    // Given: A6 subagent-spawn overdrive observed (delegation ratio far
    // above 1 spawn per turn).
    let catalog = Catalog::seeded();
    let entry = catalog.entry("claude-opus-4-6").expect("seeded");
    let mut axes = zero_axes();
    axes.delegation_ratio = 3.0;
    let profile = biased_profile(zero_taxonomy(), axes, "anthropic", "claude-opus-4-6");

    // When: throttling a complex workload.
    let plan = elicit(entry, &profile, TaskClass::Complex);

    // Then: the structural cap tightens — THROTTLE, not a prompt plea.
    assert_eq!(plan.throttle.max_concurrent_subagents, 2);
    assert!(plan.throttle.max_total_subtasks < 200);
}
