//! Flow tests against the REAL daemon server (todo 35 router + engine)
//! with scripted inference, driving the CLI binary end-to-end: headless
//! happy runs in both output modes, the permission ask answered from
//! stdin, the REPL, and doctor's clean bill. The scripted providers are
//! the daemon crate's own test pattern (tests/common/mod.rs).

use std::sync::Arc;
use std::time::Duration;

use assert_cmd::Command as AssertCommand;
use austringer_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TASK_COMPLETED_TOOL,
};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::event::ContentBlock;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::server::{DaemonState, build_router};
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

/// First call asks for a non-reserved tool (drives the permission
/// mediator), then completes.
struct AskThenComplete {
    asked: std::sync::atomic::AtomicBool,
}
#[async_trait::async_trait]
impl Inference for AskThenComplete {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        if self.asked.swap(true, std::sync::atomic::Ordering::SeqCst) {
            Ok(AssistantTurn {
                blocks: vec![ContentBlock::Text {
                    text: "finished after denial".into(),
                }],
                tool_calls: vec![PendingCall {
                    call_id: "done".into(),
                    tool: TASK_COMPLETED_TOOL.to_owned(),
                    arguments: serde_json::json!({}),
                }],
                usage: None,
            })
        } else {
            Ok(AssistantTurn {
                blocks: vec![ContentBlock::Text {
                    text: "working".into(),
                }],
                tool_calls: vec![PendingCall {
                    call_id: "t1".into(),
                    tool: "echo".into(),
                    arguments: serde_json::json!({"q": 1}),
                }],
                usage: None,
            })
        }
    }
}

struct Server {
    base: String,
    token: String,
    _dir: tempfile::TempDir,
}

async fn spawn(inference: fn() -> Box<dyn Inference>) -> Server {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let config = DaemonConfig {
        data_dir: dir.path().join("d"),
        token: Some("inproc-token".into()),
        ask_timeout: Duration::from_secs(20),
        ..DaemonConfig::default()
    };
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> = Arc::new(move || inference());
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
        token: "inproc-token".into(),
        _dir: dir,
    }
}

fn austringer() -> AssertCommand {
    AssertCommand::cargo_bin("austringer").unwrap_or_else(|e| panic!("cargo_bin austringer: {e}"))
}

fn cwd_quarantine() -> tempfile::TempDir {
    tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"))
}

fn complete_only() -> Box<dyn Inference> {
    Box::new(CompleteOnly)
}

fn ask_then_complete() -> Box<dyn Inference> {
    Box::new(AskThenComplete {
        asked: std::sync::atomic::AtomicBool::new(false),
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn headless_run_json_mode_observes_the_full_turn() {
    let server = spawn(complete_only).await;
    let guard = cwd_quarantine();
    let output = austringer()
        .current_dir(guard.path())
        .arg("run")
        .arg("--url")
        .arg(&server.base)
        .arg("--token")
        .arg(&server.token)
        .arg("--session")
        .arg("json")
        .arg("--output-json")
        .arg("hello there")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output).into_owned();
    let mut saw_user = false;
    let mut saw_assistant = false;
    let mut result: Option<serde_json::Value> = None;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let value: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("ndjson line: {e}: {line}"));
        if value.get("event").is_some() {
            let event = &value["event"];
            if event["event"]["type"] == "user" {
                saw_user = true;
                assert_eq!(event["event"]["blocks"][0]["text"], "hello there");
            }
            if event["event"]["type"] == "assistant" {
                saw_assistant = true;
                assert_eq!(event["event"]["blocks"][0]["text"], "done");
            }
        } else if value["type"] == "turn_result" {
            result = Some(value);
        }
    }
    assert!(saw_user, "must OBSERVE the recorded user prompt: {text}");
    assert!(saw_assistant, "must OBSERVE the assistant text: {text}");
    let result = result.unwrap_or_else(|| panic!("missing turn_result line: {text}"));
    assert_eq!(result["ok"], serde_json::json!(true));
    assert_eq!(result["outcome"], serde_json::json!("task_completed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn headless_run_human_mode_prints_the_assistant_text() {
    let server = spawn(complete_only).await;
    let guard = cwd_quarantine();
    austringer()
        .current_dir(guard.path())
        .arg("run")
        .arg("--url")
        .arg(&server.base)
        .arg("--token")
        .arg(&server.token)
        .arg("--session")
        .arg("human")
        .arg("say something")
        .assert()
        .success()
        .stdout(predicate::str::contains("done"))
        .stdout(predicate::str::contains("say something"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permission_ask_is_answered_from_stdin() {
    let server = spawn(ask_then_complete).await;
    let guard = cwd_quarantine();
    austringer()
        .current_dir(guard.path())
        .arg("run")
        .arg("--url")
        .arg(&server.base)
        .arg("--token")
        .arg(&server.token)
        .arg("--session")
        .arg("perm")
        .arg("--timeout")
        .arg("60")
        .arg("needs a tool")
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("permission"))
        .stdout(predicate::str::contains("denied"))
        .stdout(predicate::str::contains("task_completed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repl_streams_turns_and_exits_on_command() {
    let server = spawn(complete_only).await;
    let guard = cwd_quarantine();
    austringer()
        .current_dir(guard.path())
        .arg("repl")
        .arg("--url")
        .arg(&server.base)
        .arg("--token")
        .arg(&server.token)
        .arg("--session")
        .arg("repl1")
        .write_stdin("first line\n/exit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("first line"))
        .stdout(predicate::str::contains("done"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn doctor_is_green_against_a_clean_environment() {
    let server = spawn(complete_only).await;
    let guard = cwd_quarantine();
    austringer()
        .current_dir(guard.path())
        .arg("doctor")
        .arg("--url")
        .arg(&server.base)
        .arg("--token")
        .arg(&server.token)
        .env_remove("austringer_PROVIDER")
        .assert()
        .success()
        .stdout(predicate::str::contains("FAIL").not());
}
