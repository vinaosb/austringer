//! Agent-task environment bootstrap (plan todo 22): the repo-owned
//! `setup-steps.yml` prep-recipe — typed parsing + schema validation now,
//! daemon-side execution later.

mod common;

use common::preflightkit as kit;
use harness_exec::preflight::setup::{SETUP_STEPS_FILENAME, SetupStepsError, parse_setup_steps};
use std::path::PathBuf;

fn sample_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/setup-steps.yml")
}

// --- The committed sample file parses and validates ---

#[test]
fn committed_sample_file_parses_into_the_typed_recipe() {
    let text = kit::read_text(&sample_path());
    let steps = parse_setup_steps(&text).expect("sample must parse");
    assert_eq!(steps.trigger_branch, "main");
    assert_eq!(steps.job.name, "setup-env");
    assert_eq!(steps.job.runs_on, Some("ubuntu-latest".to_owned()));
    assert_eq!(steps.job.steps.len(), 3);
    assert_eq!(
        steps.job.steps[0].name,
        Some("Install Rust toolchain components".to_owned())
    );
    assert_eq!(
        steps.job.steps[0].run,
        "rustup component add rustfmt clippy"
    );
    assert_eq!(steps.job.steps[2].name, None, "short form has no name");
    assert_eq!(steps.job.steps[2].run, "cargo fetch");
    assert_eq!(SETUP_STEPS_FILENAME, "setup-steps.yml");
}

// --- Minimal + variant forms ---

#[test]
fn minimal_recipe_with_block_branch_form_parses() {
    let text = concat!(
        "on:\n",
        "  push:\n",
        "    branches:\n",
        "      - trunk\n",
        "jobs:\n",
        "  prep:\n",
        "    steps:\n",
        "      - run: echo ready\n",
    );
    let steps = parse_setup_steps(text).expect("minimal parses");
    assert_eq!(steps.trigger_branch, "trunk");
    assert_eq!(steps.job.name, "prep");
    assert_eq!(steps.job.runs_on, None);
    assert_eq!(steps.job.steps.len(), 1);
    assert_eq!(steps.job.steps[0].run, "echo ready");
}

#[test]
fn crlf_files_parse_identically() {
    let text = "on:\r\n  push:\r\n    branches: [ main ]\r\njobs:\r\n  p:\r\n    steps:\r\n      - run: echo hi\r\n";
    let steps = parse_setup_steps(text).expect("CRLF parses");
    assert_eq!(steps.trigger_branch, "main");
    assert_eq!(steps.job.steps[0].run, "echo hi");
}

#[test]
fn comments_and_blank_lines_are_ignored() {
    let text = concat!(
        "# repo bootstrap\n",
        "\n",
        "on:\n",
        "  push:\n",
        "    branches: [ main ] # inline after value is kept verbatim\n",
        "jobs:\n",
        "  p:\n",
        "    steps:\n",
        "      # a comment between steps\n",
        "      - run: echo hi\n",
    );
    let steps = parse_setup_steps(text).expect("comments skip");
    assert_eq!(steps.trigger_branch, "main");
    assert_eq!(steps.job.steps.len(), 1);
}

#[test]
fn quoted_values_lose_their_surrounding_quotes() {
    let text = concat!(
        "on:\n",
        "  push:\n",
        "    branches: [ \"release-1\" ]\n",
        "jobs:\n",
        "  p:\n",
        "    steps:\n",
        "      - run: \"echo \\\"quoted\\\"\"\n",
    );
    let steps = parse_setup_steps(text).expect("quotes strip");
    assert_eq!(steps.trigger_branch, "release-1");
    assert_eq!(steps.job.steps[0].run, "echo \\\"quoted\\\"");
}

// --- Schema validation: every violation is typed ---

fn err(text: &str) -> SetupStepsError {
    parse_setup_steps(text).expect_err("must be rejected")
}

