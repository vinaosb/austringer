//! STEP-0 v1 replay-corpus capture tool (plan todo M9). NOT a CI test:
//! `#[ignore]`d by design — it records the wire transcripts a REAL v1
//! client session produces against the daemon AS IT EXISTS at capture
//! time, into `tests/fixtures/v1/`. Run it ONCE per protocol era:
//!
//! ```text
//! cargo nextest run -p harness-daemon -E "binary(v1_corpus_capture)" --run-ignored only
//! ```
//!
//! The corpus is the v1-compatibility oracle: when PROTOCOL_VERSION
//! moves past 1, the replay test (`v1_replay_tests.rs`) feeds these
//! recorded client frames back through the new daemon and asserts the
//! server frames stay byte-identical AND v1-decodable (no `media`
//! content-block variant, no v2-only server messages). Once the
//! protocol changes, this capture can never be re-recorded — the v1
//! era is over — which is why the fixtures are committed pristine.
//!
//! Transcript format (JSONL, one record per line):
//! - `{"dir":"c2s","await":"turn_finished","raw":"…"}` — client frame
//!   `raw` (exact bytes sent), sent only after a server frame
//!   containing `await` was observed (`null`: send immediately).
//! - `{"dir":"s2c","raw":"…"}` — one server frame, exactly as encoded
//!   on the wire (raw string, never re-serialized through
//!   `serde_json::Value`, whose sorted keys would scramble field
//!   order).
//! - `{"dir":"srv","action":"publish_image_event"}` — a server-side
//!   scripted action the replay harness must perform at that point
//!   (the v1 protocol has no client path for image submits; the event
//!   enters through the session host's publish, the same path the
//!   engine driver uses for recorded events after a turn).
//!
//! Coverage per plan M9: all three Hello handshake sites — the WS
//! initial Hello (ws.rs), the IPC initial Hello (ipc.rs), and the
//! mid-session in-stream re-Hello checked inside the client pump
//! (transport.rs). Recorded frames include an image-block user event.

mod common;

use std::time::Duration;

use common::spawn_daemon_with_factory;
use futures_util::{SinkExt, StreamExt};
use harness_core::event::{ContentBlock, ImageBlock, MediaRef};
use harness_daemon::protocol::{
    ClientMsg, SessionCommand, SessionEvent, decode_client, decode_server, encode_client,
    encode_server,
};
use harness_daemon::server::DaemonState;

/// Deterministic image payload: fixed bytes -> fixed blake3 hash, so
/// the fixture is byte-stable across captures.
const IMAGE_PAYLOAD: &[u8] = b"harness-v1-corpus-image-payload";

fn publish_image_event(state: &DaemonState, session: &str) {
    let host = state.session(session);
    host.publish(SessionEvent::Recorded {
        seq: 900,
        event: harness_core::event::Event::User {
            blocks: vec![
                ContentBlock::Text {
                    text: "see this diagram".into(),
                },
                ContentBlock::Image(ImageBlock {
                    mime: harness_core::event::MimeType::png(),
                    width: 800,
                    height: 600,
                    media: MediaRef {
                        hash: harness_core::event::ContentHash::from_bytes(IMAGE_PAYLOAD),
                        bytes: IMAGE_PAYLOAD.len() as u64,
                    },
                }),
            ],
        },
    });
}

fn push_c2s(out: &mut Vec<String>, raw: &str, await_marker: Option<&str>) {
    let record = serde_json::json!({"dir": "c2s", "await": await_marker, "raw": raw});
    out.push(record.to_string());
}

fn push_s2c(out: &mut Vec<String>, raw: &str) {
    let record = serde_json::json!({"dir": "s2c", "raw": raw});
    out.push(record.to_string());
}

fn push_srv(out: &mut Vec<String>, action: &str) {
    let record = serde_json::json!({"dir": "srv", "action": action});
    out.push(record.to_string());
}

type WsSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Read server frames until one contains `marker`, recording each.
async fn ws_drain_until(socket: &mut WsSocket, marker: &str, out: &mut Vec<String>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        let next = tokio::time::timeout(Duration::from_secs(2), socket.next()).await;
        match next {
            Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) => {
                let raw = text.to_string();
                let hit = raw.contains(marker);
                push_s2c(out, &raw);
                if hit {
                    return;
                }
            }
            _ => break,
        }
    }
    panic!("ws capture: never saw marker {marker}");
}

async fn ws_send(socket: &mut WsSocket, raw: &str) {
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(raw.into()))
        .await
        .expect("ws send");
}

async fn capture_ws(out: &mut Vec<String>) {
    let daemon = spawn_daemon_with_factory(
        |c| c,
        std::sync::Arc::new(|| Box::new(common::CompleteOnlyInference)),
    )
    .await
    .expect("spawn");
    let url = format!("ws://{}/s/v1-ws/attach?token={}", daemon.base, daemon.token);
    let (mut socket, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("ws connect");

    let hello = encode_client(&ClientMsg::Hello {
        version: 1,
        token: daemon.token.clone(),
        client_id: "corpus-ws".into(),
        session: None,
    })
    .expect("encode hello");
    let submit = encode_client(&ClientMsg::Command {
        command: SessionCommand::Submit {
            text: "hello v1 corpus".into(),
        },
    })
    .expect("encode submit");
    let rehello = hello.clone();
    let ping = encode_client(&ClientMsg::Ping).expect("encode ping");

    push_c2s(out, &hello, None);
    ws_send(&mut socket, &hello).await;
    ws_drain_until(&mut socket, "hello_ack", out).await;

    push_c2s(out, &submit, Some("hello_ack"));
    ws_send(&mut socket, &submit).await;
    ws_drain_until(&mut socket, "turn_finished", out).await;

    // Server-side scripted image-block recorded event, then the
    // mid-session v1 re-Hello (the transport.rs handshake site).
    publish_image_event(&daemon.state, "v1-ws");
    push_srv(out, "publish_image_event");
    push_c2s(out, &rehello, Some("turn_finished"));
    ws_send(&mut socket, &rehello).await;
    ws_drain_until(&mut socket, "recorded", out).await;

    push_c2s(out, &ping, Some("recorded"));
    ws_send(&mut socket, &ping).await;
    ws_drain_until(&mut socket, "pong", out).await;
    let _ = socket.close(None).await;
}

async fn ipc_send(
    writer: &mut tokio::io::WriteHalf<interprocess::local_socket::tokio::Stream>,
    raw: &str,
) {
    use tokio::io::AsyncWriteExt;
    writer.write_all(raw.as_bytes()).await.expect("ipc write");
    writer.write_all(b"\n").await.expect("ipc newline");
    writer.flush().await.expect("ipc flush");
}

async fn ipc_drain_until(
    reader: &mut tokio::io::BufReader<tokio::io::ReadHalf<interprocess::local_socket::tokio::Stream>>,
    marker: &str,
    out: &mut Vec<String>,
) {
    use tokio::io::AsyncBufReadExt;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        let mut line = String::new();
        let next = tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut line)).await;
        match next {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
            Ok(Ok(_)) => {
                let raw = line.trim_end().to_owned();
                if raw.is_empty() {
                    continue;
                }
                let hit = raw.contains(marker);
                push_s2c(out, &raw);
                if hit {
                    return;
                }
            }
        }
    }
    panic!("ipc capture: never saw marker {marker}");
}

