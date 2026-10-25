//! Stage (c) — compiler/static-check adapters: cargo-check JSON
//! extraction quirks, tsc text parsing, timeouts, and ecosystem markers.

mod common;

use austringer_exec::preflight::{Outcome, PreFlight, Stage, SurfaceAfterTwo, default_checkers};
use common::preflightkit as kit;
use std::path::PathBuf;
use tempfile::TempDir;

fn cargo() -> Box<dyn austringer_exec::preflight::StaticChecker> {
    let checkers = default_checkers();
    checkers
        .into_iter()
        .find(|c| c.tool() == "cargo check")
        .expect("cargo checker")
}

fn tsc() -> Box<dyn austringer_exec::preflight::StaticChecker> {
    let checkers = default_checkers();
    checkers
        .into_iter()
        .find(|c| c.tool() == "tsc")
        .expect("tsc checker")
}

// --- cargo check --message-format=json extraction ---

#[test]
fn cargo_json_extracts_primary_span_file_line_and_message() {
    #[allow(clippy::useless_concat)] // readability: one JSON message per source line
    let json = concat!(
        "{\"reason\":\"compiler-artifact\",\"profile\":{\"test\":false}}\n",
        "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"error\",\"message\":\"mismatched types\",\"spans\":[{\"file_name\":\"src/lib.rs\",\"line_start\":7,\"column_start\":9,\"primary\":true},{\"file_name\":\"src/other.rs\",\"line_start\":1,\"column_start\":1,\"primary\":false}]}}\n",
    );
    let errors = cargo().extract(json);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].path, Some(PathBuf::from("src/lib.rs")));
    assert_eq!(errors[0].line, Some(7));
    assert_eq!(errors[0].column, Some(9));
    assert_eq!(errors[0].message, "mismatched types");
    assert_eq!(errors[0].tool, "cargo check");
}

#[test]
fn cargo_json_filters_warnings_out_of_errors() {
    #[allow(clippy::useless_concat)] // readability: one JSON message per source line
    let json = concat!(
        "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"warning\",\"message\":\"unused import: `x`\",\"spans\":[{\"file_name\":\"src/a.rs\",\"line_start\":1,\"column_start\":1,\"primary\":true}]}}\n",
        "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"error\",\"message\":\"unresolved import\",\"spans\":[{\"file_name\":\"src/b.rs\",\"line_start\":2,\"column_start\":5,\"primary\":true}]}}\n",
    );
    let errors = cargo().extract(json);
    assert_eq!(errors.len(), 1, "warnings never surface as errors");
    assert_eq!(errors[0].path, Some(PathBuf::from("src/b.rs")));
}

#[test]
fn cargo_json_handles_msvc_style_multiline_messages_structurally() {
    // MSVC-flavored diagnostics put the human rendering in `rendered`
    // (multi-line) and keep the PRIMARY message structured; extraction
    // reads only the structured fields, so embedded newlines and the
    // whole rendered block are irrelevant by construction.
    #[allow(clippy::useless_concat)] // readability: one JSON line per source line
    let json = concat!(
        "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"error\",\"message\":\"expected one of `!`, `(`, `+`, found `;`\\n\\n\\nthe sku is wrong\",",
        "\"rendered\":\"error: expected one of `!`, `(`\\n --> src/main.rs:3:9\\n  |\\n3 |     let ;\\n  |         ^\\n\",",
        "\"spans\":[{\"file_name\":\"src/main.rs\",\"line_start\":3,\"column_start\":9,\"primary\":true}]}}\n",
    );
    let errors = cargo().extract(json);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].line, Some(3));
    assert!(errors[0].message.contains("expected one of"));
    assert!(
        !errors[0].message.contains("-->"),
        "the multi-line rendered block must not leak in"
    );
}

#[test]
fn cargo_json_survives_non_utf8_and_wrapped_fragments() {
    // PTY screens hand us text, not bytes: invalid sequences arrive as
    // U+FFFD. Corrupt objects are counted and skipped; wrapped lines
    // (newlines injected mid-object at column boundaries) reassemble
    // because object scanning is brace-balanced, not line-based.
    let wrapped = concat!(
        "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"err",
        "or\",\"message\":\"E1\",\"spans\":[{\"file_name\":\"src/w.rs\",\"line_start\":4,\"column_start\":2,\"primary\":true}]}}\n",
    );
    let corrupt = "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"error\",\"message\":\"\u{FFFD}\u{FFFD}\",\"spans\":[{\"file_name\":\"src/\u{FFFD}.rs\",\"line_start\":1,\"column_start\":1,\"primary\":true}]}}\n";
    // The corrupt entry is still VALID JSON (U+FFFD is a legal char), so
    // it extracts; true corruption is a broken object:
    let broken = "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"err\u{FFFD}or\"...";
    let errors = cargo().extract(&format!("{wrapped}{corrupt}{broken}"));
    assert_eq!(errors.len(), 2, "valid objects extract, junk is skipped");
    assert_eq!(errors[0].message, "E1");
    assert_eq!(errors[0].line, Some(4));
}

