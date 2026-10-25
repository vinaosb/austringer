//! Task-2 acceptance (plan todo 2): the run-history and automations
//! HTTP surfaces plus the `sched`/`verdict` Notification kinds routed
//! onto the event stream — fixture automations on disk + the fake
//! completion-only provider, zero network, zero keys.

mod common;

use std::sync::Arc;
use std::time::Duration;

use austringer_core::agent_loop::Inference;
use austringer_daemon::automations::{
    AutomationDefinition, AutomationStore, FilePackGate, NetworkPolicy,
};
use austringer_daemon::bridge::RunSink;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::engine::EventStreamSink;
use austringer_daemon::gate::{CompletionGate, CriteriaGate};
use austringer_daemon::kickoff::DaemonKicker;
use austringer_daemon::notify::{MemorySink, NotificationSink, Notifier};
use austringer_daemon::run_history::{RunHistory, RunRecord};
use austringer_daemon::runner::CronRunner;
use austringer_daemon::server::{DaemonState, build_router};

use common::CompleteOnlyInference;

/// Bind a fixture daemon on an ephemeral loopback port over `dir`
/// (caller keeps the TempDir alive for the test's lifetime).
async fn spawn_in(dir: &std::path::Path) -> (String, String, Arc<DaemonState>) {
    let config = DaemonConfig {
        data_dir: dir.to_owned(),
        token: Some("e2e-token".into()),
        durable: false,
        ..DaemonConfig::default()
    };
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
        Arc::new(|| Box::new(CompleteOnlyInference));
    let state =
        Arc::new(DaemonState::new(config, factory).unwrap_or_else(|e| panic!("state: {e}")));
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
    (format!("127.0.0.1:{port}"), state.token.clone(), state)
}

