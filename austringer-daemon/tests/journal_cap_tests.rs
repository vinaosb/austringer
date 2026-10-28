//! Explicit turn-journal cap wiring: `DaemonConfig::journal_max_bytes`
//! threads through the session factory into every per-session turn
//! journal (`Journal::open_capped` at the engine seam), so a
//! configured cap actually rotates journals. Hosts built outside the
//! factory keep the 32 MiB default wave 1 enforces inside
//! `Journal::open`.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;

use austringer_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TASK_COMPLETED_TOOL,
};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::event::ContentBlock;

use austringer_daemon::attach::HostCommand;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::protocol::{ServerMsg, SessionEvent};
use austringer_daemon::server::DaemonState;

/// Always-completes inference: one submit is one finished turn, no
/// permission asks, no executor round-trips.
struct AlwaysComplete;

#[async_trait]
impl Inference for AlwaysComplete {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Ok(AssistantTurn {
            blocks: vec![ContentBlock::Text {
                text: "done".into(),
            }],
            tool_calls: vec![PendingCall {
                call_id: "done".into(),
                tool: TASK_COMPLETED_TOOL.to_owned(),
                arguments: serde_json::json!({}),
            }],
            usage: None,
        })
    }
}

async fn wait_turn_finished(mut live: broadcast::Receiver<ServerMsg>) {
    loop {
        match live.recv().await {
            Ok(ServerMsg::Event { frame }) => {
                if matches!(frame.event, SessionEvent::TurnFinished { .. }) {
                    return;
                }
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => {
                panic!("event hub closed before TurnFinished")
            }
        }
    }
}

#[tokio::test]
async fn journal_cap_from_config_rotates_turn_journals() {
    let dir = tempfile::tempdir().unwrap();
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
        Arc::new(|| Box::new(AlwaysComplete));
    let state = Arc::new(
        DaemonState::new(
            DaemonConfig {
                data_dir: dir.path().to_owned(),
                // Any TurnStarted record exceeds 8 bytes, so the
                // SECOND journal record must rotate the first away.
                journal_max_bytes: 8,
                ..DaemonConfig::default()
            },
            factory,
        )
        .unwrap(),
    );
    let host = state.session("cap-e2e");
    let live = host.hub_subscribe();
    host.commands()
        .send(HostCommand::Submit("hello".into()))
        .await
        .unwrap();
    wait_turn_finished(live).await;

    let journal_dir = dir.path().join("journal").join("cap-e2e");
    let current = std::fs::read_to_string(journal_dir.join("1.jsonl"))
        .expect("the current turn journal exists");
    assert!(
        current.contains("turn_finished"),
        "the latest state lands in the current file: {current}"
    );
    let forensic = std::fs::read_to_string(journal_dir.join("1.jsonl.1")).unwrap_or_default();
    assert!(
        forensic.contains("turn_started"),
        "a configured cap must rotate at the engine seam: {forensic}"
    );
}
