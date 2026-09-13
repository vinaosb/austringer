//! Shared kit for the pre-flight suites (plan todo 22): a scripted
//! [`CommandRunner`], mock fixers, and fixture-tree builders. Everything
//! here is deterministic by construction — no real subprocess unless a
//! test explicitly asks for the live runner.

use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use harness_exec::cancel::CancelToken;
use harness_exec::pending::CommandRunner;
use harness_exec::preflight::{
    FixerError, FixerSeam, PreFlightConfig, PreFlightDeps, PreFlightFailure, ShadowEdit,
};
use harness_exec::spec::{ExecError, ExecOutcome, ExecSpec, ExecStatus};
use harness_vfs::{ShadowHandle, ShadowStore};

// ---------------------------------------------------------------------------
// Fixture trees
// ---------------------------------------------------------------------------

/// A minimal, syntactically valid cargo crate (checker commands are mocked
/// in these suites; the tree only needs to satisfy marker-file detection).
pub fn cargo_tree(root: &Path, main_rs: &[u8]) {
    write(
        &root.join("Cargo.toml"),
        b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    write(&root.join("src/main.rs"), main_rs);
}

pub fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("fixture mkdir");
    }
    std::fs::write(path, bytes).expect("fixture write");
}

pub fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("fixture read")
}

pub fn read_text(path: &Path) -> String {
    String::from_utf8(read(path)).expect("fixture utf-8")
}

pub fn store(root: &Path) -> ShadowStore {
    ShadowStore::new(root).expect("shadow store")
}

/// A replace-edit anchored on the file's current bytes.
pub fn replace_edit(rel: &str, new_content: &[u8], current: &[u8]) -> ShadowEdit {
    ShadowEdit::replace(
        PathBuf::from(rel),
        blake3::hash(current).into(),
        new_content.to_vec(),
    )
}

// ---------------------------------------------------------------------------
// Scripted runner
// ---------------------------------------------------------------------------

/// Replays scripted results in order and records every spec it saw. An
/// exhausted queue answers `Spawn` (typed "absent") so probe-driven
/// detection paths stay exercisable without extra scripting.
pub struct ScriptedRunner {
    specs: Mutex<Vec<ExecSpec>>,
    queue: Mutex<VecDeque<Result<ExecOutcome, ExecError>>>,
}

impl ScriptedRunner {
    pub fn new(outcomes: Vec<Result<ExecOutcome, ExecError>>) -> Self {
        Self {
            specs: Mutex::new(Vec::new()),
            queue: Mutex::new(outcomes.into()),
        }
    }

    /// `(program, args)` of every run, in order.
    pub fn recorded(&self) -> Vec<(String, Vec<String>)> {
        self.specs
            .lock()
            .expect("specs lock")
            .iter()
            .map(|spec| (spec.program.clone(), spec.args.clone()))
            .collect()
    }

    /// Programs that were attempted (including spawn failures).
    pub fn programs(&self) -> Vec<String> {
        self.specs
            .lock()
            .expect("specs lock")
            .iter()
            .map(|spec| spec.program.clone())
            .collect()
    }
}

impl CommandRunner for ScriptedRunner {
    fn run(&self, spec: &ExecSpec, _cancel: &CancelToken) -> Result<ExecOutcome, ExecError> {
        self.specs.lock().expect("specs lock").push(spec.clone());
        self.queue
            .lock()
            .expect("queue lock")
            .pop_front()
            .unwrap_or_else(|| Err(spawn_error(&spec.program)))
    }
}

fn outcome(status: ExecStatus, text: &str) -> Result<ExecOutcome, ExecError> {
    Ok(ExecOutcome {
        status,
        model_text: text.to_owned(),
        spill: None,
        total_lines: text.lines().count() as u64,
        elapsed: Duration::from_millis(1),
        reader_joined: true,
    })
}

/// Completed with exit code 0 and the given screen text.
pub fn ok(text: &str) -> Result<ExecOutcome, ExecError> {
    outcome(ExecStatus::Completed { exit_code: 0 }, text)
}

/// Completed with a nonzero code and the given screen text.
pub fn fail(text: &str, code: u32) -> Result<ExecOutcome, ExecError> {
    outcome(ExecStatus::Completed { exit_code: code }, text)
}

/// The runner killed the tree at the timeout ceiling.
pub fn timed_out() -> Result<ExecOutcome, ExecError> {
    outcome(ExecStatus::TimedOut, "")
}

/// Cancellation was observed mid-run; tree killed cleanly.
pub fn cancelled() -> Result<ExecOutcome, ExecError> {
    outcome(ExecStatus::Cancelled, "")
}

/// The program could not be spawned (typed absent tool).
pub fn spawn_error(program: &str) -> ExecError {
    ExecError::Spawn {
        program: program.to_owned(),
        source: io::Error::from(io::ErrorKind::NotFound),
    }
}

// ---------------------------------------------------------------------------
// Mock fixers
// ---------------------------------------------------------------------------

/// A fixer with scripted edit plans: call N returns `plans[N - 1]`; once
/// the plans run out it returns no edits (the no-op short-circuit).
pub struct ScriptedFixer {
    pub calls: usize,
    pub seen_primaries: Vec<String>,
    plans: Vec<Vec<ShadowEdit>>,
}

impl ScriptedFixer {
    pub fn new(plans: Vec<Vec<ShadowEdit>>) -> Self {
        Self {
            calls: 0,
            seen_primaries: Vec::new(),
            plans,
        }
    }
}

impl FixerSeam for ScriptedFixer {
    fn fix(
        &mut self,
        failure: &PreFlightFailure,
        _shadow: &ShadowHandle,
    ) -> Result<Vec<ShadowEdit>, FixerError> {
        self.calls += 1;
        self.seen_primaries.push(failure.primary.message.clone());
        let plan = self.plans.get(self.calls - 1).cloned().unwrap_or_default();
        Ok(plan)
    }
}

// ---------------------------------------------------------------------------
// Deps assembly
// ---------------------------------------------------------------------------

/// Owns the long-lived deps (analyzer, cancel token) so [`Rig::deps`]
/// can hand out a fully-wired [`PreFlightDeps`] borrowing them.
pub struct Rig {
    analyzer: harness_ast::Analyzer,
    cancel: CancelToken,
}

impl Rig {
    pub fn new() -> Self {
        Self {
            analyzer: harness_ast::Analyzer::new(),
            cancel: CancelToken::new(),
        }
    }

    /// Standard deps over any runner: all default adapters, default
    /// config, the rig's analyzer and non-cancelled token.
    pub fn deps<'a, R: CommandRunner + 'a>(&'a self, runner: &'a R) -> PreFlightDeps<'a> {
        PreFlightDeps {
            runner,
            analyzer: &self.analyzer,
            cancel: &self.cancel,
            formatters: harness_exec::preflight::default_formatters(),
            checkers: harness_exec::preflight::default_checkers(),
            config: PreFlightConfig::default(),
        }
    }
}
