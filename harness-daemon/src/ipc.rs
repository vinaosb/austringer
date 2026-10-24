//! IPC transport: an interprocess local socket (named pipe on Windows,
//! Unix-domain socket on Unix — one codebase via `GenericNamespaced`
//! names) speaking the SAME client protocol through the same
//! [`crate::transport::pump_client`]. Auth is the token inside the
//! Hello handshake (there is no HTTP layer here, and the peer is the
//! local machine by construction). Frames are newline-delimited JSON
//! with a hard per-line bound.

use std::sync::Arc;

use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

use crate::protocol::{
    ClientMsg, ErrorCode, PROTOCOL_VERSION, ServerMsg, decode_client, encode_server,
    version_compatible,
};
use crate::server::DaemonState;
use crate::transport::{PumpParams, pump_client};

/// Bind and serve the IPC listener until the process ends.
///
/// # Errors
/// listener creation failure.
pub async fn serve_ipc(state: Arc<DaemonState>, name: &str) -> std::io::Result<()> {
    let ns_name = name.to_ns_name::<GenericNamespaced>()?;
    let listener = ListenerOptions::new().name(ns_name).create_tokio()?;
    loop {
        match listener.accept().await {
            Ok(stream) => {
                let state = Arc::clone(&state);
                let idle = state.config.idle_timeout;
                tokio::spawn(async move {
                    handle_ipc_conn(state, stream, idle).await;
                });
            }
            Err(_) => continue,
        }
    }
}

/// Read one bounded line (max [`crate::protocol::MAX_FRAME_BYTES`]).
/// Oversized lines are rejected as malformed without unbounded reads.
async fn read_bounded_line<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
) -> std::io::Result<Option<String>> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = reader.read(&mut byte).await?;
        if n == 0 {
            if line.is_empty() {
                return Ok(None);
            }
            break;
        }
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
        if line.len() > crate::protocol::MAX_FRAME_BYTES {
            return Err(std::io::Error::other("frame exceeds bound"));
        }
    }
    String::from_utf8(line)
        .map(Some)
        .map_err(|_| std::io::Error::other("frame not utf-8"))
}

async fn write_line<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    msg: &ServerMsg,
) -> std::io::Result<()> {
    let mut body = encode_server(msg).map_err(|_| std::io::Error::other("encode"))?;
    body.push('\n');
    writer.write_all(body.as_bytes()).await
}

async fn handle_ipc_conn(
    state: Arc<DaemonState>,
    stream: Stream,
    idle_timeout: std::time::Duration,
) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = reader;

    // Handshake: one Hello line carrying token + session binding.
    let first = match read_bounded_line(&mut reader).await {
        Ok(Some(line)) => line,
        _ => return,
    };
    let hello = match decode_client(first.as_bytes()) {
        Ok(ClientMsg::Hello {
            version,
            token,
            client_id,
            session,
            ..
        }) => Some((version, token, client_id, session)),
        _ => None,
    };
    let Some((version, token, client_id, session)) = hello else {
        let _ = write_line(
            &mut writer,
            &ServerMsg::Error {
                code: ErrorCode::MalformedFrame,
                message: "expected hello first".into(),
            },
        )
        .await;
        return;
    };
    if !version_compatible(version) {
        let _ = write_line(
            &mut writer,
            &ServerMsg::Error {
                code: ErrorCode::ProtocolVersion,
                message: format!("server speaks {PROTOCOL_VERSION}"),
            },
        )
        .await;
        return;
    }
    if !crate::auth::token_matches(&state.token, &token) {
        let _ = write_line(
            &mut writer,
            &ServerMsg::Error {
                code: ErrorCode::Auth,
                message: "token mismatch".into(),
            },
        )
        .await;
        return;
    }
    let Some(session_id) = session else {
        let _ = write_line(
            &mut writer,
            &ServerMsg::Error {
                code: ErrorCode::MalformedFrame,
                message: "ipc hello requires a session".into(),
            },
        )
        .await;
        return;
    };
    let host = state.session(&session_id);

    let (inbound_tx, inbound_rx) = mpsc::channel::<ClientMsg>(16);
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<ServerMsg>(64);
    let reader_task = tokio::spawn({
        let inbound_tx = inbound_tx.clone();
        let outbound_tx = outbound_tx.clone();
        async move {
            let mut malformed = 0u32;
            while let Ok(Some(line)) = read_bounded_line(&mut reader).await {
                match decode_client(line.as_bytes()) {
                    Ok(msg) => {
                        malformed = 0;
                        if inbound_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        malformed += 1;
                        let _ = outbound_tx
                            .send(ServerMsg::Error {
                                code: ErrorCode::MalformedFrame,
                                message: "frame rejected".into(),
                            })
                            .await;
                        if malformed >= 3 {
                            break;
                        }
                    }
                }
            }
        }
    });
    let writer_task = tokio::spawn(async move {
        while let Some(msg) = outbound_rx.recv().await {
            if write_line(&mut writer, &msg).await.is_err() {
                break;
            }
        }
    });
    let _ = pump_client(
        &host,
        &client_id,
        PumpParams {
            peer_version: version,
            after: None,
            enforce_idle: true,
            idle_timeout,
        },
        inbound_rx,
        outbound_tx,
    )
    .await;
    reader_task.abort();
    writer_task.abort();
}
