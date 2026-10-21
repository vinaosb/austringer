//! Todo 17 (ecosystem alignment) acceptance: the `run_tests` tool —
//! framework detection (cargo test + cargo nextest), execution through
//! the executor #3 lane with the todo-4 safety gate applied verbatim,
//! and results parsed into a typed [`TestReport`] (per-test
//! pass/fail/ignored + durations), never raw text.
//!
//! Invariants pinned here:
//! * (a) a fixture crate with 1 passing + 1 failing test reports
//!   EXACTLY those entries with per-test statuses (+ durations on the
//!   nextest lane, which carries them);
//! * (b) an unknown framework is a typed error carrying configuration
//!   instructions;
//! * (c) run_tests is tier-parity with execute_command — it executes
//!   build scripts and test binaries (arbitrary code), so it is NEVER
//!   auto-allowed: no capability-matrix column maps it, and under the
//!   default policy the call asks and an unanswered ask is refused;
//! * (d) a repo-local command override file is refused with a typed
//!   error naming the file — overrides are read ONLY from the
//!   daemon-owned user-global config (`DaemonConfig::test_command`,
//!   `HARNESS_TEST_COMMAND`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use harness_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TASK_COMPLETED_TOOL,
};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use harness_core::autonomy::gate::capability_of;
use harness_core::autonomy::matrix::{AutonomyMode, AutonomyMatrix, Capability};
use harness_core::event::ContentBlock;
use harness_core::tools::ProviderError;
use harness_core::tools::ToolProvider;

use harness_daemon::attach::HostCommand;
use harness_daemon::config::DaemonConfig;
use harness_daemon::mediation::{MediationPolicy, PermissionMediator};
use harness_daemon::protocol::{Answer, ServerMsg, SessionEvent};
use harness_daemon::server::DaemonState;
use harness_daemon::tools::CoreExecutors;
use harness_daemon::tools::run_tests::{RunTestsTool, TestReport};

type Factory = Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>;

// ---------------------------------------------------------------------------
// Scripted-inference kit (todo-3 pattern)
// ---------------------------------------------------------------------------

struct ScriptedInference {
    steps: std::sync::Mutex<std::collections::VecDeque<AssistantTurn>>,
}

impl ScriptedInference {
    fn from_steps(steps: Vec<AssistantTurn>) -> Self {
        Self {
            steps: std::sync::Mutex::new(steps.into()),
        }
    }
}

#[async_trait]
impl Inference for ScriptedInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        let mut steps = match self.steps.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        steps
            .pop_front()
            .ok_or_else(|| InferenceError::Failure("script exhausted".into()))
    }
}

fn text_turn(text: &str, calls: Vec<PendingCall>) -> AssistantTurn {
    AssistantTurn {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
        tool_calls: calls,
        usage: None,
    }
}

fn completion_turn() -> AssistantTurn {
    text_turn(
        "done",
        vec![PendingCall {
            call_id: "done".into(),
            tool: TASK_COMPLETED_TOOL.to_owned(),
            arguments: serde_json::json!({}),
        }],
    )
}

fn call(id: &str, tool: &str, arguments: serde_json::Value) -> PendingCall {
    PendingCall {
        call_id: id.to_owned(),
        tool: tool.to_owned(),
        arguments,
    }
}

// ---------------------------------------------------------------------------
// Event-lane kit
// ---------------------------------------------------------------------------

/// Collect the live event lane until the turn finishes.
async fn read_live_until_finished(mut live: broadcast::Receiver<ServerMsg>) -> Vec<String> {
    let mut frames = Vec::new();
    loop {
        match live.recv().await {
            Ok(ServerMsg::Event { frame }) => {
                let finished = matches!(frame.event, SessionEvent::TurnFinished { .. });
                if let Ok(encoded) = serde_json::to_string(&ServerMsg::Event { frame }) {
                    frames.push(encoded);
                }
                if finished {
                    return frames;
                }
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return frames,
        }
    }
}

/// Answers every permission ask with AllowOnce.
fn spawn_answerer(mediator: Arc<PermissionMediator>, client: &str) -> tokio::task::JoinHandle<()> {
    let client = client.to_owned();
    let mut watch = mediator.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = watch.recv().await {
            if let ServerMsg::PermissionAsk { ask } = msg {
                let _ = mediator.answer(ask.ask_id, &client, Answer::AllowOnce);
            }
        }
    })
}

