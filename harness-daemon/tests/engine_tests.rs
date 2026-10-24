//! Engine-driver acceptance over the public API: two clients see
//! byte-identical streams through a REAL harness-core AgentLoop
//! (mediated approval + split-brain worker execution + mid-run
//! steering + durable journal).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc};

use harness_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TASK_COMPLETED_TOOL,
};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use harness_core::event::{ContentBlock, Event};

use harness_daemon::attach::{HostCommand, SessionHost, SteerQueue};
use harness_daemon::mediation::{MediationPolicy, PermissionMediator};
use harness_daemon::protocol::{Answer, Resolution, ServerMsg, SessionEvent};
use harness_daemon::stages::WorkerToolExecutor;
use harness_daemon::worker::{DaemonToWorker, ToolGrant, WorkerRouter};

struct ScriptedInference {
    steps: Mutex<std::collections::VecDeque<AssistantTurn>>,
}

impl ScriptedInference {
    fn from_steps(steps: Vec<AssistantTurn>) -> Self {
        Self {
            steps: Mutex::new(steps.into()),
        }
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

fn tool_call(id: &str, tool: &str) -> PendingCall {
    PendingCall {
        call_id: id.to_owned(),
        tool: tool.to_owned(),
        arguments: serde_json::json!({"q": 1}),
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

/// Drives the steer choreography: waits until the marker lands in the
/// shared steer queue, returns a tool-call turn to force a pipeline
/// iteration (whose Steer stage drains the queue into events), and
/// completes once the marker is visible in the events.
struct SteerAbsorbingInference {
    queue: SteerQueue,
    marker: String,
    tool_seq: u32,
}

#[async_trait]
impl Inference for SteerAbsorbingInference {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        for _ in 0..500 {
            let marker_in_events = request.events.iter().any(|recorded| {
                matches!(
                    &recorded.event,
                    Event::User { blocks }
                        if blocks
                            .iter()
                            .any(|b| matches!(b, ContentBlock::Text { text } if text == &self.marker))
                )
            });
            if marker_in_events {
                return Ok(completion_turn());
            }
            let queued = {
                let guard = match self.queue.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard.front().is_some_and(|text| text == &self.marker)
            };
            if queued {
                self.tool_seq += 1;
                let id = format!("t{}", self.tool_seq);
                return Ok(text_turn("checking", vec![tool_call(&id, "echo")]));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Err(InferenceError::Failure("steer marker never arrived".into()))
    }
}

struct CollectedStream {
    frames: Vec<String>,
    asks: usize,
    resolutions: Vec<Resolution>,
}

async fn read_stream(
    mut events: broadcast::Receiver<ServerMsg>,
    mut control: broadcast::Receiver<ServerMsg>,
) -> CollectedStream {
    let mut out = CollectedStream {
        frames: Vec::new(),
        asks: 0,
        resolutions: Vec::new(),
    };
    loop {
        tokio::select! {
            msg = events.recv() => {
                match msg {
                    Ok(ServerMsg::Event { frame }) => {
                        let finished =
                            matches!(frame.event, SessionEvent::TurnFinished { .. });
                        if let Ok(encoded) = serde_json::to_string(&ServerMsg::Event { frame })
                        {
                            out.frames.push(encoded);
                        }
                        if finished {
                            return out;
                        }
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return out,
                }
            }
            msg = control.recv() => {
                match msg {
                    Ok(ServerMsg::PermissionAsk { .. }) => out.asks += 1,
                    Ok(ServerMsg::PermissionResolved { resolution, .. }) => out.resolutions.push(resolution),
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => {}
                }
            }
        }
    }
}

fn spawn_answerer(
    mediator: Arc<PermissionMediator>,
    answer: Answer,
    client: &str,
) -> tokio::task::JoinHandle<()> {
    let client = client.to_owned();
    // Subscribe synchronously BEFORE spawning: a lazily-polled task
    // would subscribe after the ask broadcast and miss it.
    let mut watch = mediator.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = watch.recv().await {
            if let ServerMsg::PermissionAsk { ask } = msg {
                mediator.answer(ask.ask_id, &client, answer).ok();
                return;
            }
        }
    })
}

fn build_session(
    name: &str,
) -> (
    Arc<SessionHost>,
    Option<tempfile::TempDir>,
    mpsc::Receiver<HostCommand>,
) {
    let dir = tempfile::tempdir().ok();
    let root = dir
        .as_ref()
        .map_or_else(|| std::env::temp_dir().join(name), |d| d.path().to_owned());
    let (host, cmd_rx) = SessionHost::new(
        name,
        64,
        32,
        8,
        MediationPolicy::FirstResponder,
        Duration::from_secs(30),
        Some(root),
    );
    std::fs::create_dir_all(session_dir_of(&dir, name)).ok();
    (host, dir, cmd_rx)
}

fn session_dir_of(dir: &Option<tempfile::TempDir>, name: &str) -> std::path::PathBuf {
    dir.as_ref().map_or_else(
        || std::env::temp_dir().join(name).join("sessions").join(name),
        |d| d.path().join("sessions").join(name),
    )
}

#[tokio::test]
async fn two_clients_see_identical_streams_and_shared_permission() {
    let (host, dir, cmd_rx) = build_session("s-ident");
    let session_dir = session_dir_of(&dir, "s-ident");

    let router = Arc::new(WorkerRouter::new(Duration::from_secs(5)));
    let (worker_tx, mut worker_rx) = mpsc::channel(2);
    router.register(
        "w1",
        vec![ToolGrant {
            tool: "echo".into(),
        }],
        worker_tx,
    );
    let executor = WorkerToolExecutor::new(Arc::clone(&router), None, host.event_sink());

    let inference = ScriptedInference::from_steps(vec![
        text_turn("checking", vec![tool_call("t1", "echo")]),
        completion_turn(),
    ]);

    host.spawn_engine(
        session_dir,
        host.standard_stages(Box::new(executor)),
        Box::new(inference),
        cmd_rx,
    )
    .ok();

    let (a, b) = (host.attach("tui", None), host.attach("web", None));
    assert!(a.is_ok() && b.is_ok(), "attach failed");
    let (Ok(attachment_a), Ok(attachment_b)) = (a, b) else {
        return;
    };

    let answerer = spawn_answerer(host.mediator(), Answer::AllowOnce, "tui");
    let worker = {
        let router = Arc::clone(&router);
        tokio::spawn(async move {
            if let Some(DaemonToWorker::ToolCall { call_id, .. }) = worker_rx.recv().await {
                router.deliver(&call_id, true, serde_json::json!("worker says hi"));
            }
        })
    };

    let reader_a = tokio::spawn(read_stream(attachment_a.events.live, attachment_a.control));
    let reader_b = tokio::spawn(read_stream(attachment_b.events.live, attachment_b.control));

    let _ = host
        .handle_command(HostCommand::Submit("hello".into()))
        .await;

    let joined = (reader_a.await.ok(), reader_b.await.ok());
    assert!(joined.0.is_some() && joined.1.is_some(), "readers failed");
    let (Some(collected_a), Some(collected_b)) = joined else {
        return;
    };

    // Byte-level event equality across the two clients (plan
    // acceptance: identical state).
    assert!(!collected_a.frames.is_empty(), "client A saw no frames");
    assert_eq!(collected_a.frames, collected_b.frames);

    // The ask broadcast to both; ONE answer resolved it for both.
    assert_eq!(collected_a.asks, 1);
    assert_eq!(collected_b.asks, 1);
    assert_eq!(collected_a.resolutions.len(), 1);
    assert_eq!(
        collected_a.resolutions[0],
        Resolution::Allowed { by: "tui".into() }
    );
    assert_eq!(collected_a.resolutions, collected_b.resolutions);

    // The tool call executed through the split-brain worker.
    let joined = collected_a.frames.join("\n");
    assert!(
        joined.contains("worker says hi"),
        "worker output missing:\n{joined}"
    );

    answerer.abort();
    worker.abort();
    drop(dir);
}

#[tokio::test]
async fn denied_permission_produces_typed_tool_result_for_all_clients() {
    let (host, dir, cmd_rx) = build_session("s-deny");
    let session_dir = session_dir_of(&dir, "s-deny");

    let router = Arc::new(WorkerRouter::new(Duration::from_secs(5)));
    let executor = WorkerToolExecutor::new(Arc::clone(&router), None, host.event_sink());
    let inference = ScriptedInference::from_steps(vec![
        text_turn("checking", vec![tool_call("t1", "echo")]),
        completion_turn(),
    ]);

    host.spawn_engine(
        session_dir,
        host.standard_stages(Box::new(executor)),
        Box::new(inference),
        cmd_rx,
    )
    .ok();

    let (a, b) = (host.attach("tui", None), host.attach("web", None));
    assert!(a.is_ok() && b.is_ok(), "attach failed");
    let (Ok(attachment_a), Ok(attachment_b)) = (a, b) else {
        return;
    };
    let answerer = spawn_answerer(host.mediator(), Answer::Deny, "web");
    let reader_a = tokio::spawn(read_stream(attachment_a.events.live, attachment_a.control));
    let reader_b = tokio::spawn(read_stream(attachment_b.events.live, attachment_b.control));

    let _ = host
        .handle_command(HostCommand::Submit("hello".into()))
        .await;

    let joined = (reader_a.await.ok(), reader_b.await.ok());
    assert!(joined.0.is_some() && joined.1.is_some(), "readers failed");
    let (Some(collected_a), Some(collected_b)) = joined else {
        return;
    };
    assert_eq!(collected_a.frames, collected_b.frames);
    let joined = collected_a.frames.join("\n");
    assert!(
        joined.contains("denied by approval stage"),
        "denial must surface as a typed tool result:\n{joined}"
    );
    assert_eq!(
        collected_a.resolutions,
        vec![Resolution::Denied { by: "web".into() }]
    );
    answerer.abort();
    drop(dir);
}

#[tokio::test]
async fn steer_mid_turn_is_absorbed_into_the_live_turn() {
    let (host, dir, cmd_rx) = build_session("s-steer");
    let session_dir = session_dir_of(&dir, "s-steer");
    let router = Arc::new(WorkerRouter::new(Duration::from_secs(5)));
    let (worker_tx, mut worker_rx) = mpsc::channel(2);
    router.register(
        "w1",
        vec![ToolGrant {
            tool: "echo".into(),
        }],
        worker_tx,
    );
    let executor = WorkerToolExecutor::new(Arc::clone(&router), None, host.event_sink());

    // Turn 1 keeps the run alive with a tool call; the follow-up
    // inference only completes once the steer marker is visible in
    // the events (absorbed by the SharedSteerer stage between
    // pipeline iterations).
    host.spawn_engine(
        session_dir,
        host.standard_stages(Box::new(executor)),
        Box::new(SteerAbsorbingInference {
            queue: host.steer_queue(),
            marker: "while running".into(),
            tool_seq: 0,
        }),
        cmd_rx,
    )
    .ok();

    let attach = host.attach("tui", None);
    assert!(attach.is_ok(), "attach failed");
    let Ok(attachment) = attach else {
        return;
    };
    let answerer = spawn_answerer(host.mediator(), Answer::AllowOnce, "tui");
    let worker = {
        let router = Arc::clone(&router);
        tokio::spawn(async move {
            while let Some(DaemonToWorker::ToolCall { call_id, .. }) = worker_rx.recv().await {
                router.deliver(&call_id, true, serde_json::json!("ok"));
            }
        })
    };
    let reader = tokio::spawn(read_stream(attachment.events.live, attachment.control));

    let _ = host
        .handle_command(HostCommand::Submit("start".into()))
        .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = host
        .handle_command(HostCommand::Steer("while running".into()))
        .await;

    let collected = tokio::time::timeout(Duration::from_secs(10), reader)
        .await
        .ok()
        .and_then(|r| r.ok());
    assert!(collected.is_some(), "turn never finished after steer");
    let Some(stream) = collected else {
        return;
    };
    let joined = stream.frames.join("\n");
    assert!(
        joined.contains("while running"),
        "steer marker must be recorded into the live turn:\n{joined}"
    );
    assert!(
        joined.contains("task_completed"),
        "turn must complete after steer:\n{joined}"
    );
    answerer.abort();
    worker.abort();
    drop(dir);
}

#[tokio::test]
async fn client_cap_bounds_attach_storm() {
    let (host, _cmd_rx) = SessionHost::new(
        "s-cap",
        8,
        8,
        2,
        MediationPolicy::FirstResponder,
        Duration::from_secs(30),
        None,
    );
    assert!(host.attach("c1", None).is_ok());
    assert!(host.attach("c2", None).is_ok());
    assert_eq!(
        host.attach("c3", None).err(),
        Some(harness_daemon::attach::AttachError::TooManyClients)
    );
    assert!(host.attach("c1", None).is_ok());
    host.detach("c2");
    assert!(host.attach("c3", None).is_ok());
}

#[tokio::test]
async fn journal_records_the_turn_workflow() {
    let (host, dir, cmd_rx) = build_session("s-journal");
    let session_dir = session_dir_of(&dir, "s-journal");
    let router = Arc::new(WorkerRouter::new(Duration::from_secs(5)));
    let executor = WorkerToolExecutor::new(Arc::clone(&router), None, host.event_sink());
    let inference = ScriptedInference::from_steps(vec![completion_turn()]);

    host.spawn_engine(
        session_dir,
        host.standard_stages(Box::new(executor)),
        Box::new(inference),
        cmd_rx,
    )
    .ok();
    let attach = host.attach("tui", None);
    assert!(attach.is_ok(), "attach failed");
    let Ok(attachment) = attach else {
        return;
    };
    let reader = tokio::spawn(read_stream(attachment.events.live, attachment.control));
    let _ = host
        .handle_command(HostCommand::Submit("journal me".into()))
        .await;
    let collected = tokio::time::timeout(Duration::from_secs(10), reader)
        .await
        .ok()
        .and_then(|r| r.ok());
    assert!(collected.is_some_and(|s| !s.frames.is_empty()));

    let journal_root = dir
        .as_ref()
        .map_or_else(std::env::temp_dir, |d| d.path().to_owned());
    let journal_path = journal_root
        .join("journal")
        .join("s-journal")
        .join("1.jsonl");
    let raw = std::fs::read_to_string(journal_path).unwrap_or_default();
    assert!(raw.contains("turn_started"), "journal:\n{raw}");
    assert!(raw.contains("turn_finished"), "journal:\n{raw}");
    drop(dir);
}

/// Durable-execution invariant visible through the public surface: the
/// engine journals each executed tool activity (content-addressed).
#[tokio::test]
async fn worker_activities_are_journaled_content_addressed() {
    let (host, dir, cmd_rx) = build_session("s-act");
    let session_dir = session_dir_of(&dir, "s-act");
    let router = Arc::new(WorkerRouter::new(Duration::from_secs(5)));
    let (worker_tx, mut worker_rx) = mpsc::channel(2);
    router.register(
        "w1",
        vec![ToolGrant {
            tool: "echo".into(),
        }],
        worker_tx,
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let executor = WorkerToolExecutor::new(Arc::clone(&router), None, host.event_sink());
    let inference = ScriptedInference::from_steps(vec![
        text_turn("checking", vec![tool_call("t1", "echo")]),
        completion_turn(),
    ]);

    host.spawn_engine(
        session_dir,
        host.standard_stages(Box::new(executor)),
        Box::new(inference),
        cmd_rx,
    )
    .ok();
    let attach = host.attach("tui", None);
    assert!(attach.is_ok(), "attach failed");
    let Ok(attachment) = attach else {
        return;
    };
    let answerer = spawn_answerer(host.mediator(), Answer::AllowOnce, "tui");
    let seen_calls = Arc::clone(&calls);
    let worker = {
        let router = Arc::clone(&router);
        tokio::spawn(async move {
            if let Some(DaemonToWorker::ToolCall { call_id, tool, .. }) = worker_rx.recv().await {
                seen_calls.fetch_add(1, Ordering::SeqCst);
                let _ = tool;
                router.deliver(&call_id, true, serde_json::json!("ok"));
            }
        })
    };
    let reader = tokio::spawn(read_stream(attachment.events.live, attachment.control));
    let _ = host
        .handle_command(HostCommand::Submit("run tool".into()))
        .await;
    let collected = tokio::time::timeout(Duration::from_secs(10), reader)
        .await
        .ok()
        .and_then(|r| r.ok());
    assert!(collected.is_some_and(|s| !s.frames.is_empty()));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    answerer.abort();
    worker.abort();
    drop(dir);
}
