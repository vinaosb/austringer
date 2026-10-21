//! Todo 11 (ecosystem alignment): `harness sessions search` end-to-end
//! against the real daemon router — ranked hits rendered, empty is a
//! clean zero exit, FTS5 syntax errors are typed nonzero failures, and
//! the missing `--url` case carries the HTTP-transport hint. The
//! daemon runs in-process (the inproc_flow pattern); the CLI rides the
//! real binary via assert_cmd.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use assert_cmd::Command as AssertCommand;
use harness_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TASK_COMPLETED_TOOL,
};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use harness_core::event::{ContentBlock, Event};
use harness_daemon::config::DaemonConfig;
use harness_daemon::server::{DaemonState, build_router};
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

struct Server {
    base: String,
    token: String,
    _dir: tempfile::TempDir,
}

async fn spawn() -> Server {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    seed_fixture(&dir.path().join("d"));
    let config = DaemonConfig {
        data_dir: dir.path().join("d"),
        token: Some("search-cli-token".into()),
        ask_timeout: Duration::from_secs(20),
        ..DaemonConfig::default()
    };
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
        Arc::new(|| Box::new(CompleteOnly));
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
        token: "search-cli-token".into(),
        _dir: dir,
    }
}

fn seed_fixture(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap_or_default();
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

fn search(server: &Server, query: &str) -> AssertCommand {
    let mut command = harness();
    command
        .arg("sessions")
        .arg("search")
        .arg("--url")
        .arg(&server.base)
        .arg("--token")
        .arg(&server.token)
        .arg(query);
    command
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_search_cli_prints_ranked_hits() {
    let server = spawn().await;
    let mut command = search(&server, "needle");
    let assert = command
        .assert()
        .success()
        .stdout(predicate::str::contains("session=s2"))
        .stdout(predicate::str::contains("session=s1"));
    // Ranked order: the tf-rich session renders before the single-hit
    // one on the same stdout.
    let output = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    println!("--- ranked output for query \"needle\" ---\n{output}");
    let s2 = output.find("session=s2").unwrap_or_default();
    let s1 = output.find("session=s1").unwrap_or_default();
    assert!(
        s2 < s1 && s1 != 0,
        "bm25 order must survive rendering:\n{output}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_search_cli_empty_result_is_a_clean_zero_exit() {
    let server = spawn().await;
    // A single bareword: hyphenated strings parse as FTS5 column
    // filters (a syntax error), not as empty matches.
    search(&server, "zzznosuchterm")
        .assert()
        .success()
        .stdout(predicate::str::contains("no hits"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_search_cli_fts_syntax_error_is_a_typed_failure() {
    let server = spawn().await;
    search(&server, "\"unbalanced")
        .assert()
        .failure()
        .stderr(predicate::str::contains("bad query"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_search_cli_without_url_carries_the_transport_hint() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    harness()
        .current_dir(dir.path())
        .arg("sessions")
        .arg("search")
        .arg("needle")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--url"));
}
