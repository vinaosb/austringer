//! Pre-flight pipeline semantics (plan todo 22): stage order, the closed
//! corrective sub-cycle (max 2), the single volatile error slot, and
//! cancellation — all against the scripted runner.

mod common;

use austringer_exec::preflight::{
    AttemptOutcome, MAX_FIX_ITERATIONS, Outcome, PreFlight, PreFlightReport, Stage, StageAttempt,
    SurfaceAfterTwo,
};
use common::preflightkit as kit;
use std::path::PathBuf;
use tempfile::TempDir;

const GOOD_MAIN: &[u8] = b"fn main() {\n    println!(\"ok\");\n}\n";
const BROKEN_MAIN: &[u8] = b"fn main() {\n    let x = 1;\n";
const BROKEN_AGAIN: &[u8] = b"fn main() {\n    let y = 2;\n";

fn setup(main: &[u8]) -> (TempDir, TempDir, austringer_vfs::ShadowStore) {
    let target = TempDir::new().expect("target tempdir");
    let store_root = TempDir::new().expect("store tempdir");
    let store = kit::store(store_root.path());
    kit::cargo_tree(target.path(), main);
    (target, store_root, store)
}

fn shadow_with_broken_main<'a>(
    store: &'a austringer_vfs::ShadowStore,
    target: &TempDir,
    broken: &[u8],
) -> austringer_vfs::ShadowHandle<'a> {
    let mut shadow = store.create_shadow(target.path()).expect("shadow");
    let current = kit::read(&target.path().join("src/main.rs"));
    let edit = kit::replace_edit("src/main.rs", broken, &current);
    shadow.apply(&[edit]).expect("apply broken edit");
    shadow
}

fn touches() -> Vec<PathBuf> {
    vec![PathBuf::from("src/main.rs")]
}

fn trace_tuples(report: &PreFlightReport) -> Vec<(usize, Stage, AttemptOutcome)> {
    report
        .trace
        .iter()
        .map(
            |StageAttempt {
                 iteration,
                 stage,
                 outcome,
             }| (*iteration, *stage, *outcome),
        )
        .collect()
}

// --- Acceptance: the (a) gate catches syntax errors before any tool runs ---

#[test]
fn syntax_error_is_caught_at_ast_gate_and_never_reaches_formatter_or_compiler() {
    let (target, _store_root, store) = setup(GOOD_MAIN);
    let mut shadow = shadow_with_broken_main(&store, &target, BROKEN_MAIN);
    let runner = kit::ScriptedRunner::new(vec![]);
    let rig = kit::Rig::new();

    let report = PreFlight::new(&rig.deps(&runner), &mut SurfaceAfterTwo)
        .run(&mut shadow, &touches())
        .expect("preflight runs");

    let Outcome::Failed(failure) = &report.outcome else {
        panic!("broken syntax must fail pre-flight");
    };
    assert_eq!(failure.stage, Stage::AstGate);
    assert_eq!(failure.primary.tool, "ast-gate");
    assert_eq!(failure.primary.path, Some(PathBuf::from("src/main.rs")));
    assert!(
        failure.primary.line.is_some(),
        "diagnostic must be position-exact"
    );
    // The acceptance core: exactly one stage reached, zero subprocesses.
    assert_eq!(
        trace_tuples(&report),
        vec![(0, Stage::AstGate, AttemptOutcome::Failed)]
    );
    assert!(
        runner.recorded().is_empty(),
        "no formatter/compiler command may run when the AST gate fails: {:?}",
        runner.recorded()
    );
    // The user tree was never touched.
    assert_eq!(
        kit::read(&target.path().join("src/main.rs")),
        GOOD_MAIN,
        "failed pre-flight leaves the user tree untouched"
    );
}

// --- Happy QA channel: 2-iteration fix loop, then success ---

