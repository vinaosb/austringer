//! The delegation child's turn await: bounded wait for the child
//! session's `TurnFinished` while accumulating the surfaces the parent
//! consumes — the last assistant text (the summary source) and the
//! Usage frames (token aggregate + the dispatch-level proof of what
//! served). Mirrors the launch path's await (lagged receivers recover
//! from the hub snapshot).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use austringer_core::event::{ContentBlock, Event};

use super::TaskRefusal;
use crate::attach::SessionHost;
use crate::protocol::{AskPayload, ServerMsg, SessionEvent};

/// What one finished child surfaced.
#[derive(Debug, Clone, PartialEq)]
pub struct ChildObserved {
    /// The child's TurnFinished outcome text.
    pub outcome: String,
    /// The child's last assistant text (empty when it produced none).
    pub summary: String,
    /// Tokens summed from the child's Usage frames.
    pub tokens: u64,
    /// One `(provider, model, effort)` row per Usage frame.
    pub usage: Vec<(String, String, Option<String>)>,
}

impl ChildObserved {
    /// Whether the observed turn completed successfully.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.outcome == "task_completed" || self.outcome == "turn_complete"
    }
}

/// Awaits the child's `TurnFinished`, bounded by `timeout`. A lagged
/// receiver recovers from the hub snapshot (the ring still holds the
/// finished turn; Usage broadcasts are lost to lag by design — the
/// aggregate undercounts, never overcounts). While waiting, the
/// child's mediation asks surface on the PARENT hub as `blocked`
/// `ChildProgress` events (todo 15: an attached client sees the child
/// is waiting, and on what); resolutions flip the phase back to
/// `running`. The two receivers arrive as params because their
/// subscription lifetimes differ — `control` must be subscribed
/// BEFORE the prompt submit, `live` only after the engine spawns — so
/// neither can be derived inside.
///
/// # Errors
/// [`TaskRefusal::Spawn`] on timeout or a closed child stream.
pub async fn await_child_turn(
    parent: &Arc<SessionHost>,
    child: &Arc<SessionHost>,
    live: &mut broadcast::Receiver<ServerMsg>,
    control: &mut broadcast::Receiver<ServerMsg>,
    timeout: Duration,
) -> Result<ChildObserved, TaskRefusal> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut observed = ChildObserved {
        outcome: String::new(),
        summary: String::new(),
        tokens: 0,
        usage: Vec::new(),
    };
    loop {
        tokio::select! {
            biased;
            _ = tokio::time::sleep_until(deadline) => {
                return Err(TaskRefusal::Spawn("child session timed out".into()));
            }
            received = control.recv() => match received {
                Ok(ServerMsg::PermissionAsk { ask }) => {
                    publish_phase(parent, child, "blocked", ask_summary(&ask));
                }
                Ok(ServerMsg::FallbackAsk { ask }) => {
                    publish_phase(
                        parent,
                        child,
                        "blocked",
                        format!(
                            "model fallback proposed: {} -> {}",
                            ask.current.model, ask.proposed.model
                        ),
                    );
                }
                Ok(ServerMsg::PermissionResolved { .. } | ServerMsg::FallbackResolved { .. }) => {
                    publish_phase(parent, child, "running", "ask resolved".to_owned());
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(TaskRefusal::Spawn(
                        "child control stream closed".into(),
                    ));
                }
            },
            received = live.recv() => match received {
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(TaskRefusal::Spawn(
                        "child session stream closed".into(),
                    ));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    if let Some(outcome) = snapshot_outcome(child) {
                        observed.outcome = outcome;
                        return Ok(observed);
                    }
                }
                Ok(ServerMsg::Event { frame }) => match frame.event {
                    SessionEvent::Recorded {
                        event: Event::Assistant { blocks },
                        ..
                    } => {
                        observed.summary = blocks
                            .into_iter()
                            .map(|block| match block {
                                ContentBlock::Text { text } => text,
                                other => other.text_marker(),
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                    }
                    SessionEvent::TurnFinished { outcome, .. } => {
                        observed.outcome = outcome;
                        return Ok(observed);
                    }
                    _ => {}
                },
                Ok(ServerMsg::Usage {
                    model,
                    effort,
                    input_tokens,
                    output_tokens,
                    cache_read,
                    cache_write,
                    ..
                }) => {
                    observed.usage.push((model.provider, model.model, effort));
                    observed.tokens = observed
                        .tokens
                        .saturating_add(input_tokens)
                        .saturating_add(output_tokens)
                        .saturating_add(cache_read)
                        .saturating_add(cache_write);
                }
                Ok(_) => {}
            },
        }
    }
}

fn publish_phase(parent: &Arc<SessionHost>, child: &Arc<SessionHost>, phase: &str, detail: String) {
    parent.publish(SessionEvent::ChildProgress {
        child: child.session_id().to_owned(),
        phase: phase.to_owned(),
        detail,
    });
}

fn ask_summary(ask: &AskPayload) -> String {
    match &ask.target {
        Some(target) => format!("permission: {} {}", ask.tool, target),
        None => format!("permission: {}", ask.tool),
    }
}

/// The last TurnFinished outcome in the hub snapshot (lag recovery).
fn snapshot_outcome(child: &Arc<SessionHost>) -> Option<String> {
    child
        .snapshot()
        .iter()
        .rev()
        .find_map(|frame| match &frame.event {
            SessionEvent::TurnFinished { outcome, .. } => Some(outcome.clone()),
            _ => None,
        })
}
