//! M32: config-driven provider composition at the seam — selection
//! parsing, key resolution, adapter composition, the unconfigured path,
//! and the adversarial tiers — all against in-memory cassette
//! transports (zero network, zero real keys).

mod common;

use std::time::{Duration, Instant};

use common::cassette::{Cassette, CassetteTransport, anthropic_text_cassette};
use harness_core::agent_loop::{CancelToken, InferenceRequest};
use harness_core::event::ContentBlock;
use harness_daemon::provider::{ProviderFamily, ProviderSelection, compose_factory};
use harness_daemon::usage::InferenceFactory;

type Factory = InferenceFactory;

const KEY_ENV: &str = "HARNESS_M32_E2E_KEY";

fn anthropic_selection() -> ProviderSelection {
    ProviderSelection {
        family: ProviderFamily::Anthropic,
        model: "claude-m32-cassette".to_owned(),
        api_key_env: Some(KEY_ENV.to_owned()),
        base_url: None,
        inference_timeout: Duration::from_secs(30),
    }
}

async fn infer_once(
    factory: &Factory,
) -> Result<harness_core::agent_loop::AssistantTurn, harness_core::agent_loop::InferenceError>
{
    let mut inference = factory().inference;
    let request = InferenceRequest {
        events: &[],
        focus_block: "M32-FOCUS question about the projection",
    };
    let cancel = CancelToken::new();
    inference.infer(&request, &cancel).await
}

fn turn_text(turn: &harness_core::agent_loop::AssistantTurn) -> String {
    turn.blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Image(_) | ContentBlock::Media(_) => None,
        })
        .collect()
}

#[tokio::test]
async fn unconfigured_factory_names_config_keys_not_the_stale_todo() {
    let factory = match compose_factory(None, CassetteTransport::new(vec![])) {
        Ok(factory) => factory,
        Err(failure) => panic!("unconfigured compose failed: {failure}"),
    };
    let failure = match infer_once(&factory).await {
        Ok(turn) => panic!("unconfigured daemon must not infer: {turn:?}"),
        Err(failure) => failure.to_string(),
    };
    assert!(
        failure.contains("no provider configured"),
        "typed unconfigured error must survive: {failure}"
    );
    assert!(
        failure.contains("HARNESS_PROVIDER") && failure.contains("HARNESS_MODEL"),
        "error must name the config keys to set: {failure}"
    );
    assert!(
        !failure.contains("todo 40"),
        "the stale 'lands with todo 40' reference must be gone: {failure}"
    );
}

#[tokio::test]
async fn unknown_family_is_typed_and_actionable() {
    let text = match ProviderSelection::from_parts("anthropik", Some("m"), None, None, None) {
        Err(failure) => failure.to_string(),
        Ok(selection) => panic!("unknown family must be rejected: {selection:?}"),
    };
    assert!(text.contains("anthropik"), "echo the bad value: {text}");
    assert!(
        text.contains("HARNESS_PROVIDER"),
        "name the config key: {text}"
    );
    for known in ["anthropic", "openai", "google", "xai", "compat"] {
        assert!(text.contains(known), "list {known} as an option: {text}");
    }
}

#[tokio::test]
async fn blank_model_is_typed_and_actionable() {
    let text = match ProviderSelection::from_parts("openai", Some("   "), None, None, None) {
        Err(failure) => failure.to_string(),
        Ok(selection) => panic!("blank model must be rejected: {selection:?}"),
    };
    assert!(text.contains("HARNESS_MODEL"), "name the key: {text}");
}

#[tokio::test]
async fn absent_model_is_typed_and_actionable() {
    let text = match ProviderSelection::from_parts("openai", None, None, None, None) {
        Err(failure) => failure.to_string(),
        Ok(selection) => panic!("absent model must be rejected: {selection:?}"),
    };
    assert!(text.contains("HARNESS_MODEL"), "name the key: {text}");
}

#[tokio::test]
async fn compat_without_base_url_is_typed_and_actionable() {
    let text = match ProviderSelection::from_parts("compat", Some("m"), None, None, None) {
        Err(failure) => failure.to_string(),
        Ok(selection) => panic!("compat without base url must be rejected: {selection:?}"),
    };
    assert!(
        text.contains("HARNESS_PROVIDER_BASE_URL"),
        "name the key: {text}"
    );
}

