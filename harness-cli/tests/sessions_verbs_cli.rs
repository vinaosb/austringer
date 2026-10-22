//! Todo 22 (ecosystem alignment): `harness sessions list|history` verbs
//! against the real daemon router, the todo-11 search regression guard
//! (ranked hits + FTS syntax error stay a typed nonzero failure through
//! the new dispatch), the attach connection's protocol-version
//! negotiation (must land on 2 — the full v2 frame set), and NDJSON
//! completeness — a usage frame from an identity-bearing fixture
//! provider on a real `run --output-json`. The daemon runs in-process
//! (the t11 pattern); the CLI rides the real binary via assert_cmd.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use assert_cmd::Command as AssertCommand;
use harness_cli::client::{Connect, LIVE_ONLY, SessionTransport};
use harness_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TASK_COMPLETED_TOOL,
};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference, TurnUsage};
use harness_core::event::{ContentBlock, Event};
use harness_daemon::automations::NetworkPolicy;
use harness_daemon::config::DaemonConfig;
use harness_daemon::run_history::{RunHistory, RunRecord};
use harness_daemon::server::{DaemonState, build_router};
use harness_daemon::usage::{ComposedInference, InferenceFactory, IntoSessionFactory};
use harness_sessions::{LeaseOwner, SessionStore};
use predicates::prelude::*;

struct CompleteOnly;
#[async_trait::async_trait]
impl Inference for CompleteOnly {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Ok(AssistantTurn {
            blocks: vec![ContentBlock::Text {
                text: "done".into(),
            }],
            tool_calls: vec![PendingCall {
                call_id: "done".into(),
                tool: TASK_COMPLETED_TOOL.to_owned(),
                arguments: serde_json::json!({}),
            }],
            usage: None,
        })
    }
}

/// Reports token usage every turn (drives the daemon's Usage-frame
/// broadcast once the factory carries an identity).
struct CompleteWithUsage;
#[async_trait::async_trait]
impl Inference for CompleteWithUsage {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Ok(AssistantTurn {
            blocks: vec![ContentBlock::Text {
                text: "done".into(),
            }],
            tool_calls: vec![PendingCall {
                call_id: "done".into(),
                tool: TASK_COMPLETED_TOOL.to_owned(),
                arguments: serde_json::json!({}),
            }],
            usage: Some(TurnUsage {
                input_tokens: 10,
                output_tokens: 20,
                cache_read_tokens: 1,
                cache_write_tokens: 2,
            }),
        })
    }
}

fn plain_factory() -> InferenceFactory {
    let legacy: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> = Arc::new(|| {
        Box::new(CompleteOnly)
    });
    legacy.into_factory()
}

