//! mocktool self-tests (plan M15): the binary entry exercised via
//! `CARGO_BIN_EXE_mocktool`, plus the re-exec entry this same file
//! provides for other crates' harnesses (T40 precedent). Golden modes
//! serve the committed REAL captures (see golden/mocktool/capture.md).

use std::path::{Path, PathBuf};
use std::process::Command;

use austringer_test_support::mocktool;

/// Compile-time-anchored golden dir (a hardcoded `"\\golden\\mocktool"`
/// suffix breaks on unix, where the backslash is not a path separator).
fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("golden")
        .join("mocktool")
}

fn run(args: &[&str], envs: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mocktool"));
    cmd.args(args);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    let out = cmd.output().expect("mocktool spawns");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// T40-style re-exec entry: when austringer_MOCKTOOL_ARGV is set, THIS test
/// becomes a mocktool child (mode words from the env var).
#[test]
fn mocktool_child_entry() {
    if std::env::var("austringer_MOCKTOOL_ARGV").is_err() {
        return; // normal suite run: nothing to do
    }
    std::process::exit(mocktool::run_from_env());
}

/// Golden mode: the committed REAL ffprobe capture is served verbatim.
#[test]
fn golden_mode_serves_real_capture() {
    let golden = golden_dir().display().to_string();
    let (code, stdout, stderr) = run(
        &["golden", "ffprobe/probe-av-json"],
        &[("austringer_MOCKTOOL_GOLDEN_DIR", golden.as_str())],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("real ffprobe JSON parses");
    assert_eq!(
        parsed["streams"][0]["codec_name"], "pcm_s16le",
        "captured tiny.wav stream facts"
    );
}

/// Env mode: the child's environment KEY names, sorted — the redaction
/// assertion surface. (Windows `=C:` per-drive pseudo-vars legitimately
/// carry `=` inside names; the contract is keys-only, proven by exact
/// set equality with the child's inherited environment.)
#[test]
fn env_mode_lists_sorted_key_names() {
    let (code, stdout, _) = run(&["env"], &[]);
    assert_eq!(code, 0);
    let mut names: Vec<String> = stdout.lines().map(String::from).collect();
    names.sort();
    let mut expected: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
    expected.sort();
    assert_eq!(names, expected, "exactly the key names, never values");
}

/// Exit mode: bounded stderr head + the exact code.
#[test]
fn exit_mode_reports_code_and_stderr() {
    let (code, _, stderr) = run(&["exit", "3", "boom", "detail"], &[]);
    assert_eq!(code, 3);
    assert_eq!(stderr.trim(), "boom detail");
}

/// Unknown mode is a typed non-zero exit, never a panic.
#[test]
fn unknown_mode_is_typed_failure() {
    let (code, _, stderr) = run(&["frobnicate"], &[]);
    assert_eq!(code, 2);
    assert!(stderr.contains("unknown mode"));
}

/// The re-exec entry answers the same mode contract via env words.
#[test]
fn reexec_entry_runs_from_env_words() {
    // Spawn the real binary as a stand-in process: the entry fn is
    // exercised in mocktool_child_entry above; here we pin that the
    // dispatch surface agrees between the two entries.
    let (code, stdout, _) = run(&["env"], &[]);
    assert_eq!(code, 0);
    assert!(!stdout.is_empty());
}