#[tokio::test]
async fn keyed_family_without_key_env_name_is_typed_and_actionable() {
    let text =
        match ProviderSelection::from_parts("google", Some("gemini-m32"), None, None, None) {
            Err(failure) => failure.to_string(),
            Ok(selection) => panic!("keyed family without key env must be rejected: {selection:?}"),
        };
    assert!(
        text.contains("HARNESS_PROVIDER_KEY_ENV"),
        "name the key: {text}"
    );
}

#[tokio::test]
async fn missing_key_env_value_fails_typed_naming_the_variable() {
    // HARNESS_M32_ABSENT_KEY is never set anywhere in the workspace.
    let selection = ProviderSelection {
        api_key_env: Some("HARNESS_M32_ABSENT_KEY".to_owned()),
        ..anthropic_selection()
    };
    let transport = CassetteTransport::new(vec![]);
    let failure = match compose_factory(Some(&selection), transport) {
        Ok(factory) => {
            let _engine = factory();
            panic!("missing key value must fail at compose")
        }
        Err(failure) => failure.to_string(),
    };
    assert!(
        failure.contains("HARNESS_M32_ABSENT_KEY"),
        "name the unset variable: {failure}"
    );
}

#[tokio::test]
async fn anthropic_cassette_round_trip_carries_prompt_and_key() {
    let answer = "m32 cassette answer";
    let transport = CassetteTransport::new(vec![Cassette::Sse(
        anthropic_text_cassette("claude-m32-cassette", answer),
    )]);
    let resolved = match anthropic_selection().resolve_with(|name| {
        (name == KEY_ENV).then(|| "m32-fake-key".to_owned())
    }) {
        Ok(resolved) => resolved,
        Err(failure) => panic!("resolve failed: {failure}"),
    };
    let factory = resolved.compose_factory(transport.clone());
    let turn = match infer_once(&factory).await {
        Ok(turn) => turn,
        Err(failure) => panic!("cassette inference failed: {failure}"),
    };
    assert_eq!(turn_text(&turn), answer, "cassette text must come back");
    let requests = transport.requests();
    assert_eq!(requests.len(), 1, "exactly one provider request");
    assert_eq!(
        requests[0].url, "https://api.anthropic.com/v1/messages",
        "anthropic family composes the documented endpoint"
    );
    assert_eq!(
        requests[0].headers.get("x-api-key").map(String::as_str),
        Some("m32-fake-key"),
        "the resolved key must ride the anthropic header"
    );
    let body = String::from_utf8_lossy(&requests[0].body).to_string();
    assert!(
        body.contains("M32-FOCUS question about the projection"),
        "prompt bytes must carry the focus block: {body}"
    );
}

#[tokio::test]
async fn every_documented_family_composes() {
    for (family, base_url) in [
        (ProviderFamily::Anthropic, None),
        (ProviderFamily::OpenAi, None),
        (ProviderFamily::Google, None),
        (ProviderFamily::Xai, None),
        (ProviderFamily::Compat, Some("http://127.0.0.1:11434/v1")),
    ] {
        let selection = ProviderSelection {
            family,
            api_key_env: match family {
                ProviderFamily::Compat => None,
                _ => Some(KEY_ENV.to_owned()),
            },
            base_url: base_url.map(str::to_owned),
            ..anthropic_selection()
        };
        let resolved = match selection.resolve_with(|_| Some("m32-fake-key".to_owned())) {
            Ok(resolved) => resolved,
            Err(failure) => panic!("{family:?} resolve failed: {failure}"),
        };
        let factory = resolved.compose_factory(CassetteTransport::new(vec![]));
        let _engine = factory();
    }
}

#[tokio::test]
async fn hanging_provider_surfaces_typed_timeout_without_hanging() {
    let selection = ProviderSelection {
        inference_timeout: Duration::from_millis(250),
        ..anthropic_selection()
    };
    let transport = CassetteTransport::new(vec![Cassette::Hang]);
    let resolved = match selection.resolve_with(|_| Some("m32-fake-key".to_owned())) {
        Ok(resolved) => resolved,
        Err(failure) => panic!("resolve failed: {failure}"),
    };
    let factory = resolved.compose_factory(transport.clone());
    let started = Instant::now();
    let guarded =
        match tokio::time::timeout(Duration::from_secs(5), infer_once(&factory)).await {
            Ok(outcome) => outcome,
            Err(_elapsed) => panic!("infer hung past the 5s outer guard"),
        };
    let failure = match guarded {
        Ok(turn) => panic!("a hanging provider must not succeed: {turn:?}"),
        Err(failure) => failure.to_string(),
    };
    assert!(
        failure.to_lowercase().contains("timeout"),
        "timeout must surface typed: {failure}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the deadline must bound the hang"
    );
}