/// Identity-bearing stack (the Usage-frame contract): identity is
/// built from its JSON fixture so the CLI crate never names the
/// providers type directly.
fn usage_factory() -> InferenceFactory {
    Arc::new(|| ComposedInference {
        inference: Box::new(CompleteWithUsage),
        identity: Some(
            serde_json::from_str(r#"{"provider":"mock","model":"mock-1"}"#)
                .unwrap_or_else(|e| panic!("identity fixture: {e}")),
        ),
    })
}

struct Server {
    base: String,
    token: String,
    data_dir: PathBuf,
    state: Arc<DaemonState>,
    _dir: tempfile::TempDir,
}

async fn spawn(factory: InferenceFactory) -> Server {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let data_dir = dir.path().join("d");
    std::fs::create_dir_all(&data_dir).unwrap_or_default();
    let config = DaemonConfig {
        data_dir: data_dir.clone(),
        token: Some("verbs-cli-token".into()),
        ask_timeout: Duration::from_secs(20),
        ..DaemonConfig::default()
    };
    let state =
        Arc::new(DaemonState::new(config, factory).unwrap_or_else(|e| panic!("state: {e}")));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap_or_else(|e| panic!("bind: {e}"));
    let port = listener
        .local_addr()
        .unwrap_or_else(|e| panic!("addr: {e}"))
        .port();
    let app = build_router(Arc::clone(&state));
    tokio::spawn(async move {
        let serve = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        let _ = serve.await;
    });
    Server {
        base: format!("http://127.0.0.1:{port}"),
        token: "verbs-cli-token".into(),
        data_dir,
        state,
        _dir: dir,
    }
}

/// The t11 transcript fixture: s2 carries three needle events (tf-rich),
/// s1 one — bm25 must rank s2's first hit before s1's.
fn seed_search_fixture(dir: &Path) {
    let store = SessionStore::open(dir).unwrap();
    for (id, texts) in [
        ("s1", vec!["unrelated opener", "the needle appears once here"]),
        (
            "s2",
            vec![
                "needle needle in the haystack",
                "another needle sighting",
                "needle once more, third time",
            ],
        ),
    ] {
        store.create(id, "").unwrap();
        let h = store.open_session_rw(id, &LeaseOwner::new("seed")).unwrap();
        for text in texts {
            h.append(Event::User {
                blocks: vec![ContentBlock::Text {
                    text: text.to_owned(),
                }],
            })
            .unwrap();
        }
    }
}

fn harness() -> AssertCommand {
    AssertCommand::cargo_bin("harness").unwrap_or_else(|e| panic!("cargo_bin harness: {e}"))
}

fn verb(server: &Server, name: &str) -> AssertCommand {
    let mut command = harness();
    command
        .arg("sessions")
        .arg(name)
        .arg("--url")
        .arg(&server.base)
        .arg("--token")
        .arg(&server.token);
    command
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_list_prints_live_sessions() {
    let server = spawn(plain_factory()).await;
    server.state.session("alpha");
    server.state.session("beta");
    let assert = verb(&server, "list").assert().success();
    let output = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    println!("--- sessions list ---\n{output}");
    assert!(output.contains("alpha"), "{output}");
    assert!(output.contains("beta"), "{output}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_history_prints_run_rows_with_verdicts() {
    let server = spawn(plain_factory()).await;
    let history = RunHistory::open(&server.data_dir).unwrap();
    history
        .append(&RunRecord {
            automation_id: "nightly".into(),
            session_id: "alpha".into(),
            fired_at_minute: 1000,
            outcome: "started".into(),
            verdict: None,
            network: NetworkPolicy::Offline,
        })
        .unwrap();
    history
        .append(&RunRecord {
            automation_id: "nightly".into(),
            session_id: "alpha".into(),
            fired_at_minute: 1000,
            outcome: "task_completed".into(),
            verdict: Some("confirmed".into()),
            network: NetworkPolicy::Online,
        })
        .unwrap();
    let assert = verb(&server, "history").arg("alpha").assert().success();
    let output = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    println!("--- sessions history alpha ---\n{output}");
    assert!(output.contains("nightly"), "{output}");
    assert!(output.contains("started"), "{output}");
    assert!(output.contains("confirmed"), "{output}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_history_unknown_session_is_a_typed_failure() {
    let server = spawn(plain_factory()).await;
    verb(&server, "history")
        .arg("ghost")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown session"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_search_regression_guard_preserves_t11_behavior() {
    let server = spawn(plain_factory()).await;
    seed_search_fixture(&server.data_dir);
    // Ranked hits: the tf-rich session's first hit renders first.
    let assert = verb(&server, "search")
        .arg("needle")
        .assert()
        .success()
        .stdout(predicate::str::contains("session=s2"))
        .stdout(predicate::str::contains("session=s1"));
    let output = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let s2 = output.find("session=s2").unwrap_or_default();
    let s1 = output.find("session=s1").unwrap_or_default();
    assert!(
        s2 < s1 && s1 != 0,
        "bm25 order must survive the new dispatch:\n{output}"
    );
    // FTS5 syntax error stays a typed nonzero failure.
    let failed = verb(&server, "search").arg("\"unbalanced").assert().failure();
    let stderr = String::from_utf8_lossy(&failed.get_output().stderr).into_owned();
    let code = failed.get_output().status.code().unwrap_or_default();
    println!("--- FTS syntax error (exit {code}) ---\n{stderr}");
    assert!(stderr.contains("bad query"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attach_connection_negotiates_protocol_version_2() {
    let server = spawn(plain_factory()).await;
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let mut transport = SessionTransport::connect(&Connect {
        url: Some(server.base.clone()),
        token: Some(server.token.clone()),
        ipc_name: "negotiation-unused".into(),
        data_dir: dir.path().to_path_buf(),
        session: "negotiate".into(),
        after: LIVE_ONLY,
        client_id: "cli-negotiation-test".into(),
    })
    .await
    .unwrap_or_else(|e| panic!("connect: {e}"));
    let mut frames = 0;
    while transport.negotiated_version().is_none() {
        assert!(
            transport.next_msg().await.unwrap_or(None).is_some(),
            "stream ended before HelloAck"
        );
        frames += 1;
        assert!(frames < 64, "no HelloAck within 64 frames");
    }
    assert_eq!(
        transport.negotiated_version(),
        Some(2),
        "the CLI connection must negotiate protocol v2"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_output_json_contains_the_usage_frame() {
    let server = spawn(usage_factory()).await;
    let guard = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let output = harness()
        .current_dir(guard.path())
        .arg("run")
        .arg("--url")
        .arg(&server.base)
        .arg("--token")
        .arg(&server.token)
        .arg("--session")
        .arg("usage")
        .arg("--output-json")
        .arg("report the tokens")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output).into_owned();
    let mut saw_usage = false;
    let mut saw_result = false;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let value: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("ndjson line: {e}: {line}"));
        if value["type"] == "usage" {
            saw_usage = true;
            assert_eq!(value["input_tokens"], serde_json::json!(10));
            assert_eq!(value["output_tokens"], serde_json::json!(20));
            assert_eq!(value["model"]["model"], serde_json::json!("mock-1"));
        }
        if value["type"] == "turn_result" {
            saw_result = true;
            assert_eq!(value["ok"], serde_json::json!(true));
        }
    }
    assert!(saw_usage, "usage frame missing from NDJSON:\n{text}");
    assert!(saw_result, "turn_result missing from NDJSON:\n{text}");
}
