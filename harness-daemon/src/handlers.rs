//! HTTP handlers (root, SPA deep-link, SSE stream, JSON commands,
//! session list, run history, automations, reload) extracted from
//! `server` for the size budget. Every handler runs BEHIND the auth
//! middleware.

use std::sync::Arc;

use axum::Json;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::attach::HostCommand;
use crate::automations::{AutomationStore, FilePackGate};
use crate::protocol::{ClientMsg, ServerMsg, SessionCommand, encode_server};
use crate::reload;
use crate::run_history::RunHistory;
use crate::scheduler::CronExpr;
use crate::server::DaemonState;
use crate::transport::{PumpParams, pump_client};
use crate::web;

fn typed_error(status: StatusCode, code: &str, extra: serde_json::Value) -> Response {
    let mut error = serde_json::json!({ "code": code });
    if let serde_json::Value::Object(fields) = extra {
        for (key, value) in fields {
            error[key] = value;
        }
    }
    (status, Json(serde_json::json!({ "error": error }))).into_response()
}
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
    let peer_version = query
        .version
        .filter(|version| crate::protocol::version_compatible(*version))
        .unwrap_or(1);
    let (outbound_tx, outbound_rx) = tokio::sync::mpsc::channel::<ServerMsg>(64);
    // SSE is read-only: no inbound, no idle eviction (the
    // client-server heartbeat is SSE's own keep-alive). The empty
    // inbound channel's sender is held by the pump task for the
    // connection's lifetime — a dropped sender reads as client-gone
    // and would cut live subscribers off right after the hello ack.
    let (inbound_tx, inbound_rx) = tokio::sync::mpsc::channel::<ClientMsg>(1);
    tokio::spawn({
        let host = Arc::clone(&host);
        async move {
            let _read_only = inbound_tx;
            let _ = pump_client(
                &host,
                "sse-readonly",
                PumpParams {
                    peer_version,
                    after,
                    enforce_idle: false,
                    idle_timeout: std::time::Duration::MAX,
                },
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
                SessionCommand::SubmitMedia { text, media } => {
                    HostCommand::SubmitMedia { text, media }
                }
                SessionCommand::Steer { text } => HostCommand::Steer(text),
                SessionCommand::Cancel => HostCommand::Cancel,
            };
            match host.handle_command(host_command).await {
                Ok(()) => (StatusCode::ACCEPTED, "accepted").into_response(),
                Err(rejection) => (StatusCode::BAD_REQUEST, rejection.to_string()).into_response(),
            }
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
        // The HTTP command route has no per-connection identity to key
        // a fallback answer on (the known "http" hardcode wart must not
        // be amplified to a new ask type); the mediated answer path
        // lives on the WS/IPC pumps, which own server-assigned ids.
        ClientMsg::FallbackAnswer { .. } => (
            StatusCode::BAD_REQUEST,
            "fallback answers require a connection identity; answer over WS/IPC",
        )
            .into_response(),
        // Plan answers carry the same identity contract, plus the
        // todo-16 receipt: no surface may auto-approve a plan
        // (Antigravity v1.3.2 skip-permissions). Mode flips (PlanMode)
        // are not decisions and stay open on this route.
        ClientMsg::PlanAnswer { .. } => (
            StatusCode::BAD_REQUEST,
            "plan answers require a connection identity; answer over WS/IPC",
        )
            .into_response(),
        ClientMsg::PlanMode { on } => {
            crate::plan::set_plan_mode(&host, on);
            (StatusCode::ACCEPTED, "accepted").into_response()
        }
        ClientMsg::Rewind { steps } => {
            match crate::rewind::run_rewind(std::sync::Arc::clone(&host), steps).await {
                Ok(()) => (StatusCode::OK, "restored").into_response(),
                Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
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

/// `GET /s/{sid}/history` — the session's scheduled-run lifecycle rows
/// (`{data}/runs.jsonl`): a bare JSON array of run records (started +
/// completion rows, each carrying its `verdict` field). A session is
/// known when it is live OR has history rows; anything else is a typed
/// 404, never a 500.
pub(crate) async fn session_history(
    State(state): State<Arc<DaemonState>>,
    Path(sid): Path<String>,
) -> Response {
    let rows = RunHistory::open(&state.config.data_dir)
        .and_then(|history| history.list())
        .unwrap_or_default()
        .into_iter()
        .filter(|row| row.session_id == sid)
        .collect::<Vec<_>>();
    if rows.is_empty() && state.try_session(&sid).is_none() {
        return typed_error(
            StatusCode::NOT_FOUND,
            "unknown_session",
            serde_json::json!({ "session": sid }),
        );
    }
    Json(rows).into_response()
}

/// `GET /automations` — the declarative automation set read from disk
/// (`automations.json` under the data dir, revalidated like every
/// load): name, cron, pack, enabled, next fire (epoch minute, UTC;
/// null while disabled), and the last gate verdict from run history.
pub(crate) async fn list_automations(State(state): State<Arc<DaemonState>>) -> Response {
    let data_dir = &state.config.data_dir;
    let gate = match FilePackGate::open(&data_dir.join("pack-tiers.json")) {
        Ok(gate) => gate,
        Err(e) => {
            return typed_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "pack_tiers_unreadable",
                serde_json::json!({ "detail": e.to_string() }),
            );
        }
    };
    let defs = match AutomationStore::load(data_dir, &gate) {
        Ok(store) => store.defs().to_vec(),
        Err(e) => {
            return typed_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "automations_invalid",
                serde_json::json!({ "detail": e.to_string() }),
            );
        }
    };
    let history = RunHistory::open(data_dir).ok();
    let now_minute = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() / 60)
        .unwrap_or(0);
    let automations = defs
        .iter()
        .map(|def| {
            let next_fire_minute = if def.enabled {
                CronExpr::parse(&def.cron)
                    .ok()
                    .and_then(|cron| cron.next_after(now_minute))
            } else {
                None
            };
            let last_verdict = history
                .as_ref()
                .and_then(|h| h.for_automation(&def.id).ok())
                .and_then(|rows| rows.into_iter().rev().find_map(|row| row.verdict));
            serde_json::json!({
                "id": def.id,
                "name": def.name,
                "cron": def.cron,
                "pack": def.pack,
                "enabled": def.enabled,
                "next_fire_minute": next_fire_minute,
                "last_verdict": last_verdict,
            })
        })
        .collect::<Vec<_>>();
    Json(serde_json::json!({ "automations": automations })).into_response()
}

/// `GET /packs` — every loaded skill pack with its trust tier and
/// status, plus every declared MCP server with its disabled marker
/// (pack-sourced declarations always land disabled; enabling one is a
/// separate explicit consent step, not this surface).
pub(crate) async fn list_packs(State(state): State<Arc<DaemonState>>) -> Response {
    Json(state.skills.packs_json()).into_response()
}

pub(crate) async fn reload_endpoint(State(state): State<Arc<DaemonState>>) -> Response {    let Ok(exe) = std::env::current_exe() else {
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
