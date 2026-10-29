//! Todo-13 wiring acceptance: the dormant core compaction engine
//! live in the daemon loop — threshold-driven auto compaction, the
//! typed `/condense` manual entry, deterministic-fallback doctrine
//! preservation without a summarizer, and the same-model condenser
//! bridge over an identity-bearing stack.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use austringer_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest,
};
use austringer_core::compaction::condense::DEFAULT_CONDENSE_PROMPT;
use austringer_core::event::{Event, RecordedEvent};
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::protocol::ServerMsg;
use austringer_daemon::server::DaemonState;
use austringer_daemon::usage::ComposedInference;

/// Session text whose doctrine-pinned facts live in the user turn:
/// both the condenser path and the deterministic fallback must carry
/// them into the compaction summary.
const DOCTRINE_PROMPT: &str = "We are building the billing service.\n\
     DECISION: the API port is 8085.\n\
     DECISION: the database is Postgres 15.\n\
     UNRESOLVED BUG: checkout returns 500 under concurrent load.";

fn text_turn(text: &str) -> AssistantTurn {
    AssistantTurn {
        blocks: vec![austringer_core::event::ContentBlock::Text {
            text: text.to_owned(),
        }],
        tool_calls: Vec::new(),
        usage: None,
    }
}

/// Turn inference: plain text (the session stays InProgress so a
/// later `/condense` can run). Condense inference (the focus block is
/// the doctrine condense prompt): return a summary carrying the
/// pinned facts.
#[derive(Clone)]
struct CondensingInference;

#[async_trait]
impl Inference for CondensingInference {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        if request.focus_block == DEFAULT_CONDENSE_PROMPT {
            return Ok(text_turn(
                "carried: the API port is 8085; Postgres 15; checkout returns 500",
            ));
        }
        Ok(text_turn("noted"))
    }
}

/// Turn inference: plain text. Condense inference: always fails —
/// the deterministic fallback must still compact.
#[derive(Clone)]
struct FailingCondenseInference;

#[async_trait]
impl Inference for FailingCondenseInference {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        if request.focus_block == DEFAULT_CONDENSE_PROMPT {
            return Err(InferenceError::Failure("condense seam down".into()));
        }
        Ok(text_turn("noted"))
    }
}

fn legacy_factory(
    inference: impl Inference + Clone + Sync + 'static,
) -> Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> {
    Arc::new(move || Box::new(inference.clone()))
}

fn identity_factory(
    inference: impl Inference + Clone + Sync + 'static,
) -> Arc<dyn Fn() -> ComposedInference + Send + Sync> {
    Arc::new(move || ComposedInference {
        inference: Box::new(inference.clone()),
        identity: Some(austringer_providers::identity::ModelIdentity::new(
            "anthropic",
            "claude-x",
        )),
    })
}

fn state_with(
    tweak: impl FnOnce(&mut DaemonConfig),
    factory: impl austringer_daemon::usage::IntoSessionFactory,
) -> Arc<DaemonState> {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut config = DaemonConfig {
        data_dir: dir.path().to_owned(),
        durable: false,
        ..DaemonConfig::default()
    };
    tweak(&mut config);
    let state = Arc::new(DaemonState::new(config, factory).expect("daemon state"));
    std::mem::forget(dir);
    state
}

fn tiny_threshold(config: &mut DaemonConfig) {
    config.compaction.engine.threshold = austringer_core::compaction::Threshold::TokensUsed(1);
}