#[test]
fn cargo_json_truncated_object_degrades_to_the_bounded_fallback() {
    let truncated = "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"error\"";
    let errors = cargo().extract(truncated);
    assert_eq!(
        errors.len(),
        1,
        "unparseable output still surfaces compactly"
    );
    assert!(
        errors[0].message.contains("\"level\"") && errors[0].message.len() <= 200,
        "fallback carries one bounded line: {:?}",
        errors[0].message
    );
}

// --- Ecosystem markers ---

#[test]
fn cargo_checker_requires_a_manifest_marker() {
    let with_manifest = TempDir::new().expect("t");
    kit::write(&with_manifest.path().join("Cargo.toml"), b"[package]\n");
    assert!(cargo().marker(with_manifest.path()));

    let plain = TempDir::new().expect("x");
    assert!(!cargo().marker(plain.path()));
}

#[test]
fn tsc_checker_requires_a_node_marker() {
    let with_tsconfig = TempDir::new().expect("t");
    kit::write(&with_tsconfig.path().join("tsconfig.json"), b"{}");
    assert!(tsc().marker(with_tsconfig.path()));

    let with_package = TempDir::new().expect("p");
    kit::write(&with_package.path().join("package.json"), b"{}");
    assert!(tsc().marker(with_package.path()));

    let plain = TempDir::new().expect("x");
    assert!(!tsc().marker(plain.path()));
}

// --- tsc --noEmit text extraction ---

#[test]
fn tsc_lines_extract_file_line_column_and_code() {
    let text = concat!(
        "src/app.ts(12,5): error TS2339: Property 'x' does not exist on type 'Y'.\n",
        "src/deep/nested.ts(3,19): error TS7006: Parameter 'a' implicitly has an 'any' type.\n",
    );
    let errors = tsc().extract(text);
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].path, Some(PathBuf::from("src/app.ts")));
    assert_eq!(errors[0].line, Some(12));
    assert_eq!(errors[0].column, Some(5));
    assert!(errors[0].message.contains("TS2339"));
    assert!(errors[0].message.contains("does not exist"));
    assert_eq!(errors[1].line, Some(3));
    assert_eq!(errors[1].column, Some(19));
}

#[test]
fn tsc_parse_survives_parens_in_file_paths() {
    // rfind-based: the LAST (line,col) group before the marker wins even
    // when the path itself contains parentheses.
    let text = "src/a(1).ts(7,3): error TS1005: ',' expected.\n";
    let errors = tsc().extract(text);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].path, Some(PathBuf::from("src/a(1).ts")));
    assert_eq!(errors[0].line, Some(7));
    assert_eq!(errors[0].column, Some(3));
}

#[test]
fn tsc_ignores_non_error_lines() {
    let text = concat!(
        "Version 5.9.2\n",
        "src/app.ts(1,1): error TS2304: Cannot find name 'z'.\n",
        "src/app.ts(2,1): warning TS6133: unused\n",
    );
    let errors = tsc().extract(text);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("TS2304"));
}

// --- Pipeline-level: exit codes, timeout surfacing, checker absence ---

fn ts_fixture() -> (TempDir, TempDir, austringer_vfs::ShadowStore) {
    let target = TempDir::new().expect("target");
    let store_root = TempDir::new().expect("store");
    let store = kit::store(store_root.path());
    kit::write(&target.path().join("tsconfig.json"), b"{}\n");
    kit::write(&target.path().join("src/app.ts"), b"const x: number = 1;\n");
    (target, store_root, store)
}

#[test]
fn checker_timeout_surfaces_as_a_typed_stage_failure() {
    let (target, store_root, store) = ts_fixture();
    let mut shadow = store.create_shadow(target.path()).expect("shadow");
    let edit = kit::replace_edit(
        "src/app.ts",
        b"const x: number = 2;\n",
        b"const x: number = 1;\n",
    );
    shadow.apply(&[edit]).expect("apply");

    let runner = kit::ScriptedRunner::new(vec![kit::ok("tsc 5.9.2"), kit::timed_out()]);
    let seam: &mut dyn austringer_exec::preflight::FixerSeam = &mut SurfaceAfterTwo;
    let deps = austringer_exec::preflight::PreFlightDeps {
        runner: &runner,
        analyzer: &austringer_ast::Analyzer::new(),
        cancel: &austringer_exec::CancelToken::new(),
        formatters: vec![], // keep the format stage quiet for this probe
        checkers: vec![tsc()],
        config: austringer_exec::preflight::PreFlightConfig::default(),
    };
    let report = PreFlight::new(&deps, &mut *seam)
        .run(&mut shadow, &[PathBuf::from("src/app.ts")])
        .expect("preflight runs");

    let Outcome::Failed(failure) = report.outcome else {
        panic!("timeout must surface");
    };
    assert_eq!(failure.stage, Stage::Check);
    assert!(failure.primary.message.contains("timed out"));
    drop(shadow);
    drop(store);
    drop(store_root);
}
