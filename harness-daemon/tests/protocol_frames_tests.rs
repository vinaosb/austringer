//! Protocol-frame acceptance (plan todo 1): the four v2-only server
//! frames (Usage, FallbackAsk, FallbackResolved, Verdict) plus the
//! fallback-answer client message. Covers emission through the real
//! engine path (fake provider, zero network), mediation of fallback
//! asks under the session's policy, and the v1 wire withholding that
//! keeps external v1 clients decodable.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use harness_core::agent_loop::inference::{InferenceError, InferenceRequest, PendingCall};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference, TurnUsage};
use harness_core::event::ContentBlock;

use harness_daemon::attach::SessionHost;
use harness_daemon::automations::{AutomationDefinition, NetworkPolicy};
use harness_daemon::bridge::RunSink;
use harness_daemon::compat::downgrade_for_v1;
use harness_daemon::gate::{CompletionGate, CriteriaGate};
use harness_daemon::mediation::{MediationError, MediationPolicy, PermissionMediator, control_bus};
use harness_daemon::notify::{MemorySink, Notifier};
use harness_daemon::protocol::{
    ClientMsg, Frame, ServerMsg, SessionEvent, VerdictKind, decode_server, encode_client,
    encode_server,
};
use harness_daemon::stages::WorkerToolExecutor;
use harness_daemon::usage::UsageInference;
use harness_daemon::worker::WorkerRouter;

use harness_providers::identity::{FallbackRequest, ModelIdentity};

fn identity(provider: &str, model: &str) -> ModelIdentity {
    ModelIdentity::new(provider, model)
}

fn completion_turn() -> AssistantTurn {
    AssistantTurn {
        blocks: vec![ContentBlock::Text {
            text: "done".to_owned(),
        }],
        tool_calls: vec![PendingCall {
            call_id: "done".into(),
            tool: "task_completed".to_owned(),
            arguments: serde_json::json!({}),
        }],
        // Provider-reported counts: a fake meter the frame must carry
        // verbatim (identity "fake" is unpriced -> cost None).
        usage: Some(TurnUsage {
            input_tokens: 120,
            output_tokens: 34,
            cache_read_tokens: 10,
            cache_write_tokens: 5,
        }),
    }
}

/// One-shot scripted fake provider (hermetic: zero network, zero keys).
struct ScriptedInference(AssistantTurn);

#[async_trait]
impl Inference for ScriptedInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Ok(self.0.clone())
    }
}

