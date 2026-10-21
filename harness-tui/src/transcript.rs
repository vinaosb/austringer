//! Session transcript view model: reduces daemon protocol frames into
//! renderable transcript entries, tool-call pairs, turn lifecycle,
//! diffs, and worker state. A snapshot rebuilds the view from zero
//! (the divergence resync path).

use std::collections::HashMap;

use harness_core::event::{ContentBlock, Event};
use harness_daemon::protocol::{Frame, ServerMsg, SessionEvent};
use harness_daemon::team::TeamStatus;

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

/// A surfaced semantic diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffEntry {
    /// Changed path.
    pub path: String,
    /// Rendered hunks.
    pub hunks: Vec<String>,
}

/// Live child-session state (feeds the team grid); the state string
/// is the todo-8 team contract vocabulary (`running`, `collision`,
/// `escalation`, `file_touched`, `blocked`, `success`, `failed`,
/// `quarantined`) and `detail` carries the latest phase context (the
/// collision glob, the surfaced ask, the finish summary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildCell {
    /// Child session id (`{launch}-child-{task}`).
    pub child: String,
    /// Task label.
    pub task: String,
    /// Latest phase/state.
    pub state: String,
    /// Latest phase detail (collision glob, ask summary, outcome).
    pub detail: String,
}

