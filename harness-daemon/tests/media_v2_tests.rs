//! Protocol v2 media surface acceptance (plan M9): additive upload +
//! submit-with-media + media-result events, with v1 connections
//! provably unaffected (byte-audited downgrade at the wire).

mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use harness_core::event::{ContentHash, MediaRef};
use harness_daemon::compat::downgrade_for_v1;
use harness_daemon::protocol::{
    ClientMsg, ServerMsg, SessionCommand, decode_server, encode_client, encode_server,
    version_compatible,
};

/// Minimal PNG-magic payload (probe only needs the header magic).
const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\n corpus-payload";

fn png_hash() -> String {
    ContentHash::from_bytes(PNG_BYTES).to_ref()
}

// ------------------------------------------------------------------
// Protocol-level (unit) acceptance
// ------------------------------------------------------------------

#[test]
fn version_gate_accepts_v1_and_v2_refuses_others() {
    assert!(version_compatible(1), "v1 Hello must never be refused");
    assert!(version_compatible(2));
    assert!(!version_compatible(0));
    assert!(!version_compatible(3));
    assert_eq!(harness_daemon::protocol::PROTOCOL_VERSION, 2);
}

#[test]
fn submit_media_decodes_and_enforces_bounds() {
    let media = MediaRef {
        hash: ContentHash::from_bytes(PNG_BYTES),
        bytes: PNG_BYTES.len() as u64,
    };
    let msg = ClientMsg::Command {
        command: SessionCommand::SubmitMedia {
            text: "describe this".into(),
            media: vec![media],
        },
    };
    let raw = encode_client(&msg).expect("encode");
    assert!(raw.contains("\"type\":\"submit_media\""));
    let back = harness_daemon::protocol::decode_client(raw.as_bytes()).expect("decode");
    assert_eq!(back, msg);

    let too_many = ClientMsg::Command {
        command: SessionCommand::SubmitMedia {
            text: "x".into(),
            media: vec![
                MediaRef {
                    hash: ContentHash::from_bytes(PNG_BYTES),
                    bytes: 1,
                };
                harness_daemon::protocol::MAX_MEDIA_PER_SUBMIT + 1
            ],
        },
    };
    let raw = encode_client(&too_many).expect("encode");
    assert_eq!(
        harness_daemon::protocol::decode_client(raw.as_bytes()),
        Err(harness_daemon::protocol::ProtocolError::OutOfBounds("media"))
    );

    // A malformed hash string fails decode typed, never panics.
    let bad = br#"{"type":"command","command":{"type":"submit_media","text":"t","media":[{"hash":"not-a-hash","bytes":3}]}}"#;
    assert!(harness_daemon::protocol::decode_client(bad).is_err());
}

#[test]
fn media_result_encodes_and_is_withheld_from_v1_wires() {
    let msg = ServerMsg::MediaResult {
        producer: "daemon-media-intake".into(),
        artifact: MediaRef {
            hash: ContentHash::from_bytes(PNG_BYTES),
            bytes: PNG_BYTES.len() as u64,
        },
        diff_summary: "appended 1 media block".into(),
    };
    let raw = encode_server(&msg).expect("encode");
    assert!(raw.contains("\"type\":\"media_result\""));
    assert_eq!(decode_server(raw.as_bytes()), Ok(msg.clone()));
    // The v1 downgrade WITHHOLDS v2-only variants entirely.
    assert_eq!(downgrade_for_v1(msg), None);
}