#[test]
fn corrective_cycle_fixes_on_second_iteration_then_all_gates_pass() {
    let (target, _store_root, store) = setup(GOOD_MAIN);
    let mut shadow = shadow_with_broken_main(&store, &target, BROKEN_MAIN);
    // Fix 1: a different broken content (hash anchored on current shadow
    // bytes). Fix 2: the corrected file (anchored on fix 1's bytes).
    let mut fixer = kit::ScriptedFixer::new(vec![
        vec![kit::replace_edit("src/main.rs", BROKEN_AGAIN, BROKEN_MAIN)],
        vec![kit::replace_edit("src/main.rs", GOOD_MAIN, BROKEN_AGAIN)],
    ]);
    // Iteration 2 reaches the tool stages: rustfmt probe + check, cargo
    // probe + check (only adapters claiming .rs files run).
    let runner = kit::ScriptedRunner::new(vec![
        kit::ok("rustfmt 1.9.0"),
        kit::ok(""),
        kit::ok("cargo 1.98.0"),
        kit::ok("{\"reason\":\"compiler-artifact\",\"target\":{\"name\":\"demo\"}}\n"),
    ]);
    let rig = kit::Rig::new();

    let report = PreFlight::new(&rig.deps(&runner), &mut fixer)
        .run(&mut shadow, &touches())
        .expect("preflight runs");

    assert!(
        matches!(report.outcome, Outcome::Passed),
        "gates must pass after the second fix: {:?}",
        report.outcome
    );
    assert_eq!(report.fix_iterations, 2);
    // Stage sequence: ast fails twice, then the full gate order passes.
    assert_eq!(
        trace_tuples(&report),
        vec![
            (0, Stage::AstGate, AttemptOutcome::Failed),
            (1, Stage::AstGate, AttemptOutcome::Failed),
            (2, Stage::AstGate, AttemptOutcome::Passed),
            (2, Stage::Format, AttemptOutcome::Passed),
            (2, Stage::Check, AttemptOutcome::Passed),
        ]
    );
    // Only the claiming tools ran, probe-then-check each.
    let programs = runner.programs();
    assert_eq!(programs, vec!["cargo", "cargo", "cargo", "cargo"]);
    // Commit-ready: the shadow materializes cleanly into the user tree.
    let stats = shadow.commit().expect("commit after green pre-flight");
    assert_eq!(stats.written, 1);
    assert_eq!(kit::read(&target.path().join("src/main.rs")), GOOD_MAIN);
}

// --- Failure QA channel: persists after 2 iterations -> typed surfacing ---

#[test]
fn failure_persisting_after_two_iterations_surfaces_typed_pre_flight_failure() {
    let (target, _store_root, store) = setup(GOOD_MAIN);
    let mut shadow = shadow_with_broken_main(&store, &target, BROKEN_MAIN);
    let mut fixer = kit::ScriptedFixer::new(vec![
        vec![kit::replace_edit("src/main.rs", BROKEN_AGAIN, BROKEN_MAIN)],
        vec![kit::replace_edit("src/main.rs", BROKEN_MAIN, BROKEN_AGAIN)],
    ]);
    let runner = kit::ScriptedRunner::new(vec![]);
    let rig = kit::Rig::new();

    let report = PreFlight::new(&rig.deps(&runner), &mut fixer)
        .run(&mut shadow, &touches())
        .expect("preflight runs");

    let Outcome::Failed(failure) = &report.outcome else {
        panic!("persistent failure must surface");
    };
    assert_eq!(report.fix_iterations, MAX_FIX_ITERATIONS);
    assert_eq!(failure.stage, Stage::AstGate);
    assert_eq!(
        failure.produced_at_iteration, 2,
        "the LAST failure surfaces"
    );
    assert!(
        failure.primary.message.len() < 200,
        "surfaced signal is compact, never a dump"
    );
    assert_eq!(
        trace_tuples(&report),
        vec![
            (0, Stage::AstGate, AttemptOutcome::Failed),
            (1, Stage::AstGate, AttemptOutcome::Failed),
            (2, Stage::AstGate, AttemptOutcome::Failed),
        ]
    );
    // User tree still untouched: the shadow is disposable, never committed.
    assert_eq!(kit::read(&target.path().join("src/main.rs")), GOOD_MAIN);
}

// --- Single volatile error slot: replace, never stack (T12 doctrine) ---

