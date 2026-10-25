//! M32 hermetic acceptance: a daemon whose session factory is composed
//! from a cassette-backed provider config completes submit->response
//! over the REAL WebSocket surface — the response EVENT carries the
//! cassette content, and the provider request carries the submitted
//! prompt bytes. Zero network, zero real keys.

mod common;

use std::time::Duration;

use austringer_daemon::provider::{ProviderFamily, ProviderSelection};
use common::cassette::{Cassette, CassetteTransport, anthropic_text_cassette};
use common::{spawn_daemon_with_factory, ws_hello};
use futures_util::StreamExt;

const ANSWER: &str = "m32 cassette answer: the projection is sufficient";
const PROMPT: &str = "M32: is the text-only projection sufficient for QA?";

#[tokio::test]
async fn cassette_configured_daemon_answers_submit_over_ws() {
    let selection = ProviderSelection {
        family: ProviderFamily::Anthropic,
        model: "claude-m32-cassette".to_owned(),
        api_key_env: Some("austringer_M32_E2E_KEY".to_owned()),
        base_url: None,
        inference_timeout: Duration::from_secs(30),
    };
    let transport = CassetteTransport::new(vec![Cassette::Sse(anthropic_text_cassette(
        "claude-m32-cassette",
        ANSWER,
    ))]);
    let resolved = match selection.resolve_with(|_| Some("m32-fake-key".to_owned())) {
        Ok(resolved) => resolved,
        Err(failure) => panic!("resolve failed: {failure}"),
    };
    let factory = resolved.compose_factory(transport.clone());
    let daemon = match spawn_daemon_with_factory(|c| c, factory).await {
        Ok(daemon) => daemon,
        Err(failure) => panic!("spawn failed: {failure}"),
    };

    let url = format!("ws://{}/s/m32/attach?token={}", daemon.base, daemon.token);
    let mut socket = match ws_hello(&url, &daemon.token, "m32-client", None).await {
        Ok(socket) => socket,
        Err(failure) => panic!("attach failed: {failure}"),
    };

    let body = serde_json::json!({
        "type": "command", "command": {"type": "submit", "text": PROMPT},
    });
    let client = reqwest::Client::new();
    let posted = client
        .post(format!("http://{}/s/m32/cmd", daemon.base))
        .bearer_auth(&daemon.token)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await;
    assert!(
        posted.is_ok_and(|r| r.status().is_success()),
        "submit must be accepted"
    );

    let mut saw_answer = false;
    let mut outcome = String::new();
    let deadline = Duration::from_secs(30);
    while tokio::time::Instant::now().elapsed() < deadline {
        let Ok(Some(Ok(msg))) = tokio::time::timeout(Duration::from_secs(10), socket.next()).await
        else {
            break;
        };
        let tokio_tungstenite::tungstenite::Message::Text(text) = msg else {
            continue;
        };
        let text = text.to_string();
        if text.contains(ANSWER) {
            saw_answer = true;
        }
        if text.contains("turn_finished") {
            outcome = text;
            break;
        }
    }
    assert!(
        saw_answer,
        "the RESPONSE EVENT must carry the cassette text"
    );
    assert!(
        outcome.contains("turn_complete"),
        "the turn must complete, not fail: {outcome}"
    );

    let requests = transport.requests();
    assert_eq!(requests.len(), 1, "exactly one provider request");
    let prompt_body = String::from_utf8_lossy(&requests[0].body).to_string();
    assert!(
        prompt_body.contains(PROMPT),
        "the submitted prompt bytes must reach the provider: {prompt_body}"
    );
    assert_eq!(
        requests[0].url, "https://api.anthropic.com/v1/messages",
        "the configured family composes its documented endpoint"
    );
}
