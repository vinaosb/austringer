//! Stage (b) — formatter adapters: per-tool output extraction, claim
//! routing, absent-tool typed skips, and the bounded file list.

mod common;

use austringer_exec::preflight::{
    AttemptOutcome, MAX_FORMAT_FILES, Outcome, PreFlight, SkipReason, Stage, SurfaceAfterTwo,
    default_formatters,
};
use austringer_exec::spec::ScreenSize;
use common::preflightkit as kit;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

fn ext_of(rel: &str) -> PathBuf {
    PathBuf::from(rel)
}

// --- Claim routing: only claiming adapters run ---

#[test]
fn adapters_claim_their_ecosystems_exactly() {
    let formatters = default_formatters();
    let rust = formatters
        .iter()
        .find(|f| f.tool() == "rustfmt")
        .expect("rustfmt");
    let prettier = formatters
        .iter()
        .find(|f| f.tool() == "prettier")
        .expect("prettier");
    let gofmt = formatters
        .iter()
        .find(|f| f.tool() == "gofmt")
        .expect("gofmt");
    let black = formatters
        .iter()
        .find(|f| f.tool() == "black")
        .expect("black");

    assert!(rust.claims(&ext_of("src/main.rs")));
    assert!(!rust.claims(&ext_of("src/main.py")));
    assert!(!rust.claims(&ext_of("src/main.go")));

    assert!(prettier.claims(&ext_of("app/components/Button.tsx")));
    assert!(prettier.claims(&ext_of("README.md")));
    assert!(prettier.claims(&ext_of("package.json")));
    assert!(!prettier.claims(&ext_of("src/main.rs")));

    assert!(gofmt.claims(&ext_of("cmd/root.go")));
    assert!(!gofmt.claims(&ext_of("cmd/root.ts")));

    assert!(black.claims(&ext_of("pkg/__init__.py")));
    assert!(!black.claims(&ext_of("pkg/__init__.pyi.md")));
}

// --- Output extraction: file:line structured, never a dump ---

#[test]
fn rustfmt_diff_lines_extract_file_and_line() {
    let formatters = default_formatters();
    let rust = formatters
        .iter()
        .find(|f| f.tool() == "rustfmt")
        .expect("rustfmt");
    let text = "Diff in src/main.rs at line 3.\nDiff in src/lib.rs at line 12.\n";
    let errors = rust.extract(text);
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].path, Some(PathBuf::from("src/main.rs")));
    assert_eq!(errors[0].line, Some(3));
    assert_eq!(errors[1].line, Some(12));
    assert_eq!(errors[0].tool, "rustfmt");
}

#[test]
fn prettier_warn_lines_extract_files_and_ignore_the_summary() {
    let formatters = default_formatters();
    let prettier = formatters
        .iter()
        .find(|f| f.tool() == "prettier")
        .expect("prettier");
    let text = concat!(
        "Checking formatting...\n",
        "[warn] src/a.ts\n",
        "[warn] docs/ünïcode.md\n",
        "[warn] Code style issues found in the above file(s).\n",
    );
    let errors = prettier.extract(text);
    assert_eq!(errors.len(), 2, "summary line must not be a file");
    assert_eq!(errors[0].path, Some(PathBuf::from("src/a.ts")));
    assert_eq!(errors[1].path, Some(PathBuf::from("docs/ünïcode.md")));
}

#[test]
fn gofmt_lists_bare_paths() {
    let formatters = default_formatters();
    let gofmt = formatters
        .iter()
        .find(|f| f.tool() == "gofmt")
        .expect("gofmt");
    let errors = gofmt.extract("cmd/root.go\ninternal/db/conn.go\n");
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[1].path, Some(PathBuf::from("internal/db/conn.go")));
}

#[test]
fn black_would_reformat_lines_extract_files() {
    let formatters = default_formatters();
    let black = formatters
        .iter()
        .find(|f| f.tool() == "black")
        .expect("black");
    let text = "would reformat pkg/model.py\nAll done! \u{2713}\n1 file would be reformatted.\n";
    let errors = black.extract(text);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].path, Some(PathBuf::from("pkg/model.py")));
}

// --- Spec construction: cwd, relative args, bounded file list ---

#[test]
fn check_specs_run_in_the_shadow_root_with_relative_paths() {
    let root = Path::new("W:/shadow/root");
    let formatters = default_formatters();
    let black = formatters
        .iter()
        .find(|f| f.tool() == "black")
        .expect("black");
    let ctx = austringer_exec::preflight::ToolContext {
        root,
        files: &[PathBuf::from("pkg/model.py")],
        timeout: Duration::from_secs(9),
        size: ScreenSize::default(),
    };
    let spec = black.check_spec(&ctx);
    assert_eq!(spec.cwd, Some(root.to_path_buf()));
    assert_eq!(spec.program, "black");
    assert!(spec.args.contains(&"pkg/model.py".to_owned()));
    assert!(spec.args.iter().any(|a| a == "--check"));
    assert_eq!(spec.timeout, Duration::from_secs(9));
}

