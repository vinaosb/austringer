//! Filesystem adversarials (plan todo 22): >260-char paths, stale base
//! drift mid/after pipeline, dirty-worktree distinction, the live
//! T19-timeout probe, and ×3 determinism.

mod common;

use common::preflightkit as kit;
use harness_exec::preflight::{
    AttemptOutcome, Outcome, PreFlight, PreFlightConfig, PreFlightDeps, Stage, SurfaceAfterTwo,
    ToolContext,
};
use harness_exec::spec::ScreenSize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

// --- Long paths (>260 chars, Windows MAX_PATH) ---

#[test]
fn deep_paths_beyond_260_chars_flow_through_the_whole_pipeline() {
    let target = TempDir::new().expect("target");
    let store_root = TempDir::new().expect("store");
    // ~40 nested segments of 8 chars: reliably >260 chars total.
    let mut deep = target.path().to_path_buf();
    for _ in 0..40 {
        deep.push("segment");
    }
    let rel_segments = "segment".repeat(0);
    let mut rel = PathBuf::new();
    for _ in 0..40 {
        rel.push("segment");
    }
    rel.push("leaf.rs");
    let rel_text = rel.to_str().expect("utf-8 fixture path").to_owned();
    let good: &[u8] = b"pub fn deep() -> u8 {\n    7\n}\n";
    kit::write(&deep.join("leaf.rs"), b"pub fn deep() -> u8 { 0 }\n");
    assert!(
        deep.join("leaf.rs").as_os_str().len() > 240,
        "fixture must actually exceed the classic path budget"
    );
    let _ = rel_segments;

    let store = kit::store(store_root.path());
    let mut shadow = store.create_shadow(target.path()).expect("shadow");
    let edit = kit::replace_edit(&rel_text, good, b"pub fn deep() -> u8 { 0 }\n");
    shadow.apply(&[edit]).expect("apply");

    let runner = kit::ScriptedRunner::new(vec![]);
    let rig = kit::Rig::new();
    let seam: &mut dyn harness_exec::preflight::FixerSeam = &mut SurfaceAfterTwo;
    let report = PreFlight::new(&rig.deps(&runner), &mut *seam)
        .run(&mut shadow, std::slice::from_ref(&rel))
        .expect("preflight runs");
    assert!(
        matches!(report.outcome, Outcome::Passed),
        "long paths must work end to end: {:?}",
        report.outcome
    );
    drop(shadow);
    drop(store);
    drop(store_root);
}

// --- stale_state: T21 BaseDrift is respected, never masked ---

#[test]
fn user_tree_edit_after_shadow_surfaces_base_drift_at_commit() {
    let target = TempDir::new().expect("target");
    let store_root = TempDir::new().expect("store");
    kit::cargo_tree(target.path(), b"fn main() {}\n");
    let store = kit::store(store_root.path());
    let mut shadow = store.create_shadow(target.path()).expect("shadow");
    let current = kit::read(&target.path().join("src/main.rs"));
    let new_main: &[u8] = b"fn main() {\n    let _ = 1;\n}\n";
    shadow
        .apply(&[kit::replace_edit("src/main.rs", new_main, &current)])
        .expect("apply");

    // Gates pass against a self-consistent shadow...
    let runner = kit::ScriptedRunner::new(vec![]);
    let rig = kit::Rig::new();
    let seam: &mut dyn harness_exec::preflight::FixerSeam = &mut SurfaceAfterTwo;
    let report = PreFlight::new(&rig.deps(&runner), &mut *seam)
        .run(&mut shadow, &[PathBuf::from("src/main.rs")])
        .expect("preflight runs");
    assert!(matches!(report.outcome, Outcome::Passed));

    // ...then the user tree moves under the transaction target. Pre-flight
    // MUST NOT mask T21's drift contract at commit time.
    kit::write(
        &target.path().join("src/main.rs"),
        b"fn main() { /* user raced */ }\n",
    );
    let error = shadow.commit().expect_err("drift must surface");
    assert!(
        matches!(error, harness_vfs::VfsError::BaseDrift { .. }),
        "T21 BaseDrift respected: {error:?}"
    );
    drop(shadow);
    drop(store);
    drop(store_root);
}

// --- dirty_worktree: unrelated dirt is distinguished from gate failure ---

#[test]
fn unrelated_dirty_file_never_blocks_a_clean_preflight_commit() {
    let target = TempDir::new().expect("target");
    let store_root = TempDir::new().expect("store");
    kit::cargo_tree(target.path(), b"fn main() {}\n");
    // The user has an uncommitted change in a file the transaction never
    // touches — dirty, but unrelated.
    kit::write(&target.path().join("README.md"), b"dirty but untouched\n");

    let store = kit::store(store_root.path());
    let mut shadow = store.create_shadow(target.path()).expect("shadow");
    let current = kit::read(&target.path().join("src/main.rs"));
    let new_main: &[u8] = b"fn main() {\n    let _ = 2;\n}\n";
    shadow
        .apply(&[kit::replace_edit("src/main.rs", new_main, &current)])
        .expect("apply");

    let runner = kit::ScriptedRunner::new(vec![]);
    let rig = kit::Rig::new();
    let seam: &mut dyn harness_exec::preflight::FixerSeam = &mut SurfaceAfterTwo;
    let report = PreFlight::new(&rig.deps(&runner), &mut *seam)
        .run(&mut shadow, &[PathBuf::from("src/main.rs")])
        .expect("preflight runs");
    assert!(
        matches!(report.outcome, Outcome::Passed),
        "unrelated dirt is not a gate failure"
    );
    let stats = shadow.commit().expect("commit must succeed");
    assert_eq!(stats.written, 1);
    drop(shadow);
    drop(store);
    drop(store_root);
}

