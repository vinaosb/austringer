//! Cancellation semantics (plan todo 22): pre-cancelled tokens run
//! nothing; a cancelled check aborts the whole pipeline cleanly.

mod common;

use austringer_exec::preflight::{
    FixerSeam, PreFlight, PreFlightConfig, PreFlightDeps, PreFlightError, SurfaceAfterTwo,
};
use common::preflightkit as kit;
use std::path::PathBuf;
use tempfile::TempDir;

const GOOD_MAIN: &[u8] = b"fn main() {\n    println!(\"ok\");\n}\n";
const BROKEN_MAIN: &[u8] = b"fn main() {\n    let x = 1;\n";

#[test]
fn pre_cancelled_token_runs_nothing_and_aborts_cleanly() {
    let target = TempDir::new().expect("target");
    let store_root = TempDir::new().expect("store");
    let store = kit::store(store_root.path());
    kit::cargo_tree(target.path(), GOOD_MAIN);
    let mut shadow = {
        let mut s = store.create_shadow(target.path()).expect("shadow");
        let edit = kit::replace_edit("src/main.rs", BROKEN_MAIN, GOOD_MAIN);
        s.apply(&[edit]).expect("apply broken edit");
        s
    };
    let runner = kit::ScriptedRunner::new(vec![]);
    let cancel = austringer_exec::CancelToken::new();
    cancel.cancel();
    let deps = PreFlightDeps {
        runner: &runner,
        analyzer: &austringer_ast::Analyzer::new(),
        cancel: &cancel,
        formatters: austringer_exec::preflight::default_formatters(),
        checkers: austringer_exec::preflight::default_checkers(),
        config: PreFlightConfig::default(),
    };

    let mut policy = SurfaceAfterTwo;
    let seam: &mut dyn FixerSeam = &mut policy;
    let error = PreFlight::new(&deps, seam)
        .run(&mut shadow, &[PathBuf::from("src/main.rs")])
        .expect_err("cancelled pipeline must abort");
    assert!(matches!(error, PreFlightError::Cancelled));
    assert!(
        runner.recorded().is_empty(),
        "a pre-cancelled token runs nothing"
    );
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn cancelled_check_status_aborts_the_whole_pipeline() {
    let target = TempDir::new().expect("target");
    let store_root = TempDir::new().expect("store");
    let store = kit::store(store_root.path());
    kit::cargo_tree(target.path(), GOOD_MAIN);
    let mut shadow = {
        let mut s = store.create_shadow(target.path()).expect("shadow");
        let edit = kit::replace_edit("src/main.rs", GOOD_MAIN, GOOD_MAIN);
        s.apply(&[edit]).expect("apply");
        s
    };
    // Formatter probe succeeds, then the check command reports a clean
    // cancellation kill.
    let runner = kit::ScriptedRunner::new(vec![kit::ok("rustfmt 1.9.0"), kit::cancelled()]);
    let rig = kit::Rig::new();

    let error = PreFlight::new(&rig.deps(&runner), &mut SurfaceAfterTwo)
        .run(&mut shadow, &[PathBuf::from("src/main.rs")])
        .expect_err("mid-check cancel must abort");
    assert!(matches!(error, PreFlightError::Cancelled));
    drop(shadow);
    drop(store);
    drop(store_root);
}
