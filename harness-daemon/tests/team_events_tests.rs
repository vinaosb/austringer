//! Task-8 acceptance (plan todo 8): the orchestration launch path live
//! in the daemon — Launcher-driven child sessions with per-session
//! dirs, write-set collision alerts with pre-launch quarantine, REAL
//! team events (`ChildStarted`/`ChildProgress`/`ChildFinished`) on the
//! session event stream, and the v1 compat gate for the new
//! `SessionEvent` payloads riding `ServerMsg::Event`.
//!
//! Hermetic: scripted completion-only inference, on-disk fixtures,
//! zero network beyond loopback, zero keys.

mod common;

use std::sync::Arc;
use std::time::Duration;

use harness_core::agent_loop::Inference;
use harness_daemon::compat::downgrade_for_v1;
use harness_daemon::config::DaemonConfig;
use harness_daemon::launch::{LaunchServices, run_launch};
use harness_daemon::mediation::MediationPolicy;
use harness_daemon::protocol::{Frame, ServerMsg, SessionEvent};
use harness_daemon::server::{DaemonState, build_router};
use harness_daemon::usage::ComposedInference;
use harness_daemon::attach::SessionHost;

use common::CompleteOnlyInference;

/// Two parallel workers with DISJOINT write sets (happy path).
const DISJOINT_SPEC: &str = "\
objective: prove the launch path
task t1:
  role: worker
  objective: map the layout
  writes: src/**
task t2:
  role: worker
  objective: write the docs
  writes: docs/**
";

/// Two parallel workers whose DECLARED write sets overlap (the
/// collision/quarantine fixture).
const OVERLAP_SPEC: &str = "\
objective: prove the collision quarantine
task t1:
  role: worker
  objective: own the source tree
  writes: src/**
task t2:
  role: worker
  objective: also touch the source tree
  writes: src/lib/**
";

fn services_in(dir: &std::path::Path) -> Arc<LaunchServices> {
    Arc::new(LaunchServices {
        inference_factory: Arc::new(|| {
            ComposedInference::plain(Box::new(CompleteOnlyInference))
        }),
        executor_factory: Arc::new(|_session_dir, _event_sink| {
            Box::new(harness_core::agent_loop::stages::defaults::NoToolExecutor)
        }),
        sessions_root: dir.join("sessions"),
        ask_policy: MediationPolicy::FirstResponder,
        ask_timeout: Duration::from_millis(200),
    })
}

fn host(id: &str) -> Arc<SessionHost> {
    let (host, _cmd_rx) = SessionHost::new(
        id,
        64,
        32,
        8,
        MediationPolicy::FirstResponder,
        Duration::from_millis(200),
        None,
    );
    host
}

/// Subscribes BEFORE the launch so every published frame lands in the
/// receiver, then drains the buffered stream.
fn drain(rx: &mut tokio::sync::broadcast::Receiver<ServerMsg>) -> Vec<SessionEvent> {
    let mut out = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let ServerMsg::Event { frame } = msg {
            out.push(frame.event);
        }
    }
    out
}

fn child_started(events: &[SessionEvent], child: &str) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            SessionEvent::ChildStarted { child: c, .. } if c == child
        )
    })
}

fn child_progress(events: &[SessionEvent], child: &str, phase: &str) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            SessionEvent::ChildProgress { child: c, phase: p, .. }
                if c == child && p == phase
        )
    })
}

fn child_finished(events: &[SessionEvent], child: &str, state: &str) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            SessionEvent::ChildFinished { child: c, state: s, .. }
                if c == child && s == state
        )
    })
}

// ---------------------------------------------------------------------------
// (1) compat gate: Child* payloads riding Event frames are downgraded
//     for v1 wires (frame id preserved, Notification lane)
// ---------------------------------------------------------------------------

#[test]
fn compat_downgrades_child_events_to_v1_notifications() {
    let frames = vec![
        (
            SessionEvent::ChildStarted {
                child: "l-child-t1".into(),
                task: "t1".into(),
                role: "worker".into(),
                model: "gpt-5-5".into(),
            },
            "child_started",
        ),
        (
            SessionEvent::ChildProgress {
                child: "l-child-t2".into(),
                phase: "collision".into(),
                detail: "src/** overlaps l-child-t1".into(),
            },
            "child_progress",
        ),
        (
            SessionEvent::ChildFinished {
                child: "l-child-t2".into(),
                task: "t2".into(),
                state: "quarantined".into(),
                detail: "write set `src/lib/**` overlaps l-child-t1".into(),
            },
            "child_finished",
        ),
    ];
    for (event, expected_kind) in frames {
        let frame = Frame { id: 7, event };
        let downgraded = downgrade_for_v1(ServerMsg::Event { frame }).expect("downgraded, not withheld");
        let ServerMsg::Event { frame } = downgraded else {
            panic!("shape");
        };
        assert_eq!(frame.id, 7, "frame id survives the downgrade");
        match frame.event {
            SessionEvent::Notification { kind, message } => {
                assert_eq!(kind, expected_kind, "v1 kind channel: {message}");
                assert!(!message.is_empty(), "message carries the payload");
            }
            other => panic!("expected notification downgrade, got {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// (2) a v1-shaped SessionEvent enum (pre-team-variant set) ERRORS
//     decoding the new payloads — the decode side of the contract
// ---------------------------------------------------------------------------

/// The v1 wire shape: exactly the pre-todo-8 variant set (worker claim
/// vocabulary included — it was v1-decodable). Fields exist for serde's
/// decoder only, so the dead-code lint is silenced test-wide.
#[allow(dead_code)]
#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum V1ShapedSessionEvent {
    Recorded {
        seq: u64,
        event: serde_json::Value,
    },
    TurnStarted {
        turn: u64,
    },
    TurnFinished {
        turn: u64,
        outcome: String,
    },
    Diff {
        path: String,
        before_hash: String,
        after_hash: String,
        hunks: Vec<String>,
    },
    Notification {
        kind: String,
        message: String,
    },
    WorkerClaimed {
        queue: String,
        worker: String,
    },
    WorkerHealed {
        queue: String,
    },
}

#[test]
fn v1_shaped_enum_errors_decoding_child_events() {
    let events: Vec<SessionEvent> = vec![
        SessionEvent::ChildStarted {
            child: "l-child-t1".into(),
            task: "t1".into(),
            role: "worker".into(),
            model: "gpt-5-5".into(),
        },
        SessionEvent::ChildProgress {
            child: "l-child-t1".into(),
            phase: "file_touched".into(),
            detail: "write src/lib.rs".into(),
        },
        SessionEvent::ChildFinished {
            child: "l-child-t1".into(),
            task: "t1".into(),
            state: "success".into(),
            detail: "completed".into(),
        },
        // The todo-15 budget variants ride the same gate: a v1 client
        // cannot decode them, so the Notification downgrade is
        // mandatory on v1 wires.
        SessionEvent::TeamBudget {
            spawned: 1,
            tokens_spent: 25,
            fan_out_cap: 4,
            token_budget: 1000,
        },
        SessionEvent::TeamBudgetExhausted {
            spent: 1000,
            budget: 1000,
        },
    ];
    for event in events {
        let encoded = serde_json::to_string(&event).unwrap_or_default();
        let decoded: Result<V1ShapedSessionEvent, _> = serde_json::from_str(&encoded);
        assert!(
            decoded.is_err(),
            "a v1 client must fail decoding {encoded} — the downgrade is mandatory"
        );
    }
}

// ---------------------------------------------------------------------------
// (3) FAILURE (plan QA): overlapping declared write sets produce the
//     collision alert and the second child is quarantined BEFORE
//     launching (it never receives auto-allow; it never runs)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn overlapping_write_sets_alert_and_quarantine_second_child() {
    let dir = tempfile::tempdir().unwrap();
    let services = services_in(dir.path());
    let host = host("s-collide");
    let mut rx = host.hub_subscribe();

    let report = run_launch(
        Arc::clone(&host),
        Arc::clone(&services),
        OVERLAP_SPEC,
        None,
        "l",
    )
    .await
    .expect("launch runs with quarantine");

    // The first declarer ran to completion; the second never launched.
    assert_eq!(report.children.len(), 1, "only the non-overlapping child ran");
    assert_eq!(report.children[0].child_id, "l-child-t1");
    assert!(report.failures.is_empty(), "quarantine is not a failure");

    let events = drain(&mut rx);
    assert!(
        child_started(&events, "l-child-t1"),
        "first child started: {events:?}"
    );
    assert!(
        child_progress(&events, "l-child-t2", "collision"),
        "collision alert names the overlapping child: {events:?}"
    );
    assert!(
        child_finished(&events, "l-child-t2", "quarantined"),
        "second child quarantined: {events:?}"
    );
    assert!(
        child_finished(&events, "l-child-t1", "success"),
        "first child completed: {events:?}"
    );
    assert!(
        !child_started(&events, "l-child-t2"),
        "the quarantined child never started"
    );

    // SPAWNED EFFECTS: the first child is a real session dir; the
    // quarantined child has none.
    assert!(
        services.sessions_root.join("l-child-t1").exists(),
        "child session dir for the launched child"
    );
    assert!(
        !services.sessions_root.join("l-child-t2").exists(),
        "no session dir for the quarantined child"
    );
}

// ---------------------------------------------------------------------------
// (4) HAPPY (plan QA): parallel children with disjoint write sets
//     complete as real child sessions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn disjoint_write_sets_complete_as_child_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let services = services_in(dir.path());
    let host = host("s-happy");
    let mut rx = host.hub_subscribe();

    let report = run_launch(
        Arc::clone(&host),
        Arc::clone(&services),
        DISJOINT_SPEC,
        None,
        "l",
    )
    .await
    .expect("launch runs");

    assert_eq!(report.children.len(), 2, "both children ran");
    assert!(report.failures.is_empty());

    let events = drain(&mut rx);
    assert!(child_started(&events, "l-child-t1"), "{events:?}");
    assert!(child_started(&events, "l-child-t2"), "{events:?}");
    assert!(child_finished(&events, "l-child-t1", "success"), "{events:?}");
    assert!(child_finished(&events, "l-child-t2", "success"), "{events:?}");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SessionEvent::ChildProgress { .. })),
        "no collision phases on disjoint sets: {events:?}"
    );

    // Child ids are in the hidden {launch}-child-{task} namespace and
    // each owns a real per-session dir.
    assert!(report
        .child_ids
        .iter()
        .all(|id| harness_orchestration::journal::is_child_session(id)));
    for child in ["l-child-t1", "l-child-t2"] {
        assert!(
            services.sessions_root.join(child).exists(),
            "child session dir {child}"
        );
    }
}

// ---------------------------------------------------------------------------
// (5) team events visible to attached clients over SSE: v2 sees the
//     typed Child* frames, v1 sees the downgraded Notification lane
//     (snapshot replay proves the Snapshot path downgrades too)
// ---------------------------------------------------------------------------

async fn read_sse_until(base: &str, token: &str, path: &str, marker: &str) -> String {
    let mut response = reqwest::Client::new()
        .get(format!("http://{base}{path}"))
        .bearer_auth(token)
        .send()
        .await
        .expect("sse connect");
    let mut buffer = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while std::time::Instant::now() < deadline {
        let chunk = tokio::time::timeout(Duration::from_secs(5), response.chunk()).await;
        match chunk {
            Ok(Ok(Some(bytes))) => {
                buffer.push_str(&String::from_utf8_lossy(&bytes));
                if buffer.contains(marker) {
                    return buffer;
                }
            }
            Ok(Ok(None)) => break,
            Ok(Err(e)) => panic!("sse read: {e}"),
            Err(_) => continue,
        }
    }
    buffer
}

#[tokio::test]
async fn team_events_visible_over_sse_v2_typed_and_v1_downgraded() {
    let dir = tempfile::tempdir().unwrap();
    let config = DaemonConfig {
        data_dir: dir.path().to_owned(),
        token: Some("e2e-token".into()),
        durable: false,
        ..DaemonConfig::default()
    };
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
        Arc::new(|| Box::new(CompleteOnlyInference));
    let state = Arc::new(
        DaemonState::new(config, factory).unwrap_or_else(|e| panic!("state: {e}")),
    );
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
    let base = format!("127.0.0.1:{port}");
    let token = state.token.clone();

    let spec_path = dir.path().join("ship.md");
    std::fs::write(&spec_path, DISJOINT_SPEC).expect("spec fixture");

    // v2 client attached BEFORE the launch: typed Child* frames.
    let v2_stream = tokio::spawn({
        let base = base.clone();
        let token = token.clone();
        async move {
            read_sse_until(
                &base,
                &token,
                "/s/s-team/events?version=2",
                "\"type\":\"child_finished\"",
            )
            .await
        }
    });

    let submit = serde_json::json!({
        "type": "command",
        "command": {
            "type": "submit",
            "text": format!("/launch {}", spec_path.display()),
        }
    });
    let response = reqwest::Client::new()
        .post(format!("http://{base}/s/s-team/cmd"))
        .bearer_auth(&token)
        .header("content-type", "application/json")
        .body(submit.to_string())
        .send()
        .await
        .expect("post command");
    assert_eq!(response.status().as_u16(), 202, "launch accepted");

    let v2 = v2_stream.await.expect("v2 reader");
    assert!(
        v2.contains("\"type\":\"child_started\""),
        "v2 SSE carries typed ChildStarted: {v2}"
    );
    assert!(
        v2.contains("\"type\":\"child_finished\""),
        "v2 SSE carries typed ChildFinished: {v2}"
    );

    // v1 client attaches AFTER the fact: the snapshot replay downgrades
    // the same frames onto the Notification lane.
    let v1 = read_sse_until(&base, &token, "/s/s-team/events", "\"kind\":\"child_started\"").await;
    assert!(
        v1.contains("\"kind\":\"child_started\""),
        "v1 SSE carries the downgraded notification: {v1}"
    );
}
