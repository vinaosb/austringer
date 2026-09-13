//! HTTP handlers (root, SPA deep-link, SSE stream, JSON commands,
//! session list, reload) extracted from `server` for the size budget.
//! Every handler runs BEHIND the auth middleware.

use std::sync::Arc;

use axum::Json;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::attach::HostCommand;
use crate::protocol::{ClientMsg, ServerMsg, SessionCommand, encode_server};
use crate::reload;
use crate::server::DaemonState;
use crate::transport::pump_client;
use crate::web;
pub(crate) async fn root() -> Response {
    web::serve("/")
}

pub(crate) async fn session_page() -> Response {
    web::spa_shell()
}

pub(crate) async fn attach_ws(
    State(state): State<Arc<DaemonState>>,
    Path(sid): Path<String>,
    Query(query): Query<crate::server::TokenQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let host = state.session(&sid);
    let after = query.after;
    let idle = state.config.idle_timeout;
    ws.on_upgrade(move |socket| async move {
        crate::ws::attach_ws_socket(host, socket, after, idle).await;
    })
}

pub(crate) async fn sse_events(
    State(state): State<Arc<DaemonState>>,
    Path(sid): Path<String>,
    Query(query): Query<crate::server::TokenQuery>,
) -> Response {
    let host = state.session(&sid);
    let after = query.after;
    let (outbound_tx, outbound_rx) = tokio::sync::mpsc::channel::<ServerMsg>(64);
    tokio::spawn({
        let host = Arc::clone(&host);
        async move {
            // SSE is read-only: no inbound, no idle eviction (the
            // client-server heartbeat is SSE's own keep-alive).
            let (_, inbound_rx) = tokio::sync::mpsc::channel::<ClientMsg>(1);
            let _ = pump_client(
                &host,
                "sse-readonly",
                after,
                false,
                std::time::Duration::MAX,
                inbound_rx,
                outbound_tx,
            )
            .await;
        }
    });
    let stream = futures_util::stream::unfold(outbound_rx, |mut rx| async move {
        match rx.recv().await {
            Some(msg) => {
                let body = encode_server(&msg).unwrap_or_else(|_| "{}".into());
                Some((Ok::<_, std::convert::Infallible>(sse_frame(&body)), rx))
            }
            None => None,
        }
    });
    axum::response::sse::Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::default())
        .into_response()
}

fn sse_frame(body: &str) -> axum::response::sse::Event {
    axum::response::sse::Event::default().data(body)
}

pub(crate) async fn post_command(
    State(state): State<Arc<DaemonState>>,
    Path(sid): Path<String>,
    Json(msg): Json<ClientMsg>,
) -> Response {
    let host = state.session(&sid);
    match msg {
        ClientMsg::Command { command } => {
            let host_command = match command {
                SessionCommand::Submit { text } => HostCommand::Submit(text),
                SessionCommand::Steer { text } => HostCommand::Steer(text),
                SessionCommand::Cancel => HostCommand::Cancel,
            };
            host.handle_command(host_command).await;
            (StatusCode::ACCEPTED, "accepted").into_response()
        }
        ClientMsg::PermissionAnswer { ask_id, answer } => {
            match host.mediator().answer(ask_id, "http", answer) {
                Ok(_) => (StatusCode::ACCEPTED, "recorded").into_response(),
                Err(_) => (StatusCode::NOT_FOUND, format!("unknown ask {ask_id}")).into_response(),
            }
        }
        ClientMsg::PermissionCancel { ask_id } => {
            match host.mediator().cancel_vote(ask_id, "http") {
                Ok(_) => (StatusCode::ACCEPTED, "recorded").into_response(),
                Err(_) => (StatusCode::NOT_FOUND, "unknown ask").into_response(),
            }
        }
        ClientMsg::Hello { .. } | ClientMsg::Ping => {
            (StatusCode::BAD_REQUEST, "not a command").into_response()
        }
    }
}

pub(crate) async fn list_sessions(State(state): State<Arc<DaemonState>>) -> Response {
    Json(serde_json::json!({ "sessions": state.session_ids() })).into_response()
}

pub(crate) async fn reload_endpoint(State(state): State<Arc<DaemonState>>) -> Response {
    let Ok(exe) = std::env::current_exe() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "no exe").into_response();
    };
    let listener = match state.listener.lock() {
        Ok(guard) => guard.as_ref().and_then(|l| l.try_clone().ok()),
        Err(poisoned) => poisoned
            .into_inner()
            .as_ref()
            .and_then(|l| l.try_clone().ok()),
    };
    let Some(listener) = listener else {
        return (StatusCode::CONFLICT, "no listener registered").into_response();
    };
    match reload::handoff_to_successor(&listener, &exe, &[]) {
        Ok(()) => {
            let _ = state.drain.send(true);
            (
                StatusCode::OK,
                "successor holds the socket; draining".to_owned(),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("reload failed: {e}"),
        )
            .into_response(),
    }
}
