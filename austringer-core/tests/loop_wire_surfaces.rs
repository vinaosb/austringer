//! The loop's inference seam carries the configured wire surfaces:
//! `LoopConfig.system` and `LoopConfig.tools` ride EVERY InferenceRequest
//! (the production wiring of research §24.6/§33.1 — the daemon sets them
//! once per session from the pinned tool registry).

mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use austringer_core::agent_loop::{
    AgentLoop, AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest, LoopConfig,
    RunOutcome,
};
use austringer_core::tools::{ToolSchema, core_schema};

/// Captures the wire surfaces of every request (owned copies). The loop
/// owns one clone; the test reads through the other.
#[derive(Clone, Default)]
struct SurfaceCapture {
    seen: Arc<Mutex<CapturedSurfaces>>,
}

type CapturedSurfaces = Vec<(String, Vec<String>)>;

impl SurfaceCapture {
    fn captures(&self) -> CapturedSurfaces {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl Inference for SurfaceCapture {
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
        Ok(common::loopkit::text_turn("noted"))
    }
}

#[tokio::test]
async fn loop_passes_configured_system_and_tools_into_every_inference_request() {
    let dir = common::TempSession::new("wire-surfaces");
    let tools: Vec<ToolSchema> = ["view_window", "edit_lines"]
        .iter()
        .map(|name| core_schema(name))
        .collect();
    let config = LoopConfig {
        system: "STABLE SYSTEM PROMPT".to_owned(),
        tools,
        ..LoopConfig::default()
    };
    let capture = SurfaceCapture::default();
    let mut session = AgentLoop::open(
        dir.path(),
        config,
        common::stages_kit::stages(|_| {}),
        Box::new(capture.clone()),
    )
    .expect("open session");
    session.submit("work");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let seen = capture.captures();
    assert!(!seen.is_empty(), "at least one inference ran");
    for (system, names) in &seen {
        assert_eq!(system, "STABLE SYSTEM PROMPT", "system rides every request");
        assert_eq!(
            names,
            &["view_window".to_owned(), "edit_lines".to_owned()],
            "the declared surface rides every request"
        );
    }
}

#[tokio::test]
async fn default_loop_config_declares_no_wire_surfaces() {
    let dir = common::TempSession::new("wire-surfaces-default");
    let capture = SurfaceCapture::default();
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        common::stages_kit::stages(|_| {}),
        Box::new(capture.clone()),
    )
    .expect("open session");
    session.submit("work");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let seen = capture.captures();
    assert!(!seen.is_empty(), "at least one inference ran");
    for (system, names) in &seen {
        assert!(system.is_empty(), "default system stays empty");
        assert!(names.is_empty(), "default surface stays empty");
    }
}