/// The binary's startup wiring, mirrored: the executor bundle is
/// constructed once and the factory injected into the state.
fn bound_state(data_dir: &Path, factory: Factory) -> Arc<DaemonState> {
    let mut state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            token: Some("run-tests-token".into()),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_secs(30),
            ..DaemonConfig::default()
        },
        factory,
    )
    .expect("daemon state");
    let executors = Arc::new(CoreExecutors::in_process());
    state.executor_factory = executors.executor_factory();
    Arc::new(state)
}

async fn drive_turn(state: &DaemonState, sid: &str) -> Vec<String> {
    let host = state.session(sid);
    let attach = host.attach("tui", None).expect("attach");
    let answerer = spawn_answerer(host.mediator(), "tui");
    let reader = tokio::spawn(read_live_until_finished(attach.events.live));
    host.handle_command(HostCommand::Submit("go".into()))
        .await
        .expect("submit");
    let collected = tokio::time::timeout(Duration::from_secs(240), reader)
        .await
        .expect("turn never finished")
        .expect("reader failed");
    answerer.abort();
    collected
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

// ---------------------------------------------------------------------------
// Fixture crates (real cargo projects in temp dirs; unique package names
// so parallel builds never collide in the shared target dir)
// ---------------------------------------------------------------------------

fn write_fixture_crate(root: &Path, package: &str, lib_rs: &str) -> PathBuf {
    std::fs::create_dir_all(root.join("src")).expect("fixture src dir");
    // The bare [workspace] table pins the fixture as its own workspace
    // root — cargo never walks up into temp-dir ancestors.
    std::fs::write(
        root.join("Cargo.toml"),
        format!("[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n"),
    )
    .expect("fixture Cargo.toml");
    std::fs::write(root.join("src").join("lib.rs"), lib_rs).expect("fixture lib.rs");
    root.to_path_buf()
}

fn pass_fail_lib() -> &'static str {
    "#[test]\nfn passes() { assert_eq!(1 + 1, 2); }\n\n#[test]\nfn fails() { panic!(\"fixture failure 51f2\"); }\n"
}

// ---------------------------------------------------------------------------
// (a) The typed report over a real fixture crate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_tests_fixture_crate_report_has_exact_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = write_fixture_crate(&dir.path().join("crate"), "rtfx_direct", pass_fail_lib());
    let tool = RunTestsTool::new(fixture.clone(), None);

    let bytes = tool
        .execute(&serde_json::json!({ "path": path_string(&fixture) }))
        .await
        .expect("run_tests must succeed on a cargo fixture");
    let text = String::from_utf8(bytes).expect("report is utf-8");
    let report: TestReport = serde_json::from_str(
        text.lines()
            .next()
            .expect("report json is the first line"),
    )
    .expect("observation must deserialize as a TestReport");

    assert!(
        matches!(report.framework.as_str(), "cargo-nextest" | "cargo-libtest"),
        "the cargo framework is detected: {:?}",
        report.framework
    );
    assert_eq!(
        report.tests.len(),
        2,
        "EXACTLY the fixture's two tests: {report:?}"
    );
    let by_name = |fragment: &str| {
        report
            .tests
            .iter()
            .find(|entry| entry.name.contains(fragment))
            .unwrap_or_else(|| panic!("missing test containing {fragment:?}: {report:?}"))
    };
    let passing = by_name("passes");
    let failing = by_name("fails");
    assert_eq!(passing.status, "passed", "{report:?}");
    assert_eq!(failing.status, "failed", "{report:?}");
    assert_eq!(report.summary.passed, 1, "{report:?}");
    assert_eq!(report.summary.failed, 1, "{report:?}");
    assert_eq!(report.summary.ignored, 0, "{report:?}");
    // The nextest lane carries per-test durations (its human output has
    // them); the libtest fallback structurally cannot (stable libtest
    // text has only the suite total) — duration is Option for exactly
    // that honesty.
    if report.framework == "cargo-nextest" {
        assert!(
            report.tests.iter().all(|entry| entry.duration_ms.is_some()),
            "nextest reports per-test durations: {report:?}"
        );
    }
    // A failing TEST is data, not a tool error: the report is returned.
    assert!(!report.timed_out);
}

// ---------------------------------------------------------------------------
// (b) Unknown framework -> typed error with config instructions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_tests_unknown_framework_is_typed_error_with_config_instructions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let not_a_crate = dir.path().join("plain");
    std::fs::create_dir_all(&not_a_crate).expect("dir");
    let tool = RunTestsTool::new(dir.path().to_path_buf(), None);

    let error = tool
        .execute(&serde_json::json!({ "path": path_string(&not_a_crate) }))
        .await
        .expect_err("a non-cargo root with no override must fail typed");
    let ProviderError::Execution { message } = &error else {
        panic!("unknown framework must be an Execution failure: {error:?}");
    };
    assert!(
        message.contains("unknown test framework"),
        "the error names the condition: {message}"
    );
    assert!(
        message.contains("HARNESS_TEST_COMMAND"),
        "the error carries configuration instructions: {message}"
    );
}

