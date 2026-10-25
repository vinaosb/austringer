//! Autonomy-matrix tests (todo 8): four modes as level presets over ten
//! capability toggle columns, review policy defaults, and the shared handle.

mod common;

use austringer_core::autonomy::{
    AutonomyHandle, AutonomyMatrix, AutonomyMode, Capability, Fallback, ReviewPolicy,
};

#[test]
fn ten_capability_columns_exist_and_are_distinct() {
    assert_eq!(Capability::ALL.len(), 10);
    let mut sorted = Capability::ALL.to_vec();
    sorted.sort_by_key(|c| format!("{c:?}"));
    sorted.dedup_by_key(|c| format!("{c:?}"));
    assert_eq!(sorted.len(), 10);
}

#[test]
fn chat_preset_is_the_headless_default_and_asks_for_everything() {
    let matrix = AutonomyMatrix::default();
    assert_eq!(matrix.mode(), AutonomyMode::Chat);
    assert_eq!(matrix, AutonomyMatrix::for_mode(AutonomyMode::Chat));
    // Everything asks except completion reporting (metadata, never gated).
    for capability in Capability::ALL {
        let expected = capability == Capability::Completion;
        assert_eq!(
            matrix.allows(capability),
            expected,
            "chat preset wrong for {capability:?}"
        );
    }
    assert_eq!(matrix.fallback(), Fallback::Ask);
    assert_eq!(matrix.review(), ReviewPolicy::RequestReview);
}

#[test]
fn approve_preset_auto_reads_but_asks_for_writes() {
    let matrix = AutonomyMatrix::for_mode(AutonomyMode::Approve);
    for on in [
        Capability::Read,
        Capability::Context,
        Capability::Search,
        Capability::Completion,
        Capability::Compact,
        Capability::PairFold,
        Capability::GrantMemory,
    ] {
        assert!(matrix.allows(on), "approve should allow {on:?}");
    }
    for off in [Capability::Edit, Capability::Exec, Capability::SecretRead] {
        assert!(!matrix.allows(off), "approve should ask for {off:?}");
    }
    assert_eq!(matrix.fallback(), Fallback::Ask);
}

#[test]
fn smart_approve_routes_writes_through_the_classifier() {
    let matrix = AutonomyMatrix::for_mode(AutonomyMode::SmartApprove);
    assert!(matrix.allows(Capability::Read));
    assert!(!matrix.allows(Capability::Edit));
    assert!(!matrix.allows(Capability::Exec));
    assert!(!matrix.allows(Capability::SecretRead));
    assert_eq!(matrix.fallback(), Fallback::Classify);
    assert_eq!(matrix.review(), ReviewPolicy::RequestReview);
}

#[test]
fn auto_preset_auto_edits_but_still_classifies_every_command() {
    let matrix = AutonomyMatrix::for_mode(AutonomyMode::Auto);
    assert!(matrix.allows(Capability::Edit));
    assert!(
        !matrix.allows(Capability::Exec),
        "commands go to the classifier"
    );
    assert!(!matrix.allows(Capability::SecretRead));
    assert_eq!(matrix.fallback(), Fallback::Classify);
    assert_eq!(matrix.review(), ReviewPolicy::AlwaysProceed);
}

#[test]
fn toggles_flip_individual_capabilities_independently() {
    let mut matrix = AutonomyMatrix::for_mode(AutonomyMode::Approve);
    assert!(!matrix.allows(Capability::Edit));
    matrix.toggle(Capability::Edit, true);
    // Only Edit moved; its siblings are untouched.
    assert!(matrix.allows(Capability::Edit));
    assert!(!matrix.allows(Capability::Exec));
    assert!(matrix.allows(Capability::Read));
    matrix.toggle(Capability::Read, false);
    assert!(!matrix.allows(Capability::Read));
    assert!(matrix.allows(Capability::Edit));
    // A different matrix built from the same preset is unaffected.
    let fresh = AutonomyMatrix::for_mode(AutonomyMode::Approve);
    assert!(!fresh.allows(Capability::Edit));
    assert!(fresh.allows(Capability::Read));
}

#[test]
fn set_mode_resets_to_preset_and_plan_is_an_independent_flag() {
    let mut matrix = AutonomyMatrix::for_mode(AutonomyMode::Chat);
    matrix.toggle(Capability::Exec, true);
    matrix.set_mode(AutonomyMode::SmartApprove);
    assert_eq!(matrix.mode(), AutonomyMode::SmartApprove);
    assert!(
        !matrix.allows(Capability::Exec),
        "preset wins after set_mode"
    );
    assert!(!matrix.is_plan());
    matrix.enter_plan();
    assert!(matrix.is_plan());
    // Switching to a mode clears the plan pairing.
    matrix.set_mode(AutonomyMode::Auto);
    assert!(!matrix.is_plan());
    // Review is a first-class dimension, independently overridable.
    matrix.set_review(ReviewPolicy::AlwaysProceed);
    assert_eq!(matrix.review(), ReviewPolicy::AlwaysProceed);
}

#[test]
fn modes_parse_from_slash_words() {
    for (word, mode) in [
        ("auto", AutonomyMode::Auto),
        ("approve", AutonomyMode::Approve),
        ("chat", AutonomyMode::Chat),
        ("smartapprove", AutonomyMode::SmartApprove),
        ("smart-approve", AutonomyMode::SmartApprove),
    ] {
        assert_eq!(AutonomyMode::parse(word), Some(mode));
    }
    assert_eq!(AutonomyMode::parse("yolo"), None);
    assert_eq!(AutonomyMode::ALL.len(), 4);
}

#[test]
fn handle_shares_one_matrix_across_clones() {
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::Chat));
    let clone = handle.shared();
    handle.write(|matrix| matrix.toggle(Capability::Exec, true));
    assert!(clone.read(|matrix| matrix.allows(Capability::Exec)));
    assert_eq!(clone.read(|matrix| matrix.mode()), AutonomyMode::Chat);
}