#[test]
fn downgrade_rewrites_media_blocks_to_text_markers_keeps_image_and_text() {
    use harness_core::event::{ContentBlock, Event, MediaBlock, MimeType};
    let mut meta = std::collections::BTreeMap::new();
    meta.insert("format".to_owned(), "png".to_owned());
    let media_block = MediaBlock::new(
        MimeType::new("image/png").expect("mime"),
        MediaRef {
            hash: ContentHash::from_bytes(PNG_BYTES),
            bytes: PNG_BYTES.len() as u64,
        },
        meta,
    )
    .expect("meta bounds");
    let recorded = ServerMsg::Event {
        frame: harness_daemon::protocol::Frame {
            id: 7,
            event: harness_daemon::protocol::SessionEvent::Recorded {
                seq: 3,
                event: Event::User {
                    blocks: vec![
                        ContentBlock::Text {
                            text: "see this".into(),
                        },
                        ContentBlock::Media(media_block),
                    ],
                },
            },
        },
    };
    let downgraded = downgrade_for_v1(recorded).expect("recorded frames survive the downgrade");
    let raw = encode_server(&downgraded).expect("encode");
    assert!(
        !raw.contains("\"type\":\"media\""),
        "v1 wire must not carry the media variant tag: {raw}"
    );
    assert!(
        raw.contains("[media: image/png blake3:"),
        "v1 wire carries the deterministic text marker: {raw}"
    );
    assert!(raw.contains("\"type\":\"text\""));
    assert!(raw.contains("\"id\":7"), "frame identity is preserved");
}

// ------------------------------------------------------------------
// HTTP upload + fetch acceptance
// ------------------------------------------------------------------

/// Media turns in these tests must complete without asks or workers:
/// the corpus/capture factory (CompleteOnlyInference) keeps the
/// transcript deterministic.
async fn complete_only_daemon() -> common::Spawned {
    let factory: std::sync::Arc<
        dyn Fn() -> Box<dyn harness_core::agent_loop::Inference> + Send + Sync,
    > = std::sync::Arc::new(|| Box::new(common::CompleteOnlyInference));
    common::spawn_daemon_with_factory(|c| c, factory)
        .await
        .expect("spawn")
}

fn http() -> reqwest::Client {
    reqwest::Client::new()
}

/// The workspace reqwest carries no `json` feature (transport-only);
/// bodies decode through text + serde_json.
async fn json_of(response: reqwest::Response) -> serde_json::Value {
    let text = response.text().await.expect("body");
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("json body ({e}): {text}"))
}

async fn upload(
    base: &str,
    token: &str,
    session: &str,
    bytes: &[u8],
    filename: Option<&str>,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut url = format!("http://{base}/s/{session}/media");
    if let Some(filename) = filename {
        url.push_str("?filename=");
        url.push_str(filename);
    }
    http()
        .post(url)
        .bearer_auth(token)
        .body(bytes.to_vec())
        .send()
        .await
}

#[tokio::test]
async fn upload_stores_dedups_and_probes() {
    let daemon = complete_only_daemon().await;
    let response = upload(&daemon.base, &daemon.token, "m9-up", PNG_BYTES, Some("corpus.png"))
        .await
        .expect("upload");
    assert_eq!(response.status(), 201, "first upload stores");
    let body = json_of(response).await;
    assert_eq!(body["media"]["hash"].as_str(), Some(png_hash().as_str()));
    assert_eq!(body["media"]["bytes"].as_u64(), Some(PNG_BYTES.len() as u64));
    assert_eq!(body["probe"]["outcome"].as_str(), Some("identified"));
    assert_eq!(body["probe"]["format"].as_str(), Some("png"));

    // Duplicate upload dedups by content hash: same ref, no new file.
    let again = upload(&daemon.base, &daemon.token, "m9-up", PNG_BYTES, None)
        .await
        .expect("upload again");
    assert_eq!(again.status(), 201);
    let body2 = json_of(again).await;
    assert_eq!(body2["media"]["hash"].as_str(), Some(png_hash().as_str()));
    assert_eq!(body2["dedup"].as_bool(), Some(true));

    let media_dir = daemon.state.sessions_root.join("m9-up").join("media");
    let stored: Vec<_> = std::fs::read_dir(&media_dir)
        .expect("media dir")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(stored.len(), 1, "content addressing stores one file");
}

