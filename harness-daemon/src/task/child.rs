//! The delegation child's turn await: bounded wait for the child
//! session's `TurnFinished` while accumulating the surfaces the parent
//! consumes — the last assistant text (the summary source) and the
//! Usage frames (token aggregate + the dispatch-level proof of what
//! served). Mirrors the launch path's await (lagged receivers recover
//! from the hub snapshot).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use harness_core::event::{ContentBlock, Event};

use super::TaskRefusal;
use crate::attach::SessionHost;
use crate::protocol::{ServerMsg, SessionEvent};

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
/// aggregate undercounts, never overcounts).
///
/// # Errors
/// [`TaskRefusal::Spawn`] on timeout or a closed child stream.
pub async fn await_child_turn(
    child: &Arc<SessionHost>,
    live: &mut broadcast::Receiver<ServerMsg>,
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
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(TaskRefusal::Spawn("child session timed out".into()));
        }
        match tokio::time::timeout(remaining, live.recv()).await {
            Err(_) => {
                return Err(TaskRefusal::Spawn("child session timed out".into()));
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                return Err(TaskRefusal::Spawn(
                    "child session stream closed".into(),
                ));
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                if let Some(outcome) = snapshot_outcome(child) {
                    observed.outcome = outcome;
                    return Ok(observed);
                }
            }
            Ok(Ok(ServerMsg::Event { frame })) => match frame.event {
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
            Ok(Ok(ServerMsg::Usage {
                model,
                effort,
                input_tokens,
                output_tokens,
                cache_read,
                cache_write,
                ..
            })) => {
                observed.usage.push((model.provider, model.model, effort));
                observed.tokens = observed
                    .tokens
                    .saturating_add(input_tokens)
                    .saturating_add(output_tokens)
                    .saturating_add(cache_read)
                    .saturating_add(cache_write);
            }
            Ok(Ok(_)) => {}
        }
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
