//! PendingCommandSet semantics (plan todo 19): accumulate → inspect →
//! apply-atomically / abort, over a scripted fake runner plus the real
//! seam shape.

use std::time::Duration;

use harness_exec::pending::{CommandRunner, PendingCommandSet};
use harness_exec::runner::CancelToken;
use harness_exec::spec::{ExecError, ExecOutcome, ExecSpec, ExecStatus, ScreenSize};

fn spec(name: &str) -> ExecSpec {
    ExecSpec {
        program: name.to_string(),
        args: Vec::new(),
        cwd: None,
        env: Default::default(),
        timeout: Duration::from_secs(5),
        size: ScreenSize::default(),
    }
}

fn outcome(status: ExecStatus) -> ExecOutcome {
    ExecOutcome {
        status,
        model_text: String::new(),
        spill: None,
        total_lines: 0,
        elapsed: Duration::from_secs(0),
    }
}

/// Scripted runner: yields one queued status per call.
struct FakeRunner {
    statuses: Vec<ExecStatus>,
    calls: std::cell::RefCell<Vec<String>>,
}

impl FakeRunner {
    fn new(statuses: Vec<ExecStatus>) -> Self {
        Self {
            statuses,
            calls: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, spec: &ExecSpec, _cancel: &CancelToken) -> Result<ExecOutcome, ExecError> {
        self.calls.borrow_mut().push(spec.program.clone());
        let index = self.calls.borrow().len() - 1;
        match self.statuses.get(index) {
            Some(status) => Ok(outcome(*status)),
            // A script that ran out is a test bug; fail loudly, not green.
            None => Err(ExecError::Spawn {
                program: spec.program.clone(),
                source: std::io::Error::other("fake runner script exhausted"),
            }),
        }
    }
}

#[test]
fn apply_runs_all_commands_in_order_when_everything_succeeds() {
    let mut set = PendingCommandSet::new();
    set.push(spec("cargo")).push(spec("fmt")).push(spec("test"));
    assert_eq!(set.commands().len(), 3);

    let runner = FakeRunner::new(vec![
        ExecStatus::Completed { exit_code: 0 },
        ExecStatus::Completed { exit_code: 0 },
        ExecStatus::Completed { exit_code: 0 },
    ]);
    let report = set.apply(&runner, &CancelToken::new());
    assert_eq!(report.failed_at, None);
    assert_eq!(report.remaining, 0);
    assert_eq!(report.executed.len(), 3);
    assert_eq!(
        runner.calls.into_inner(),
        vec!["cargo", "fmt", "test"],
        "commands run in queue order"
    );
}

#[test]
fn apply_stops_at_first_nonzero_exit_and_keeps_remainder_queued() {
    let mut set = PendingCommandSet::new();
    set.push(spec("ok")).push(spec("boom")).push(spec("never"));
    let runner = FakeRunner::new(vec![
        ExecStatus::Completed { exit_code: 0 },
        ExecStatus::Completed { exit_code: 101 },
        ExecStatus::Completed { exit_code: 0 },
    ]);
    let report = set.apply(&runner, &CancelToken::new());
    assert_eq!(report.failed_at, Some(1));
    assert_eq!(report.remaining, 1, "the un-run command stays queued");
    assert_eq!(report.executed.len(), 2);
    // The failing command's status is recorded for the review surface.
    assert_eq!(
        report.executed[1],
        (1, ExecStatus::Completed { exit_code: 101 })
    );
    // Retry semantics: after a fix, the set still holds `never`.
    assert_eq!(
        set.commands().last().map(|s| s.program.as_str()),
        Some("never")
    );
}

#[test]
fn apply_stops_at_timeout_without_running_the_rest() {
    let mut set = PendingCommandSet::new();
    set.push(spec("slow")).push(spec("after"));
    let runner = FakeRunner::new(vec![ExecStatus::TimedOut]);
    let report = set.apply(&runner, &CancelToken::new());
    assert_eq!(report.failed_at, Some(0));
    assert_eq!(report.remaining, 1);
    assert_eq!(report.executed, vec![(0, ExecStatus::TimedOut)]);
}

#[test]
fn apply_records_spawn_failures_as_batch_stops() {
    let mut set = PendingCommandSet::new();
    set.push(spec("missing")).push(spec("next"));
    struct ErrRunner;
    impl CommandRunner for ErrRunner {
        fn run(&self, _spec: &ExecSpec, _cancel: &CancelToken) -> Result<ExecOutcome, ExecError> {
            Err(ExecError::Spawn {
                program: "missing".into(),
                source: std::io::Error::other("not found"),
            })
        }
    }
    let report = set.apply(&ErrRunner, &CancelToken::new());
    assert_eq!(report.failed_at, Some(0));
    assert!(report.executed.is_empty());
    assert_eq!(report.remaining, 2, "nothing drained on spawn failure");
}

#[test]
fn pre_cancelled_token_runs_nothing() {
    let mut set = PendingCommandSet::new();
    set.push(spec("a")).push(spec("b"));
    let runner = FakeRunner::new(vec![]);
    let cancel = CancelToken::new();
    cancel.cancel();
    let report = set.apply(&runner, &cancel);
    assert!(report.executed.is_empty(), "nothing may run when cancelled");
    assert_eq!(report.failed_at, None);
    assert_eq!(report.remaining, 2);
}

#[test]
fn abort_discards_the_whole_change_set() {
    let mut set = PendingCommandSet::new();
    set.push(spec("x")).push(spec("y"));
    let aborted = set.abort();
    assert_eq!(aborted.len(), 2);
    assert!(set.commands().is_empty());
    let report = set.apply(&FakeRunner::new(vec![]), &CancelToken::new());
    assert_eq!(report.remaining, 0);
}