/// Collect every server message (hub lane) until the TurnFinished event.
async fn collect_hub_until_turn_finished(
    mut live: broadcast::Receiver<ServerMsg>,
) -> Vec<ServerMsg> {
    let mut seen = Vec::new();
    loop {
        match live.recv().await {
            Ok(msg) => {
                let finished = matches!(
                    &msg,
                    ServerMsg::Event { frame }
                        if matches!(frame.event, SessionEvent::TurnFinished { .. })
                );
                seen.push(msg);
                if finished {
                    return seen;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(_) => return seen,
        }
    }
}

/// A session host with the engine driven by `make` (the decorator
/// under test needs the host before the engine spawns).
async fn host_with_engine(
    name: &str,
    make: impl FnOnce(&Arc<SessionHost>) -> Box<dyn Inference>,
) -> (Arc<SessionHost>, Option<tempfile::TempDir>) {
    let dir = tempfile::tempdir().ok();
    let root = dir
        .as_ref()
        .map_or_else(|| std::env::temp_dir().join(name), |d| d.path().to_owned());
    let (host, cmd_rx) = SessionHost::new(
        name,
        64,
        32,
        8,
        MediationPolicy::FirstResponder,
        Duration::from_secs(30),
        Some(root.clone()),
    );
    let session_dir = root.join("sessions").join(name);
    std::fs::create_dir_all(&session_dir).ok();
    let executor = WorkerToolExecutor::new(
        Arc::new(WorkerRouter::new(Duration::from_secs(5))),
        None,
        host.event_sink(),
    );
    let inference = make(&host);
    host.spawn_engine(session_dir, host.standard_stages(executor), inference, cmd_rx)
        .ok();
    (host, dir)
}

// ---------------------------------------------------------------------------
// (a) Usage frame
// ---------------------------------------------------------------------------

#[tokio::test]
async fn usage_frame_emitted_once_per_assistant_turn() {
    let fake_model = identity("fake", "test-model");
    let (host, dir) = host_with_engine("s-usage", |host| {
        Box::new(UsageInference::new(
            Arc::clone(host),
            fake_model.clone(),
            Some("high".to_owned()),
            Box::new(ScriptedInference(completion_turn())),
        ))
    })
    .await;

    let attach = host.attach("tui", None);
    assert!(attach.is_ok(), "attach failed");
    let Ok(attachment) = attach else {
        return;
    };
    let reader = tokio::spawn(collect_hub_until_turn_finished(attachment.events.live));

    let _ = host
        .commands()
        .send(harness_daemon::attach::HostCommand::Submit("hello".into()))
        .await;

    let seen = tokio::time::timeout(Duration::from_secs(10), reader)
        .await
        .ok()
        .and_then(|r| r.ok());
    let Some(seen) = seen else {
        panic!("turn never finished");
    };
    let usage: Vec<&ServerMsg> = seen
        .iter()
        .filter(|m| matches!(m, ServerMsg::Usage { .. }))
        .collect();
    assert_eq!(usage.len(), 1, "exactly one Usage frame per turn: {seen:?}");
    let ServerMsg::Usage {
        turn,
        model,
        effort,
        input_tokens,
        output_tokens,
        cache_read,
        cache_write,
        cost_usd_micros,
    } = usage[0]
    else {
        panic!("shape");
    };
    assert_eq!(*turn, 1);
    assert_eq!(*model, fake_model, "Usage must carry model identity");
    assert_eq!(effort.as_deref(), Some("high"));
    assert_eq!((*input_tokens, *output_tokens, *cache_read, *cache_write), (120, 34, 10, 5));
    assert_eq!(
        *cost_usd_micros, None,
        "unpriced fake identity must report absent cost, never zero"
    );
    drop(dir);
}

// ---------------------------------------------------------------------------
// (b) FallbackAsk -> FallbackResolved
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fallback_ask_then_fallback_resolved_on_answer() {
    let (host, _cmd_rx) = SessionHost::new(
        "s-fallback",
        64,
        16,
        8,
        MediationPolicy::FirstResponder,
        Duration::from_secs(5),
        None,
    );
    let mut control = host.mediator().subscribe();
    let mut hub = host.hub_subscribe();
    let request = FallbackRequest::new(
        identity("anthropic", "claude-4-5"),
        identity("openai", "gpt-5-5"),
        "outage; verbatim hop 1 of 2",
    );
    let mediating = tokio::spawn({
        let host = Arc::clone(&host);
        async move { host.mediate_fallback(request).await }
    });

    let ask = loop {
        match control.recv().await {
            Ok(ServerMsg::FallbackAsk { ask }) => break ask,
            Ok(_) => continue,
            Err(e) => panic!("control bus died: {e:?}"),
        }
    };
    assert!(ask.ask_id > 0);
    assert_eq!(ask.current, identity("anthropic", "claude-4-5"));
    assert_eq!(ask.proposed, identity("openai", "gpt-5-5"));
    assert!(ask.reason.contains("outage"), "{}", ask.reason);
    assert!(ask.deadline_ms > 0, "deadline must be carried");

    // A v1-decodable Notification rides the hub so no v1 client
    // silently misses the ask (FallbackAsk itself is v1-withheld).
    let saw_notice = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match hub.recv().await {
                Ok(ServerMsg::Event {
                    frame:
                        Frame {
                            event: SessionEvent::Notification { kind, message },
                            ..
                        },
                }) if kind == "fallback" => return message,
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(e) => panic!("hub died: {e:?}"),
            }
        }
    })
    .await;
    assert!(
        saw_notice.is_ok_and(|message| message.contains("gpt-5-5")),
        "v1-visible fallback notification missing"
    );

    // Answer the ask over the mediated path: chosen == proposed hops.
    let resolved = host
        .mediator()
        .answer_fallback(ask.ask_id, "tui", ask.proposed.clone());
    assert_eq!(
        resolved,
        Ok(Some(harness_daemon::protocol::Resolution::Allowed {
            by: "tui".into()
        }))
    );

    match control.recv().await {
        Ok(ServerMsg::FallbackResolved { ask_id, chosen }) => {
            assert_eq!(ask_id, ask.ask_id);
            assert_eq!(chosen, identity("openai", "gpt-5-5"));
        }
        other => panic!("expected FallbackResolved, got {other:?}"),
    }
    let chosen = tokio::time::timeout(Duration::from_secs(2), mediating)
        .await
        .ok()
        .and_then(|r| r.ok());
    assert_eq!(
        chosen,
        Some(identity("openai", "gpt-5-5")),
        "acknowledged ask must retarget the proposed identity"
    );
}

