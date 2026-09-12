//! Crash-restart-degrade lifecycle against the mock LS: crash → backoff →
//! restart → serve; second crash → typed degraded-to-search fallback;
//! hung initialize → typed timeout → restart → degrade; document-change
//! staleness via didChange; and config-reuse (resumable) across routers.

mod common;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use common::{Action, ClientEnds, Handler, MockLsp, spawn_mock, utf8_caps};
use harness_lsp::config::{Language, LspConfig};
use harness_lsp::error::{DegradeReason, LspError};
use harness_lsp::provider::{Answer, DocumentPosition, LspProvider, SEARCH_FALLBACK_HINT};
use harness_lsp::router::{LspRouter, RouterEvent};
use harness_lsp::uri::file_to_uri;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::sync::mpsc;

fn config(root: &TempDir) -> LspConfig {
    let mut cfg = LspConfig::new(root.path().to_path_buf());
    cfg.backoff.initial = Duration::from_millis(40);
    cfg.backoff.multiplier = 2.0;
    cfg.backoff.max = Duration::from_millis(200);
    cfg.initialize_timeout = Duration::from_millis(300);
    cfg.request_timeout = Duration::from_secs(2);
    cfg
}

fn fixture(root: &TempDir, rel: &str, text: &str) -> PathBuf {
    let path = root.path().join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdirs");
    std::fs::write(&path, text).expect("fixture write");
    path
}

fn pos(file: &Path, line: u32, column: u32) -> DocumentPosition {
    DocumentPosition {
        file: file.to_path_buf(),
        line,
        column,
    }
}

fn doc_version(params: &Value) -> Option<(String, i64)> {
    let doc = params.get("textDocument")?;
    let uri = doc.get("uri")?.as_str()?.to_owned();
    let version = doc.get("version").and_then(Value::as_i64)?;
    Some((uri, version))
}

/// Installs a mock-server FACTORY: every (re)spawn of the language's
/// server connects a fresh mock sharing one handler; mocks are retained
/// for control and inspection.
struct MockFarm {
    mocks: StdMutex<Vec<MockLsp>>,
    handler: Handler,
}

impl MockFarm {
    fn new(handler: Handler) -> Arc<Self> {
        Arc::new(Self {
            mocks: StdMutex::new(Vec::new()),
            handler,
        })
    }

    fn install(self: &Arc<Self>, router: &LspRouter) {
        let farm = Arc::clone(self);
        router.set_server_factory(
            Language::Rust,
            Arc::new(move || {
                let farm = Arc::clone(&farm);
                Box::pin(async move { farm.grow().await })
            }),
        );
    }

    async fn grow(&self) -> std::io::Result<ClientEnds> {
        let ((read, write), mock) = spawn_mock(Arc::clone(&self.handler)).await;
        self.mocks.lock().expect("test lock").push(mock);
        Ok((read, write))
    }

    fn count(&self) -> usize {
        self.mocks.lock().expect("test lock").len()
    }

    fn crash_latest(&self) {
        if let Some(mock) = self.mocks.lock().expect("test lock").last_mut() {
            mock.crash();
        }
    }

    fn pop_seen(&self) -> Option<(String, Value)> {
        let mut mocks = self.mocks.lock().expect("test lock");
        let mock = mocks.last_mut()?;
        mock.seen.try_recv().ok()
    }
}

fn default_definition(uri: &str) -> Action {
    Action::Result(json!([
        {"uri": uri, "range": {"start": {"line": 2, "character": 7},
                                "end": {"line": 2, "character": 7}}}
    ]))
}

