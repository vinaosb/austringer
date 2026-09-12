//! QA happy path: a scripted MULTI-TURN conversation through the Anthropic
//! adapter, served from a T2 cassette with strict replay — request
//! rendering must match the recorded interactions byte-for-byte (model +
//! full prompt), and the cassette must finish exhausted.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use harness_providers::adapters::AnthropicAdapter;
use harness_providers::auth::StaticApiKey;
use harness_providers::cost::{CostTable, Priced};
use harness_providers::identity::{CompletionRequest, ModelIdentity};
use harness_providers::provider::Provider;
use harness_test_support::cassette::{CassetteRecorder, CassetteReplayer};
use harness_test_support::provider::{ProviderRequest, ScriptedReply};
use harness_test_support::sse::SseByteStream;
use serde_json::Value;

use common::wirekit;

mod common;

use common::frames::{an_message_delta, an_message_start, an_text_delta, anthropic_happy};

/// Serves cassette interactions strictly: the adapter's rendered request is
/// mapped back onto the T2 `ProviderRequest` shape and must equal the next
/// recorded interaction exactly (stale-state strictness).
struct CassetteTransport {
    replayer: Mutex<CassetteReplayer>,
}

#[async_trait]
impl harness_providers::transport::Transport for CassetteTransport {
    async fn send(
        &self,
        request: harness_providers::transport::TransportRequest,
    ) -> Result<
        harness_providers::transport::TransportResponse,
        harness_providers::transport::TransportFailure,
    > {
        let body: Value = serde_json::from_slice(&request.body).map_err(|err| {
            harness_providers::transport::TransportFailure::Protocol(format!(
                "request body is not JSON: {err}"
            ))
        })?;
        let model = body["model"].as_str().ok_or_else(|| {
            harness_providers::transport::TransportFailure::Protocol("model missing".to_owned())
        })?;
        let prompt = body["messages"][0]["content"].as_str().ok_or_else(|| {
            harness_providers::transport::TransportFailure::Protocol("prompt missing".to_owned())
        })?;
        let mapped = ProviderRequest::new(model, prompt);
        let reply = self
            .replayer
            .lock()
            .ok()
            .and_then(|mut replayer| replayer.next_interaction(&mapped).ok())
            .ok_or_else(|| {
                harness_providers::transport::TransportFailure::Protocol(
                    "cassette mismatch or exhausted".to_owned(),
                )
            })?;
        match reply {
            ScriptedReply::Stream(script) => {
                Ok(harness_providers::transport::TransportResponse::ok(
                    harness_providers::transport::Body::Stream(
                        harness_providers::transport::ByteStream::new(Box::new(CassetteSource(
                            SseByteStream::from_script(&script),
                        ))),
                    ),
                ))
            }
            ScriptedReply::ServerError => {
                Err(harness_providers::transport::TransportFailure::Protocol(
                    "cassette scripted a server error".to_owned(),
                ))
            }
            failure => Err(harness_providers::transport::TransportFailure::Protocol(
                format!("cassette scripted {failure:?}"),
            )),
        }
    }
}

struct CassetteSource(SseByteStream);

#[async_trait]
impl harness_providers::transport::ByteSource for CassetteSource {
    async fn next_chunk(&mut self) -> Option<Vec<u8>> {
        self.0.next_chunk().await
    }
}

#[tokio::test]
async fn multi_turn_conversation_replays_from_cassette_with_cost_accounting() {
    let turn1_prompt = "user: find the todos";
    let turn2_prompt =
        "user: find the todos\nassistant: Let me check the todos.\nuser: run the grep";

    let mut recorder = CassetteRecorder::new("anthropic-multi-turn");
    recorder.record(
        &ProviderRequest::new("claude-sonnet-4-5", turn1_prompt),
        &ScriptedReply::Stream(anthropic_happy("claude-sonnet-4-5")),
    );
    let second_turn = harness_test_support::sse::SseScriptBuilder::new()
        .event(an_message_start("claude-sonnet-4-5", 2_000, 1_500, 0))
        .event(an_text_delta("All clear."))
        .event(an_message_delta(12, "end_turn"))
        .event(harness_test_support::sse::ev_message_stop())
        .build();
    recorder.record(
        &ProviderRequest::new("claude-sonnet-4-5", turn2_prompt),
        &ScriptedReply::Stream(second_turn),
    );

    let transport = Arc::new(CassetteTransport {
        replayer: Mutex::new(CassetteReplayer::new(recorder.finish())),
    });
    let adapter = AnthropicAdapter::new(
        transport.clone(),
        Arc::new(StaticApiKey::x_api_key("sk-ant")),
        "claude-sonnet-4-5",
    );
    let identity = ModelIdentity::new("anthropic", "claude-sonnet-4-5");

    let (events1, terminal1) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity.clone(), turn1_prompt))
            .await
            .expect("turn 1 opens"),
    )
    .await;
    assert!(terminal1.is_none());
    assert_eq!(wirekit::text_of(&events1), "Let me check the todos.");
    let usage1 = wirekit::usage_of(&events1);

    let (events2, terminal2) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity.clone(), turn2_prompt))
            .await
            .expect("turn 2 opens"),
    )
    .await;
    assert!(terminal2.is_none());
    assert_eq!(wirekit::text_of(&events2), "All clear.");
    let usage2 = wirekit::usage_of(&events2);

    let table = CostTable::builtin();
    let report1 = table.account(&identity, usage1);
    let report2 = table.account(&identity, usage2);
    let Priced::Priced(costs1) = report1.priced else {
        panic!("priced");
    };
    let Priced::Priced(costs2) = report2.priced else {
        panic!("priced");
    };
    let conversation_total = costs1.total_usd_micros + costs2.total_usd_micros;
    // turn1: in 1234*3µ + out 42*15µ + cache-read 5000*0.3µ + cache-write 1000*3.75µ
    // turn2: in 2000*3µ + out 12*15µ + cache-read 1500*0.3µ
    assert_eq!(costs1.total_usd_micros, 3_702 + 630 + 1_500 + 3_750);
    assert_eq!(costs2.total_usd_micros, 6_000 + 180 + 450);
    assert_eq!(conversation_total, 16_212);

    // Strict cassette finish: every interaction consumed, none left over.
    let exhausted = transport
        .replayer
        .lock()
        .map(|replayer| replayer.is_exhausted())
        .unwrap_or(false);
    assert!(exhausted, "cassette must finish exhausted (strict replay)");
}
