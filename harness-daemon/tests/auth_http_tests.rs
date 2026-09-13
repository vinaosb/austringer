//! Hardened-auth negative regression tests (CVE-2026-82533 class):
//! loopback bind + token required + spoofed Host refused. Plus the
//! deep-link SPA route.

mod common;

use common::spawn_daemon;

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
            let body = response.text().await.unwrap_or_default();
            (status, body)
        }
        Err(_) => (0, String::new()),
    }
}

#[tokio::test]
async fn missing_token_is_refused() {
    let daemon = spawn_daemon(|c| c).await.ok();
    assert!(daemon.is_some(), "spawn failed");
    let Some(daemon) = daemon else { return };
    let (status, _) = get(&format!("http://{}/sessions", daemon.base), None, None).await;
    assert_eq!(status, 401, "no token must be 401");
}

#[tokio::test]
async fn token_mismatch_is_refused() {
    let daemon = spawn_daemon(|c| c).await.ok();
    assert!(daemon.is_some(), "spawn failed");
    let Some(daemon) = daemon else { return };
    let (status, _) = get(
        &format!("http://{}/sessions", daemon.base),
        Some("wrong-token"),
        None,
    )
    .await;
    assert_eq!(status, 401, "wrong token must be 401");
}

#[tokio::test]
async fn spoofed_host_is_refused_with_or_without_token() {
    // The CVE-2026-82533-class negative: a browser DNS-rebinding probe
    // sends an attacker-chosen Host at the loopback listener. The Host
    // header is never an auth factor and non-loopback Hosts are
    // refused outright.
    let daemon = spawn_daemon(|c| c).await.ok();
    assert!(daemon.is_some(), "spawn failed");
    let Some(daemon) = daemon else { return };
    let (status_no_token, _) = get(
        &format!("http://{}/sessions", daemon.base),
        None,
        Some("evil.example.com"),
    )
    .await;
    assert_eq!(status_no_token, 403, "spoofed host, no token");
    let (status_with_token, _) = get(
        &format!("http://{}/sessions", daemon.base),
        Some(&daemon.token),
        Some("evil.example.com"),
    )
    .await;
    assert_eq!(status_with_token, 403, "spoofed host, valid token");
    // Control: loopback host names pass the Host layer.
    let (ok_status, _) = get(
        &format!("http://{}/sessions", daemon.base),
        Some(&daemon.token),
        Some("127.0.0.1"),
    )
    .await;
    assert_eq!(ok_status, 200);
}

#[tokio::test]
async fn valid_token_serves_sessions_json() {
    let daemon = spawn_daemon(|c| c).await.ok();
    assert!(daemon.is_some(), "spawn failed");
    let Some(daemon) = daemon else { return };
    let (status, body) = get(
        &format!("http://{}/sessions", daemon.base),
        Some(&daemon.token),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert!(body.contains("sessions"), "{body}");
}

#[tokio::test]
async fn root_serves_web_assets_publicly() {
    let daemon = spawn_daemon(|c| c).await.ok();
    assert!(daemon.is_some(), "spawn failed");
    let Some(daemon) = daemon else { return };
    let (status, body) = get(&format!("http://{}/", daemon.base), None, None).await;
    assert_eq!(status, 200);
    assert!(
        body.contains("harness web"),
        "placeholder SPA expected: {body}"
    );
}

#[tokio::test]
async fn session_deep_link_serves_the_spa_shell() {
    // URL-addressable sessions (Amp pattern): the stable session id is
    // the route; any client can open the deep link and attach.
    let daemon = spawn_daemon(|c| c).await.ok();
    assert!(daemon.is_some(), "spawn failed");
    let Some(daemon) = daemon else { return };
    let (status, body) = get(&format!("http://{}/s/my-session", daemon.base), None, None).await;
    assert_eq!(status, 200);
    assert!(body.contains("harness web"));
}
