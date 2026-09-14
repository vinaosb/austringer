//! QA transcript mode: HTTP probes against a real scripted daemon —
//! served content + headers, auth negatives, one full mediated turn
//! on the SSE stream, and the absent-daemon failure.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use harness_daemon::config::DaemonConfig;
use harness_daemon::server::{DaemonState, build_router};

use super::daemon::{factory, spawn_worker};

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default()
}

async fn probe(base: &str, path: &str, token: Option<&str>, host: Option<&str>) -> (u16, String) {
    let mut request = client().get(format!("http://{base}{path}"));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(host) = host {
        request = request.header("host", host);
    }
    match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let header = |name: reqwest::header::HeaderName| {
                response
                    .headers()
                    .get(name)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-")
                    .to_owned()
            };
            let content_type = header(reqwest::header::CONTENT_TYPE);
            let cache = header(reqwest::header::CACHE_CONTROL);
            let body = response.text().await.unwrap_or_default();
            (
                status,
                format!(
                    "{content_type} | cache-control={cache} | {} bytes",
                    body.len()
                ),
            )
        }
        Err(e) => (0, format!("transport failure: {e}")),
    }
}

async fn post(base: &str, body: serde_json::Value) -> Result<reqwest::Response, String> {
    client()
        .post(format!("http://{base}/s/qa-session/cmd"))
        .bearer_auth("e2e-token")
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| format!("post: {e}"))
}

/// Answer the first broadcast ask (first-responder mediation) with
/// `allow_once` through the same HTTP endpoint the SPA uses.
async fn answer_first_ask(state: &Arc<DaemonState>, base: &str) {
    let mut watch = state.session("qa-session").mediator().subscribe();
    for _ in 0..200 {
        if let Ok(harness_daemon::protocol::ServerMsg::PermissionAsk { ask }) = watch.try_recv() {
            println!("ask broadcast: tool={} ask_id={}", ask.tool, ask.ask_id);
            match post(
                base,
                serde_json::json!({
                    "type": "permission_answer", "ask_id": ask.ask_id, "answer": "allow_once"
                }),
            )
            .await
            {
                Ok(answer) => println!("answer allow_once: HTTP {}", answer.status()),
                Err(e) => println!("answer failed: {e}"),
            }
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    println!("ask: NEVER ARRIVED (unexpected)");
}

pub(crate) async fn run() -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let config = DaemonConfig {
        data_dir: dir.path().to_owned(),
        token: Some("e2e-token".into()),
        ..DaemonConfig::default()
    };
    let state = Arc::new(DaemonState::new(config, factory()).map_err(|e| format!("state: {e}"))?);
    spawn_worker(&state).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("bind: {e}"))?;
    let base = listener
        .local_addr()
        .map_err(|e| format!("addr: {e}"))?
        .to_string();
    let app = build_router(Arc::clone(&state));
    tokio::spawn(async move {
        let serve = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        let _ = serve.await;
    });
    println!("=== daemon on http://{base} token=e2e-token (scripted two-step inference) ===");

    println!("\n[HAPPY] GET / (self-contained SPA entry)");
    let (status, meta) = probe(&base, "/", None, None).await;
    println!("HTTP {status} | {meta}");
    println!("\n[HAPPY] GET /s/qa-session (deep link, SPA shell)");
    let (status, meta) = probe(&base, "/s/qa-session", None, None).await;
    println!("HTTP {status} | {meta}");

    println!("\n[FAILURE] auth negatives (content routes are gated)");
    for (label, token, host) in [
        ("no token", None, None),
        ("wrong token", Some("wrong"), None),
        (
            "spoofed Host + valid token",
            Some("e2e-token"),
            Some("evil.example.com"),
        ),
        ("spoofed Host + no token", None, Some("evil.example.com")),
    ] {
        let (status, _) = probe(&base, "/sessions", token, host).await;
        println!("{label}: HTTP {status}");
    }

    println!("\n[HAPPY] submitted turn through the real engine + mediator");
    let submitted = post(
        &base,
        serde_json::json!({
            "type": "command", "command": {"type": "submit", "text": "hello web qa"}
        }),
    )
    .await?;
    println!("submit: HTTP {}", submitted.status());
    answer_first_ask(&state, &base).await;

    let mut stream = client()
        .get(format!("http://{base}/s/qa-session/events"))
        .bearer_auth("e2e-token")
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("sse: {e}"))?
        .bytes_stream();
    let mut seen = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), stream.next()).await {
            Ok(Some(Ok(chunk))) => {
                seen.push_str(&String::from_utf8_lossy(&chunk));
                if seen.contains("turn_finished") {
                    break;
                }
            }
            Ok(Some(Err(_))) | Err(_) | Ok(None) => break,
        }
    }
    for line in seen.lines().filter(|line| {
        line.contains("recorded")
            || line.contains("\"diff\"")
            || line.contains("turn_finished")
            || line.contains("permission_resolved")
    }) {
        println!("SSE| {line}");
    }
    println!(
        "turn produced diff event: {}, finished: {}",
        seen.contains("\"diff\""),
        seen.contains("turn_finished")
    );

    println!("\n[FAILURE] absent daemon (dead port)");
    let (status, meta) = probe("127.0.0.1:9", "/", None, None).await;
    println!("HTTP {status} | {meta}");
    println!("\nQA transcript complete.");
    let _ = dir.keep();
    Ok(())
}