// ---------------------------------------------------------------------------
// (c) Tier parity with execute_command — never auto-allow
// ---------------------------------------------------------------------------

#[test]
fn run_tests_tier_parity_no_capability_auto_allow_path() {
    // run_tests executes build scripts and test binaries = arbitrary
    // code. Structural parity with execute_command: NO capability
    // column maps it (no matrix toggle can ever allow it), and the
    // column execute_command maps to (Exec) is OFF in every preset —
    // neither tool has an auto-allow path through the matrix.
    assert_eq!(capability_of("execute_command"), Some(Capability::Exec));
    assert!(
        capability_of("run_tests").is_none(),
        "run_tests must stay outside the capability matrix: {:?}",
        capability_of("run_tests")
    );
    for mode in AutonomyMode::ALL {
        let matrix = AutonomyMatrix::for_mode(mode);
        assert!(
            !matrix.allows(Capability::Exec),
            "execute_command is never matrix-allowed in {mode:?}"
        );
    }
}

#[tokio::test]
async fn run_tests_unanswered_ask_under_default_policy_is_refused() {
    const SID: &str = "s-run-tests-ask";
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = write_fixture_crate(
        &dir.path().join("crate"),
        "rtfx_ask",
        "#[test]\nfn passes() {}",
    );
    let args_path = path_string(&fixture);
    let script: Factory = Arc::new(move || {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "running tests",
                vec![call(
                    "t1",
                    "run_tests",
                    serde_json::json!({ "path": args_path }),
                )],
            ),
            completion_turn(),
        ]))
    });
    // Short ask deadline: the ask fires, times out, and is REFUSED —
    // execution never happens under the default (empty-rules) policy.
    let mut state = DaemonState::new(
        DaemonConfig {
            data_dir: dir.path().to_owned(),
            token: Some("ask-token".into()),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_millis(300),
            ..DaemonConfig::default()
        },
        script,
    )
    .expect("daemon state");
    let executors = Arc::new(CoreExecutors::in_process());
    state.executor_factory = executors.executor_factory();
    let state = Arc::new(state);

    let host = state.session(SID);
    let attach = host.attach("tui", None).expect("attach");
    // Record asks from the mediator control bus; answer NOTHING.
    let mut asks: Vec<String> = Vec::new();
    {
        let mut watch = host.mediator().subscribe();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let reader = tokio::spawn(read_live_until_finished(attach.events.live));
        host.handle_command(HostCommand::Submit("go".into()))
            .await
            .expect("submit");
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(50), watch.recv()).await {
                Ok(Ok(ServerMsg::PermissionAsk { ask })) => {
                    asks.push(ask.tool.clone());
                }
                Ok(_) => {}
                Err(_) => {
                    if reader.is_finished() {
                        break;
                    }
                }
            }
        }
        let joined = tokio::time::timeout(Duration::from_secs(30), reader)
            .await
            .expect("turn never finished")
            .expect("reader failed")
            .join("\n");
        assert!(
            asks.iter().any(|tool| tool == "run_tests"),
            "run_tests must ASK (never silently run): asks were {asks:?}\n{joined}"
        );
        assert!(
            joined.contains("the ask was refused"),
            "the unanswered ask must be refused: {joined}"
        );
        assert!(
            !joined.contains("\"framework\":\"cargo-"),
            "no TestReport may exist — execution never happened: {joined}"
        );
    }
}

// ---------------------------------------------------------------------------
// (d) Repo-local override refused; override read only from user-global config
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_tests_repo_local_override_file_is_refused_with_typed_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = write_fixture_crate(
        &dir.path().join("crate"),
        "rtfx_local",
        "#[test]\nfn passes() {}",
    );
    let repo_local = fixture.join(".harness").join("run_tests.json");
    std::fs::create_dir_all(repo_local.parent().expect("parent")).expect("harness dir");
    std::fs::write(&repo_local, r#"{ "command": "cargo test --evil" }"#).expect("override file");
    let tool = RunTestsTool::new(dir.path().to_path_buf(), None);

    let error = tool
        .execute(&serde_json::json!({ "path": path_string(&fixture) }))
        .await
        .expect_err("a repo-local override file must be refused");
    let ProviderError::Execution { message } = &error else {
        panic!("repo-local override refusal must be an Execution failure: {error:?}");
    };
    assert!(
        message.contains(".harness/run_tests.json") || message.contains(".harness\\run_tests.json"),
        "the refusal names the file: {message}"
    );
    assert!(
        message.contains("user-global"),
        "the refusal points at the user-global config: {message}"
    );
}

