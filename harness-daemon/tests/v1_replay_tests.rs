//! v1 replay acceptance (plan M9): the STEP-0 corpus captured from the
//! v1-era daemon (see `v1_corpus_capture.rs`) replays against the
//! CURRENT daemon — the server frames a v1 client receives must be
//! byte-identical to the recorded transcript AND confined to the v1
//! wire vocabulary (no `media` content-block variant, no v2-only
//! server messages). This is the "v1 clients provably unaffected"
//! gate: it was green before the v2 change and must stay green.

mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use harness_daemon::protocol::SessionEvent;
use harness_daemon::server::DaemonState;

struct FixtureRecord {
    dir: String,
    await_marker: Option<String>,
    action: Option<String>,
    raw: Option<String>,
}

fn load_fixture(name: &str) -> Vec<FixtureRecord> {
    let path = format!("{}/tests/fixtures/v1/{name}", env!("CARGO_MANIFEST_DIR"));
    let body = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("fixture {name}: {e}"));
    body.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value: serde_json::Value =
                serde_json::from_str(line).unwrap_or_else(|e| panic!("fixture line: {e}"));
            FixtureRecord {
                dir: value["dir"].as_str().unwrap_or_default().to_owned(),
                await_marker: value["await"].as_str().map(str::to_owned),
                action: value["action"].as_str().map(str::to_owned),
                raw: value["raw"].as_str().map(str::to_owned),
            }
        })
        .collect()
}

/// The v1 wire vocabulary: server message tags and content-block tags
/// a v0.1 frontend's serde schema can decode. Anything outside this
/// set on a v1 connection is a protocol break.
const V1_SERVER_TAGS: &[&str] = &[
    "hello_ack",
    "event",
    "permission_ask",
    "permission_resolved",
    "state_resync_required",
    "snapshot",
    "error",
    "pong",
    "bye",
];
const V1_BLOCK_TAGS: &[&str] = &["text", "image"];

/// Assert one raw server frame is v1-decodable: top-level tag in the
/// v1 vocabulary, every content-block tag inside `blocks` arrays in
/// the v1 block vocabulary, and no v2-only variant tag anywhere in
/// the bytes.
fn assert_v1_decodable(raw: &str) {
    assert!(
        !raw.contains("\"media_result\""),
        "v2-only server message on a v1 wire: {raw}"
    );
    assert!(
        !raw.contains("\"type\":\"media\""),
        "media content-block variant on a v1 wire: {raw}"
    );
    let value: serde_json::Value = serde_json::from_str(raw)
        .unwrap_or_else(|e| panic!("v1 client cannot parse frame ({e}): {raw}"));
    let tag = value["type"].as_str().unwrap_or_default();
    assert!(
        V1_SERVER_TAGS.contains(&tag),
        "server tag {tag:?} outside v1 vocabulary: {raw}"
    );
    walk_blocks(&value, raw);
}

fn walk_blocks(value: &serde_json::Value, raw: &str) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                walk_blocks(item, raw);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Array(blocks)) = map.get("blocks") {
                for block in blocks {
                    let block_type = block["type"].as_str().unwrap_or_default();
                    assert!(
                        V1_BLOCK_TAGS.contains(&block_type),
                        "content block {block_type:?} outside v1 vocabulary: {raw}"
                    );
                }
            }
            for item in map.values() {
                walk_blocks(item, raw);
            }
        }
        _ => {}
    }
}

fn publish_image_event(state: &DaemonState, session: &str) {
    // Same scripted action as the capture tool (v1_corpus_capture.rs):
    // the recorded image-block user event, byte-identical payload, so
    // the transcript's frame id 7 reproduces exactly.
    const IMAGE_PAYLOAD: &[u8] = b"harness-v1-corpus-image-payload";
    let host = state.session(session);
    host.publish(SessionEvent::Recorded {
        seq: 900,
        event: harness_core::event::Event::User {
            blocks: vec![
                harness_core::event::ContentBlock::Text {
                    text: "see this diagram".into(),
                },
                harness_core::event::ContentBlock::Image(harness_core::event::ImageBlock {
                    mime: harness_core::event::MimeType::png(),
                    width: 800,
                    height: 600,
                    media: harness_core::event::MediaRef {
                        hash: harness_core::event::ContentHash::from_bytes(IMAGE_PAYLOAD),
                        bytes: IMAGE_PAYLOAD.len() as u64,
                    },
                }),
            ],
        },
    });
}

type WsSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// One observed server frame must equal the next expected transcript
/// frame, byte for byte.
fn expect_next(observed: &str, expected: &mut std::collections::VecDeque<String>) {
    let next = expected
        .pop_front()
        .unwrap_or_else(|| panic!("server sent an extra frame: {observed}"));
    assert_eq!(observed, next, "v1 wire bytes drifted from the v1 corpus");
    assert_v1_decodable(observed);
}

async fn ws_read(socket: &mut WsSocket) -> Option<String> {
    let deadline = Duration::from_secs(5);
    match tokio::time::timeout(deadline, socket.next()).await {
        Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) => {
            Some(text.to_string())
        }
        _ => None,
    }
}

async fn ipc_read_line(
    reader: &mut tokio::io::BufReader<
        tokio::io::ReadHalf<interprocess::local_socket::tokio::Stream>,
    >,
) -> Option<String> {
    use tokio::io::AsyncBufReadExt;
    let mut line = String::new();
    match tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut line)).await {
        Ok(Ok(n)) if n > 0 => Some(line.trim_end().to_owned()),
        _ => None,
    }
}

