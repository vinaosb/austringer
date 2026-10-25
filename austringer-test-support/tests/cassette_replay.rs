//! Self-test suite for cassettes (plan todo 2e): strict replay that dies on
//! unused interactions, clear corruption aborts, redaction, tmp-dir isolation,
//! and the record→replay conversation roundtrip. Every test owns a fresh
//! [`TestDir`] (nextest runs tests as separate processes; dirs also key on
//! pid+counter, so no state ever leaks between tests).

use austringer_test_support::cassette::{
    Cassette, CassetteError, CassetteRecorder, CassetteReplayer, RedactionFilter,
};
use austringer_test_support::provider::{ProviderRequest, ScriptedReply};
use austringer_test_support::sse::{SseByteStream, SseScript};
use austringer_test_support::util::TestDir;

fn convo_turns() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "test-model",
            "What is the capital of France?",
            "The capital of France is Paris.",
        ),
        ("test-model", "And 2+2?", "2+2 equals 4."),
        ("test-model", "Great, thank you!", "You're welcome!"),
    ]
}

fn record_conversation(path: &std::path::Path) {
    let mut recorder = CassetteRecorder::new("convo");
    for (model, prompt, answer) in convo_turns() {
        recorder.record(
            &ProviderRequest::new(model, prompt),
            &ScriptedReply::Stream(SseScript::single_text(answer)),
        );
    }
    recorder.finish().save(path).expect("save must succeed");
}

async fn reply_text(reply: ScriptedReply) -> String {
    match reply {
        ScriptedReply::Stream(script) => SseByteStream::from_script(&script).read_all().await.0,
        other => panic!("expected scripted stream, got: {other:?}"),
    }
}

#[tokio::test]
async fn strict_replay_dies_on_unused_interactions() {
    let dir = TestDir::new("unused").unwrap();
    let path = dir.path().join("cassette.json");
    record_conversation(&path);

    let cassette = Cassette::load(&path).unwrap();
    let mut replayer = CassetteReplayer::new(cassette);
    // Consume only the FIRST interaction of three.
    let reply = replayer
        .next_interaction(&ProviderRequest::new(
            "test-model",
            "What is the capital of France?",
        ))
        .expect("in-order first turn replays");
    let _ = reply;
    let err = replayer
        .finish()
        .expect_err("2 unused interactions must be fatal");
    assert!(matches!(
        err,
        CassetteError::UnusedInteractions { count: 2, .. }
    ));
    assert!(err.to_string().contains("unused interaction"));
}

#[tokio::test]
async fn corrupted_cassette_aborts_with_clear_error() {
    let dir = TestDir::new("corrupt").unwrap();
    let path = dir.path().join("corrupt.json");
    std::fs::write(&path, b"{ this is not valid json !!!").unwrap();

    let err = Cassette::load(&path).expect_err("garbage must not load");
    let msg = err.to_string();
    match &err {
        CassetteError::Corrupt { path: p, .. } => assert_eq!(p, &path),
        other => panic!("expected Corrupt, got: {other:?}"),
    }
    assert!(msg.contains("corrupted cassette"), "message was: {msg}");
    assert!(msg.contains("corrupt.json"), "message was: {msg}");
    if std::env::var_os("AUSTRINGER_TS_DEMO").is_some() {
        eprintln!("[demo] corrupted cassette error: {msg}");
    }
}

#[tokio::test]
async fn redaction_masks_secret_headers_and_values() {
    let dir = TestDir::new("redact").unwrap();
    let path = dir.path().join("redacted.json");

    let filter = RedactionFilter::new().mask_value("sk-super-secret-123");
    let mut recorder = CassetteRecorder::with_filter("secrets", filter);
    recorder.record(
        &ProviderRequest::new("m", "use key sk-super-secret-123 please")
            .with_header("authorization", "Bearer sk-super-secret-123")
            .with_header("x-request-id", "trace-7"),
        &ScriptedReply::Stream(SseScript::single_text("done")),
    );
    recorder.finish().save(&path).unwrap();

    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(
        !raw.contains("sk-super-secret-123"),
        "secret leaked into cassette"
    );
    assert!(raw.contains("[REDACTED]"));
    let cassette = Cassette::load(&path).unwrap();
    let headers = &cassette.interactions[0].request.headers;
    assert_eq!(
        headers.get("authorization").map(String::as_str),
        Some("[REDACTED]")
    );
    assert_eq!(
        headers.get("x-request-id").map(String::as_str),
        Some("trace-7")
    );
}