// ---------------------------------------------------------------------------
// (c) Verdict frame
// ---------------------------------------------------------------------------

fn nightly_def() -> AutomationDefinition {
    AutomationDefinition {
        id: "nightly".into(),
        name: "nightly".into(),
        cron: "0 3 * * *".into(),
        pack: "review".into(),
        goal: Some("nightly review".into()),
        criteria: vec!["task_completed".into()],
        budget_turns: None,
        max_invocations_total: None,
        max_invocations_per_day: None,
        network: NetworkPolicy::Online,
        enabled: true,
    }
}

fn memory_sink() -> (Arc<MemorySink>, Arc<Notifier>) {
    let memory = Arc::new(MemorySink::new());
    let mut notifier = Notifier::new();
    notifier.add(Arc::clone(&memory) as Arc<dyn harness_daemon::notify::NotificationSink>);
    (memory, Arc::new(notifier))
}

#[tokio::test]
async fn verdict_frame_carries_non_empty_evidence() {
    let dir = tempfile::tempdir().ok();
    let root = dir
        .as_ref()
        .map_or_else(|| std::env::temp_dir().join("s-verdict-root"), |d| d.path().to_owned());
    let (_memory, notifier) = memory_sink();
    let sink = Arc::new(
        RunSink::open(
            &root,
            Arc::new(CriteriaGate) as Arc<dyn CompletionGate>,
            notifier,
        )
        .unwrap_or_else(|e| panic!("sink: {e}")),
    );
    let (host, _cmd_rx) = SessionHost::new(
        "s-verdict",
        64,
        16,
        8,
        MediationPolicy::FirstResponder,
        Duration::from_millis(200),
        None,
    );
    sink.run_started(&nightly_def(), 60, "s-verdict", NetworkPolicy::Online);
    sink.spawn_watcher(&host, "s-verdict");
    let mut hub = host.hub_subscribe();

    host.publish(SessionEvent::TurnStarted { turn: 1 });
    host.publish(SessionEvent::TurnFinished {
        turn: 1,
        outcome: "task_completed".into(),
    });

    let verdict = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match hub.recv().await {
                Ok(msg @ ServerMsg::Verdict { .. }) => return msg,
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(e) => panic!("hub died: {e:?}"),
            }
        }
    })
    .await
    .expect("verdict frame never arrived");
    let ServerMsg::Verdict {
        turn,
        verdict,
        evidence,
    } = verdict
    else {
        unreachable!("matched above");
    };
    assert_eq!(turn, 1);
    assert_eq!(verdict, VerdictKind::Confirmed);
    assert!(
        !evidence.is_empty(),
        "verdict must cite at least one event id"
    );
    assert!(evidence.iter().all(|id| *id > 0));
}

// ---------------------------------------------------------------------------
// (d) v1 withholding (compat downgrade)
// ---------------------------------------------------------------------------

