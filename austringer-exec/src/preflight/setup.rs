//! Agent-task environment bootstrap (plan todo 22): the repo-owned
//! `setup-steps.yml` prep-recipe, Copilot-shaped. Parsing + schema
//! validation land NOW; the daemon executes the typed description later.
//!
//! # Supported grammar (strict, line-oriented; a tiny frozen subset of
//! YAML — no anchors, no multi-line scalars, no flow maps beyond the
//! branch list)
//!
//! ```text
//! on:
//!   push:
//!     branches: [ main ]     # inline flow list, or block form:
//!     branches:
//!       - main               # exactly ONE branch (the default branch)
//! jobs:
//!   <job-name>:              # exactly ONE job
//!     runs-on: <label>       # optional
//!     steps:                 # at least one
//!       - name: <label>      # optional
//!         run: <command>     # required, non-empty, single line
//!       - run: <command>     # short form
//! ```
//!
//! Values are the verbatim rest of the line (surrounding quote pairs
//! stripped, no escape processing); full-line `#` comments and blank
//! lines are skipped anywhere, inline text after a bracket list is
//! ignored. Unknown keys, tabs, extra jobs/branches, and missing `run`
//! are typed errors with line numbers. The lexical layer lives in
//! [`super::setup_parse`], the block parsers in [`super::setup_job`].

use std::path::{Path, PathBuf};

pub const SETUP_STEPS_FILENAME: &str = "setup-steps.yml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupSteps {
    /// The single (default) branch that triggers the prep job.
    pub trigger_branch: String,
    pub job: SetupJob,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupJob {
    pub name: String,
    pub runs_on: Option<String>,
    pub steps: Vec<SetupStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupStep {
    pub name: Option<String>,
    pub run: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SetupStepsError {
    #[error("line {line}: required field `{field}` is missing or empty")]
    MissingField { field: &'static str, line: usize },
    #[error(
        "line {line}: unknown key `{key}` (allowed: on/push/branches, one job, runs-on, steps, name, run)"
    )]
    UnknownKey { key: String, line: usize },
    #[error("line {line}: exactly one trigger branch is required")]
    MultipleBranches { line: usize },
    #[error("line {line}: exactly one job is allowed")]
    MultipleJobs { line: usize },
    #[error("line {line}: steps must contain at least one step")]
    EmptySteps { line: usize },
    #[error("line {line}: field `{field}` must be a list")]
    NotAList { field: &'static str, line: usize },
    #[error("line {line}: tabs are not valid indentation")]
    TabIndent { line: usize },
    #[error("line {line}: unexpected content: {text}")]
    Unexpected { line: usize, text: String },
}

pub fn parse_setup_steps(text: &str) -> Result<SetupSteps, SetupStepsError> {
    let lines = super::setup_parse::preprocess(text)?;
    let mut cursor = super::setup_parse::Lines::new(&lines);

    super::setup_parse::expect_top_key(&mut cursor, "on")?;
    super::setup_parse::expect_nested(&mut cursor, 2, "push")?;
    let trigger_branch = super::setup_job::parse_branches(&mut cursor)?;

    super::setup_parse::expect_top_key(&mut cursor, "jobs")?;
    let job = super::setup_job::parse_job(&mut cursor)?;

    if let Some(line) = cursor.peek() {
        if line.indent == 2 && line.content.ends_with(':') {
            return Err(SetupStepsError::MultipleJobs { line: line.number });
        }
        return Err(SetupStepsError::UnknownKey {
            key: super::setup_parse::split_key(line.content).0.to_owned(),
            line: line.number,
        });
    }
    Ok(SetupSteps {
        trigger_branch,
        job,
    })
}

/// Locate the repo-owned file in a workspace root (daemon wiring reads
/// it; the austringer only validates).
#[must_use]
pub fn setup_steps_path(root: &Path) -> PathBuf {
    root.join(SETUP_STEPS_FILENAME)
}