#[tokio::test]
async fn upload_requires_token() {
    let daemon = complete_only_daemon().await;
    let response = http()
        .post(format!("http://{}/s/m9-auth/media", daemon.base))
        .body(PNG_BYTES.to_vec())
        .send()
        .await
        .expect("upload");
    assert_eq!(response.status(), 401, "upload sits behind token auth");
}

#[tokio::test]
async fn upload_over_cap_is_typed_413() {
    let daemon = common::spawn_daemon(|c| harness_daemon::config::DaemonConfig {
        media_upload_cap_bytes: 8,
        ..c
    })
    .await
    .expect("spawn");
    let response = upload(&daemon.base, &daemon.token, "m9-cap", PNG_BYTES, None)
        .await
        .expect("upload");
    assert_eq!(response.status(), 413, "beyond the configured cap");
    let body = json_of(response).await;
    assert_eq!(body["error"]["code"].as_str(), Some("media_too_large"));
    assert_eq!(body["error"]["limit"].as_u64(), Some(8));
    // The partial temp file was cleaned up.
    let media_dir = daemon.state.sessions_root.join("m9-cap").join("media");
    let temps: usize = std::fs::read_dir(&media_dir)
        .map(|dir| {
            dir.filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(".upload"))
                .count()
        })
        .unwrap_or(0);
    assert_eq!(temps, 0, "streamed temp files must be cleaned on 413");
}

#[tokio::test]
async fn upload_abort_mid_stream_cleans_temp_and_daemon_survives() {
    let daemon = common::spawn_daemon(|c| harness_daemon::config::DaemonConfig {
        media_upload_cap_bytes: 1024 * 1024,
        ..c
    })
    .await
    .expect("spawn");
    // A body stream that dies after the first chunk (client abort).
    let stream = futures_util::stream::iter(vec![Ok::<_, std::io::Error>(vec![b'x'; 64].into())])
        .chain(futures_util::stream::unfold((), |()| async {
            std::future::pending::<Option<(Result<bytes::Bytes, std::io::Error>, ())>>().await
        }));
    let body = reqwest::Body::wrap_stream(stream);
    let request = http()
        .post(format!("http://{}/s/m9-abort/media", daemon.base))
        .bearer_auth(&daemon.token)
        .body(body)
        .build()
        .expect("build");
    // Abandon the request mid-stream (bounded wait, then drop): the
    // daemon must clean up and keep serving (no hang, no temp residue).
    let _ = tokio::time::timeout(Duration::from_secs(2), http().execute(request)).await;
    // Drop the connection mid-stream: the daemon must clean up and
    // keep serving (no hang, no temp residue).
    let media_dir = daemon.state.sessions_root.join("m9-abort").join("media");
    let mut clean = false;
    for _ in 0..50 {
        let temps: usize = std::fs::read_dir(&media_dir)
            .map(|dir| {
                dir.filter_map(Result::ok)
                    .filter(|entry| {
                        entry.file_name().to_string_lossy().starts_with(".upload")
                    })
                    .count()
            })
            .unwrap_or(0);
        let stored: usize = std::fs::read_dir(&media_dir)
            .map(|dir| dir.filter_map(Result::ok).count())
            .unwrap_or(0);
        if temps == 0 && stored == 0 {
            clean = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(clean, "aborted upload leaves no temp or stored file");
    let alive = http()
        .get(format!("http://{}/sessions", daemon.base))
        .bearer_auth(&daemon.token)
        .send()
        .await
        .expect("daemon still serving");
    assert_eq!(alive.status(), 200);
}

#[tokio::test]
async fn get_media_serves_bytes_with_probed_content_type() {
    let daemon = complete_only_daemon().await;
    let stored = upload(&daemon.base, &daemon.token, "m9-get", PNG_BYTES, Some("corpus.png"))
        .await
        .expect("upload");
    assert_eq!(stored.status(), 201);
    let hash = png_hash();
    let colon = hash.strip_prefix("blake3:").expect("prefix");

    let ok = http()
        .get(format!("http://{}/s/m9-get/media/blake3:{colon}", daemon.base))
        .bearer_auth(&daemon.token)
        .send()
        .await
        .expect("get");
    assert_eq!(ok.status(), 200);
    assert_eq!(
        ok.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()),
        Some("image/png"),
        "content type comes from the probed mime"
    );
    let bytes = ok.bytes().await.expect("body");
    assert_eq!(&bytes[..], PNG_BYTES);

    let no_token = http()
        .get(format!("http://{}/s/m9-get/media/blake3:{colon}", daemon.base))
        .send()
        .await
        .expect("get");
    assert_eq!(no_token.status(), 401);

    let bad_hash = http()
        .get(format!("http://{}/s/m9-get/media/not-a-hash", daemon.base))
        .bearer_auth(&daemon.token)
        .send()
        .await
        .expect("get");
    assert_eq!(bad_hash.status(), 400);
    let body = json_of(bad_hash).await;
    assert_eq!(body["error"]["code"].as_str(), Some("bad_hash"));

    let unknown = ContentHash::from_bytes(b"never-uploaded-anywhere").to_ref();
    let unknown_hash = unknown.strip_prefix("blake3:").expect("prefix");
    let missing = http()
        .get(format!("http://{}/s/m9-get/media/blake3:{unknown_hash}", daemon.base))
        .bearer_auth(&daemon.token)
        .send()
        .await
        .expect("get");
    assert_eq!(missing.status(), 404);
    let body = json_of(missing).await;
    assert_eq!(body["error"]["code"].as_str(), Some("unknown_media"));
}

// ------------------------------------------------------------------
// End-to-end: v2 submit-with-media + v1 downgrade byte-audit
// ------------------------------------------------------------------

type WsSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn ws_attach(base: &str, token: &str, session: &str, version: u32, client_id: &str) -> WsSocket {
    let url = format!("ws://{base}/s/{session}/attach?token={token}");
    let (mut socket, _) = tokio_tungstenite::connect_async(&url).await.expect("ws connect");
    let hello = encode_client(&ClientMsg::Hello {
        version,
        token: token.to_owned(),
        client_id: client_id.into(),
        session: None,
    })
    .expect("encode hello");
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(hello.into()))
        .await
        .expect("send hello");
    socket
}

