//! Session transcript view model: reduces daemon protocol frames into
//! renderable transcript entries, tool-call pairs, turn lifecycle,
//! diffs, and worker state. A snapshot rebuilds the view from zero
//! (the divergence resync path). The team grid's cells and budget view
//! live in [`team`].

mod team;

pub use team::{ChildCell, TeamBudgetView};
mod status;

use std::collections::HashMap;

use austringer_core::event::{ContentBlock, Event};
use austringer_daemon::protocol::{Frame, ServerMsg, SessionEvent};

pub use crate::diffs::{DiffEntry, HunkDecision};

/// One transcript row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// User turn text.
    User(String),
    /// Assistant markdown (streamed chunks concatenated by arrival).
    Assistant(String),
    /// Engine system notice.
    System(String),
    /// Daemon-level notification (`kind: message`).
    Notice(String),
    /// Internal condensation marker surfaced as a dim notice.
    Condensed(String),
}

/// A tool call and (once it arrives) its result, paired by call id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolEntry {
    /// Correlation id.
    pub call_id: String,
    /// Tool name.
    pub tool: String,
    /// Bounded argument summary.
    pub args: String,
    /// Result text once the ToolResult frame arrived.
    pub result: Option<String>,
}

/// Turn lifecycle row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnState {
    /// Turn number.
    pub turn: u64,
    /// Outcome string once `turn_finished` arrived.
    pub outcome: Option<String>,
}

/// Reduced view of everything the UI shows for one session.
#[derive(Debug, Clone, Default)]
pub struct SessionView {
    /// Highest frame id applied (reconnect `after` cursor).
    pub last_event_id: u64,
    /// True between `state_resync_required` and the snapshot that
    /// follows it.
    pub resync_pending: bool,
    entries: Vec<Entry>,
    tools: Vec<ToolEntry>,
    turns: Vec<TurnState>,
    diffs: Vec<DiffEntry>,
    children: HashMap<String, ChildCell>,
    team_budget: Option<TeamBudgetView>,
    notes: Vec<String>,
}