#[test]
fn multiple_trigger_branches_are_rejected() {
    let error =
        err("on:\n  push:\n    branches: [ main, dev ]\njobs:\n  p:\n    steps:\n      - run: x\n");
    assert!(
        matches!(error, SetupStepsError::MultipleBranches { .. }),
        "{error}"
    );
}

#[test]
fn missing_branch_is_rejected() {
    let error = err("on:\n  push: {}\njobs:\n  p:\n    steps:\n      - run: x\n");
    assert!(
        matches!(error, SetupStepsError::MissingField { field, .. } if field == "branches"),
        "{error}"
    );
}

#[test]
fn multiple_jobs_are_rejected() {
    let error = err(concat!(
        "on:\n  push:\n    branches: [ main ]\njobs:\n",
        "  first:\n    steps:\n      - run: x\n",
        "  second:\n    steps:\n      - run: y\n",
    ));
    assert!(
        matches!(error, SetupStepsError::MultipleJobs { .. }),
        "{error}"
    );
}

#[test]
fn zero_jobs_are_rejected() {
    let error = err("on:\n  push:\n    branches: [ main ]\njobs: {}\n");
    assert!(
        matches!(error, SetupStepsError::MissingField { field, .. } if field == "jobs entry"),
        "{error}"
    );
}

#[test]
fn empty_steps_are_rejected() {
    let error = err("on:\n  push:\n    branches: [ main ]\njobs:\n  p:\n    steps: []\n");
    assert!(
        matches!(error, SetupStepsError::EmptySteps { .. }),
        "{error}"
    );
}

#[test]
fn step_without_run_is_rejected() {
    let error = err(concat!(
        "on:\n  push:\n    branches: [ main ]\njobs:\n  p:\n    steps:\n",
        "      - name: only a name\n",
    ));
    assert!(
        matches!(error, SetupStepsError::MissingField { field, .. } if field == "run"),
        "{error}"
    );
}

#[test]
fn unknown_keys_are_rejected_with_line_numbers() {
    let error = err(concat!(
        "on:\n  push:\n    branches: [ main ]\n",
        "tools:\n  allowed: all\n",
        "jobs:\n  p:\n    steps:\n      - run: x\n",
    ));
    match error {
        SetupStepsError::UnknownKey { key, line } => {
            assert_eq!(key, "tools");
            assert_eq!(line, 4);
        }
        other => panic!("wrong error: {other}"),
    }
}

#[test]
fn unknown_job_keys_are_rejected() {
    let error = err(concat!(
        "on:\n  push:\n    branches: [ main ]\njobs:\n  p:\n",
        "    timeout-minutes: 5\n",
        "    steps:\n      - run: x\n",
    ));
    assert!(
        matches!(error, SetupStepsError::UnknownKey { ref key, .. } if key == "timeout-minutes"),
        "{error}"
    );
}

#[test]
fn tab_indentation_is_rejected() {
    let error = err("on:\n\tpush:\n");
    assert!(
        matches!(error, SetupStepsError::TabIndent { line: 2 }),
        "{error}"
    );
}

#[test]
fn missing_on_block_is_rejected() {
    let error = err("jobs:\n  p:\n    steps:\n      - run: x\n");
    assert!(
        matches!(error, SetupStepsError::MissingField { field, .. } if field == "on"),
        "{error}"
    );
}

#[test]
fn wrong_trigger_kind_is_rejected() {
    let error = err(
        "on:\n  pull_request:\n    branches: [ main ]\njobs:\n  p:\n    steps:\n      - run: x\n",
    );
    assert!(
        matches!(error, SetupStepsError::UnknownKey { ref key, .. } if key == "pull_request"),
        "{error}"
    );
}

#[test]
fn flow_branch_without_list_is_rejected() {
    let error = err("on:\n  push:\n    branches: main\njobs:\n  p:\n    steps:\n      - run: x\n");
    assert!(
        matches!(error, SetupStepsError::NotAList { field, .. } if field == "branches"),
        "{error}"
    );
}