#[tokio::test]
async fn replay_mismatch_aborts_with_clear_error() {
    let dir = TestDir::new("mismatch").unwrap();
    let path = dir.path().join("ordered.json");
    record_conversation(&path);

    let mut replayer = CassetteReplayer::new(Cassette::load(&path).unwrap());
    let err = replayer
        .next_interaction(&ProviderRequest::new("test-model", "out of order turn"))
        .expect_err("wrong first turn must not replay");
    match &err {
        CassetteError::Mismatch {
            index, expected, ..
        } => {
            assert_eq!(*index, 0);
            assert_eq!(expected.prompt, "What is the capital of France?");
        }
        other => panic!("expected Mismatch, got: {other:?}"),
    }
    assert!(err.to_string().contains("replay mismatch"));
}

#[tokio::test]
async fn replay_exhausted_when_cassette_runs_dry() {
    let dir = TestDir::new("exhausted").unwrap();
    let path = dir.path().join("small.json");
    record_conversation(&path);

    let mut replayer = CassetteReplayer::new(Cassette::load(&path).unwrap());
    let turns = convo_turns();
    for (model, prompt, _) in &turns {
        let reply = replayer
            .next_interaction(&ProviderRequest::new(model, prompt))
            .expect("in-order turns replay");
        let _ = reply;
    }
    assert!(replayer.is_exhausted());
    let err = replayer
        .next_interaction(&ProviderRequest::new("test-model", "one turn too many"))
        .expect_err("dry cassette must fail");
    assert!(matches!(err, CassetteError::Exhausted { .. }));
}

#[tokio::test]
async fn cassette_dirs_are_isolated_per_test_dir_instance() {
    let a = TestDir::new("iso").unwrap();
    let b = TestDir::new("iso").unwrap();
    assert_ne!(
        a.path(),
        b.path(),
        "same tag must still yield distinct dirs"
    );

    let pa = a.path().join("c.json");
    let pb = b.path().join("c.json");
    record_conversation(&pa);
    record_conversation(&pb);
    // Both coexist and load independently — no cross-test clobbering.
    assert_eq!(Cassette::load(&pa).unwrap().interactions.len(), 3);
    assert_eq!(Cassette::load(&pb).unwrap().interactions.len(), 3);
}

#[tokio::test]
async fn record_then_replay_full_conversation_roundtrip() {
    // QA happy path: the scripted conversation replays start-to-finish from a
    // saved cassette, byte-for-byte identical answers, strict finish green.
    let dir = TestDir::new("roundtrip").unwrap();
    let path = dir.path().join("convo.json");

    // 1. Record (live answers come from the fake provider scripts).
    let api = austringer_test_support::provider::FakeApi::new();
    let mut recorder = CassetteRecorder::new("convo");
    for (model, prompt, answer) in convo_turns() {
        let script = SseScript::single_text(answer);
        api.on(austringer_test_support::provider::RequestMatcher::prompt_contains(prompt))
            .reply_stream(script.clone());
        let live = api
            .complete(&ProviderRequest::new(model, prompt))
            .await
            .unwrap();
        assert_eq!(live, answer, "live pass must match the script");
        recorder.record(
            &ProviderRequest::new(model, prompt),
            &ScriptedReply::Stream(script),
        );
    }
    recorder.finish().save(&path).unwrap();

    // 2. Replay strictly.
    let mut replayer = CassetteReplayer::new(Cassette::load(&path).unwrap());
    for (model, prompt, answer) in convo_turns() {
        let reply = replayer
            .next_interaction(&ProviderRequest::new(model, prompt))
            .unwrap_or_else(|e| panic!("replay of {prompt:?} failed: {e}"));
        let text = reply_text(reply).await;
        assert_eq!(text, answer);
        if std::env::var_os("AUSTRINGER_TS_DEMO").is_some() {
            eprintln!("[demo] replay {prompt:?} -> {text:?}");
        }
    }
    replayer.finish().expect("all interactions consumed");
}