fn text_of(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn bounded_args(arguments: &serde_json::Value) -> String {
    let full = arguments.to_string();
    if full.chars().count() > 80 {
        let cut: String = full.chars().take(79).collect();
        format!("{cut}\u{2026}")
    } else {
        full
    }
}

impl SessionView {
    /// Empty view.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one numbered frame.
    pub fn apply_frame(&mut self, frame: &Frame) {
        self.last_event_id = frame.id;
        match &frame.event {
            SessionEvent::Recorded { event, .. } => self.apply_event(event),
            SessionEvent::TurnStarted { turn } => {
                if !self.turns.iter().any(|state| state.turn == *turn) {
                    self.turns.push(TurnState {
                        turn: *turn,
                        outcome: None,
                    });
                }
            }
            SessionEvent::TurnFinished { turn, outcome } => {
                match self.turns.iter_mut().find(|state| state.turn == *turn) {
                    Some(state) => state.outcome = Some(outcome.clone()),
                    None => self.turns.push(TurnState {
                        turn: *turn,
                        outcome: Some(outcome.clone()),
                    }),
                }
            }
            SessionEvent::Diff { path, hunks, .. } => {
                self.diffs.push(DiffEntry::new(path, hunks.clone()));
            }
            SessionEvent::Notification { kind, message } => {
                self.entries
                    .push(Entry::Notice(format!("{kind}: {message}")));
            }
            SessionEvent::ChildStarted { child, task, .. } => {
                self.apply_child_started(child, task);
            }
            SessionEvent::ChildProgress {
                child,
                phase,
                detail,
            } => {
                self.apply_child_progress(child, phase, detail);
            }
            SessionEvent::ChildFinished {
                child,
                state,
                detail,
                ..
            } => {
                self.apply_child_finished(child, state, detail);
            }
            SessionEvent::TeamBudget {
                spawned,
                tokens_spent,
                fan_out_cap,
                token_budget,
            } => {
                self.apply_team_budget(*spawned, *tokens_spent, *fan_out_cap, *token_budget);
            }
            SessionEvent::TeamBudgetExhausted { spent, budget } => {
                self.apply_team_exhausted(*spent, *budget);
            }
            SessionEvent::CostWarning {
                consumed_micros,
                cap_micros,
            } => {
                self.apply_cost_warning(*consumed_micros, *cap_micros);
            }
            SessionEvent::CostStop {
                consumed_micros,
                cap_micros,
            } => {
                self.apply_cost_stop(*consumed_micros, *cap_micros);
            }
            SessionEvent::PlanModeEntered { plan } => {
                self.apply_plan_entered(*plan);
            }
            SessionEvent::PlanModeExited { plan } => {
                self.apply_plan_exited(*plan);
            }
            SessionEvent::PlanBlocked { plan, tool, target } => {
                self.apply_plan_blocked(*plan, tool, target);
            }
            SessionEvent::PlanUpdated { plan, revision } => {
                self.apply_plan_updated(*plan, *revision);
            }
            SessionEvent::PlanApproved { plan } => {
                self.apply_plan_approved(*plan);
            }
            SessionEvent::PlanRejected { plan } => {
                self.apply_plan_rejected(*plan);
            }
        }
    }

    fn apply_event(&mut self, event: &Event) {
        match event {
            Event::User { blocks } => {
                self.entries.push(Entry::User(text_of(blocks)));
            }
            Event::Assistant { blocks } => {
                self.entries.push(Entry::Assistant(text_of(blocks)));
            }
            Event::System { text } => {
                self.entries.push(Entry::System(text.clone()));
            }
            Event::Condensation { summary, .. } => {
                self.entries.push(Entry::Condensed(summary.clone()));
            }
            Event::ToolCall {
                call_id,
                tool,
                arguments,
            } => {
                self.tools.push(ToolEntry {
                    call_id: call_id.clone(),
                    tool: tool.clone(),
                    args: bounded_args(arguments),
                    result: None,
                });
            }
            Event::ToolResult { call_id, blocks } => {
                if let Some(tool) = self.tools.iter_mut().find(|t| &t.call_id == call_id) {
                    tool.result = Some(text_of(blocks));
                }
            }
            Event::Snapshot { label } => {
                self.apply_rewind_point(label);
            }
        }
    }

    /// Apply any server message the view cares about (events,
    /// snapshots, resync markers).
    pub fn apply_server_msg(&mut self, msg: &ServerMsg) {
        match msg {
            ServerMsg::Event { frame } => self.apply_frame(frame),
            ServerMsg::StateResyncRequired { .. } => self.resync_pending = true,
            ServerMsg::Snapshot { frames } => self.apply_snapshot(frames),
            _ => {}
        }
    }

    /// Rebuild from a snapshot: drops all state, then applies frames.
    pub fn apply_snapshot(&mut self, frames: &[Frame]) {
        self.entries.clear();
        self.tools.clear();
        self.turns.clear();
        self.diffs.clear();
        self.children.clear();
        self.team_budget = None;
        self.last_event_id = 0;
        for frame in frames {
            self.apply_frame(frame);
        }
        self.resync_pending = false;
    }

    /// Transcript rows.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Tool call/result pairs, arrival order.
    #[must_use]
    pub fn tools(&self) -> &[ToolEntry] {
        &self.tools
    }

    /// Turn lifecycle rows.
    #[must_use]
    pub fn turns(&self) -> &[TurnState] {
        &self.turns
    }

    /// Surfaced diffs.
    #[must_use]
    pub fn diffs(&self) -> &[DiffEntry] {
        &self.diffs
    }

    /// Hunks still awaiting review, across all diffs.
    #[must_use]
    pub fn pending_hunks(&self) -> usize {
        self.diffs.iter().map(DiffEntry::pending_count).sum()
    }

    /// Decide the first pending hunk (hunk-review gate); returns the
    /// owning path when a hunk was decided. Client-side state until
    /// the wire carries hunk decisions.
    pub fn decide_next_hunk(&mut self, decision: HunkDecision) -> Option<String> {
        for diff in &mut self.diffs {
            if diff.decide_next_pending(decision).is_some() {
                return Some(diff.path.clone());
            }
        }
        None
    }

    /// Total transcript characters (HUD context estimate input).
    #[must_use]
    pub fn context_chars(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| match entry {
                Entry::User(text) | Entry::Assistant(text) | Entry::System(text) => text.len(),
                Entry::Notice(text) => text.len(),
                Entry::Condensed(summary) => summary.len(),
            })
            .sum()
    }

    /// Record a client-side notice (local commands, resolutions).
    /// Notes are UI-only: they never count toward [`Self::context_chars`].
    pub fn note(&mut self, text: &str) {
        self.notes.push(text.to_owned());
    }

    /// Client-side notices, oldest first.
    #[must_use]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use austringer_daemon::protocol::SessionEvent;

    fn user_frame(id: u64, text: &str) -> Frame {
        Frame {
            id,
            event: SessionEvent::Recorded {
                seq: id,
                event: Event::User {
                    blocks: vec![ContentBlock::Text {
                        text: text.to_owned(),
                    }],
                },
            },
        }
    }

    fn tool_call_frame(id: u64, call_id: &str, tool: &str) -> Frame {
        Frame {
            id,
            event: SessionEvent::Recorded {
                seq: id,
                event: Event::ToolCall {
                    call_id: call_id.to_owned(),
                    tool: tool.to_owned(),
                    arguments: serde_json::json!({"q": 1}),
                },
            },
        }
    }

    fn tool_result_frame(id: u64, call_id: &str, text: &str) -> Frame {
        Frame {
            id,
            event: SessionEvent::Recorded {
                seq: id,
                event: Event::ToolResult {
                    call_id: call_id.to_owned(),
                    blocks: vec![ContentBlock::Text {
                        text: text.to_owned(),
                    }],
                },
            },
        }
    }

    #[test]
    fn recorded_events_reduce_to_entries_and_track_ids() {
        let mut view = SessionView::new();
        view.apply_frame(&user_frame(1, "hello"));
        view.apply_frame(&user_frame(2, "again"));
        assert_eq!(
            view.entries(),
            &[Entry::User("hello".into()), Entry::User("again".into())]
        );
        assert_eq!(view.last_event_id, 2);
    }

    #[test]
    fn tool_call_pairs_with_result_by_call_id() {
        let mut view = SessionView::new();
        view.apply_frame(&tool_call_frame(1, "c1", "echo"));
        view.apply_frame(&tool_result_frame(2, "c1", "worker output"));
        assert_eq!(view.tools().len(), 1);
        assert_eq!(view.tools()[0].call_id, "c1");
        assert_eq!(view.tools()[0].tool, "echo");
        assert_eq!(view.tools()[0].result.as_deref(), Some("worker output"));
    }

    #[test]
    fn turn_lifecycle_and_daemon_events_tracked() {
        let mut view = SessionView::new();
        view.apply_frame(&Frame {
            id: 1,
            event: SessionEvent::TurnStarted { turn: 1 },
        });
        view.apply_frame(&Frame {
            id: 2,
            event: SessionEvent::Notification {
                kind: "sched".into(),
                message: "nightly pack ran".into(),
            },
        });
        view.apply_frame(&Frame {
            id: 3,
            event: SessionEvent::Diff {
                path: "src/lib.rs".into(),
                before_hash: String::new(),
                after_hash: "abc".into(),
                hunks: vec!["@@ -1 +1 @@".into()],
            },
        });
        view.apply_frame(&Frame {
            id: 4,
            event: SessionEvent::TurnFinished {
                turn: 1,
                outcome: "task_completed".into(),
            },
        });
        assert_eq!(view.turns().len(), 1);
        assert_eq!(view.turns()[0].outcome.as_deref(), Some("task_completed"));
        assert!(
            view.entries()
                .contains(&Entry::Notice("sched: nightly pack ran".into()))
        );
        assert_eq!(view.diffs().len(), 1);
        assert_eq!(view.diffs()[0].path, "src/lib.rs");
    }

    #[test]
    fn resync_marker_flags_and_snapshot_rebuilds() {
        let mut view = SessionView::new();
        view.apply_frame(&user_frame(1, "stale"));
        view.apply_server_msg(&ServerMsg::StateResyncRequired { last_event_id: 9 });
        assert!(view.resync_pending);
        view.apply_server_msg(&ServerMsg::Snapshot {
            frames: vec![user_frame(5, "fresh"), user_frame(6, "newest")],
        });
        assert!(!view.resync_pending);
        assert_eq!(
            view.entries(),
            &[Entry::User("fresh".into()), Entry::User("newest".into())]
        );
        assert_eq!(view.last_event_id, 6);
    }

    #[test]
    fn context_chars_sum_transcript_text() {
        let mut view = SessionView::new();
        view.apply_frame(&user_frame(1, "12345"));
        view.apply_frame(&Frame {
            id: 2,
            event: SessionEvent::Recorded {
                seq: 2,
                event: Event::Assistant {
                    blocks: vec![ContentBlock::Text { text: "678".into() }],
                },
            },
        });
        assert_eq!(view.context_chars(), 8);
    }

    #[test]
    fn event_frame_via_server_msg_applies() {
        let mut view = SessionView::new();
        view.apply_server_msg(&ServerMsg::Event {
            frame: user_frame(1, "via msg"),
        });
        assert_eq!(view.entries(), &[Entry::User("via msg".into())]);
        assert_eq!(view.last_event_id, 1);
    }

    #[test]
    fn snapshot_marker_and_restore_notice_render() {
        let mut view = SessionView::new();
        view.apply_frame(&Frame {
            id: 4,
            event: SessionEvent::Recorded {
                seq: 4,
                event: Event::Snapshot {
                    label: "turn-2".into(),
                },
            },
        });
        view.apply_frame(&Frame {
            id: 5,
            event: SessionEvent::Recorded {
                seq: 4,
                event: Event::System {
                    text: "restored to turn 2".into(),
                },
            },
        });
        assert_eq!(
            view.entries(),
            &[
                Entry::Notice("rewind point: turn-2".into()),
                Entry::System("restored to turn 2".into()),
            ]
        );
    }
}