#[test]
fn file_list_is_capped_at_the_bound_with_a_typed_notice() {
    let files: Vec<PathBuf> = (0..MAX_FORMAT_FILES + 7)
        .map(|i| PathBuf::from(format!("m{i:03}/file.py")))
        .collect();
    let formatters = default_formatters();
    let black = formatters
        .iter()
        .find(|f| f.tool() == "black")
        .expect("black");
    let ctx = austringer_exec::preflight::ToolContext {
        root: Path::new("X:/s"),
        files: &files,
        timeout: Duration::from_secs(5),
        size: ScreenSize::default(),
    };
    let spec = black.check_spec(&ctx);
    let file_args = spec.args.iter().filter(|a| a.contains("file.py")).count();
    assert_eq!(file_args, MAX_FORMAT_FILES, "bounded by construction");
}

// --- Pipeline-level: absent formatter is a typed skip, never a failure ---

#[test]
fn absent_formatter_is_typed_skip_and_stage_still_passes() {
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

    // rustfmt probe fails to spawn; the checker stages are never scripted
    // (cargo probe would also fail) — every tool is absent.
    let runner = kit::ScriptedRunner::new(vec![]);
    let rig = kit::Rig::new();
    let seam: &mut dyn austringer_exec::preflight::FixerSeam = &mut SurfaceAfterTwo;
    let report = PreFlight::new(&rig.deps(&runner), &mut *seam)
        .run(&mut shadow, &[PathBuf::from("src/main.rs")])
        .expect("preflight runs");

    assert!(
        matches!(report.outcome, Outcome::Passed),
        "absent tools never fail pre-flight: {:?}",
        report.outcome
    );
    assert!(
        report
            .skips
            .iter()
            .any(|s| s.tool == "rustfmt" && matches!(s.reason, SkipReason::ToolAbsent)),
        "absence must be disclosed: {:?}",
        report.skips
    );
    assert!(
        report
            .skips
            .iter()
            .any(|s| s.tool == "cargo check" && matches!(s.reason, SkipReason::ToolAbsent)),
        "checker absence must be disclosed: {:?}",
        report.skips
    );
    // Stage order preserved; the format stage ran and passed via skips.
    assert_eq!(report.trace.len(), 3);
    assert_eq!(report.trace[0].stage, Stage::AstGate);
    assert_eq!(report.trace[1].stage, Stage::Format);
    assert_eq!(report.trace[2].stage, Stage::Check);
    assert_eq!(report.trace[1].outcome, AttemptOutcome::Passed);
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn unformatted_files_fail_the_format_stage_with_structured_paths() {
    let target = TempDir::new().expect("target");
    let store_root = TempDir::new().expect("store");
    kit::cargo_tree(target.path(), b"fn main() {}\n");
    let store = kit::store(store_root.path());
    let mut shadow = store.create_shadow(target.path()).expect("shadow");
    let current = kit::read(&target.path().join("src/main.rs"));
    let ugly: &[u8] = b"fn main(){let x=1;}\n"; // valid syntax, badly formatted
    shadow
        .apply(&[kit::replace_edit("src/main.rs", ugly, &current)])
        .expect("apply");

    let runner = kit::ScriptedRunner::new(vec![
        kit::ok("rustfmt 1.9.0"),
        kit::fail("Diff in src/main.rs at line 1.\n", 1),
    ]);
    let rig = kit::Rig::new();
    let seam: &mut dyn austringer_exec::preflight::FixerSeam = &mut SurfaceAfterTwo;
    let report = PreFlight::new(&rig.deps(&runner), &mut *seam)
        .run(&mut shadow, &[PathBuf::from("src/main.rs")])
        .expect("preflight runs");

    let Outcome::Failed(failure) = report.outcome else {
        panic!("unformatted file must fail the format gate");
    };
    assert_eq!(failure.stage, Stage::Format);
    assert_eq!(failure.primary.tool, "rustfmt");
    assert_eq!(failure.primary.path, Some(PathBuf::from("src/main.rs")));
    assert_eq!(failure.primary.line, Some(1));
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn non_matching_formatter_output_falls_back_to_one_bounded_line() {
    let formatters = default_formatters();
    let rust = formatters
        .iter()
        .find(|f| f.tool() == "rustfmt")
        .expect("rustfmt");
    let errors = rust.extract("some\nunparseable\nnoise\n");
    assert_eq!(
        errors.len(),
        1,
        "exit-code failure still surfaces compactly"
    );
    assert!(
        errors[0].message.len() <= 200,
        "bounded to a single line, never a dump"
    );
    assert!(errors[0].message.contains("some"), "{}", errors[0].message);
}