async fn ws_read(socket: &mut WsSocket) -> Option<String> {
    match tokio::time::timeout(Duration::from_secs(5), socket.next()).await {
        Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) => {
            Some(text.to_string())
        }
        _ => None,
    }
}

async fn collect_until(socket: &mut WsSocket, marker: &str) -> Vec<String> {
    let mut frames = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        let Some(frame) = ws_read(socket).await else { break };
        let hit = frame.contains(marker);
        frames.push(frame);
        if hit {
            break;
        }
    }
    frames
}

#[tokio::test]
async fn v2_submit_media_round_trips_blocks_log_and_media_result() {
    let daemon = complete_only_daemon().await;
    let stored = upload(&daemon.base, &daemon.token, "m9-v2", PNG_BYTES, Some("corpus.png"))
        .await
        .expect("upload");
    assert_eq!(stored.status(), 201);
    let body = json_of(stored).await;
    let hash_str = body["media"]["hash"].as_str().expect("hash").to_owned();
    let hash = ContentHash::parse(&hash_str).expect("parse");

    let mut v2 = ws_attach(&daemon.base, &daemon.token, "m9-v2", 2, "v2-client").await;
    let handshake = collect_until(&mut v2, "hello_ack").await;
    let ack = handshake
        .iter()
        .find(|frame| frame.contains("hello_ack"))
        .expect("hello ack seen");
    assert!(ack.contains("\"version\":2"), "v2 hello echoes version 2: {ack}");

    let submit = encode_client(&ClientMsg::Command {
        command: SessionCommand::SubmitMedia {
            text: "describe this image".into(),
            media: vec![MediaRef {
                hash,
                bytes: PNG_BYTES.len() as u64,
            }],
        },
    })
    .expect("encode submit_media");
    v2.send(tokio_tungstenite::tungstenite::Message::Text(submit.into()))
        .await
        .expect("send submit");

    let frames = collect_until(&mut v2, "turn_finished").await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("\"type\":\"media_result\""),
        "v2 connection observes MediaResult: {joined}"
    );
    assert!(
        joined.contains("\"type\":\"media\""),
        "v2 connection sees the TYPED media block in recorded frames: {joined}"
    );
    assert!(
        !joined.contains("\"type\":\"submit_media\""),
        "commands are never echoed back"
    );

    // The durable log keeps the typed form.
    let log_path = daemon.state.sessions_root.join("m9-v2").join("events.jsonl");
    let log = std::fs::read_to_string(log_path).expect("durable log");
    assert!(
        log.contains("\"type\":\"media\""),
        "durable log keeps the typed media block:\n{log}"
    );
}