async fn capture_ipc(out: &mut Vec<String>) {
    use interprocess::local_socket::tokio::prelude::*;
    use interprocess::local_socket::{GenericNamespaced, tokio::Stream};
    use tokio::io::BufReader;

    let dir = tempfile::tempdir().expect("tempdir");
    let state = std::sync::Arc::new(
        DaemonState::new(
            harness_daemon::config::DaemonConfig {
                data_dir: dir.path().to_owned(),
                token: Some("v1-corpus-ipc".into()),
                ..harness_daemon::config::DaemonConfig::default()
            },
            std::sync::Arc::new(|| Box::new(common::CompleteOnlyInference)),
        )
        .expect("state"),
    );
    let name = format!("harness-v1-corpus-ipc-{}", std::process::id());
    let server = tokio::spawn({
        let state = std::sync::Arc::clone(&state);
        let name = name.clone();
        async move { harness_daemon::ipc::serve_ipc(state, &name).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;

    let ns_name = name.to_ns_name::<GenericNamespaced>().expect("ns name");
    let stream = Stream::connect(ns_name).await.expect("ipc connect");
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);

    let hello = encode_client(&ClientMsg::Hello {
        version: 1,
        token: "v1-corpus-ipc".into(),
        client_id: "corpus-ipc".into(),
        session: Some("v1-ipc".into()),
    })
    .expect("encode hello");
    let submit = encode_client(&ClientMsg::Command {
        command: SessionCommand::Submit {
            text: "ipc v1 corpus".into(),
        },
    })
    .expect("encode submit");

    push_c2s(out, &hello, None);
    ipc_send(&mut write_half, &hello).await;
    ipc_drain_until(&mut reader, "hello_ack", out).await;

    push_c2s(out, &submit, Some("hello_ack"));
    ipc_send(&mut write_half, &submit).await;
    ipc_drain_until(&mut reader, "turn_finished", out).await;

    publish_image_event(&state, "v1-ipc");
    push_srv(out, "publish_image_event");
    ipc_drain_until(&mut reader, "recorded", out).await;
    server.abort();
}

fn write_fixture(name: &str, records: &[String]) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v1");
    std::fs::create_dir_all(dir).expect("fixture dir");
    let path = format!("{dir}/{name}");
    std::fs::write(&path, records.join("\n") + "\n").expect("write fixture");
    println!("captured {} ({} records)", path, records.len());
}

/// Sanity guard inside the capture itself: the transcript must contain
/// a recorded image-block frame and every frame must round-trip
/// through the v1 codec byte-stably, otherwise the fixture is not a
/// v1 oracle.
fn validate_fixture(records: &[String]) {
    let joined = records.join("\n");
    assert!(
        joined.contains("image"),
        "corpus must carry an image-block recorded frame"
    );
    for line in records {
        let record: serde_json::Value = serde_json::from_str(line).expect("capture line JSON");
        if record["dir"] == "srv" {
            continue;
        }
        let raw = record["raw"].as_str().expect("raw frame").to_owned();
        if record["dir"] == "s2c" {
            let decoded = decode_server(raw.as_bytes())
                .unwrap_or_else(|e| panic!("server frame failed v1 decode ({e}): {raw}"));
            let reencoded = encode_server(&decoded)
                .unwrap_or_else(|e| panic!("server frame failed re-encode ({e}): {raw}"));
            assert_eq!(reencoded, raw, "v1 server encoding must be byte-stable");
        } else if record["dir"] == "c2s" {
            let decoded = decode_client(raw.as_bytes())
                .unwrap_or_else(|e| panic!("client frame failed v1 decode ({e}): {raw}"));
            let reencoded = encode_client(&decoded)
                .unwrap_or_else(|e| panic!("client frame failed re-encode ({e}): {raw}"));
            assert_eq!(reencoded, raw, "v1 client encoding must be byte-stable");
        }
    }
}

#[tokio::test]
#[ignore = "STEP-0 capture tool: records the v1-era corpus once; re-running after a protocol change would falsify history"]
async fn capture_v1_ws_and_ipc_corpus() {
    let mut ws = Vec::new();
    capture_ws(&mut ws).await;
    validate_fixture(&ws);
    write_fixture("ws-session.jsonl", &ws);

    let mut ipc = Vec::new();
    capture_ipc(&mut ipc).await;
    validate_fixture(&ipc);
    write_fixture("ipc-session.jsonl", &ipc);
}