// --- Determinism: the acceptance fixture is byte-stable across runs ---

#[test]
fn acceptance_fixture_is_deterministic_across_three_runs() {
    for _ in 0..3 {
        let target = TempDir::new().expect("target");
        let store_root = TempDir::new().expect("store");
        kit::cargo_tree(target.path(), b"fn main() {}\n");
        let store = kit::store(store_root.path());
        let mut shadow = store.create_shadow(target.path()).expect("shadow");
        let current = kit::read(&target.path().join("src/main.rs"));
        let broken: &[u8] = b"fn main() {\n";
        shadow
            .apply(&[kit::replace_edit("src/main.rs", broken, &current)])
            .expect("apply");

        let runner = kit::ScriptedRunner::new(vec![]);
        let rig = kit::Rig::new();
        let seam: &mut dyn harness_exec::preflight::FixerSeam = &mut SurfaceAfterTwo;
        let report = PreFlight::new(&rig.deps(&runner), &mut *seam)
            .run(&mut shadow, &[PathBuf::from("src/main.rs")])
            .expect("preflight runs");

        let Outcome::Failed(failure) = report.outcome else {
            panic!("identical fixture must fail identically every run");
        };
        assert_eq!(failure.stage, Stage::AstGate);
        assert_eq!(
            report.trace,
            vec![harness_exec::preflight::StageAttempt {
                iteration: 0,
                stage: Stage::AstGate,
                outcome: AttemptOutcome::Failed,
            }]
        );
        drop(shadow);
        drop(store);
        drop(store_root);
    }
}

// --- hung_or_long: the real T19 timeout machinery, probed live ---

/// A checker whose "check" is a 30-second sleep: only the live PTY
/// runner's timeout ceiling can end it.
struct SleepyCheck;

impl harness_exec::preflight::StaticChecker for SleepyCheck {
    fn tool(&self) -> &'static str {
        "sleepy-check"
    }
    fn claims(&self, rel: &Path) -> bool {
        rel.extension().is_some_and(|e| e == "slp")
    }
    fn marker(&self, _root: &Path) -> bool {
        true
    }
    fn probe_spec(&self, root: &Path) -> harness_exec::spec::ExecSpec {
        harness_exec::spec::ExecSpec {
            program: "cmd".into(),
            args: vec!["/c".into(), "echo probe".into()],
            cwd: Some(root.to_path_buf()),
            env: Default::default(),
            timeout: Duration::from_secs(10),
            size: ScreenSize::default(),
            run_in: harness_exec::spec::RunIn::Native,
        }
    }
    fn check_spec(&self, ctx: &ToolContext) -> harness_exec::spec::ExecSpec {
        harness_exec::spec::ExecSpec {
            program: "cmd".into(),
            args: vec!["/c".into(), "ping -n 31 127.0.0.1 > nul".into()],
            cwd: Some(ctx.root.to_path_buf()),
            env: Default::default(),
            timeout: ctx.timeout,
            size: ScreenSize::default(),
            run_in: harness_exec::spec::RunIn::Native,
        }
    }
    fn extract(&self, _text: &str) -> Vec<harness_exec::preflight::StructuredError> {
        Vec::new()
    }
}

#[cfg(windows)]
#[test]
fn live_compiler_hang_is_killed_by_the_timeout_ceiling() {
    let target = TempDir::new().expect("target");
    let store_root = TempDir::new().expect("store");
    kit::write(&target.path().join("thing.slp"), b"data\n");
    let store = kit::store(store_root.path());
    let mut shadow = store.create_shadow(target.path()).expect("shadow");
    let current = kit::read(&target.path().join("thing.slp"));
    shadow
        .apply(&[kit::replace_edit("thing.slp", b"data2\n", &current)])
        .expect("apply");

    let runner = harness_exec::PtyRunner::new();
    let deps = PreFlightDeps {
        runner: &runner,
        analyzer: &harness_ast::Analyzer::new(),
        cancel: &harness_exec::CancelToken::new(),
        formatters: vec![],
        checkers: vec![Box::new(SleepyCheck)],
        config: PreFlightConfig {
            format_timeout: Duration::from_secs(5),
            check_timeout: Duration::from_secs(2),
        },
    };
    let seam: &mut dyn harness_exec::preflight::FixerSeam = &mut SurfaceAfterTwo;
    let started = std::time::Instant::now();
    let report = PreFlight::new(&deps, &mut *seam)
        .run(&mut shadow, &[PathBuf::from("thing.slp")])
        .expect("preflight runs");
    let elapsed = started.elapsed();

    let Outcome::Failed(failure) = report.outcome else {
        panic!("a hung checker must surface a typed failure");
    };
    assert_eq!(failure.stage, Stage::Check);
    assert!(failure.primary.message.contains("timed out"));
    assert!(
        elapsed < Duration::from_secs(25),
        "the kill must come from the 2s ceiling, not the command's own 30s: {elapsed:?}"
    );
    drop(shadow);
    drop(store);
    drop(store_root);
}
