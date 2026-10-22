//! Task 24 (ecosystem alignment): OSC program-status emission over the
//! daemon's notification bus — the structured OSC 777 `warp://cli-agent`
//! lifecycle frame (receipt: Warp's cli-agent OSC 777 listener,
//! app/src/terminal/cli_agent_sessions/event/v1.rs) AND the OSC 7501
//! Program Status Protocol report (receipt: Warp issue #16407 — Crush
//! emits, cmux receives; spec: superlogical.com/rex/docs/build/
//! program-status) fire together at every working / blocked / done /
//! error transition. Emit-only: the daemon receives NEITHER sequence.

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64ct::{Base64, Encoding};
use harness_daemon::attach::SessionHost;
use harness_daemon::bridge::RunSink;
use harness_daemon::gate::CriteriaGate;
use harness_daemon::mediation::MediationPolicy;
use harness_daemon::notify::{Notification, NotificationKind, NotificationSink, Notifier};
use harness_daemon::osc::OscSink;

/// Shared capture buffer: the raw stdout bytes the sink writes.
#[derive(Clone, Default)]
struct SharedBuf(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBuf {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self.0.lock() {
            Ok(mut guard) => {
                guard.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            Err(poisoned) => {
                poisoned.into_inner().extend_from_slice(bytes);
                Ok(bytes.len())
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl SharedBuf {
    fn take(&self) -> Vec<u8> {
        match self.0.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

fn b64(text: &str) -> String {
    Base64::encode_string(text.as_bytes())
}

fn notification(kind: NotificationKind, session: &str, message: &str) -> Notification {
    Notification {
        kind,
        session_id: Some(session.into()),
        message: message.into(),
    }
}

const STATUS_777_PREFIX: &str = "\x1b]777;notify;warp://cli-agent;";
/// 7501 ST terminator (`ESC \`).
const ST: &str = "\x1b\\";

/// A full session lifecycle drives the four program-status transitions
/// and each emits BOTH the structured 777 lifecycle frame and the 7501
/// report with the spec's pair grammar (`state` first, `kind` only on
/// blocked, `app` + base64 `msg`).
#[test]
fn session_lifecycle_emits_777_and_7501_sequences() {
    let buf = SharedBuf::default();
    let sink = OscSink::new(buf.clone(), "harness");

    // working: a run kicked off.
    sink.deliver(&notification(
        NotificationKind::ScheduledFired,
        "s-osc",
        "run kicked off",
    ))
    .expect("deliver working");
    let bytes = buf.take();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    assert!(
        text.contains(STATUS_777_PREFIX),
        "no structured 777 lifecycle frame: {text:?}"
    );
    assert!(
        text.contains("\"event\":\"prompt_submit\""),
        "working must map to Warp prompt_submit: {text:?}"
    );
    assert!(
        text.contains("\"agent\":\"harness\"") && text.contains("\"v\":1"),
        "lifecycle identity fields: {text:?}"
    );
    assert!(
        text.contains("\"session_id\":\"s-osc\""),
        "session id must ride the lifecycle frame: {text:?}"
    );
    let expected = format!(
        "\x1b]7501;state=working:app=harness:msg={}{}",
        b64("run kicked off"),
        ST
    );
    assert!(
        text.contains(&expected),
        "working 7501 report: {text:?} (want {expected:?})"
    );

    // blocked: a permission ask is waiting (attention state).
    sink.deliver(&notification(
        NotificationKind::ApprovalNeeded,
        "s-osc",
        "approval needed for tool edit_lines (ask 1)",
    ))
    .expect("deliver blocked");
    let text = String::from_utf8_lossy(&buf.take()).into_owned();
    assert!(
        text.contains("\"event\":\"permission_request\""),
        "blocked must map to Warp permission_request: {text:?}"
    );
    let expected = format!(
        "\x1b]7501;state=blocked:kind=permission:app=harness:msg={}{}",
        b64("approval needed for tool edit_lines (ask 1)"),
        ST
    );
    assert!(
        text.contains(&expected),
        "blocked 7501 report carries kind=permission: {text:?} (want {expected:?})"
    );

    // done: the turn finished.
    sink.deliver(&notification(
        NotificationKind::TurnComplete,
        "s-osc",
        "run s-osc finished: task_completed (gate: confirmed)",
    ))
    .expect("deliver done");
    let text = String::from_utf8_lossy(&buf.take()).into_owned();
    assert!(
        text.contains("\"event\":\"stop\""),
        "done must map to Warp stop: {text:?}"
    );
    let expected = format!(
        "\x1b]7501;state=done:app=harness:msg={}{}",
        b64("run s-osc finished: task_completed (gate: confirmed)"),
        ST
    );
    assert!(
        text.contains(&expected),
        "done 7501 report: {text:?} (want {expected:?})"
    );

    // error: a worker vanished mid-operation.
    sink.deliver(&notification(
        NotificationKind::WorkerLost,
        "s-osc",
        "worker gone",
    ))
    .expect("deliver error");
    let text = String::from_utf8_lossy(&buf.take()).into_owned();
    assert!(
        text.contains("\"event\":\"stop_failure\""),
        "error must map to Warp stop_failure: {text:?}"
    );
    let expected = format!(
        "\x1b]7501;state=error:app=harness:msg={}{}",
        b64("worker gone"),
        ST
    );
    assert!(
        text.contains(&expected),
        "error 7501 report: {text:?} (want {expected:?})"
    );
}

/// Every 7501 report the lifecycle emits must satisfy the spec grammar:
/// `OSC 7501 ; pairs ST`, pairs = `key=value` joined by `:`, values
/// inside `[A-Za-z0-9_.,+/=-]`, and the whole sequence under the
/// 4096-byte limit.
#[test]
fn osc_7501_reports_follow_the_spec_grammar() {
    let buf = SharedBuf::default();
    let sink = OscSink::new(buf.clone(), "harness");
    let long = "x".repeat(200);
    for kind in [
        NotificationKind::ScheduledFired,
        NotificationKind::ApprovalNeeded,
        NotificationKind::TurnComplete,
        NotificationKind::WorkerLost,
    ] {
        sink.deliver(&notification(kind, "s-osc", &long))
            .expect("deliver");
    }
    let bytes = buf.take();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let mut reports = 0;
    for frame in text.split("\x1b]").filter(|f| f.starts_with("7501;")) {
        reports += 1;
        let Some((body, rest)) = frame.strip_prefix("7501;").and_then(|b| b.split_once(ST)) else {
            panic!("7501 report not ST-terminated: {frame:?}");
        };
        assert!(
            rest.is_empty() || rest == "\n",
            "trailing bytes after ST terminator: {frame:?}"
        );
        assert!(
            body.len() + 7 <= 4096,
            "whole-sequence limit: {}",
            body.len() + 7
        );
        for pair in body.split(':') {
            let (key, value) = pair
                .split_once('=')
                .unwrap_or_else(|| panic!("malformed pair {pair:?} in {body:?}"));
            assert!(!key.is_empty(), "empty key in {body:?}");
            assert!(
                key.chars().all(|c| c.is_ascii_lowercase()),
                "key grammar: {key:?}"
            );
            assert!(
                value
                    .chars()
                    .all(|c| matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '.' | ',' | '+' | '/' | '=' | '-')),
                "value grammar: {value:?} in {body:?}"
            );
        }
        assert!(
            body.starts_with("state="),
            "state must lead the pairs: {body:?}"
        );
    }
    assert_eq!(reports, 4, "one 7501 report per mapped kind: {text:?}");
}

/// A live permission ask flips the terminal status to blocked within
/// one event-loop turn of the broadcast — the attention state lands
/// WHILE the ask is pending, not after it resolves.
#[tokio::test]
async fn permission_ask_flips_status_to_blocked_within_one_event_loop_turn() {
    let dir = tempfile::tempdir().expect("tempdir");
    let buf = SharedBuf::default();
    let mut notifier = Notifier::new();
    notifier.add(Arc::new(OscSink::new(buf.clone(), "harness")));
    let sink = Arc::new(
        RunSink::open(dir.path(), Arc::new(CriteriaGate), Arc::new(notifier))
            .unwrap_or_else(|e| panic!("sink: {e}")),
    );
    let (host, _cmd_rx) = SessionHost::new(
        "s-osc-ask",
        64,
        16,
        8,
        MediationPolicy::FirstResponder,
        Duration::from_secs(30),
        None,
    );
    let mut asks = host.mediator().subscribe();
    sink.spawn_watcher(&host, "s-osc-ask");

    let mediating = {
        let mediator = host.mediator();
        tokio::spawn(async move {
            mediator
                .mediate(harness_core::inspectors::store::AskRequest {
                    tool: "edit_lines".into(),
                    target: Some("src/lib.rs".into()),
                    reason: "scheduled edit".into(),
                })
                .await
        })
    };

    let mut flipped_at = None;
    let deadline = Duration::from_secs(2);
    let started = std::time::Instant::now();
    while started.elapsed() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let text = String::from_utf8_lossy(&buf.take()).into_owned();
        if text.contains("\x1b]7501;state=blocked:kind=permission") {
            flipped_at = Some(started.elapsed());
            break;
        }
    }
    let elapsed = flipped_at.expect("status never flipped to blocked");
    assert!(
        elapsed < deadline,
        "flip took {elapsed:?}; the ask broadcast must reach the sink within one event-loop turn"
    );
    // The flip is the attention state: the ask is STILL pending.
    assert!(
        !mediating.is_finished(),
        "blocked status must land while the ask is pending"
    );

    // Resolve the ask so the session can continue (cleanup path).
    let ask_id = loop {
        match asks.try_recv() {
            Ok(harness_daemon::protocol::ServerMsg::PermissionAsk { ask }) => break ask.ask_id,
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(_) => panic!("ask broadcast lost"),
        }
    };
    host.mediator()
        .answer(ask_id, "tui", harness_daemon::protocol::Answer::AllowOnce)
        .expect("answer");
    let _ = mediating.await;
}

/// Injection: a session label and message carrying ESC / BEL / C0 / C1
/// control characters must produce EXACTLY the framing escape bytes —
/// the 7501 path inherits `sanitize_osc_text` the same way the 777
/// notify path always has. Per mapped deliver: 5 ESC (notify-777,
/// OSC-9, structured-777, 7501 OSC-start, 7501 ST) and 3 BEL.
#[test]
fn esc_payload_produces_exactly_the_framing_esc_bytes() {
    let buf = SharedBuf::default();
    let sink = OscSink::new(buf.clone(), "harness");
    // C0 ESC/BEL/NUL + C1 CSI (U+009B) + C1 NEL (U+0085).
    sink.deliver(&Notification {
        kind: NotificationKind::TurnComplete,
        session_id: Some("s-\u{1b}]777;evil\u{7}\u{9b}".into()),
        message: "ok\u{1b}]9;injected\u{7}\u{0}\u{85}".into(),
    })
    .expect("deliver");
    let mut bytes = buf.take();
    // The sink newline-terminates each write batch (pre-existing
    // framing); everything before it is escape-sequence payload.
    assert_eq!(bytes.pop(), Some(b'\n'), "batch must be newline-terminated");
    let bytes = bytes;

    let esc = bytes.iter().filter(|b| **b == 0x1b).count();
    let bel = bytes.iter().filter(|b| **b == 0x07).count();
    assert_eq!(esc, 5, "injected ESC survived: {bytes:?}");
    assert_eq!(bel, 3, "injected BEL survived: {bytes:?}");
    // No other C0 controls (or DEL) anywhere in the stream.
    for (idx, byte) in bytes.iter().enumerate() {
        let control = (*byte < 0x20 && *byte != 0x1b && *byte != 0x07) || *byte == 0x7f;
        assert!(!control, "raw control byte {byte:#x} at {idx}: {bytes:?}");
    }

    let text = String::from_utf8_lossy(&bytes).into_owned();
    // The 7501 msg must decode to control-free single-line text.
    let start = text.find("\x1b]7501;").expect("7501 report present");
    let body = &text[start + "\x1b]7501;".len()..];
    let body = &body[..body.find(ST).expect("ST terminator")];
    let msg = body
        .split(':')
        .find_map(|pair| pair.strip_prefix("msg="))
        .expect("msg pair");
    let decoded = Base64::decode_vec(msg).expect("base64 decodes");
    let decoded = String::from_utf8(decoded).expect("msg is UTF-8");
    assert!(
        decoded.chars().all(|c| !c.is_control()),
        "decoded msg carries control characters: {decoded:?}"
    );
    assert!(
        decoded.starts_with("ok"),
        "payload text survives: {decoded:?}"
    );

    // The structured 777 JSON body carries the SANITIZED session id.
    let frame = text
        .split('\n')
        .find(|line| line.contains("warp://cli-agent"))
        .expect("structured 777 frame");
    assert!(
        !frame.contains("evil]"),
        "raw ESC payload leaked into the lifecycle JSON: {frame:?}"
    );
    assert!(
        frame.contains("s- ]777;evil"),
        "sanitized session id (controls -> spaces) must ride the JSON: {frame:?}"
    );
}

/// Task 24 QA (happy path): the SHIPPED daemon binary on a REAL PTY
/// (ConPTY on Windows) through a full scheduled session — automation
/// fires (working), the fixture provider completes the turn (done) —
/// and both program-status lanes are visible as RAW bytes on the
/// master side. Ignored by default: the cron fire waits for the next
/// minute boundary, so the run costs up to ~90 s wall-clock.
#[test]
#[ignore = "qa: full-session PTY capture, ~90s wall-clock"]
fn real_binary_pty_session_emits_program_status() {
    use portable_pty::{CommandBuilder, PtySize};

    let data = tempfile::tempdir().expect("data dir");
    std::fs::write(
        data.path().join("pack-tiers.json"),
        serde_json::json!({"review": "approved"}).to_string(),
    )
    .expect("pack tiers");

    // Fire at the next minute boundary (with one minute of spawn
    // slack when the current minute is nearly over) under an
    // automation id whose deterministic jitter stays inside one
    // scheduler tick, so the wait stays bounded.
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let slack = if now_secs % 60 < 45 { 1 } else { 2 };
    let fire_minute = now_secs / 60 + slack;
    let automation_id = (0..10_000_u32)
        .map(|i| format!("osc-pty-{i}"))
        .find(|id| harness_daemon::runner::jitter_secs(id, fire_minute, 30) < 5)
        .expect("an id with sub-tick jitter exists within 10k probes");
    let automation = serde_json::json!([{
        "id": automation_id,
        "name": "osc qa run",
        "cron": "* * * * *",
        "pack": "review",
        "goal": Some("emit program status"),
        "criteria": ["task_completed"],
        "budget_turns": null,
        "max_invocations_total": null,
        "max_invocations_per_day": null,
        "network": "online",
        "enabled": true,
    }]);
    std::fs::write(data.path().join("automations.json"), automation.to_string())
        .expect("automations");

    // Compat fixture provider on a background tokio runtime: one
    // task_completed tool-call turn completes the scheduled session.
    let base_url = {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("fixture runtime");
            runtime.block_on(async move {
                let app = axum::Router::new().route(
                    "/v1/chat/completions",
                    axum::routing::post(|| async {
                        let body = format!(
                            "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                            serde_json::json!({
                                "model": "fixture-model",
                                "choices": [{"index": 0, "delta": {"tool_calls": [{
                                    "index": 0,
                                    "id": "c1",
                                    "type": "function",
                                    "function": {
                                        "name": "task_completed",
                                        "arguments": "{}"
                                    }
                                }]}}]}
                            ),
                            serde_json::json!({
                                "model": "fixture-model",
                                "choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]
                            }),
                        );
                        (
                            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                            body,
                        )
                    }),
                );
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("fixture bind");
                tx.send(format!("http://{}/v1", listener.local_addr().expect("addr")))
                    .expect("addr channel");
                axum::serve(listener, app).await.expect("fixture serve");
            });
        });
        rx.recv().expect("fixture url")
    };

    let pty = portable_pty::native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_harness-daemon"));
    command.arg("--data-dir");
    command.arg(data.path());
    command.env("HARNESS_PROVIDER", "compat");
    command.env("HARNESS_MODEL", "fixture-model");
    command.env("HARNESS_PROVIDER_BASE_URL", &base_url);
    command.env_remove("HARNESS_PROVIDER_KEY_ENV");
    let mut child = pty
        .slave
        .spawn_command(command)
        .expect("spawn daemon on pty");
    let mut reader = pty.master.try_clone_reader().expect("master reader");
    let mut writer = pty.master.take_writer().expect("master writer");
    // Reader thread: the master read blocks indefinitely when the
    // daemon is quiet, so the deadline loop consumes bounded chunks
    // through a channel instead of reading directly.
    let (chunks_tx, chunks_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if chunks_tx.send(chunk[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(150);
    let mut captured = Vec::new();
    let mut answered = 0;
    while std::time::Instant::now() < deadline {
        match chunks_rx.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(chunk) => captured.extend_from_slice(&chunk),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
        // This test IS the terminal: conhost's ConPTY handshake sends
        // cursor/DA queries and stalls the child until they are
        // answered (the todo-20 pathology — unanswered queries hang
        // the pipe for ~60 s each).
        loop {
            let scan = String::from_utf8_lossy(&captured[answered..]).into_owned();
            let dsr = scan.find("\x1b[6n");
            let da = scan.find("\x1b[c");
            let (at, len, reply) = match (dsr, da) {
                (Some(i), Some(j)) if i <= j => (i, 4, &b"\x1b[1;1R"[..]),
                (Some(i), None) => (i, 4, &b"\x1b[1;1R"[..]),
                (_, Some(j)) => (j, 3, &b"\x1b[?1;2c"[..]),
                (None, None) => break,
            };
            let _ = writer.write_all(reply);
            let _ = writer.flush();
            answered += at + len;
        }
        let text = String::from_utf8_lossy(&captured).into_owned();
        let working = text.contains("\x1b]7501;state=working:app=harness:msg=");
        let done = text.contains("\x1b]7501;state=done:app=harness:msg=");
        let lifecycle = text.contains("warp://cli-agent");
        if working && done && lifecycle {
            let _ = child.kill();
            let sequences: Vec<&str> = text
                .split('\x1b')
                .filter(|s| s.starts_with("]7501;") || s.starts_with("]777;notify;warp"))
                .map(|s| s.split(['\x07', '\x1b']).next().unwrap_or(s))
                .collect();
            println!("captured program-status sequences on the PTY master:");
            for seq in sequences {
                println!("  ESC{seq}");
            }
            return;
        }
    }
    panic!(
        "full-session sequences never appeared on the PTY master; captured:\n{}",
        String::from_utf8_lossy(&captured)
    );
}
