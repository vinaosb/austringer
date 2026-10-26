//! The team grid's view state: child cells, the delegation-budget
//! view, and the apply arms' bodies for the Child*/Team* frames (the
//! todo-19-named extraction; the exhaustive `apply_frame` match stays
//! in [`super`] by the workspace rule).

use austringer_daemon::team::TeamStatus;

use super::{Entry, SessionView};

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

impl SessionView {
    /// `ChildStarted` arm body.
    pub(crate) fn apply_child_started(&mut self, child: &str, task: &str) {
        self.children.insert(
            child.to_owned(),
            ChildCell {
                child: child.to_owned(),
                task: task.to_owned(),
                state: "running".to_owned(),
                detail: String::new(),
            },
        );
    }

    /// `ChildProgress` arm body.
    pub(crate) fn apply_child_progress(&mut self, child: &str, phase: &str, detail: &str) {
        if let Some(cell) = self.children.get_mut(child) {
            cell.state = phase.to_owned();
            cell.detail = detail.to_owned();
        } else {
            self.children.insert(
                child.to_owned(),
                ChildCell {
                    child: child.to_owned(),
                    task: String::new(),
                    state: phase.to_owned(),
                    detail: detail.to_owned(),
                },
            );
        }
    }

    /// `ChildFinished` arm body.
    pub(crate) fn apply_child_finished(&mut self, child: &str, state: &str, detail: &str) {
        if let Some(cell) = self.children.get_mut(child) {
            cell.state = state.to_owned();
            cell.detail = detail.to_owned();
        }
    }

    /// `TeamBudget` arm body.
    pub(crate) fn apply_team_budget(
        &mut self,
        spawned: u32,
        tokens_spent: u64,
        fan_out_cap: u32,
        token_budget: u64,
    ) {
        self.team_budget = Some(TeamBudgetView {
            spawned,
            tokens_spent,
            fan_out_cap,
            token_budget,
        });
    }

    /// `TeamBudgetExhausted` arm body (rides the transcript so the
    /// halt stays visible even with the team accordion closed).
    pub(crate) fn apply_team_exhausted(&mut self, spent: u64, budget: u64) {
        self.entries.push(Entry::Notice(format!(
            "team budget exhausted: {spent} of {budget} tokens; further task spawns refused"
        )));
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
        austringer_daemon::team::aggregate_of_states(
            self.children.values().map(|cell| cell.state.as_str()),
        )
    }

    /// Latest delegation-budget view (None before any spawn).
    #[must_use]
    pub fn team_budget(&self) -> Option<TeamBudgetView> {
        self.team_budget.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Entry, SessionView};
    use super::{ChildCell, TeamBudgetView};
    use austringer_daemon::protocol::{Frame, SessionEvent};
    use austringer_daemon::team::TeamStatus;

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
    fn child_cell_fields_are_the_grid_contract() {
        let cell = ChildCell {
            child: "l-1".into(),
            task: "t".into(),
            state: "running".into(),
            detail: String::new(),
        };
        assert_eq!(cell.state, "running");
    }
}