#[tokio::test]
async fn v1_connection_receives_no_media_variant_byte_audit() {
    let daemon = complete_only_daemon().await;
    // v1 client attaches FIRST: it must witness the whole media turn.
    let mut v1 = ws_attach(&daemon.base, &daemon.token, "m9-v1", 1, "v1-client").await;
    let _ = collect_until(&mut v1, "hello_ack").await;

    let stored = upload(&daemon.base, &daemon.token, "m9-v1", PNG_BYTES, Some("corpus.png"))
        .await
        .expect("upload");
    assert_eq!(stored.status(), 201);
    let body = json_of(stored).await;
    let hash_str = body["media"]["hash"].as_str().expect("hash").to_owned();

    let mut v2 = ws_attach(&daemon.base, &daemon.token, "m9-v1", 2, "v2-client").await;
    let _ = collect_until(&mut v2, "hello_ack").await;
    let submit = serde_json::json!({
        "type": "command",
        "command": {
            "type": "submit_media",
            "text": "describe this image",
            "media": [{"hash": hash_str, "bytes": PNG_BYTES.len()}],
        },
    });
    v2.send(tokio_tungstenite::tungstenite::Message::Text(
        submit.to_string().into(),
    ))
    .await
    .expect("send submit");

    let _ = collect_until(&mut v2, "turn_finished").await;
    let v1_frames = collect_until(&mut v1, "turn_finished").await;
    let joined = v1_frames.join("\n");
    assert!(
        !joined.contains("\"type\":\"media\""),
        "BYTE-AUDIT: v1 wire carries no media variant tag:\n{joined}"
    );
    assert!(
        !joined.contains("media_result"),
        "BYTE-AUDIT: v1 wire carries no v2-only server message:\n{joined}"
    );
    assert!(
        joined.contains("[media: image/png blake3:"),
        "v1 wire carries the downgraded text marker:\n{joined}"
    );
}

