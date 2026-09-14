//! Model profiles: build-from-transcript, neutral fallback on insufficient
//! data, per-model tips rendering (the todo-34 happy QA: "profile renders
//! per-model tips + correct dialect"), the local profile store, and the
//! typed feed into the T30/T31 per-model skill-guidance surface.

use harness_intel::axes::BehaviorAxes;
use harness_intel::catalog::{Catalog, UnknownModel};
use harness_intel::model_profile::{build_profile, render_tips, to_model_guidance};
use harness_intel::store::ProfileStore;
use harness_intel::taxonomy::{FailureClass, TurnObservation};
use harness_providers::event::Usage;
use harness_providers::identity::ModelIdentity;

fn opus() -> ModelIdentity {
    ModelIdentity::new("anthropic", "claude-opus-4-7")
}

/// `count` turns; the failure list is SPREAD one-per-turn over the first
/// turns (so a 2-failure list over 10 turns = 2 failing turns, giving
/// honest sub-1.0 rates).
fn turns(count: usize, failures: &[FailureClass]) -> Vec<TurnObservation> {
    (0..count)
        .map(|i| TurnObservation {
            model: opus(),
            tool_calls: 2,
            edit_calls: 1,
            subagent_spawns: 0,
            thinking_tokens: 0,
            failures: failures.get(i).cloned().into_iter().collect(),
            usage: Usage::new(1000, 400),
        })
        .collect()
}

#[test]
fn profile_renders_per_model_tips_with_dialect_and_traps() {
    // Given: a catalog profile over a transcript with a visible weakness
    // (edit rejections) on a model carrying the budget_tokens 400-trap.
    let catalog = Catalog::seeded();
    let observations = turns(
        10,
        &[FailureClass::EditRejection, FailureClass::EditRejection],
    );

    let profile = build_profile(&catalog, &opus(), &observations).expect("model is in the catalog");

    // When: rendering the per-model tips.
    let tips = render_tips(catalog.entry("claude-opus-4-7").expect("seeded"), &profile);

    // Then: the tips carry the model identity, the EXPLICIT dialect, the
    // 400-trap warning, and the OBSERVED weakness section — content asserted
    // by string, not by return code.
    assert!(tips.contains("claude-opus-4-7"));
    assert!(tips.contains("dialect: anthropic"));
    assert!(tips.contains("budget_tokens"));
    assert!(tips.contains("edit-rejection"));
    assert!(tips.contains("observed"));
    // And the taxonomy the profile was built from is the observed one.
    let taxonomy = profile.taxonomy.expect("sufficient data");
    assert_eq!(taxonomy.edit_rejection_rate, 0.2);
}

#[test]
fn profile_feeds_skill_per_model_guidance_typed() {
    // Given: the same profile.
    let catalog = Catalog::seeded();
    let profile = build_profile(&catalog, &opus(), &turns(10, &[])).expect("seeded model");

    // When: mapping to the T30/T31 per-model guidance vocabulary.
    let guidance = to_model_guidance(catalog.entry("claude-opus-4-7").expect("seeded"), &profile);

    // Then: the typed feed targets exactly this model and carries the tips.
    assert_eq!(guidance.model, "claude-opus-4-7");
    assert_eq!(
        guidance.guidance,
        render_tips(catalog.entry("claude-opus-4-7").expect("seeded"), &profile)
    );
}

#[test]
fn insufficient_data_yields_a_neutral_profile() {
    // Given: three turns — below the documented minimum.
    let catalog = Catalog::seeded();
    let profile = build_profile(&catalog, &opus(), &turns(3, &[FailureClass::SyntaxError]))
        .expect("seeded model");

    // Then: the profile is honestly neutral — no invented taxonomy, no
    // invented axes — and the tips carry no "observed" section.
    assert!(profile.is_neutral());
    assert!(profile.taxonomy.is_none());
    assert!(profile.axes.is_none());
    let tips = render_tips(catalog.entry("claude-opus-4-7").expect("seeded"), &profile);
    assert!(!tips.contains("observed"));
    // The dialect + trap lines still render (they are catalog facts, not
    // observations).
    assert!(tips.contains("dialect: anthropic"));
}

