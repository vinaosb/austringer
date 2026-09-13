//! WebSocket session surfaces: the client attach socket (handshake +
//! bounded frame pump) and the worker connection socket (outbound-only
//! split-brain worker channel). Extracted from `server` to keep files
//! within the size budget.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};

use crate::attach::SessionHost;
use crate::protocol::{
    ClientMsg, ErrorCode, PROTOCOL_VERSION, ServerMsg, decode_client, encode_server,
};
use crate::server::DaemonState;
use crate::transport::pump_client;
use crate::worker::{DaemonToWorker, ToolGrant, WorkerRouter, WorkerToDaemon};

/// Upgrade handler for `/s/{sid}/attach`.
pub async fn attach_ws_socket(
    host: Arc<SessionHost>,
    socket: WebSocket,
    after: Option<u64>,
    idle_timeout: Duration,
) {
    run_ws_client(host, socket, after, idle_timeout).await;
}

async fn run_ws_client(
    host: Arc<SessionHost>,
    socket: WebSocket,
    after: Option<u64>,
    idle_timeout: Duration,
) {
    let (mut ws_sink, mut ws_source) = socket.split();
    // Handshake: the first frame must be a versioned Hello naming the
    // client. Authentication already happened at the HTTP layer (the
    // WS transport cannot see headers).
    let client_id = loop {
        let message = ws_source.next().await;
        let Some(Ok(message)) = message else {
            return;
        };
        let bytes: Vec<u8> = match message {
            Message::Text(text) => text.as_bytes().to_vec(),
            Message::Binary(bytes) => bytes.to_vec(),
            Message::Close(_) => return,
            _ => continue,
        };
        match decode_client(&bytes) {
            Ok(ClientMsg::Hello {
                version, client_id, ..
            }) => {
                if version != PROTOCOL_VERSION {
                    let _ = ws_sink
                        .send(Message::Text(
                            serde_json::to_string(&ServerMsg::Error {
                                code: ErrorCode::ProtocolVersion,
                                message: format!("server speaks {PROTOCOL_VERSION}"),
                            })
                            .unwrap_or_default()
                            .into(),
                        ))
                        .await;
                    return;
                }
                break client_id;
            }
            _ => {
                let _ = ws_sink
                    .send(Message::Text(
                        serde_json::to_string(&ServerMsg::Error {
                            code: ErrorCode::MalformedFrame,
                            message: "expected hello first".into(),
                        })
                        .unwrap_or_default()
                        .into(),
                    ))
                    .await;
                return;
            }
        }
    };

    let (inbound_tx, inbound_rx) = tokio::sync::mpsc::channel::<ClientMsg>(16);
    let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::channel::<ServerMsg>(64);
    let outbound_for_reader = outbound_tx.clone();
    let reader = tokio::spawn(async move {
        let outbound_tx = outbound_for_reader;
        let mut malformed = 0u32;
        while let Some(Ok(message)) = ws_source.next().await {
            let bytes: Vec<u8> = match message {
                Message::Text(text) => text.as_bytes().to_vec(),
                Message::Binary(bytes) => bytes.to_vec(),
                Message::Close(_) => break,
                _ => continue,
            };
            match decode_client(&bytes) {
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
    });
    let writer = tokio::spawn(async move {
        while let Some(msg) = outbound_rx.recv().await {
            let Ok(encoded) = encode_server(&msg) else {
                continue;
            };
            if ws_sink.send(Message::Text(encoded.into())).await.is_err() {
                break;
            }
        }
    });
    let _ = pump_client(
        &host,
        &client_id,
        after,
        true,
        idle_timeout,
        inbound_rx,
        outbound_tx,
    )
    .await;
    reader.abort();
    // Drain the writer instead of aborting: the final Bye/Error frame
    // may still be queued.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(1), writer).await;
}

/// Upgrade handler for `/worker` (worker connections dial IN — the
/// user side opens no inbound ports).
pub async fn worker_ws(State(state): State<Arc<DaemonState>>, ws: WebSocketUpgrade) -> Response {
    let router = Arc::clone(&state.worker_router);
    ws.on_upgrade(move |socket| async move {
        run_worker_socket(router, socket).await;
    })
}

async fn run_worker_socket(router: Arc<WorkerRouter>, socket: WebSocket) {
    let (mut sink, mut source) = socket.split();
    let (to_worker_tx, mut to_worker_rx) = tokio::sync::mpsc::channel::<DaemonToWorker>(16);
    let mut worker_id: Option<String> = None;
    let writer = tokio::spawn(async move {
        while let Some(msg) = to_worker_rx.recv().await {
            let Ok(encoded) = serde_json::to_string(&msg) else {
                continue;
            };
            if sink.send(Message::Text(encoded.into())).await.is_err() {
                break;
            }
        }
    });
    while let Some(Ok(message)) = source.next().await {
        let bytes: Vec<u8> = match message {
            Message::Text(text) => text.as_bytes().to_vec(),
            Message::Binary(bytes) => bytes.to_vec(),
            Message::Close(_) => break,
            _ => continue,
        };
        let Ok(decoded) = serde_json::from_slice::<WorkerToDaemon>(&bytes) else {
            continue;
        };
        match decoded {
            WorkerToDaemon::Hello {
                worker_id: id,
                grants,
                ..
            } => {
                let tools: Vec<ToolGrant> = grants;
                router.register(&id, tools, to_worker_tx.clone());
                let _ = to_worker_tx
                    .send(DaemonToWorker::Grant { tools: Vec::new() })
                    .await;
                worker_id = Some(id);
            }
            WorkerToDaemon::ToolOutput {
                call_id,
                ok,
                output,
            } => {
                router.deliver(&call_id, ok, output);
            }
            WorkerToDaemon::Ping => {
                let _ = to_worker_tx.send(DaemonToWorker::Pong).await;
            }
        }
    }
    if let Some(id) = worker_id {
        router.disconnect(&id);
    }
    writer.abort();
}