#[test]
fn compat_withholds_v2_frames_from_v1_wires() {
    assert_eq!(
        downgrade_for_v1(ServerMsg::Usage {
            turn: 1,
            model: identity("fake", "m"),
            effort: None,
            input_tokens: 0,
            output_tokens: 0,
            cache_read: 0,
            cache_write: 0,
            cost_usd_micros: None,
        }),
        None
    );
    assert_eq!(
        downgrade_for_v1(ServerMsg::FallbackAsk {
            ask: harness_daemon::protocol::FallbackAskPayload {
                ask_id: 1,
                current: identity("a", "one"),
                proposed: identity("b", "two"),
                reason: "outage".into(),
                deadline_ms: 5_000,
            },
        }),
        None
    );
    assert_eq!(
        downgrade_for_v1(ServerMsg::FallbackResolved {
            ask_id: 1,
            chosen: identity("b", "two"),
        }),
        None
    );
    assert_eq!(
        downgrade_for_v1(ServerMsg::Verdict {
            turn: 1,
            verdict: VerdictKind::Inconclusive,
            evidence: vec![3],
        }),
        None
    );
    // v1-native messages keep passing through untouched.
    assert_eq!(downgrade_for_v1(ServerMsg::Pong), Some(ServerMsg::Pong));
}

// ---------------------------------------------------------------------------
// (e) a v1-shaped enum (no new variants) ERRORS decoding the new frames
// ---------------------------------------------------------------------------

/// The v1 wire shape: exactly the pre-v2 variant set. Fields exist for
/// serde's decoder only, so the dead-code lint is silenced test-wide.
#[allow(dead_code)]
#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum V1ShapedServerMsg {
    HelloAck { version: u32, last_event_id: u64 },
    Event { frame: Frame },
    PermissionAsk {
        ask: harness_daemon::protocol::AskPayload,
    },
    PermissionResolved {
        ask_id: u64,
        resolution: harness_daemon::protocol::Resolution,
    },
    StateResyncRequired { last_event_id: u64 },
    Snapshot { frames: Vec<Frame> },
    Error {
        code: harness_daemon::protocol::ErrorCode,
        message: String,
    },
    MediaResult {
        producer: String,
        artifact: harness_core::event::MediaRef,
        diff_summary: String,
    },
    Pong,
    Bye { reason: String },
}

#[test]
fn v1_shaped_enum_errors_decoding_v2_frames() {
    let frames: Vec<ServerMsg> = vec![
        ServerMsg::Usage {
            turn: 1,
            model: identity("fake", "m"),
            effort: None,
            input_tokens: 1,
            output_tokens: 2,
            cache_read: 3,
            cache_write: 4,
            cost_usd_micros: Some(9),
        },
        ServerMsg::FallbackAsk {
            ask: harness_daemon::protocol::FallbackAskPayload {
                ask_id: 1,
                current: identity("a", "one"),
                proposed: identity("b", "two"),
                reason: "outage".into(),
                deadline_ms: 1_000,
            },
        },
        ServerMsg::FallbackResolved {
            ask_id: 1,
            chosen: identity("b", "two"),
        },
        ServerMsg::Verdict {
            turn: 1,
            verdict: VerdictKind::Refuted,
            evidence: vec![7],
        },
    ];
    for frame in frames {
        let encoded = encode_server(&frame).unwrap_or_default();
        let decoded: Result<V1ShapedServerMsg, _> = serde_json::from_str(&encoded);
        assert!(
            decoded.is_err(),
            "a v1 client must fail decoding {encoded} — withholding is mandatory"
        );
    }
}

