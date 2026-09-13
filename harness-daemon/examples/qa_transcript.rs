//! Manual-QA transcript driver (task 35 evidence): spawns the REAL
//! daemon binary, then exercises the public HTTP/WS surface with a
//! real reqwest client and real WebSocket clients, printing a pasteable
//! transcript. Run:
//! `cargo run -p harness-daemon --example qa_transcript`.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};

struct Daemon {
    child: Child,
    port: u16,
    token: String,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

trait NonEmpty {
    fn into_non_empty(self) -> Option<String>;
}

impl NonEmpty for String {
    fn into_non_empty(self) -> Option<String> {
        if self.is_empty() { None } else { Some(self) }
    }
}

fn spawn_daemon() -> Result<Daemon, String> {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().and_then(|d| d.parent()).map(|d| d.to_path_buf()))
        .map(|dir| {
            #[cfg(windows)]
            let name = "harness-daemon.exe";
            #[cfg(not(windows))]
            let name = "harness-daemon";
            dir.join(name)
        })
        .ok_or_else(|| "no exe dir".to_owned())?;
    let data_dir = std::env::temp_dir().join(format!("harness-qa-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data_dir);
    let mut child = Command::new(exe)
        .args([
            "--data-dir",
            data_dir.to_str().unwrap_or("."),
            "--port",
            "18908",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    let mut banner = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if let Some(out) = child.stdout.as_mut() {
            let mut buf = [0u8; 512];
            if let Ok(n) = out.read(&mut buf) {
                banner.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
        }
        if banner.contains("token: ") {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let token = banner
        .split("token: ")
        .nth(1)
        .and_then(|rest| {
            rest.chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .collect::<String>()
                .into_non_empty()
        })
        .ok_or_else(|| format!("no token in banner: {banner}"))?;
    Ok(Daemon {
        child,
        port: 18908,
        token,
    })
}

async fn get(url: &str, token: Option<&str>, host: Option<&str>) -> (u16, String) {
    let client = reqwest::Client::new();
    let mut request = client.get(url);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(host) = host {
        request = request.header("host", host);
    }
    match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let headers = format!(
                "content-type={} cache-control={}",
                response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-"),
                response
                    .headers()
                    .get(reqwest::header::CACHE_CONTROL)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-"),
            );
            let body = response.text().await.unwrap_or_default();
            (status, format!("{headers} | {body}"))
        }
        Err(_) => (0, String::new()),
    }
}

async fn ws_collect(
    url: &str,
    token: &str,
    client_id: &str,
    submit: Option<&str>,
    window: Duration,
) -> Vec<String> {
    let (mut socket, _) = match tokio_tungstenite::connect_async(url).await {
        Ok(pair) => pair,
        Err(_) => return vec!["CONNECT-FAILED".into()],
    };
    let hello = serde_json::json!({
        "type": "hello", "version": 1, "token": token, "client_id": client_id,
    });
    let _ = socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            hello.to_string().into(),
        ))
        .await;
    if let Some(text) = submit {
        let cmd = serde_json::json!({
            "type": "command", "command": {"type": "submit", "text": text},
        });
        let _ = socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                cmd.to_string().into(),
            ))
            .await;
    }
    let mut frames = Vec::new();
    let deadline = tokio::time::Instant::now() + window;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), socket.next()).await {
            Ok(Some(Ok(msg))) => {
                if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                    frames.push(text.to_string());
                }
            }
            _ => continue,
        }
    }
    frames
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let daemon = spawn_daemon()?;
    let base = format!("127.0.0.1:{}", daemon.port);
    let token = daemon.token.clone();
    println!("=== DAEMON BANNER (real binary, loopback bind, generated token) ===");
    println!("listening on http://{base} token={token}");

    println!("\n=== [HAPPY] GET / -> SPA (public; content-type + cache headers) ===");
    let (status, meta) = get(&format!("http://{base}/"), None, None).await;
    println!("HTTP {status} | {meta}");

    println!("\n=== [HAPPY] GET /s/qa-session -> URL-addressable deep link ===");
    let (status, meta) = get(&format!("http://{base}/s/qa-session"), None, None).await;
    println!(
        "HTTP {status} | {}",
        meta.chars().take(80).collect::<String>()
    );

    println!("\n=== [FAILURE] no token ===");
    let (status, body) = get(&format!("http://{base}/sessions"), None, None).await;
    println!("HTTP {status} | {body}");

    println!("\n=== [FAILURE] wrong token ===");
    let (status, body) = get(
        &format!("http://{base}/sessions"),
        Some("wrong-token"),
        None,
    )
    .await;
    println!("HTTP {status} | {body}");

    println!("\n=== [FAILURE] spoofed Host + VALID token (CVE-2026-82533 class) ===");
    let (status, body) = get(
        &format!("http://{base}/sessions"),
        Some(&token),
        Some("evil.example.com"),
    )
    .await;
    println!("HTTP {status} | {body}");

    println!("\n=== [FAILURE] spoofed Host + NO token ===");
    let (status, body) = get(
        &format!("http://{base}/sessions"),
        None,
        Some("evil.example.com"),
    )
    .await;
    println!("HTTP {status} | {body}");

    println!("\n=== [HAPPY] two simultaneous WS clients (submit + typed failure turn) ===");
    let url = format!("ws://{base}/s/qa-session/attach?token={token}");
    let url_a = url.clone();
    let url_b = url.clone();
    let token_a = token.clone();
    let token_b = token.clone();
    let client_a = tokio::spawn(async move {
        ws_collect(
            &url_a,
            &token_a,
            "qa-tui",
            Some("hello qa"),
            Duration::from_secs(2),
        )
        .await
    });
    let client_b = tokio::spawn(async move {
        ws_collect(&url_b, &token_b, "qa-web", None, Duration::from_secs(2)).await
    });
    let frames_a = client_a.await.unwrap_or_default();
    let frames_b = client_b.await.unwrap_or_default();
    println!("client A frames ({}):", frames_a.len());
    for frame in &frames_a {
        println!("  A| {frame}");
    }
    println!("client B frames ({}):", frames_b.len());
    for frame in &frames_b {
        println!("  B| {frame}");
    }
    let identical = frames_a == frames_b;
    println!("BYTE-IDENTICAL STREAMS: {identical}");

    println!("\n=== [HAPPY] SSE endpoint speaks the same typed protocol ===");
    let client = reqwest::Client::new();
    if let Ok(response) = client
        .get(format!("http://{base}/s/qa-session/events"))
        .bearer_auth(&token)
        .timeout(Duration::from_secs(2))
        .send()
        .await
    {
        println!(
            "HTTP {} content-type={}",
            response.status().as_u16(),
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-")
        );
        if let Ok(bytes) = response.bytes().await {
            let text = String::from_utf8_lossy(&bytes);
            for line in text.lines().take(3) {
                println!("  SSE| {line}");
            }
        }
    }
    drop(daemon);
    println!("\nQA transcript complete.");
    Ok(())
}