#[test]
fn unknown_model_is_a_typed_error() {
    let catalog = Catalog::seeded();
    let mystery = ModelIdentity::new("anthropic", "mystery-model");
    let err = build_profile(&catalog, &mystery, &turns(10, &[]))
        .expect_err("unknown model must fail closed");
    assert_eq!(
        err,
        UnknownModel {
            model: "mystery-model".to_string()
        }
    );
}

#[test]
fn store_round_trips_and_reload_recomputes() {
    // Given: a profile store in a fresh temp dir.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ProfileStore::new(dir.path().to_path_buf());

    // When: a clean transcript is folded in, then (later) a worse one.
    let first = store
        .update(&opus(), &turns(10, &[]), 1_000)
        .expect("io")
        .expect("sufficient data");
    assert_eq!(first.taxonomy.edit_rejection_rate, 0.0);

    let second = store
        .update(&opus(), &turns(10, &[FailureClass::EditRejection]), 2_000)
        .expect("io")
        .expect("sufficient data");

    // Then: the reloaded record reflects the NEW observation window (the
    // store profiles the window it is handed — no stale state, no eternal
    // average) and the newer clock.
    assert_eq!(second.taxonomy.edit_rejection_rate, 0.1);
    assert_eq!(second.observation_turns, 10);
    assert_eq!(second.updated_unix, 2_000);
    let reloaded = store.get(&opus()).expect("io").expect("stored");
    assert_eq!(reloaded.taxonomy, second.taxonomy);
    assert_eq!(reloaded.updated_unix, 2_000);
}

#[test]
fn thin_data_update_writes_nothing() {
    // Given: a store and a below-minimum transcript.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ProfileStore::new(dir.path().to_path_buf());

    // When: updating with 3 turns.
    let outcome = store.update(&opus(), &turns(3, &[]), 1_000).expect("io");

    // Then: nothing was persisted — no thin-data record exists to mislead a
    // later reader.
    assert!(outcome.is_none());
    assert!(store.get(&opus()).expect("io").is_none());
}

#[test]
fn malformed_store_file_is_a_typed_corrupt_error() {
    // Given: a hand-corrupted store file at the exact path the store owns.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ProfileStore::new(dir.path().to_path_buf());
    let path = dir.path().join("anthropic.claude-opus-4-7.json");
    std::fs::write(&path, b"{ not json").expect("write");

    // When / Then: reading is a typed Corrupt error naming the file.
    let err = store.get(&opus()).expect_err("corrupt file must not pass");
    let rendered = err.to_string();
    assert!(rendered.contains("corrupt"), "error was: {rendered}");
    assert!(
        rendered.contains("claude-opus-4-7"),
        "error was: {rendered}"
    );
}

#[test]
fn axes_are_recomputed_not_merged_on_reload() {
    // Given: a stored profile with high verbosity, then new low-verbosity
    // observations.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ProfileStore::new(dir.path().to_path_buf());
    let verbose: Vec<TurnObservation> = (0..10)
        .map(|_| TurnObservation {
            model: opus(),
            tool_calls: 1,
            edit_calls: 0,
            subagent_spawns: 0,
            thinking_tokens: 0,
            failures: vec![],
            usage: Usage::new(1000, 5000),
        })
        .collect();
    store
        .update(&opus(), &verbose, 1_000)
        .expect("io")
        .expect("stored");

    let quiet: Vec<TurnObservation> = (0..10)
        .map(|_| TurnObservation {
            model: opus(),
            tool_calls: 1,
            edit_calls: 0,
            subagent_spawns: 0,
            thinking_tokens: 0,
            failures: vec![],
            usage: Usage::new(1000, 100),
        })
        .collect();
    let reloaded = store
        .update(&opus(), &quiet, 2_000)
        .expect("io")
        .expect("stored");

    // Then: the reloaded axes reflect only the new window (5000 -> 100
    // tokens/turn), never an average with stale history.
    let axes: &BehaviorAxes = &reloaded.axes;
    assert_eq!(axes.verbosity_tokens_per_turn, 100.0);
}