#[tokio::test]
async fn v1_ws_corpus_replays_byte_identically_against_current_daemon() {
    let records = load_fixture("ws-session.jsonl");
    let mut expected: std::collections::VecDeque<String> = records
        .iter()
        .filter(|record| record.dir == "s2c")
        .filter_map(|record| record.raw.clone())
        .collect();
    let factory: std::sync::Arc<
        dyn Fn() -> Box<dyn harness_core::agent_loop::Inference> + Send + Sync,
    > = std::sync::Arc::new(|| Box::new(common::CompleteOnlyInference));
    let daemon = common::spawn_daemon_with_factory(|c| c, factory)
        .await
        .expect("spawn");
    let url = format!("ws://{}/s/v1-ws/attach?token={}", daemon.base, daemon.token);
    let (mut socket, _) = tokio_tungstenite::connect_async(&url).await.expect("ws connect");

    // Cumulative observation: a c2s barrier only waits when the marker
    // was not already seen (earlier s2c records may have satisfied it).
    let mut seen = String::new();
    for record in &records {
        match record.dir.as_str() {
            "c2s" => {
                if let Some(marker) = &record.await_marker
                    && !seen.contains(marker.as_str())
                {
                    loop {
                        let Some(frame) = ws_read(&mut socket).await else {
                            panic!("connection closed waiting for {marker}");
                        };
                        expect_next(&frame, &mut expected);
                        seen.push_str(&frame);
                        if frame.contains(marker.as_str()) {
                            break;
                        }
                    }
                }
                let raw = record.raw.as_deref().unwrap_or_default();
                socket
                    .send(tokio_tungstenite::tungstenite::Message::Text(raw.into()))
                    .await
                    .expect("send");
            }
            "srv" => {
                assert_eq!(record.action.as_deref(), Some("publish_image_event"));
                publish_image_event(&daemon.state, "v1-ws");
            }
            _ => {
                let Some(frame) = ws_read(&mut socket).await else {
                    panic!("connection closed expecting a transcript frame");
                };
                expect_next(&frame, &mut expected);
                seen.push_str(&frame);
            }
        }
    }
    // No extra frames beyond the transcript.
    if let Some(extra) = ws_read(&mut socket).await {
        panic!("unexpected extra frame after transcript: {extra}");
    }
    assert!(expected.is_empty(), "transcript frames never sent: {expected:?}");
}

#[tokio::test]
async fn v1_ipc_corpus_replays_byte_identically_against_current_daemon() {
    use interprocess::local_socket::tokio::prelude::*;
    use interprocess::local_socket::{GenericNamespaced, tokio::Stream};
    use tokio::io::{AsyncWriteExt, BufReader};

    let records = load_fixture("ipc-session.jsonl");
    let mut expected: std::collections::VecDeque<String> = records
        .iter()
        .filter(|record| record.dir == "s2c")
        .filter_map(|record| record.raw.clone())
        .collect();
    let dir = tempfile::tempdir().expect("tempdir");
    let factory: std::sync::Arc<
        dyn Fn() -> Box<dyn harness_core::agent_loop::Inference> + Send + Sync,
    > = std::sync::Arc::new(|| Box::new(common::CompleteOnlyInference));
    let state = std::sync::Arc::new(
        DaemonState::new(
            harness_daemon::config::DaemonConfig {
                data_dir: dir.path().to_owned(),
                token: Some("v1-corpus-ipc".into()),
                ..harness_daemon::config::DaemonConfig::default()
            },
            factory,
        )
        .expect("state"),
    );
    let name = format!("harness-v1-replay-ipc-{}", std::process::id());
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

    let read_line = ipc_read_line;

    // Cumulative observation (see the WS replay test).
    let mut seen = String::new();
    for record in &records {
        match record.dir.as_str() {
            "c2s" => {
                if let Some(marker) = &record.await_marker
                    && !seen.contains(marker.as_str())
                {
                    loop {
                        let Some(frame) = read_line(&mut reader).await else {
                            panic!("ipc closed waiting for {marker}");
                        };
                        expect_next(&frame, &mut expected);
                        seen.push_str(&frame);
                        if frame.contains(marker.as_str()) {
                            break;
                        }
                    }
                }
                let raw = record.raw.as_deref().unwrap_or_default();
                write_half.write_all(raw.as_bytes()).await.expect("ipc write");
                write_half.write_all(b"\n").await.expect("ipc newline");
                write_half.flush().await.expect("ipc flush");
            }
            "srv" => {
                assert_eq!(record.action.as_deref(), Some("publish_image_event"));
                publish_image_event(&state, "v1-ipc");
            }
            _ => {
                let Some(frame) = read_line(&mut reader).await else {
                    panic!("ipc closed expecting a transcript frame");
                };
                expect_next(&frame, &mut expected);
                seen.push_str(&frame);
            }
        }
    }
    server.abort();
    assert!(expected.is_empty(), "transcript frames never sent: {expected:?}");
}

/// The v1 image-block recorded frame stays byte-stable through the
/// current codec (fixture pins the durable form; see M3's session-log
/// compat pin for the same discipline at the log layer).
#[test]
fn v1_corpus_image_frame_round_trips_byte_stably() {
    let records = load_fixture("ws-session.jsonl");
    let image_frame = records
        .iter()
        .find_map(|record| {
            record
                .raw
                .as_deref()
                .filter(|raw| raw.contains("\"type\":\"image\""))
        })
        .expect("corpus carries an image frame");
    let decoded = harness_daemon::protocol::decode_server(image_frame.as_bytes())
        .unwrap_or_else(|e| panic!("image frame failed v1 decode: {e}"));
    let reencoded = harness_daemon::protocol::encode_server(&decoded)
        .unwrap_or_else(|e| panic!("image frame failed re-encode: {e}"));
    assert_eq!(reencoded, image_frame);
}
