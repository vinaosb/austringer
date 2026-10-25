//! Stage (a) — the AST gate: new-error-node semantics, the no-grammar
//! balanced-delimiter fallback, and UTF-8/CRLF/unicode handling.

mod common;

use austringer_exec::preflight::{
    AttemptOutcome, Outcome, PreFlight, PreFlightReport, SkipReason, Stage, StageAttempt,
    SurfaceAfterTwo,
};
use common::preflightkit as kit;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_with(extra: &[(&str, &[u8])]) -> (TempDir, TempDir, austringer_vfs::ShadowStore) {
    let target = TempDir::new().expect("target tempdir");
    let store_root = TempDir::new().expect("store tempdir");
    let store = kit::store(store_root.path());
    kit::cargo_tree(target.path(), b"fn main() {}\n");
    for (rel, bytes) in extra {
        kit::write(&target.path().join(rel), bytes);
    }
    (target, store_root, store)
}

fn gate<'a>(
    target: &TempDir,
    store: &'a austringer_vfs::ShadowStore,
    edits: &[austringer_vfs::ShadowEdit],
    touched: &[&str],
) -> (austringer_vfs::ShadowHandle<'a>, PreFlightReport) {
    let mut shadow = store.create_shadow(target.path()).expect("shadow");
    shadow.apply(edits).expect("apply edits");
    let touched: Vec<PathBuf> = touched.iter().map(PathBuf::from).collect();
    let runner = kit::ScriptedRunner::new(vec![]);
    let rig = kit::Rig::new();
    let seam: &mut dyn austringer_exec::preflight::FixerSeam = &mut SurfaceAfterTwo;
    let report = PreFlight::new(&rig.deps(&runner), &mut *seam)
        .run(&mut shadow, &touched)
        .expect("preflight runs");
    (shadow, report)
}

fn first_stage(report: &PreFlightReport) -> (usize, Stage, AttemptOutcome) {
    let StageAttempt {
        iteration,
        stage,
        outcome,
    } = &report.trace[0];
    (*iteration, *stage, *outcome)
}

