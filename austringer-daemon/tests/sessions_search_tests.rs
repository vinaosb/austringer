//! Todo 11 (ecosystem alignment): the event_fts SELECT surface — the
//! `GET /sessions/search` route (cross-session over the daemon's ONE
//! store root, empty = 200 `[]`, FTS5 syntax errors = typed 400) and
//! the model-facing `search_sessions` tool through the task-3
//! per-session registry (session-scoped BY DEFAULT, bounded, explicit
//! ids for cross-session). Hermetic: scripted inference, zero network,
//! zero keys.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use austringer_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TASK_COMPLETED_TOOL,
};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::event::{ContentBlock, Event};
use austringer_daemon::attach::HostCommand;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::mediation::MediationPolicy;
use austringer_daemon::protocol::{Answer, ServerMsg, SessionEvent};
use austringer_daemon::server::{DaemonState, build_router};
use austringer_daemon::tools::CoreExecutors;
use austringer_sessions::{LeaseOwner, SessionStore};

type Factory = Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>;

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

/// Bind a fixture daemon on an ephemeral loopback port over `dir` with
/// the in-process CORE_TOOLS executors injected (the binary's startup
/// wiring, mirrored — search_sessions registers through the same
/// registry).
async fn spawn_in(dir: &Path, factory: Factory) -> (String, String) {
    let mut state = DaemonState::new(
        DaemonConfig {
            data_dir: dir.to_owned(),
            token: Some("search-token".into()),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_secs(30),
            durable: false,
            ..DaemonConfig::default()
        },
        factory,
    )
    .expect("daemon state");
    let executors = Arc::new(CoreExecutors::in_process());
    state.executor_factory = executors.executor_factory();
    let state = Arc::new(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let app = build_router(Arc::clone(&state));
    tokio::spawn(async move {
        let serve = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        let _ = serve.await;
    });
    (format!("127.0.0.1:{port}"), "search-token".into())
}

async fn get(base: &str, token: &str, path_and_query: &str) -> (u16, serde_json::Value) {
    let response = reqwest::Client::new()
        .get(format!("http://{base}{path_and_query}"))
        .bearer_auth(token)
        .send()
        .await
        .expect("http");
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    let json = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn post_cmd(base: &str, token: &str, sid: &str, text: &str) -> u16 {
    let body = serde_json::json!({
        "type": "command",
        "command": { "type": "submit", "text": text }
    })
    .to_string();
    let response = reqwest::Client::new()
        .post(format!("http://{base}/s/{sid}/cmd"))
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .expect("http");
    response.status().as_u16()
}

fn user(text: &str) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

/// Seed the persisted store under the daemon's data dir (the engine's
/// in-memory ring never touches it; the search surface reads the store).
fn seed_store(dir: &Path, id: &str, texts: &[&str]) {
    let store = SessionStore::open(dir).unwrap();
    store.create(id, "").unwrap();
    let h = store.open_session_rw(id, &LeaseOwner::new("seed")).unwrap();
    for text in texts {
        h.append(user(text)).unwrap();
    }
}

/// The ranked fixture: s1 mentions the needle once, s2 three times.
fn seed_ranked_fixture(dir: &Path) {
    seed_store(
        dir,
        "s1",
        &["unrelated opener", "the needle appears once here"],
    );
    seed_store(
        dir,
        "s2",
        &[
            "needle needle in the haystack",
            "another needle sighting",
            "needle once more, third time",
        ],
    );
    seed_store(dir, "s3", &["no mention at all"]);
}

// ---------------------------------------------------------------------------
// GET /sessions/search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sessions_search_route_ranks_hits_across_sessions() {
    let dir = tempfile::tempdir().unwrap();
    seed_ranked_fixture(dir.path());
    let complete: Factory =
        Arc::new(|| Box::new(ScriptedInference::from_steps(vec![completion_turn()])));
    let (base, token) = spawn_in(dir.path(), complete).await;

    let (status, body) = get(&base, &token, "/sessions/search?q=needle").await;
    assert_eq!(status, 200, "{body}");
    let hits = body.as_array().unwrap_or_else(|| panic!("array: {body}"));
    assert_eq!(hits.len(), 4, "every needle event ranks: {body}");
    // bm25 ascending, ids + seq on every hit, s2 (term-frequency rich)
    // first.
    let mut previous = f64::NEG_INFINITY;
    for hit in hits {
        let bm25 = hit["bm25"]
            .as_f64()
            .unwrap_or_else(|| panic!("bm25: {body}"));
        assert!(bm25 >= previous, "ranked best-first: {body}");
        previous = bm25;
        assert!(hit["session_id"].is_string(), "{body}");
        assert!(hit["seq"].as_u64().is_some(), "{body}");
        assert!(hit["snippet"].is_string(), "{body}");
    }
    assert_eq!(hits[0]["session_id"], "s2", "tf-rich session first: {body}");
}

#[tokio::test]
async fn sessions_search_route_scopes_to_listed_sessions() {
    let dir = tempfile::tempdir().unwrap();
    seed_ranked_fixture(dir.path());
    let complete: Factory =
        Arc::new(|| Box::new(ScriptedInference::from_steps(vec![completion_turn()])));
    let (base, token) = spawn_in(dir.path(), complete).await;

    let (status, body) = get(&base, &token, "/sessions/search?q=needle&sessions=s1").await;
    assert_eq!(status, 200, "{body}");
    let hits = body.as_array().unwrap_or_else(|| panic!("array: {body}"));
    assert_eq!(hits.len(), 1, "{body}");
    assert_eq!(hits[0]["session_id"], "s1", "{body}");
}

#[tokio::test]
async fn sessions_search_route_empty_result_is_200_empty_array() {
    let dir = tempfile::tempdir().unwrap();
    seed_ranked_fixture(dir.path());
    let complete: Factory =
        Arc::new(|| Box::new(ScriptedInference::from_steps(vec![completion_turn()])));
    let (base, token) = spawn_in(dir.path(), complete).await;

    // A single bareword: hyphenated strings parse as FTS5 column
    // filters (a syntax error), not as empty matches.
    let (status, body) = get(&base, &token, "/sessions/search?q=zzznosuchterm").await;
    assert_eq!(status, 200, "empty is a 200, never a 404: {body}");
    assert_eq!(body, serde_json::json!([]), "bare empty array: {body}");
}

#[tokio::test]
async fn sessions_search_route_fts_syntax_error_is_typed_400_not_500() {
    let dir = tempfile::tempdir().unwrap();
    seed_ranked_fixture(dir.path());
    let complete: Factory =
        Arc::new(|| Box::new(ScriptedInference::from_steps(vec![completion_turn()])));
    let (base, token) = spawn_in(dir.path(), complete).await;

    let (status, body) = get(&base, &token, "/sessions/search?q=%22unbalanced").await;
    assert_ne!(status, 500, "syntax errors must not surface as 500: {body}");
    assert_eq!(status, 400, "typed 400 required: {body}");
    assert_eq!(
        body["error"]["code"], "bad_query",
        "typed error body: {body}"
    );
    assert!(
        body["error"]["detail"].as_str().is_some(),
        "the parser detail rides along: {body}"
    );
}

#[tokio::test]
async fn sessions_search_route_missing_query_is_typed_400() {
    let dir = tempfile::tempdir().unwrap();
    let complete: Factory =
        Arc::new(|| Box::new(ScriptedInference::from_steps(vec![completion_turn()])));
    let (base, token) = spawn_in(dir.path(), complete).await;

    let (status, body) = get(&base, &token, "/sessions/search").await;
    assert_eq!(status, 400, "missing q is a typed 400: {body}");
    assert_eq!(body["error"]["code"], "missing_query", "{body}");
}

// ---------------------------------------------------------------------------
// The model-facing search_sessions tool (task-3 registry)
// ---------------------------------------------------------------------------

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

/// Answers every permission ask with AllowOnce (aux tools like
/// search_sessions ask per call; subscribe happens BEFORE the spawn so
/// the engine's ask can never race ahead of the subscription).
fn spawn_answerer(
    mediator: Arc<austringer_daemon::mediation::PermissionMediator>,
) -> tokio::task::JoinHandle<()> {
    let mut watch = mediator.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = watch.recv().await {
            if let ServerMsg::PermissionAsk { ask } = msg {
                let _ = mediator.answer(ask.ask_id, "tui", Answer::AllowOnce);
            }
        }
    })
}