async fn get(base: &str, token: &str, path: &str) -> (u16, serde_json::Value) {
    let response = reqwest::Client::new()
        .get(format!("http://{base}{path}"))
        .bearer_auth(token)
        .send()
        .await
        .expect("http");
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    let json = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

fn record(
    automation: &str,
    session: &str,
    minute: u64,
    outcome: &str,
    verdict: Option<&str>,
) -> RunRecord {
    RunRecord {
        automation_id: automation.into(),
        session_id: session.into(),
        fired_at_minute: minute,
        outcome: outcome.into(),
        verdict: verdict.map(str::to_owned),
        network: NetworkPolicy::Online,
    }
}

fn fixture_def(id: &str, cron: &str, criteria: &[&str]) -> AutomationDefinition {
    AutomationDefinition {
        id: id.into(),
        name: format!("{id} pack run"),
        cron: cron.into(),
        pack: "review".into(),
        goal: Some("run the fixture pack".into()),
        criteria: criteria.iter().map(|c| (*c).to_owned()).collect(),
        budget_turns: None,
        max_invocations_total: None,
        max_invocations_per_day: None,
        network: NetworkPolicy::Online,
        enabled: true,
    }
}

/// Write an approved pack-tiers file and register one fixture
/// automation through the production store path.
fn fixture_automation(dir: &std::path::Path, def: &AutomationDefinition) {
    std::fs::write(dir.join("pack-tiers.json"), br#"{"review": "approved"}"#).expect("pack tiers");
    let gate = FilePackGate::open(&dir.join("pack-tiers.json")).expect("gate");
    let mut store = AutomationStore::load(dir, &gate).expect("store");
    store.register(def, &gate).expect("register");
}

#[tokio::test]
async fn history_route_returns_runs_with_verdict() {
    let dir = tempfile::tempdir().unwrap();
    let history = RunHistory::open(dir.path()).unwrap();
    history
        .append(&record("nightly", "auto-nightly-600", 600, "started", None))
        .unwrap();
    history
        .append(&record(
            "nightly",
            "auto-nightly-600",
            600,
            "task_completed",
            Some("confirmed"),
        ))
        .unwrap();
    history
        .append(&record("other", "auto-other-1", 60, "started", None))
        .unwrap();
    let (base, token, _state) = spawn_in(dir.path()).await;

    let (status, body) = get(&base, &token, "/s/auto-nightly-600/history").await;
    assert_eq!(status, 200, "{body}");
    let runs = body.as_array().unwrap_or_else(|| panic!("array: {body}"));
    assert_eq!(
        runs.len(),
        2,
        "rows for other sessions must be filtered out: {body}"
    );
    assert!(
        runs.iter().all(|row| row.get("verdict").is_some()),
        "every entry carries a verdict field: {body}"
    );
    assert_eq!(runs[0]["verdict"], serde_json::Value::Null);
    assert_eq!(runs[1]["verdict"], "confirmed");
    assert_eq!(runs[1]["outcome"], "task_completed");
}

#[tokio::test]
async fn history_route_unknown_sid_is_typed_404() {
    let dir = tempfile::tempdir().unwrap();
    let (base, token, _state) = spawn_in(dir.path()).await;

    let (status, body) = get(&base, &token, "/s/never-heard-of/history").await;
    assert_ne!(status, 500, "unknown sid must not be a 500: {body}");
    assert_eq!(status, 404, "typed 404 required: {body}");
    assert_eq!(
        body["error"]["code"], "unknown_session",
        "typed error body: {body}"
    );
}

#[tokio::test]
async fn automations_route_lists_fixture_with_next_fire() {
    let dir = tempfile::tempdir().unwrap();
    fixture_automation(dir.path(), &fixture_def("nightly", "* * * * *", &[]));
    // A completed run row joins in as the automation's last verdict.
    RunHistory::open(dir.path())
        .unwrap()
        .append(&record(
            "nightly",
            "auto-nightly-1",
            1,
            "task_completed",
            Some("confirmed"),
        ))
        .unwrap();
    let (base, token, _state) = spawn_in(dir.path()).await;

    let minute_before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() / 60)
        .unwrap_or(0);
    let (status, body) = get(&base, &token, "/automations").await;
    assert_eq!(status, 200, "{body}");
    let entries = body["automations"]
        .as_array()
        .unwrap_or_else(|| panic!("automations array: {body}"));
    assert_eq!(entries.len(), 1, "{body}");
    let entry = &entries[0];
    assert_eq!(entry["id"], "nightly");
    assert_eq!(entry["name"], "nightly pack run");
    assert_eq!(entry["cron"], "* * * * *");
    assert_eq!(entry["enabled"], true);
    let next_fire = entry["next_fire_minute"]
        .as_u64()
        .unwrap_or_else(|| panic!("next_fire_minute must be a number: {body}"));
    assert!(
        next_fire > minute_before,
        "next fire must lie in the future: {next_fire} <= {minute_before}"
    );
    assert_eq!(entry["last_verdict"], "confirmed");
}

#[tokio::test]
async fn sched_notification_surfaces_to_sse_client() {
    let dir = tempfile::tempdir().unwrap();
    fixture_automation(
        dir.path(),
        &fixture_def("nightly", "* * * * *", &["task_completed"]),
    );

    let (base, token, state) = spawn_in(dir.path()).await;

    // The scheduler composition mirroring the binary's: the new
    // event-stream sink rides the notifier fan-out beside the memory
    // sink (which must keep receiving every kind unchanged).
    let memory = Arc::new(MemorySink::new());
    let mut notifier = Notifier::new();
    notifier.add(Arc::clone(&memory) as Arc<dyn NotificationSink>);
    notifier.add(Arc::new(EventStreamSink::new(Arc::new({
        let state = Arc::clone(&state);
        move |sid: &str| state.try_session(sid)
    }))));
    let sink = Arc::new(
        RunSink::open(
            dir.path(),
            Arc::new(CriteriaGate) as Arc<dyn CompletionGate>,
            Arc::new(notifier),
        )
        .unwrap_or_else(|e| panic!("sink: {e}")),
    );
    let mut runner = CronRunner::open(
        dir.path(),
        {
            let gate = FilePackGate::open(&dir.path().join("pack-tiers.json")).unwrap();
            AutomationStore::load(dir.path(), &gate)
                .unwrap()
                .defs()
                .to_vec()
        },
        Arc::new(DaemonKicker::new(Arc::clone(&state))),
        Arc::clone(&sink),
        0,
        NetworkPolicy::Online,
    )
    .unwrap_or_else(|e| panic!("runner: {e}"));

    // Attach the SSE client to the deterministic scheduled session id
    // (auto-nightly-2 for a minute-2 fire) BEFORE pumping; the attach
    // itself creates the live session.
    let mut sse = reqwest::Client::new()
        .get(format!("http://{base}/s/auto-nightly-2/events"))
        .bearer_auth(&token)
        .query(&[("version", "2"), ("after", "0")])
        .send()
        .await
        .expect("sse attach");
    assert_eq!(sse.status().as_u16(), 200);

    // Live session + zero history rows: history answers 200 + [].
    let (status, body) = get(&base, &token, "/s/auto-nightly-2/history").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, serde_json::json!([]), "live session, no rows yet");

    // Fire minute 2 (pump(0) baselines, pump(120) crosses it).
    runner.pump(0).unwrap();
    let report = runner.pump(120).unwrap();
    assert_eq!(report.kicked.len(), 1);

    // The attached SSE client must see BOTH event-stream notification
    // kinds from the scheduled run: `sched` (kickoff) and `verdict`
    // (gated completion).
    let seen = tokio::time::timeout(Duration::from_secs(10), async {
        let mut buffer = String::new();
        loop {
            if buffer.contains(r#""kind":"sched""#) && buffer.contains(r#""kind":"verdict""#) {
                return Ok(buffer);
            }
            match sse.chunk().await {
                Ok(Some(chunk)) => buffer.push_str(&String::from_utf8_lossy(&chunk)),
                Ok(None) => {
                    return Err(format!("stream ended before both kinds: {buffer}"));
                }
                Err(e) => return Err(format!("stream error: {e}")),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("sched/verdict never reached the SSE client"))
    .expect("sse kinds");
    assert!(seen.contains(r#""kind":"sched""#), "{seen}");
    assert!(seen.contains(r#""kind":"verdict""#), "{seen}");

    // The fan-out is additive: the memory sink saw the daemon's full
    // notification vocabulary for the run, unchanged.
    assert!(
        memory
            .delivered()
            .iter()
            .any(|n| n.kind == austringer_daemon::notify::NotificationKind::ScheduledFired)
    );
    assert!(
        memory
            .delivered()
            .iter()
            .any(|n| n.kind == austringer_daemon::notify::NotificationKind::TurnComplete)
    );
}