#[tokio::test]
async fn run_tests_user_global_override_runs_and_is_safety_gated() {
    // The override from the daemon-owned user-global config is honored —
    // and rides the SAME todo-4 safety gate as execute_command: a
    // network-shaped override is blocked before anything spawns.
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = write_fixture_crate(
        &dir.path().join("crate"),
        "rtfx_gate",
        "#[test]\nfn passes() {}",
    );
    let tool = RunTestsTool::new(
        dir.path().to_path_buf(),
        Some("curl -s https://egress-blocked.invalid/lease".to_owned()),
    );
    let bytes = tool
        .execute(&serde_json::json!({ "path": path_string(&fixture) }))
        .await
        .expect("a blocked override is a typed block result, not an error");
    let text = String::from_utf8(bytes).expect("utf-8");
    assert!(
        text.contains("blocked by sandbox policy"),
        "the override goes through the execute_command safety gate: {text}"
    );
    assert!(
        text.contains("egress-blocked.invalid"),
        "the block names the denied destination: {text}"
    );
}

// ---------------------------------------------------------------------------
// Failure QA: a test binary crash surfaces as a typed error with the exit code
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_tests_binary_crash_is_typed_error_with_exit_code() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = write_fixture_crate(
        &dir.path().join("crate"),
        "rtfx_crash",
        "#[test]\nfn crashes() { std::process::abort(); }",
    );
    // Force the libtest lane via the user-global override: nextest's
    // process-per-test model catches an aborting test as DATA (a
    // failed entry with a duration — asserted in the fixture-report
    // test's lane), while libtest's single process dies with ZERO
    // parsed results — the typed crash path this test pins. $CARGO
    // names the exact binary (PATH-independent).
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let tool = RunTestsTool::new(fixture.clone(), Some(format!("\"{cargo}\" test")));

    let error = tool
        .execute(&serde_json::json!({ "path": path_string(&fixture) }))
        .await
        .expect_err("a crashed test binary with zero parsed results is a typed error");
    let ProviderError::Execution { message } = &error else {
        panic!("crash must be an Execution failure: {error:?}");
    };
    assert!(
        message.contains("exit code"),
        "the crash error carries the exit code: {message}"
    );
    assert!(
        !message.contains("exit code 0"),
        "the crash exit code is nonzero: {message}"
    );
}

// ---------------------------------------------------------------------------
// E2E: the tool rides the task-3 registry end to end
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_tests_e2e_structured_report_flows_through_registry() {
    const SID: &str = "s-run-tests-e2e";
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = write_fixture_crate(&dir.path().join("crate"), "rtfx_e2e", pass_fail_lib());
    let args_path = path_string(&fixture);
    let script: Factory = Arc::new(move || {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "running tests",
                vec![call(
                    "t1",
                    "run_tests",
                    serde_json::json!({ "path": args_path }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);

    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    assert!(
        !joined.contains("no executor registered"),
        "run_tests must be registered through the task-3 registry:\n{joined}"
    );
    // The structured report arrives WITHOUT parsing prose: it is JSON.
    assert!(
        joined.contains("\\\"framework\\\":\\\"cargo-"),
        "the typed TestReport (JSON) is the tool result:\n{joined}"
    );
    assert!(
        joined.contains("\\\"status\\\":\\\"passed\\\"")
            && joined.contains("\\\"status\\\":\\\"failed\\\""),
        "per-test statuses ride the report:\n{joined}"
    );
    assert!(
        joined.contains("\\\"summary\\\":"),
        "the summary counts ride the report:\n{joined}"
    );
    assert!(
        !joined.contains("no executor registered"),
        "run_tests is registry-served"
    );
}

// ---------------------------------------------------------------------------
// Config shape: the override extends the EXISTING DaemonConfig
// ---------------------------------------------------------------------------

#[test]
fn run_tests_config_override_extends_existing_daemon_config() {
    let config = DaemonConfig::default();
    assert!(
        config.test_command.is_none(),
        "the override defaults to absent (detection is the default path)"
    );
    let overridden = DaemonConfig {
        test_command: Some("cargo nextest run -p harness-daemon".to_owned()),
        ..DaemonConfig::default()
    };
    assert_eq!(
        overridden.test_command.as_deref(),
        Some("cargo nextest run -p harness-daemon")
    );
}