async fn drive_turn(state: &DaemonState, sid: &str) -> Vec<String> {
    let host = state.session(sid);
    let attach = host.attach("tui", None).expect("attach");
    let answerer = spawn_answerer(host.mediator());
    let reader = tokio::spawn(read_live_until_finished(attach.events.live));
    host.handle_command(HostCommand::Submit("go".into()))
        .await
        .expect("submit");
    let collected = tokio::time::timeout(Duration::from_secs(90), reader)
        .await
        .expect("turn never finished")
        .expect("reader failed");
    answerer.abort();
    collected
}

fn bound_state(data_dir: &Path, factory: Factory) -> Arc<DaemonState> {
    let mut state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            token: Some("bound-token".into()),
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

#[tokio::test]
async fn search_tool_is_session_scoped_by_default() {
    const SID: &str = "s-tool-default";
    let dir = tempfile::tempdir().unwrap();
    // The live session's transcript rows in the persisted store …
    seed_store(dir.path(), SID, &["needle in the live session"]);
    // … and OTHER sessions that must NOT leak into the default scope.
    seed_store(dir.path(), "s-other", &["needle in a different session"]);

    let script: Factory = Arc::new(|| {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "searching",
                vec![call(
                    "t1",
                    "search_sessions",
                    serde_json::json!({ "query": "needle" }),
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
        "search_sessions must be registered through the task-3 registry:\n{joined}"
    );
    assert!(
        joined.contains(&format!("session={SID}")),
        "the default scope finds THIS session's hit:\n{joined}"
    );
    assert!(
        !joined.contains("session=s-other"),
        "other sessions never enter the default scope (provider-egress boundary):\n{joined}"
    );
}

#[tokio::test]
async fn search_tool_cross_session_requires_explicit_session_ids() {
    const SID: &str = "s-tool-cross";
    let dir = tempfile::tempdir().unwrap();
    seed_store(dir.path(), SID, &["needle in the live session"]);
    seed_store(dir.path(), "s-other", &["needle in a different session"]);

    let script: Factory = Arc::new(|| {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "searching wider",
                vec![call(
                    "t1",
                    "search_sessions",
                    serde_json::json!({
                        "query": "needle",
                        "session_ids": ["s-other"],
                    }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);
    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("session=s-other"),
        "explicit ids unlock cross-session search:\n{joined}"
    );
    assert!(
        !joined.contains(&format!("session={SID}")),
        "only the LISTED ids are searched:\n{joined}"
    );
}

#[tokio::test]
async fn search_tool_results_are_bounded_even_with_a_huge_limit() {
    const SID: &str = "s-tool-bound";
    let dir = tempfile::tempdir().unwrap();
    // More matching events than the tool's hard cap: 30 needle rows.
    let owned: Vec<String> = (0..30)
        .map(|i| format!("needle occurrence number {i}"))
        .collect();
    let rows: Vec<&str> = owned.iter().map(String::as_str).collect();
    seed_store(dir.path(), SID, &rows);

    let script: Factory = Arc::new(|| {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "searching big",
                vec![call(
                    "t1",
                    "search_sessions",
                    serde_json::json!({ "query": "needle", "limit": 1000 }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);
    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("25 hit(s)"),
        "a limit of 1000 over 30 matches must clamp to the hard cap of 25:\n{joined}"
    );
}

#[tokio::test]
async fn search_tool_bad_fts_syntax_is_a_typed_invalid_arguments_error() {
    const SID: &str = "s-tool-syntax";
    let dir = tempfile::tempdir().unwrap();
    seed_store(dir.path(), SID, &["plain text"]);

    let script: Factory = Arc::new(|| {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "searching badly",
                vec![call(
                    "t1",
                    "search_sessions",
                    serde_json::json!({ "query": "\"unbalanced" }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);
    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("invalid arguments for tool \\\"search_sessions\\\""),
        "FTS5 syntax errors are typed invalid-arguments (model-correctable), not 500s:\n{joined}"
    );
    assert!(
        !joined.contains("panic"),
        "the session continues past the bad query:\n{joined}"
    );
}

// ---------------------------------------------------------------------------
// Live feed: the engine's recorded events land in the store, so a real
// conversation's terms rank through the same daemon's search route
// (the 2026-10-23 ledger follow-up's acceptance, verbatim)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn live_conversation_events_reach_the_search_index() {
    let dir = tempfile::tempdir().unwrap();
    let complete: Factory = Arc::new(|| {
        Box::new(ScriptedInference::from_steps(vec![text_turn(
            "the quokka phenomenon is unlike anything else",
            vec![],
        )]))
    });
    let (base, token) = spawn_in(dir.path(), complete).await;
    const SID: &str = "live-feed-1";

    let status = post_cmd(&base, &token, SID, "tell me about quokkas").await;
    assert_eq!(status, 202, "submit accepted");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let (status, body) = get(&base, &token, "/sessions/search?q=quokka").await;
        assert_eq!(status, 200, "{body}");
        let hits = body.as_array().expect("array");
        let ours = hits
            .iter()
            .any(|hit| hit["session_id"] == SID && hit["snippet"].to_string().contains("quokka"));
        if ours || std::time::Instant::now() > deadline {
            assert!(
                ours,
                "the live session's terms must rank in the same daemon's search route: {body}"
            );
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