/// Submits and collects the session's hub stream until the turn
/// finishes; returns the recorded events plus daemon notifications.
async fn run_turn_collect(
    state: &Arc<DaemonState>,
    session: &str,
    text: &str,
) -> (Vec<RecordedEvent>, Vec<(String, String)>) {
    let host = state.session(session);
    let mut live = host.hub_subscribe();
    let _ = host
        .handle_command(austringer_daemon::attach::HostCommand::Submit(text.into()))
        .await;
    let mut events = Vec::new();
    let mut notifications = Vec::new();
    let deadline = Duration::from_secs(30);
    let outcome = tokio::time::timeout(deadline, async {
        loop {
            match live.recv().await {
                Ok(ServerMsg::Event { frame }) => match frame.event {
                    austringer_daemon::events::SessionEvent::Recorded { seq, event } => {
                        events.push(RecordedEvent {
                            seq: austringer_core::event::Sequence(seq),
                            event,
                        });
                    }
                    austringer_daemon::events::SessionEvent::Notification { kind, message } => {
                        notifications.push((kind, message));
                    }
                    austringer_daemon::events::SessionEvent::TurnFinished { .. } => return,
                    _ => {}
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                Ok(_) => {}
            }
        }
    })
    .await;
    assert!(outcome.is_ok(), "turn never finished for submit {text:?}");
    (events, notifications)
}

fn condensations(events: &[RecordedEvent]) -> Vec<&Event> {
    events
        .iter()
        .map(|recorded| &recorded.event)
        .filter(|event| matches!(event, Event::Condensation { .. }))
        .collect()
}

fn condensation_summary(events: &[RecordedEvent]) -> String {
    condensations(events)
        .iter()
        .map(|event| match event {
            Event::Condensation { summary, .. } => summary.clone(),
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn telemetry_markers(events: &[RecordedEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|recorded| match &recorded.event {
            Event::System { text } if text.starts_with("<compaction_telemetry") => {
                Some(text.clone())
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn auto_compaction_fires_over_threshold_with_doctrine_preserved() {
    let state = state_with(tiny_threshold, legacy_factory(CondensingInference));
    // Identity-less (scripted) stack: no condenser is registered, so
    // the over-threshold run lands on the deterministic fallback.
    let (events, notifications) = run_turn_collect(&state, "auto-fallback", DOCTRINE_PROMPT).await;
    assert!(!condensations(&events).is_empty(), "no compaction ran");
    let summary = condensation_summary(&events);
    assert!(
        summary.contains("the API port is 8085"),
        "doctrine decision must survive the fallback:\n{summary}"
    );
    assert!(
        summary.contains("Postgres 15"),
        "doctrine decision must survive the fallback:\n{summary}"
    );
    assert!(
        summary.contains("checkout returns 500"),
        "unresolved bug must survive the fallback:\n{summary}"
    );
    let markers = telemetry_markers(&events);
    assert!(
        markers
            .iter()
            .any(|marker| marker.contains("outcome=\"auto\"")),
        "auto telemetry marker: {markers:?}"
    );
    assert!(
        markers
            .iter()
            .any(|marker| marker.contains("fallback=\"true\"")),
        "fallback engagement must be recorded: {markers:?}"
    );
    assert!(
        notifications.iter().any(|(kind, message)| kind == "condense"
            && message.contains("deterministic fallback")),
        "fallback engagement must surface as a daemon notice: {notifications:?}"
    );
    let compaction_dir = state
        .config
        .data_dir
        .join("sessions")
        .join("auto-fallback")
        .join("compaction");
    assert!(
        compaction_dir.join("index.json").exists(),
        "segment store must persist under the session dir"
    );
}

#[tokio::test]
async fn condense_command_runs_manual_compaction_below_threshold() {
    // Default Percent(75)/200k: a one-prompt session is far below it;
    // the manual entry must still compact.
    let state = state_with(|_| {}, legacy_factory(CondensingInference));
    let (events, _notifications) = run_turn_collect(&state, "manual", DOCTRINE_PROMPT).await;
    assert!(
        condensations(&events).is_empty(),
        "nothing auto-compacts below threshold"
    );
    let (events, notifications) = run_turn_collect(&state, "manual", "/condense").await;
    assert_eq!(condensations(&events).len(), 1, "manual compaction ran");
    let markers = telemetry_markers(&events);
    assert!(
        markers
            .iter()
            .any(|marker| marker.contains("outcome=\"manual\"")),
        "manual telemetry marker: {markers:?}"
    );
    assert!(
        notifications.iter().any(|(kind, _)| kind == "condense"),
        "condense notice published: {notifications:?}"
    );
}

#[tokio::test]
async fn same_model_condenser_path_produces_model_summary() {
    let state = state_with(tiny_threshold, identity_factory(CondensingInference));
    let (events, _) = run_turn_collect(&state, "condenser", DOCTRINE_PROMPT).await;
    assert!(!condensations(&events).is_empty(), "no compaction ran");
    let summary = condensation_summary(&events);
    assert!(
        summary.contains("<model_summary>"),
        "the condenser path appends the model summary:\n{summary}"
    );
    assert!(
        summary.contains("the API port is 8085"),
        "condensed facts carried:\n{summary}"
    );
    let markers = telemetry_markers(&events);
    assert!(
        markers
            .iter()
            .any(|marker| marker.contains("fallback=\"false\"")),
        "condenser path is not the fallback: {markers:?}"
    );
}

#[tokio::test]
async fn failing_condenser_still_compacts_through_the_fallback() {
    let state = state_with(tiny_threshold, identity_factory(FailingCondenseInference));
    let (events, notifications) = run_turn_collect(&state, "fail-condense", DOCTRINE_PROMPT).await;
    assert!(
        !condensations(&events).is_empty(),
        "fallback must still compact"
    );
    let summary = condensation_summary(&events);
    assert!(
        summary.contains("the API port is 8085"),
        "doctrine survives a failed summarizer:\n{summary}"
    );
    assert!(
        notifications.iter().any(|(kind, message)| kind == "condense"
            && message.contains("deterministic fallback")),
        "fallback surfacing is mandatory: {notifications:?}"
    );
}

#[tokio::test]
async fn disabled_compaction_answers_condense_honestly_and_never_fires() {
    let state = state_with(
        |config| {
            config.compaction.enabled = false;
            tiny_threshold(config);
        },
        legacy_factory(CondensingInference),
    );
    let (events, _) = run_turn_collect(&state, "disabled", DOCTRINE_PROMPT).await;
    assert!(
        condensations(&events).is_empty(),
        "disabled compaction must never fire"
    );
    let (events, notifications) = run_turn_collect(&state, "disabled", "/condense").await;
    assert!(
        condensations(&events).is_empty(),
        "disabled compaction must not run manually either"
    );
    assert!(
        notifications
            .iter()
            .any(|(kind, message)| kind == "condense" && message.contains("disabled")),
        "the refusal must surface: {notifications:?}"
    );
}