// ---------------------------------------------------------------------------
// (f) fallback mediation policy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fallback_mediation_policy_local_only_and_exactly_once() {
    let mediator = Arc::new(PermissionMediator::new(
        MediationPolicy::LocalOnly {
            client_id: "local".into(),
        },
        Duration::from_secs(30),
        control_bus(16),
    ));
    let mut watch = mediator.subscribe();
    let current = identity("anthropic", "claude-4-5");
    let proposed = identity("openai", "gpt-5-5");
    let mediating = tokio::spawn({
        let mediator = Arc::clone(&mediator);
        let request = FallbackRequest::new(current.clone(), proposed.clone(), "outage");
        async move { mediator.mediate_fallback(request).await }
    });

    let ask_id = loop {
        match watch.try_recv() {
            Ok(ServerMsg::FallbackAsk { ask }) => break ask.ask_id,
            Ok(_) => continue,
            Err(broadcast::error::TryRecvError::Empty) => {
                tokio::task::yield_now().await;
            }
            Err(e) => panic!("control bus died: {e:?}"),
        }
    };

    // A non-designated client's FallbackAnswer is IGNORED: the hop is
    // not granted, the ask stays pending.
    assert_eq!(
        mediator.answer_fallback(ask_id, "remote-attacker", proposed.clone()),
        Ok(None)
    );

    // The designated connection confirms the hop: resolves exactly once.
    assert_eq!(
        mediator.answer_fallback(ask_id, "local", proposed.clone()),
        Ok(Some(harness_daemon::protocol::Resolution::Allowed {
            by: "local".into()
        }))
    );

    // A second, divergent designated answer (stay on the current
    // model) must not change anything — the ask is already resolved.
    assert_eq!(
        mediator.answer_fallback(ask_id, "local", current.clone()),
        Err(MediationError::UnknownAsk(ask_id))
    );

    let mut resolved_count = 0;
    while let Ok(msg) = watch.try_recv() {
        if matches!(msg, ServerMsg::FallbackResolved { .. }) {
            resolved_count += 1;
        }
    }
    assert_eq!(resolved_count, 1, "exactly one FallbackResolved broadcast");

    let chosen = tokio::time::timeout(Duration::from_secs(2), mediating)
        .await
        .ok()
        .and_then(|r| r.ok());
    assert_eq!(chosen, Some(proposed));
}

/// A denied/timeout fallback ask resolves to the SAME model: the
/// mediated retry never substitutes.
#[tokio::test(start_paused = true)]
async fn fallback_deny_and_timeout_retry_the_same_model() {
    let mediator = Arc::new(PermissionMediator::new(
        MediationPolicy::FirstResponder,
        Duration::from_secs(1),
        control_bus(16),
    ));
    let current = identity("anthropic", "claude-4-5");
    let proposed = identity("openai", "gpt-5-5");

    // Deny side: chosen == current maps to deny.
    let denied = tokio::spawn({
        let mediator = Arc::clone(&mediator);
        let request = FallbackRequest::new(current.clone(), proposed.clone(), "outage");
        async move { mediator.mediate_fallback(request).await }
    });
    let mut watch = mediator.subscribe();
    let ask_id = loop {
        match watch.recv().await {
            Ok(ServerMsg::FallbackAsk { ask }) => break ask.ask_id,
            Ok(_) => continue,
            Err(e) => panic!("control bus died: {e:?}"),
        }
    };
    assert_eq!(
        mediator.answer_fallback(ask_id, "web", current.clone()),
        Ok(Some(harness_daemon::protocol::Resolution::Denied {
            by: "web".into()
        }))
    );
    assert_eq!(
        denied.await.ok(),
        Some(current.clone()),
        "deny must keep the current model"
    );

    // Timeout side: no answer at all.
    let timed_out = tokio::spawn({
        let mediator = Arc::clone(&mediator);
        let request = FallbackRequest::new(current.clone(), proposed, "outage");
        async move { mediator.mediate_fallback(request).await }
    });
    tokio::time::advance(Duration::from_secs(2)).await;
    assert_eq!(
        timed_out.await.ok(),
        Some(current),
        "timeout must retry the SAME model"
    );
}

/// The FallbackAnswer client message round-trips the wire codec.
#[test]
fn fallback_answer_client_roundtrip() {
    let msg = ClientMsg::FallbackAnswer {
        ask: 7,
        chosen: identity("openai", "gpt-5-5"),
    };
    let encoded = encode_client(&msg).unwrap_or_default();
    let decoded =
        decode_server(encoded.as_bytes()).map(|_: ServerMsg| panic!("wrong direction"));
    assert!(decoded.is_err(), "server decode must reject a client frame");
    let raw: ClientMsg = serde_json::from_str(&encoded).unwrap_or(ClientMsg::Ping);
    assert_eq!(raw, msg);
}