#[tokio::test]
async fn ipc_v1_connection_byte_audit() {
    use interprocess::local_socket::tokio::prelude::*;
    use interprocess::local_socket::{GenericNamespaced, tokio::Stream};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let daemon = complete_only_daemon().await;
    let stored = upload(&daemon.base, &daemon.token, "m9-ipc", PNG_BYTES, Some("corpus.png"))
        .await
        .expect("upload");
    assert_eq!(stored.status(), 201);
    let body = json_of(stored).await;
    let hash_str = body["media"]["hash"].as_str().expect("hash").to_owned();

    // IPC carrier: named pipe from the daemon's own config.
    let state = std::sync::Arc::clone(&daemon.state);
    let name = format!("harness-m9-ipc-{}", std::process::id());
    let server = tokio::spawn({
        let state = std::sync::Arc::clone(&state);
        let name = name.clone();
        async move { harness_daemon::ipc::serve_ipc(state, &name).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    let ns_name = name.to_ns_name::<GenericNamespaced>().expect("ns");
    let stream = Stream::connect(ns_name).await.expect("ipc connect");
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);

    let hello = encode_client(&ClientMsg::Hello {
        version: 1,
        token: daemon.token.clone(),
        client_id: "ipc-v1".into(),
        session: Some("m9-ipc".into()),
    })
    .expect("encode hello");
    write_half.write_all(hello.as_bytes()).await.expect("write");
    write_half.write_all(b"\n").await.expect("newline");
    write_half.flush().await.expect("flush");

    let submit = serde_json::json!({
        "type": "command",
        "command": {
            "type": "submit_media",
            "text": "over ipc",
            "media": [{"hash": hash_str, "bytes": PNG_BYTES.len()}],
        },
    });

    let mut lines = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    // hello handshake frames
    while tokio::time::Instant::now() < deadline {
        let mut line = String::new();
        match tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut line)).await {
            Ok(Ok(n)) if n > 0 => {
                lines.push(line.trim_end().to_owned());
                if lines.last().is_some_and(|l| l.contains("hello_ack")) {
                    break;
                }
            }
            _ => break,
        }
    }
    write_all(&mut write_half, &submit.to_string()).await;
    while tokio::time::Instant::now() < deadline {
        let mut line = String::new();
        match tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut line)).await {
            Ok(Ok(n)) if n > 0 => {
                lines.push(line.trim_end().to_owned());
                if lines.last().is_some_and(|l| l.contains("turn_finished")) {
                    break;
                }
            }
            _ => break,
        }
    }
    server.abort();
    let joined = lines.join("\n");
    assert!(
        joined.contains("turn_finished"),
        "ipc v1 session observed the media turn:\n{joined}"
    );
    assert!(
        !joined.contains("\"type\":\"media\""),
        "BYTE-AUDIT (ipc): no media variant tag on the v1 wire:\n{joined}"
    );
    assert!(
        !joined.contains("media_result"),
        "BYTE-AUDIT (ipc): no v2-only server message on the v1 wire:\n{joined}"
    );
    assert!(
        joined.contains("[media: image/png blake3:"),
        "ipc v1 wire carries the downgraded marker:\n{joined}"
    );
}

async fn write_all(
    writer: &mut tokio::io::WriteHalf<interprocess::local_socket::tokio::Stream>,
    raw: &str,
) {
    use tokio::io::AsyncWriteExt;
    writer.write_all(raw.as_bytes()).await.expect("write");
    writer.write_all(b"\n").await.expect("newline");
    writer.flush().await.expect("flush");
}

#[tokio::test]
async fn forged_media_ref_is_typed_refused_no_turn() {
    let daemon = complete_only_daemon().await;
    let mut v2 = ws_attach(&daemon.base, &daemon.token, "m9-forged", 2, "v2-client").await;
    let _ = collect_until(&mut v2, "hello_ack").await;
    let ghost = ContentHash::from_bytes(b"never uploaded").to_ref();
    let submit = serde_json::json!({
        "type": "command",
        "command": {
            "type": "submit_media",
            "text": "forged ref",
            "media": [{"hash": ghost, "bytes": 3}],
        },
    });
    v2.send(tokio_tungstenite::tungstenite::Message::Text(
        submit.to_string().into(),
    ))
    .await
    .expect("send");
    let frames = collect_until(&mut v2, "error").await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("unknown media"),
        "forged ref must be typed-refused to the submitting connection:\n{joined}"
    );
    assert!(
        !joined.contains("turn_started"),
        "no turn may start from a forged ref:\n{joined}"
    );
    // The daemon stays up.
    let alive = http()
        .get(format!("http://{}/sessions", daemon.base))
        .bearer_auth(&daemon.token)
        .send()
        .await
        .expect("daemon alive");
    assert_eq!(alive.status(), 200);
}