#[test]
fn surfaced_errors_replace_never_stack() {
    let (target, _store_root, store) = setup(GOOD_MAIN);
    let mut shadow = shadow_with_broken_main(&store, &target, BROKEN_MAIN);
    // One fix that swaps error A (missing brace) for error B (missing
    // paren), then a no-op fix: the surfaced slot must hold ONLY B.
    let error_b: &[u8] = b"fn main( {\n    let x = (1;\n}\n";
    let mut fixer = kit::ScriptedFixer::new(vec![vec![kit::replace_edit(
        "src/main.rs",
        error_b,
        BROKEN_MAIN,
    )]]);
    let runner = kit::ScriptedRunner::new(vec![]);
    let rig = kit::Rig::new();

    let report = PreFlight::new(&rig.deps(&runner), &mut fixer)
        .run(&mut shadow, &touches())
        .expect("preflight runs");

    let Outcome::Failed(failure) = &report.outcome else {
        panic!("must surface the second failure");
    };
    assert_eq!(failure.produced_at_iteration, 1);
    // The slot carries only the LATEST failure's diagnostics; the
    // iteration-0 brace error is gone (replaced, not stacked).
    for error in std::iter::once(&failure.primary).chain(failure.rest.iter()) {
        assert_ne!(
            error.message, "missing \"}\"",
            "the replaced (iteration-0) error must not be present: {error:?}"
        );
    }
    assert!(
        !failure.rest.is_empty() || failure.primary.tool == "ast-gate",
        "failure carries structured content"
    );
}

// --- Misleading-success guard: stage ORDER + structured CONTENT at Check ---

#[test]
fn check_stage_failure_carries_structured_primary_first_content() {
    let (target, _store_root, store) = setup(GOOD_MAIN);
    let mut shadow = {
        let mut s = store.create_shadow(target.path()).expect("shadow");
        // Valid syntax, undefined symbol: passes the AST gate, fails cargo.
        let semantic: &[u8] = b"fn main() {\n    nope();\n}\n";
        let edit = kit::replace_edit("src/main.rs", semantic, GOOD_MAIN);
        s.apply(&[edit]).expect("apply semantic edit");
        s
    };
    #[allow(clippy::useless_concat)] // readability: one JSON message per source line
    let cargo_json = concat!(
        "{\"reason\":\"compiler-artifact\",\"profile\":{\"test\":false}}\n",
        "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"warning\",\"message\":\"unused variable\",\"spans\":[]}}\n",
        "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"error\",\"message\":\"cannot find function `nope` in this scope\",\"code\":{\"code\":\"E0425\"},\"spans\":[{\"file_name\":\"src/main.rs\",\"line_start\":2,\"column_start\":5,\"primary\":true}]}}\n",
    );
    let runner = kit::ScriptedRunner::new(vec![
        kit::ok("rustfmt 1.9.0"),
        kit::ok(""),
        kit::ok("cargo 1.98.0"),
        kit::fail(cargo_json, 101),
    ]);
    let rig = kit::Rig::new();

    let report = PreFlight::new(&rig.deps(&runner), &mut SurfaceAfterTwo)
        .run(&mut shadow, &touches())
        .expect("preflight runs");

    let Outcome::Failed(failure) = &report.outcome else {
        panic!("semantic error must fail the check stage");
    };
    assert_eq!(failure.stage, Stage::Check);
    assert_eq!(
        trace_tuples(&report),
        vec![
            (0, Stage::AstGate, AttemptOutcome::Passed),
            (0, Stage::Format, AttemptOutcome::Passed),
            (0, Stage::Check, AttemptOutcome::Failed),
        ]
    );
    // Structured extraction: file:line + PRIMARY message only; the warning
    // never surfaces as an error.
    assert_eq!(failure.primary.tool, "cargo check");
    assert_eq!(failure.primary.path, Some(PathBuf::from("src/main.rs")));
    assert_eq!(failure.primary.line, Some(2));
    assert_eq!(failure.primary.column, Some(5));
    assert!(
        failure
            .primary
            .message
            .contains("cannot find function `nope`"),
        "primary message first: {:?}",
        failure.primary
    );
    assert!(
        failure
            .rest
            .iter()
            .chain(std::iter::once(&failure.primary))
            .all(|e| !e.message.contains("unused variable")),
        "warnings must not surface as errors"
    );
}
