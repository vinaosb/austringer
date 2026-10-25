//! Plan todo 34 acceptance: synthetic transcripts produce correct failure
//! taxonomies. Every rate is asserted against a hand-computed expectation
//! written out in the test body — never a recomputed projection of the
//! output under test.

use austringer_intel::taxonomy::{
    FailureClass, MIN_OBSERVED_TURNS, TurnObservation, compute_taxonomy,
};
use austringer_providers::event::Usage;
use austringer_providers::identity::ModelIdentity;

fn model(name: &str) -> ModelIdentity {
    ModelIdentity::new("anthropic", name)
}

/// A turn with `failures` and the given activity counts, at a flat usage.
fn turn(
    target: &ModelIdentity,
    tool_calls: u32,
    edit_calls: u32,
    failures: &[FailureClass],
) -> TurnObservation {
    TurnObservation {
        model: target.clone(),
        tool_calls,
        edit_calls,
        subagent_spawns: 0,
        thinking_tokens: 0,
        failures: failures.to_vec(),
        usage: Usage::new(1000, 500),
    }
}

#[test]
fn synthetic_transcript_produces_exact_rates() {
    // Given: MIN_OBSERVED_TURNS (10) turns for one model. Totals across the
    // transcript: 20 tool calls, 10 edit calls; failures: 2 syntax, 4
    // misuse, 3 edit rejections, 1 context overflow, 1 retry storm.
    let target = model("claude-opus-4-7");
    let others = model("claude-sonnet-4-6");
    let transcript = vec![
        turn(
            &target,
            2,
            1,
            &[FailureClass::SyntaxError, FailureClass::SyntaxError],
        ),
        turn(&target, 2, 1, &[FailureClass::ToolMisuse; 2]),
        turn(
            &target,
            2,
            1,
            &[FailureClass::EditRejection, FailureClass::EditRejection],
        ),
        turn(&target, 2, 1, &[FailureClass::ContextOverflow]),
        turn(&target, 2, 1, &[FailureClass::RetryStorm]),
        turn(&target, 2, 1, &[FailureClass::EditRejection]),
        turn(&target, 2, 1, &[FailureClass::ToolMisuse; 2]),
        turn(&target, 2, 1, &[]),
        turn(&target, 2, 1, &[]),
        turn(&target, 2, 1, &[]),
        // Decoys from a different model: must not contaminate the rates.
        turn(&others, 5, 5, &[FailureClass::SyntaxError; 5]),
    ];

    // When: the taxonomy is computed for the target model.
    let taxonomy =
        compute_taxonomy(&transcript, &target).expect("10 observed turns is sufficient data");

    // Then: hand-computed expectations —
    assert_eq!(taxonomy.turns, 10);
    // syntax-error rate = 2 failures / 10 turns
    assert_eq!(taxonomy.syntax_error_rate, 0.2);
    // tool-misuse rate = 4 failures / 20 tool calls
    assert_eq!(taxonomy.tool_misuse_rate, 0.2);
    // edit-rejection rate = 3 rejections / 10 edit calls
    assert_eq!(taxonomy.edit_rejection_rate, 0.3);
    // context-overflow rate = 1 / 10 turns
    assert_eq!(taxonomy.context_overflow_rate, 0.1);
    // retry-storm rate = 1 / 10 turns
    assert_eq!(taxonomy.retry_storm_rate, 0.1);
    // token cost per turn = (1000 input + 500 output) flat usage
    assert_eq!(taxonomy.tokens_per_turn, 1500.0);
}

#[test]
fn observations_from_other_models_are_excluded() {
    // Given: 10 clean turns for model A and 10 failure-heavy turns for B.
    let a = model("claude-opus-4-7");
    let b = model("claude-sonnet-4-6");
    let mut transcript: Vec<TurnObservation> = (0..10).map(|_| turn(&a, 1, 0, &[])).collect();
    transcript.extend((0..10).map(|_| turn(&b, 1, 0, &[FailureClass::SyntaxError])));

    // When / Then: A's taxonomy sees none of B's failures.
    let taxonomy = compute_taxonomy(&transcript, &a).expect("sufficient");
    assert_eq!(taxonomy.syntax_error_rate, 0.0);
    assert_eq!(taxonomy.turns, 10);
}

#[test]
fn insufficient_data_returns_none() {
    // Given: fewer than MIN_OBSERVED_TURNS observed turns.
    let target = model("claude-opus-4-7");
    let transcript = vec![
        turn(&target, 1, 1, &[FailureClass::SyntaxError]),
        turn(&target, 1, 1, &[]),
        turn(&target, 1, 1, &[]),
    ];

    // When / Then: no taxonomy is invented from thin data; the caller falls
    // back to a neutral profile (see tests/profile.rs).
    assert!(compute_taxonomy(&transcript, &target).is_none());
    assert_eq!(transcript.len() as u32, MIN_OBSERVED_TURNS - 7);
}

#[test]
fn zero_denominators_report_zero_not_nan() {
    // Given: sufficient turns but zero tool calls and zero edit calls.
    let target = model("claude-opus-4-7");
    let transcript: Vec<TurnObservation> = (0..MIN_OBSERVED_TURNS)
        .map(|_| turn(&target, 0, 0, &[]))
        .collect();

    // When / Then: rates are honest zeros, never NaN.
    let taxonomy = compute_taxonomy(&transcript, &target).expect("sufficient turns");
    assert_eq!(taxonomy.tool_misuse_rate, 0.0);
    assert_eq!(taxonomy.edit_rejection_rate, 0.0);
    assert!(taxonomy.tool_misuse_rate.is_finite());
    assert!(taxonomy.edit_rejection_rate.is_finite());
}

#[test]
fn axes_cover_the_behavior_vocabulary() {
    // Given: the behavioral-axes companion computed over the same synthetic
    // transcript (draft amendment T): verbosity, delegation, precision,
    // edit competence, drift, thinking depth, tool volume.
    use austringer_intel::axes::compute_axes;

    let target = model("claude-opus-4-7");
    let clean = turn(&target, 3, 1, &[]);
    let mut transcript: Vec<TurnObservation> = (0..MIN_OBSERVED_TURNS)
        .map(|i| {
            let mut t = clean.clone();
            // 2 of the 10 turns spawn a subagent and report reasoning tokens.
            t.subagent_spawns = u32::from(i % 5 == 0);
            t.thinking_tokens = u64::from(i % 5 == 0) * 900;
            t
        })
        .collect();
    // Final turn fails late: the drift curve must see it in the late half.
    transcript
        .last_mut()
        .expect("non-empty")
        .failures
        .push(FailureClass::EditRejection);

    let axes = compute_axes(&transcript, &target).expect("sufficient data");

    // Then: hand-computed expectations —
    // verbosity = 500 output tokens/turn flat
    assert_eq!(axes.verbosity_tokens_per_turn, 500.0);
    // delegation ratio = 2 spawns / 10 turns
    assert_eq!(axes.delegation_ratio, 0.2);
    // tool volume = 30 calls / 10 turns
    assert_eq!(axes.tool_calls_per_turn, 3.0);
    // precision = 1 - 0 misuse; edit competence = 1 - 1/10 rejections
    assert_eq!(axes.tool_precision, 1.0);
    assert_eq!(axes.edit_competence, 0.9);
    // drift: late-half failure rate (1/5) minus early-half rate (0/5)
    assert_eq!(axes.drift_delta, 0.2);
    // thinking depth: mean over the 2 turns that reported reasoning tokens
    assert_eq!(axes.thinking_tokens_per_turn, Some(900.0));
}