async fn wait_for_crash_event(events: &mut mpsc::Receiver<RouterEvent>) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        if matches!(events.try_recv(), Ok(RouterEvent::ServerCrashed { .. })) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "ServerCrashed never arrived"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
async fn crash_then_restart_with_backoff_then_second_crash_degrades() {
    let root = TempDir::new().expect("tempdir");
    let file = fixture(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let target_uri = file_to_uri(&file).unwrap_or_default();
    let farm = MockFarm::new(Arc::new(move |m: &str, _p: &Value| match m {
        "initialize" => Action::Result(utf8_caps()),
        "textDocument/definition" => default_definition(&target_uri),
        _ => Action::Result(Value::Null),
    }));
    let router = LspRouter::new(config(&root));
    farm.install(&router);
    let mut events = router.take_events().expect("first take wins");

    // Round 1: first spawn, serve.
    let Answer::Locations(one) = router
        .goto_definition(pos(&file, 8, 5))
        .await
        .expect("first goto")
    else {
        panic!("expected locations")
    };
    assert_eq!(one.len(), 1);
    assert_eq!(farm.count(), 1, "exactly one server so far");
    assert!(matches!(
        events.try_recv(),
        Ok(RouterEvent::ServerSpawned { .. })
    ));

    // Kill -9 the server mid-session.
    farm.crash_latest();
    wait_for_crash_event(&mut events).await;

    // Round 2: one restart with backoff, then service resumes.
    let started = std::time::Instant::now();
    let Answer::Locations(two) = router
        .goto_definition(pos(&file, 8, 5))
        .await
        .expect("second goto")
    else {
        panic!("expected locations")
    };
    assert_eq!(two.len(), 1);
    assert_eq!(farm.count(), 2, "the crash consumed a fresh server");
    assert!(
        started.elapsed() >= Duration::from_millis(40),
        "restart waited the initial backoff (40ms), took {:?}",
        started.elapsed()
    );
    assert_eq!(
        events.try_recv(),
        Ok(RouterEvent::RestartScheduled {
            language: Language::Rust,
            attempt: 1,
            wait: Duration::from_millis(40)
        }),
        "backoff wait is exactly the deterministic initial backoff"
    );
    assert_eq!(
        events.try_recv(),
        Ok(RouterEvent::ServerRestarted {
            language: Language::Rust
        })
    );

    // Second kill: the restart budget (max_restarts = 1) is spent.
    farm.crash_latest();
    wait_for_crash_event(&mut events).await;

    let answer = router
        .goto_definition(pos(&file, 8, 5))
        .await
        .expect("third goto resolves");
    assert_eq!(
        answer,
        Answer::DegradedToSearch {
            reason: DegradeReason::RestartBudgetExhausted { crashes: 2 },
            hint: SEARCH_FALLBACK_HINT,
        },
        "second crash degrades to the typed search fallback"
    );
    assert_eq!(farm.count(), 2, "no further spawn attempts after degrade");
    assert!(matches!(
        events.try_recv(),
        Ok(RouterEvent::Degraded {
            reason: DegradeReason::RestartBudgetExhausted { crashes: 2 },
            ..
        })
    ));
}

#[tokio::test]
async fn hung_initialize_times_out_then_restarts_then_degrades() {
    let root = TempDir::new().expect("tempdir");
    let file = fixture(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let farm = MockFarm::new(Arc::new(|m: &str, _p: &Value| match m {
        "initialize" => Action::Hang, // never answers the handshake
        _ => Action::Result(Value::Null),
    }));
    let router = LspRouter::new(config(&root));
    farm.install(&router);

    // Call 1: fresh spawn, hung initialize -> typed timeout.
    let err = router
        .goto_definition(pos(&file, 1, 1))
        .await
        .expect_err("first call times out");
    assert!(
        matches!(err, LspError::InitializeTimeout { language: Language::Rust, waited } if waited == Duration::from_millis(300)),
        "got: {err}"
    );

    // Call 2: backoff, second spawn, hung again -> typed timeout again.
    let err = router
        .goto_definition(pos(&file, 1, 1))
        .await
        .expect_err("second call times out");
    assert!(
        matches!(err, LspError::InitializeTimeout { .. }),
        "got: {err}"
    );
    assert_eq!(farm.count(), 2);

    // Call 3: crash budget spent -> degraded typed fallback (not an error).
    let answer = router
        .goto_definition(pos(&file, 1, 1))
        .await
        .expect("degrades as a result");
    assert!(
        matches!(answer, Answer::DegradedToSearch { .. }),
        "got: {answer:?}"
    );
    assert_eq!(farm.count(), 2, "degraded slots never spawn again");
}

#[tokio::test]
async fn did_change_refreshes_results_and_versions_increment() {
    let root = TempDir::new().expect("tempdir");
    let file = fixture(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    std::fs::write(&file, "fn target() {}\nfn caller() { target() }\n").expect("fixture");
    let target_uri = file_to_uri(&file).unwrap_or_default();
    // The mock tracks per-document versions from didOpen/didChange and
    // answers definitions on the tracked line — a stale client cache would
    // keep returning the old position.
    let versions: Arc<StdMutex<HashMap<String, i64>>> = Arc::new(StdMutex::new(HashMap::new()));
    let handler: Handler = {
        let versions = Arc::clone(&versions);
        Arc::new(move |method: &str, params: &Value| match method {
            "initialize" => Action::Result(utf8_caps()),
            "textDocument/didOpen" | "textDocument/didChange" => {
                if let Some((uri, version)) = doc_version(params) {
                    versions.lock().expect("test lock").insert(uri, version);
                }
                Action::Result(Value::Null)
            }
            "textDocument/definition" => {
                let uri = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let version = versions
                    .lock()
                    .expect("test lock")
                    .get(&uri)
                    .copied()
                    .unwrap_or(1);
                Action::Result(json!([
                    {"uri": target_uri,
                     "range": {"start": {"line": version, "character": 0},
                               "end": {"line": version, "character": 0}}}
                ]))
            }
            _ => Action::Result(Value::Null),
        })
    };
    let farm = MockFarm::new(handler);
    let router = LspRouter::new(config(&root));
    farm.install(&router);

    let Answer::Locations(v1) = router
        .goto_definition(pos(&file, 2, 13))
        .await
        .expect("goto")
    else {
        panic!("expected locations")
    };
    assert_eq!(v1[0].line, 2, "version 1 -> wire line 1 (0-based)");

    let new_text = "// shifted\nfn target() {}\nfn caller() { target() }\n";
    router
        .notify_did_change(&file, new_text)
        .await
        .expect("did change");

    let Answer::Locations(v2) = router
        .goto_definition(pos(&file, 3, 13))
        .await
        .expect("goto 2")
    else {
        panic!("expected locations")
    };
    assert_eq!(v2[0].line, 3, "version 2 -> wire line 2: results are fresh");
}

#[tokio::test]
async fn did_change_carries_the_full_new_text() {
    let root = TempDir::new().expect("tempdir");
    let file = fixture(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    std::fs::write(&file, "fn target() {}\n").expect("fixture");
    let farm = MockFarm::new(Arc::new(|m: &str, _p: &Value| match m {
        "initialize" => Action::Result(utf8_caps()),
        _ => Action::Result(Value::Null),
    }));
    let router = LspRouter::new(config(&root));
    farm.install(&router);
    // Opens the document server-side.
    let _ = router.goto_definition(pos(&file, 1, 1)).await;

    let new_text = "// one\n// two\nfn target() {}\n";
    router
        .notify_did_change(&file, new_text)
        .await
        .expect("did change");

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        match farm.pop_seen() {
            Some((method, msg)) if method == "textDocument/didChange" => {
                let change = msg
                    .get("params")
                    .and_then(|p| p.get("contentChanges"))
                    .and_then(|c| c.get(0))
                    .expect("didChange has content changes");
                let text = change
                    .get("text")
                    .and_then(Value::as_str)
                    .expect("full text");
                assert_eq!(
                    text, new_text,
                    "full-text sync: the new document text is on the wire"
                );
                let version = msg
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.get("version"))
                    .and_then(Value::as_i64);
                assert_eq!(version, Some(2), "version increments per change");
                return;
            }
            Some(_) => continue,
            None => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "no didChange observed"
                );
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }
}

#[tokio::test]
async fn a_restarted_router_reuses_the_configured_table() {
    let root = TempDir::new().expect("tempdir");
    let file = fixture(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let target_uri = file_to_uri(&file).unwrap_or_default();
    let shared_cfg = config(&root);
    let handler: Handler = Arc::new(move |m: &str, _p: &Value| match m {
        "initialize" => Action::Result(utf8_caps()),
        "textDocument/definition" => default_definition(&target_uri),
        _ => Action::Result(Value::Null),
    });

    // "Client restart": router 1 dies, router 2 builds from the same config.
    let farm1 = MockFarm::new(Arc::clone(&handler));
    let router1 = LspRouter::new(shared_cfg.clone());
    farm1.install(&router1);
    let Answer::Locations(a) = router1.goto_definition(pos(&file, 8, 5)).await.expect("r1") else {
        panic!("expected locations")
    };
    assert_eq!(a.len(), 1);
    drop(router1);

    let farm2 = MockFarm::new(handler);
    let router2 = LspRouter::new(shared_cfg.clone());
    farm2.install(&router2);
    let Answer::Locations(b) = router2.goto_definition(pos(&file, 8, 5)).await.expect("r2") else {
        panic!("expected locations")
    };
    assert_eq!(b.len(), 1, "same config table serves the new client");
    assert_eq!(b[0].line, 3);
}

#[tokio::test]
async fn graceful_shutdown_of_all_servers() {
    let root = TempDir::new().expect("tempdir");
    let file = fixture(
        &root,
        "src/lib.rs",
        "fn target() {}\nfn caller() { target() }\n",
    );
    let target_uri = file_to_uri(&file).unwrap_or_default();
    let farm = MockFarm::new(Arc::new(move |m: &str, _p: &Value| match m {
        "initialize" => Action::Result(utf8_caps()),
        "textDocument/definition" => default_definition(&target_uri),
        _ => Action::Result(Value::Null),
    }));
    let router = LspRouter::new(config(&root));
    farm.install(&router);
    router
        .goto_definition(pos(&file, 8, 5))
        .await
        .expect("goto");

    router.shutdown().await.expect("shutdown");
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        match farm.pop_seen() {
            Some((method, _)) if method == "shutdown" => return,
            Some(_) => continue,
            None => {
                assert!(std::time::Instant::now() < deadline, "no shutdown observed");
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }
}