// ---------------------------------------------------------------------------
// Production emission: the SHIPPED composition path (compose_factory
// over a cassette transport -> DaemonState -> session engine), not a
// hand-built decorator stack.
// ---------------------------------------------------------------------------

mod common; // tests/common: the cassette transport

#[tokio::test]
async fn production_factory_emits_real_provider_usage() {
    use common::cassette::{Cassette, CassetteTransport, anthropic_text_cassette};
    use harness_daemon::config::DaemonConfig;
    use harness_daemon::provider::{ProviderFamily, ProviderSelection};
    use harness_daemon::server::DaemonState;

    // A real priced model from the builtin cost tables (claude-haiku-4-5:
    // 1e6 u/mtok input, 5e6 u/mtok output, 1e5 read, 1.25e6 write).
    let model = "claude-haiku-4-5";
    let selection = ProviderSelection {
        family: ProviderFamily::Anthropic,
        model: model.to_owned(),
        api_key_env: Some("HARNESS_T1_PROD_KEY".to_owned()),
        base_url: None,
        inference_timeout: Duration::from_secs(30),
    };
    let resolved = selection
        .resolve_with(|name| (name == "HARNESS_T1_PROD_KEY").then(|| "k".to_owned()))
        .unwrap_or_else(|e| panic!("resolve: {e}"));
    // The cassette reply carries usage on the wire: message_start
    // input_tokens=12, message_delta output_tokens=7.
    let transport = CassetteTransport::new(vec![Cassette::Sse(anthropic_text_cassette(
        model,
        "done",
    ))]);
    let factory = resolved.compose_factory(transport);

    let dir = tempfile::tempdir().ok();
    let root = dir
        .as_ref()
        .map_or_else(|| std::env::temp_dir().join("s-prod-usage"), |d| d.path().to_owned());
    let config = DaemonConfig {
        data_dir: root,
        token: Some("t1-prod".into()),
        ..DaemonConfig::default()
    };
    let state = Arc::new(DaemonState::new(config, factory).expect("daemon state"));
    let host = state.session("s-prod-usage");

    let attach = host.attach("tui", None);
    assert!(attach.is_ok(), "attach failed");
    let Ok(attachment) = attach else {
        return;
    };
    let reader = tokio::spawn(collect_hub_until_turn_finished(attachment.events.live));
    let _ = host
        .handle_command(harness_daemon::attach::HostCommand::Submit("hello".into()))
        .await;

    let seen = tokio::time::timeout(Duration::from_secs(10), reader)
        .await
        .ok()
        .and_then(|r| r.ok());
    let Some(seen) = seen else {
        panic!("turn never finished through the composed factory");
    };
    let usage: Vec<&ServerMsg> = seen
        .iter()
        .filter(|m| matches!(m, ServerMsg::Usage { .. }))
        .collect();
    assert_eq!(
        usage.len(),
        1,
        "the shipped path must emit exactly one Usage frame: {seen:?}"
    );
    let ServerMsg::Usage {
        turn,
        model: served,
        effort,
        input_tokens,
        output_tokens,
        cache_read,
        cache_write,
        cost_usd_micros,
    } = usage[0]
    else {
        panic!("shape");
    };
    assert_eq!(*turn, 1);
    assert_eq!(
        *served,
        identity("anthropic", model),
        "the frame must carry the composed provider identity"
    );
    assert_eq!(effort.as_deref(), None);
    // The cassette's REAL reported counts, parsed by the anthropic
    // adapter — never invented.
    assert_eq!((*input_tokens, *output_tokens, *cache_read, *cache_write), (12, 7, 0, 0));
    // Hand-computed from the builtin haiku table: 12 * 1_000_000/1e6 +
    // 7 * 5_000_000/1e6 = 12 + 35 = 47 micro-USD.
    assert_eq!(*cost_usd_micros, Some(47));
}
