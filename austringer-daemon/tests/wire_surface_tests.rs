//! Wire-surface production wiring (research §24.6/§27.6/§33.1): the
//! session's pinned tool registry becomes the declared wire surface, the
//! daemon's stable system prompt rides every request, and the composed
//! provider factory renders them natively. Hermetic: cassette transports
//! and scripted stacks only.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use austringer_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest,
};
use austringer_core::tools::{CORE_TOOLS, IoCap, ToolRegistry, ToolSchema};
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::protocol::ServerMsg;
use austringer_daemon::server::DaemonState;
use austringer_daemon::tools::CoreExecutors;
use austringer_daemon::usage::ComposedInference;

// ------------------------------------------------- the registry surface

#[test]
fn executor_factory_surfaces_the_six_core_schemas_via_the_render_path() {
    let factory = Arc::new(CoreExecutors::in_process()).executor_factory();
    let dir = tempfile::tempdir().expect("tempdir");
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    let (_executor, tools) = factory(dir.path(), tx);

    // Exactly the canonical six, in canonical order, at the pinned
    // versions — the same content the existing render path serializes.
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(
        names,
        CORE_TOOLS.to_vec(),
        "canonical order is the wire order"
    );

    // The surface IS the render path's output (parse of its JSON).
    let registry = ToolRegistry::open(dir.path(), IoCap::DEFAULT).expect("registry");
    let pinned = registry.pin_current();
    let rendered: Vec<ToolSchema> =
        serde_json::from_str(&registry.render_core_schemas(&pinned)).expect("render round-trip");
    assert_eq!(
        tools, rendered,
        "the wire surface equals render_core_schemas"
    );
}

// ------------------------------------------- the full-session wiring

/// Captures the wire surfaces of every inference request and answers
/// with a plain text turn so the session completes.
#[derive(Clone, Default)]
struct RequestCapture {
    seen: Arc<Mutex<Vec<CapturedSurface>>>,
}

type CapturedSurface = (String, Vec<String>);

impl RequestCapture {
    fn captures(&self) -> Vec<CapturedSurface> {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl Inference for RequestCapture {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push((
                request.system.to_owned(),
                request.tools.iter().map(|tool| tool.name.clone()).collect(),
            ));
        }
        Ok(AssistantTurn {
            blocks: vec![austringer_core::event::ContentBlock::Text {
                text: "noted".to_owned(),
            }],
            tool_calls: Vec::new(),
            usage: None,
        })
    }
}

fn state_with_capture(capture: RequestCapture) -> Arc<DaemonState> {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = DaemonConfig {
        data_dir: dir.path().to_owned(),
        durable: false,
        ..DaemonConfig::default()
    };
    let factory: austringer_daemon::usage::InferenceFactory = Arc::new(move || ComposedInference {
        inference: Box::new(capture.clone()),
        identity: Some(austringer_providers::identity::ModelIdentity::new(
            "anthropic",
            "claude-ws",
        )),
    });
    let mut state = DaemonState::new(config, factory).expect("daemon state");
    // The registry-backed executor factory: the session's registry feeds
    // the wire surface (the worker-backed default has no registry).
    state.executor_factory = Arc::new(CoreExecutors::in_process()).executor_factory();
    std::mem::forget(dir);
    Arc::new(state)
}

#[tokio::test]
async fn session_requests_carry_the_daemon_system_and_the_registry_surface() {
    let capture = RequestCapture::default();
    let state = state_with_capture(capture.clone());
    let host = state.session("wire-surface-session");
    let mut live = host.hub_subscribe();
    let _ = host
        .handle_command(austringer_daemon::attach::HostCommand::Submit(
            "work".into(),
        ))
        .await;
    let mut turn_finished = false;
    let deadline = std::time::Duration::from_secs(30);
    let watched = tokio::time::timeout(deadline, async {
        while let Ok(msg) = live.recv().await {
            if let ServerMsg::Event { frame } = msg
                && let austringer_daemon::events::SessionEvent::TurnFinished { .. } = frame.event
            {
                turn_finished = true;
                break;
            }
        }
    })
    .await;
    assert!(watched.is_ok(), "the stream did not finish in time");
    assert!(turn_finished, "the turn completed");

    let seen = capture.captures();
    assert!(!seen.is_empty(), "the loop inferred");
    for (system, names) in &seen {
        assert_eq!(
            system,
            austringer_daemon::engine::SESSION_SYSTEM_PROMPT,
            "the daemon's stable system prompt rides every session request"
        );
        assert_eq!(
            names.as_slice(),
            CORE_TOOLS.to_vec().as_slice(),
            "the session registry's core schemas ride every session request"
        );
    }
}