/// Latest delegation-budget view (the `TeamBudget` event payload;
/// todo 15's budget surfacing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamBudgetView {
    /// Task children spawned this session.
    pub spawned: u32,
    /// Tokens spent across those children.
    pub tokens_spent: u64,
    /// The effective fan-out cap.
    pub fan_out_cap: u32,
    /// The per-parent token budget.
    pub token_budget: u64,
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
                self.diffs.push(DiffEntry {
                    path: path.clone(),
                    hunks: hunks.clone(),
                });
            }
            SessionEvent::Notification { kind, message } => {
                self.entries
                    .push(Entry::Notice(format!("{kind}: {message}")));
            }
            SessionEvent::ChildStarted { child, task, .. } => {
                self.children.insert(
                    child.clone(),
                    ChildCell {
                        child: child.clone(),
                        task: task.clone(),
                        state: "running".to_owned(),
                        detail: String::new(),
                    },
                );
            }
            SessionEvent::ChildProgress { child, phase, detail } => {
                if let Some(cell) = self.children.get_mut(child) {
                    cell.state = phase.clone();
                    cell.detail = detail.clone();
                } else {
                    self.children.insert(
                        child.clone(),
                        ChildCell {
                            child: child.clone(),
                            task: String::new(),
                            state: phase.clone(),
                            detail: detail.clone(),
                        },
                    );
                }
            }
            SessionEvent::ChildFinished { child, state, detail, .. } => {
                if let Some(cell) = self.children.get_mut(child) {
                    cell.state = state.clone();
                    cell.detail = detail.clone();
                }
            }
            SessionEvent::TeamBudget {
                spawned,
                tokens_spent,
                fan_out_cap,
                token_budget,
            } => {
                self.team_budget = Some(TeamBudgetView {
                    spawned: *spawned,
                    tokens_spent: *tokens_spent,
                    fan_out_cap: *fan_out_cap,
                    token_budget: *token_budget,
                });
            }
            SessionEvent::TeamBudgetExhausted { spent, budget } => {
                self.entries.push(Entry::Notice(format!(
                    "team budget exhausted: {spent} of {budget} tokens; further task spawns refused"
                )));
            }
            SessionEvent::CostWarning { consumed_micros, cap_micros } => {
                self.entries.push(Entry::Notice(format!(
                    "cost warning: {consumed_micros} of {cap_micros} micro-USD (80% of cap)"
                )));
            }
            SessionEvent::CostStop { consumed_micros, cap_micros } => {
                self.entries.push(Entry::Notice(format!(
                    "cost stop: cap reached ({consumed_micros} of {cap_micros} micro-USD); further turns refused"
                )));
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
                self.entries
                    .push(Entry::Notice(format!("snapshot: {label}")));
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

    /// Child cells for the team grid.
    #[must_use]
    pub fn children(&self) -> Vec<&ChildCell> {
        self.children.values().collect()
    }

    /// The Warp-precedence aggregate over child states (todo 15;
    /// never terminal while any child is in-progress).
    #[must_use]
    pub fn team_status(&self) -> TeamStatus {
        harness_daemon::team::aggregate_of_states(
            self.children.values().map(|cell| cell.state.as_str()),
        )
    }

    /// Latest delegation-budget view (None before any spawn).
    #[must_use]
    pub fn team_budget(&self) -> Option<TeamBudgetView> {
        self.team_budget.clone()
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
    use harness_daemon::protocol::SessionEvent;

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
    fn child_events_drive_team_cells() {
        let mut view = SessionView::new();
        view.apply_frame(&Frame {
            id: 1,
            event: SessionEvent::ChildStarted {
                child: "l-child-build".into(),
                task: "build".into(),
                role: "worker".into(),
                model: "gpt-5-5".into(),
            },
        });
        assert_eq!(view.children().len(), 1);
        assert_eq!(view.children()[0].state, "running");
        view.apply_frame(&Frame {
            id: 2,
            event: SessionEvent::ChildProgress {
                child: "l-child-build".into(),
                phase: "collision".into(),
                detail: "src/** overlaps".into(),
            },
        });
        assert_eq!(view.children()[0].state, "collision");
        assert_eq!(view.children()[0].detail, "src/** overlaps");
        view.apply_frame(&Frame {
            id: 3,
            event: SessionEvent::ChildFinished {
                child: "l-child-build".into(),
                task: "build".into(),
                state: "quarantined".into(),
                detail: "src/** overlaps".into(),
            },
        });
        assert_eq!(view.children()[0].state, "quarantined");
    }

    #[test]
    fn team_aggregate_never_flips_terminal_while_a_child_runs() {
        // Given: the todo-15 fixture — one child in-progress, one
        // blocked on an ask, one succeeded.
        let mut view = SessionView::new();
        for (child, task) in [("c-run", "build"), ("c-block", "scan"), ("c-done", "docs")] {
            view.apply_frame(&Frame {
                id: 1,
                event: SessionEvent::ChildStarted {
                    child: child.into(),
                    task: task.into(),
                    role: "worker".into(),
                    model: "gpt-5-5".into(),
                },
            });
        }
        view.apply_frame(&Frame {
            id: 2,
            event: SessionEvent::ChildProgress {
                child: "c-block".into(),
                phase: "blocked".into(),
                detail: "permission: edit_lines src/lib.rs".into(),
            },
        });
        view.apply_frame(&Frame {
            id: 3,
            event: SessionEvent::ChildFinished {
                child: "c-done".into(),
                task: "docs".into(),
                state: "success".into(),
                detail: "done".into(),
            },
        });
        // Then: the Warp-precedence aggregate is InProgress — the
        // running child holds the team open past the terminal sibling.
        assert_eq!(view.team_status(), TeamStatus::InProgress);
        // And: the blocked child's ask detail rides its cell.
        let blocked = view
            .children()
            .into_iter()
            .find(|cell| cell.child == "c-block")
            .expect("blocked cell");
        assert_eq!(blocked.state, "blocked");
        assert_eq!(blocked.detail, "permission: edit_lines src/lib.rs");
    }

    #[test]
    fn team_budget_frames_surface_and_empty_team_waits() {
        // Given: a fresh view — no child has reported state.
        let mut view = SessionView::new();
        assert_eq!(view.team_status(), TeamStatus::WaitingForEvents);
        assert_eq!(view.team_budget(), None);
        // When: budget frames arrive (spawn authorization, then the
        // recorded spend), then the typed exhaustion halt.
        view.apply_frame(&Frame {
            id: 1,
            event: SessionEvent::TeamBudget {
                spawned: 1,
                tokens_spent: 0,
                fan_out_cap: 4,
                token_budget: 1000,
            },
        });
        view.apply_frame(&Frame {
            id: 2,
            event: SessionEvent::TeamBudget {
                spawned: 1,
                tokens_spent: 25,
                fan_out_cap: 4,
                token_budget: 1000,
            },
        });
        view.apply_frame(&Frame {
            id: 3,
            event: SessionEvent::TeamBudgetExhausted {
                spent: 1000,
                budget: 1000,
            },
        });
        // Then: the panel input is the LATEST budget snapshot…
        assert_eq!(
            view.team_budget(),
            Some(TeamBudgetView {
                spawned: 1,
                tokens_spent: 25,
                fan_out_cap: 4,
                token_budget: 1000,
            })
        );
        // …and the halt stays visible even with the team accordion
        // closed (it is a transcript notice, not only panel state).
        assert!(
            view.entries()
                .iter()
                .any(|entry| matches!(entry, Entry::Notice(text) if text.contains("team budget exhausted"))),
            "exhaustion notice: {:?}",
            view.entries()
        );
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
                Entry::Notice("snapshot: turn-2".into()),
                Entry::System("restored to turn 2".into()),
            ]
        );
    }
}
