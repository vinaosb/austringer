//! Self-test suite for the rule-based fake provider (plan todo 2a, goose
//! `DummyApi` pattern): every scripted failure mode fires with the right typed
//! error, `up_to_n_times` exhausts, matchers route, and a full scripted
//! conversation replays start-to-finish. All hermetic — no network.

use harness_test_support::provider::{FakeApi, ProviderError, ProviderRequest, RequestMatcher};
use harness_test_support::sse::SseScript;

fn req(model: &str, prompt: &str) -> ProviderRequest {
    ProviderRequest::new(model, prompt)
}

#[tokio::test]
async fn no_choices_failure_mode_fires() {
    let api = FakeApi::new();
    api.on(RequestMatcher::Any).fail_no_choices();
    let err = api.complete(&req("m1", "hi")).await.unwrap_err();
    assert!(matches!(err, ProviderError::NoChoices), "got: {err:?}");
}

#[tokio::test]
async fn output_limit_failure_mode_fires() {
    let api = FakeApi::new();
    api.on(RequestMatcher::Any).fail_output_limit();
    let err = api.complete(&req("m1", "hi")).await.unwrap_err();
    assert!(matches!(err, ProviderError::OutputLimit), "got: {err:?}");
}

#[tokio::test]
async fn context_limit_failure_mode_fires() {
    let api = FakeApi::new();
    api.on(RequestMatcher::Any).fail_context_limit();
    let err = api.complete(&req("m1", "hi")).await.unwrap_err();
    assert!(matches!(err, ProviderError::ContextLimit), "got: {err:?}");
}

#[tokio::test]
async fn server_error_failure_mode_fires_as_http_500() {
    let api = FakeApi::new();
    api.on(RequestMatcher::Any).fail_server_error();
    let err = api.complete(&req("m1", "hi")).await.unwrap_err();
    match err {
        ProviderError::Http { status, code, .. } => {
            assert_eq!(status, 500);
            assert_eq!(code, "server_error");
        }
        other => panic!("expected Http, got: {other:?}"),
    }
}

#[tokio::test]
async fn reply_then_server_error_succeeds_once_then_fails() {
    let api = FakeApi::new();
    api.on(RequestMatcher::Any)
        .reply_then_server_error(SseScript::single_text("ok"));

    let first = api.complete(&req("m1", "hi")).await;
    assert_eq!(first.expect("scripted reply must stream"), "ok");

    let second = api.complete(&req("m1", "hi again")).await;
    match second.unwrap_err() {
        ProviderError::Http { status, .. } => assert_eq!(status, 500),
        other => panic!("expected Http 500 on second use, got: {other:?}"),
    }
}

#[tokio::test]
async fn up_to_n_times_exhaustion_returns_unexpected_request() {
    let api = FakeApi::new();
    api.on(RequestMatcher::Any)
        .reply_stream(SseScript::single_text("once"))
        .up_to_n_times(1);

    let first = api.complete(&req("m1", "hi")).await;
    assert_eq!(first.expect("first use is within the cap"), "once");

    let err = api.complete(&req("m1", "hi")).await.unwrap_err();
    match err {
        ProviderError::UnexpectedRequest { model, prompt } => {
            assert_eq!(model, "m1");
            assert_eq!(prompt, "hi");
        }
        other => panic!("expected UnexpectedRequest, got: {other:?}"),
    }
}

#[tokio::test]
async fn rules_route_by_prompt_matcher_and_model() {
    let api = FakeApi::new();
    api.on(RequestMatcher::prompt_contains("capital"))
        .reply_stream(SseScript::single_text("Paris"));
    api.on(RequestMatcher::model("tiny-model"))
        .reply_stream(SseScript::single_text("from tiny"));

    let a = api
        .complete(&req("big-model", "What is the capital of France?"))
        .await
        .expect("prompt rule matches");
    assert_eq!(a, "Paris");

    let b = api
        .complete(&req("tiny-model", "anything"))
        .await
        .expect("model rule matches");
    assert_eq!(b, "from tiny");

    let err = api
        .complete(&req("big-model", "unmatched prompt"))
        .await
        .unwrap_err();
    assert!(matches!(err, ProviderError::UnexpectedRequest { .. }));
}

#[tokio::test]
async fn from_str_matcher_is_prompt_contains() {
    let api = FakeApi::new();
    api.on(RequestMatcher::from("hello"))
        .reply_stream(SseScript::single_text("hi"));
    let text = api
        .complete(&req("m", "well hello there"))
        .await
        .expect("From<&str> matcher matches by substring");
    assert_eq!(text, "hi");
}

#[tokio::test]
async fn full_scripted_conversation_replays_start_to_finish() {
    // QA happy path: a 3-turn conversation where every turn is served by its
    // own rule, replayed start-to-finish through the fake provider.
    let api = FakeApi::new();
    api.on(RequestMatcher::prompt_contains("capital of France"))
        .reply_stream(SseScript::single_text("The capital of France is Paris."));
    api.on(RequestMatcher::prompt_contains("2+2"))
        .reply_stream(SseScript::single_text("2+2 equals 4."));
    api.on(RequestMatcher::prompt_contains("thank you"))
        .reply_stream(SseScript::single_text("You're welcome!"));

    let turns = [
        (
            "user",
            "What is the capital of France?",
            "The capital of France is Paris.",
        ),
        ("user", "And 2+2?", "2+2 equals 4."),
        ("user", "Great, thank you!", "You're welcome!"),
    ];
    for (speaker, prompt, expected) in turns {
        let answer = api
            .complete(&req("test-model", prompt))
            .await
            .unwrap_or_else(|e| panic!("{speaker} turn failed: {e}"));
        assert_eq!(answer, expected);
    }
}