#[test]
fn clean_edit_passes_the_gate() {
    let (target, store_root, store) = setup_with(&[]);
    let good: &[u8] = b"fn helper() -> u32 {\n    42\n}\n";
    let current = kit::read(&target.path().join("src/main.rs"));
    let (shadow, report) = gate(
        &target,
        &store,
        &[kit::replace_edit("src/main.rs", good, &current)],
        &["src/main.rs"],
    );
    assert!(matches!(report.outcome, Outcome::Passed));
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn new_missing_node_in_previously_clean_file_fails() {
    let (target, store_root, store) = setup_with(&[]);
    let broken: &[u8] = b"fn main() {\n";
    let current = kit::read(&target.path().join("src/main.rs"));
    let (shadow, report) = gate(
        &target,
        &store,
        &[kit::replace_edit("src/main.rs", broken, &current)],
        &["src/main.rs"],
    );
    let Outcome::Failed(ref failure) = report.outcome else {
        panic!("must fail");
    };
    assert_eq!(
        first_stage(&report),
        (0, Stage::AstGate, AttemptOutcome::Failed)
    );
    assert_eq!(failure.primary.tool, "ast-gate");
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn pre_existing_errors_do_not_fail_when_unchanged() {
    // The user's file is ALREADY broken at line 1; the edit appends a
    // comment at the end. Same diagnostics before and after -> no NEW
    // error nodes -> the gate passes (pre-existing breakage is the
    // baseline, not something pre-flight re-litigates).
    let (target, store_root, store) = setup_with(&[]);
    let pre_broken: &[u8] = b"fn main() {\n";
    kit::write(&target.path().join("src/main.rs"), pre_broken);
    let appended: &[u8] = b"fn main() {\n// touched below the error\n";
    let (shadow, report) = gate(
        &target,
        &store,
        &[kit::replace_edit("src/main.rs", appended, pre_broken)],
        &["src/main.rs"],
    );
    assert_eq!(
        first_stage(&report),
        (0, Stage::AstGate, AttemptOutcome::Passed),
        "unchanged baseline diagnostics are not new"
    );
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn adding_errors_on_top_of_pre_existing_ones_fails_with_only_the_new_one() {
    let (target, store_root, store) = setup_with(&[]);
    let pre_broken: &[u8] = b"fn main() {\n";
    kit::write(&target.path().join("src/main.rs"), pre_broken);
    let worse: &[u8] = b"fn main() {\nfn other( {\n}\n";
    let (shadow, report) = gate(
        &target,
        &store,
        &[kit::replace_edit("src/main.rs", worse, pre_broken)],
        &["src/main.rs"],
    );
    let Outcome::Failed(ref failure) = report.outcome else {
        panic!("a genuinely new error must fail");
    };
    // Only the NEW diagnostics surface; the baseline missing-"}" at line 1
    // is excluded from the structured errors.
    let all = std::iter::once(&failure.primary).chain(failure.rest.iter());
    for error in all {
        assert_eq!(error.path, Some(PathBuf::from("src/main.rs")));
        if error.line == Some(1) {
            panic!("baseline (pre-existing) diagnostic must not surface: {error:?}");
        }
    }
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn created_file_with_syntax_error_fails() {
    let (target, store_root, store) = setup_with(&[]);
    let edit = austringer_vfs::ShadowEdit::create(
        PathBuf::from("src/lib.rs"),
        b"pub fn broken( {\n".to_vec(),
    );
    let (shadow, report) = gate(&target, &store, &[edit], &["src/lib.rs"]);
    assert_eq!(
        first_stage(&report),
        (0, Stage::AstGate, AttemptOutcome::Failed),
        "a created file has an empty baseline: every error is new"
    );
    drop(shadow);
    drop(store);
    drop(store_root);
}

// --- No-grammar fallback: deterministic balanced-delimiter heuristic ---

#[test]
fn no_grammar_unbalanced_new_delimiter_fails_via_heuristic() {
    let (target, store_root, store) = setup_with(&[]);
    let edit = austringer_vfs::ShadowEdit::create(
        PathBuf::from("notes/help.txt"),
        b"call me ( maybe\n".to_vec(),
    );
    let (shadow, report) = gate(&target, &store, &[edit], &["notes/help.txt"]);
    let Outcome::Failed(ref failure) = report.outcome else {
        panic!("new unbalanced delimiter must fail the fallback check");
    };
    assert_eq!(failure.primary.tool, "delimiter-heuristic");
    assert_eq!(failure.primary.path, Some(PathBuf::from("notes/help.txt")));
    assert!(
        failure.primary.message.contains("unbalanced"),
        "{:?}",
        failure.primary
    );
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn no_grammar_pre_unbalanced_stays_passing() {
    // Baseline already unbalanced: appending text that keeps it
    // unbalanced introduces nothing NEW.
    let (target, store_root, store) = setup_with(&[("notes/a.txt", b"oops [ \n")]);
    let worse_same: &[u8] = b"oops [ \nmore text\n";
    let (shadow, report) = gate(
        &target,
        &store,
        &[kit::replace_edit("notes/a.txt", worse_same, b"oops [ \n")],
        &["notes/a.txt"],
    );
    assert_eq!(
        first_stage(&report),
        (0, Stage::AstGate, AttemptOutcome::Passed)
    );
    assert!(
        report.skips.iter().any(|s| s.stage == Stage::AstGate
            && matches!(s.reason, SkipReason::NoGrammarFallback)),
        "fallback usage must be disclosed as a notice: {:?}",
        report.skips
    );
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn no_grammar_balanced_content_passes() {
    let (target, store_root, store) = setup_with(&[]);
    let edit = austringer_vfs::ShadowEdit::create(
        PathBuf::from("notes/ok.txt"),
        b"balanced ( yes ) [ ok ] { fine }\n".to_vec(),
    );
    let (shadow, report) = gate(&target, &store, &[edit], &["notes/ok.txt"]);
    assert!(matches!(report.outcome, Outcome::Passed));
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn non_utf8_no_grammar_file_is_a_typed_skip_notice_never_a_failure() {
    let (target, store_root, store) = setup_with(&[]);
    let edit = austringer_vfs::ShadowEdit::create(
        PathBuf::from("assets/blob.bin"),
        vec![0xFF, 0xFE, 0x00, 0x01, 0xF0, 0x9F],
    );
    let (shadow, report) = gate(&target, &store, &[edit], &["assets/blob.bin"]);
    assert!(
        matches!(report.outcome, Outcome::Passed),
        "binary content cannot be judged: typed skip, not failure"
    );
    assert!(
        report
            .skips
            .iter()
            .any(|s| matches!(s.reason, SkipReason::NotText))
    );
    drop(shadow);
    drop(store);
    drop(store_root);
}
