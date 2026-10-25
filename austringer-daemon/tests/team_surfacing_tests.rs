//! Task-15 acceptance (plan todo 15): team surfacing — the
//! Warp-precedence aggregate over the Child* team-event vocabulary and
//! the v1 compat gate for the new `TeamBudget` / `TeamBudgetExhausted`
//! session events (the budget surface of the delegation aggregate).
//! The end-to-end surfacing tests (budget-exhaustion halt, blocked
//! child ask) live beside the task-delegation kit in
//! `task_delegation_tests.rs`.
//!
//! Receipt: Warp PR#11367 / #12084 — status precedence
//! InProgress > Blocked > Error > Cancelled > Success plus
//! WaitingForEvents; an aggregate NEVER flips terminal while any
//! child is in-progress.

use austringer_daemon::compat::downgrade_for_v1;
use austringer_daemon::protocol::{Frame, ServerMsg, SessionEvent};
use austringer_daemon::team::{TeamStatus, aggregate_of_states};

// ---------------------------------------------------------------------------
// (1) the Warp precedence aggregate (receipts Warp PR#11367/#12084)
// ---------------------------------------------------------------------------

#[test]
fn three_child_fixture_aggregates_in_progress_not_terminal() {
    // Given: the todo-15 fixture — one child in-progress, one blocked
    // on an ask, one succeeded.
    let states = ["running", "blocked", "success"];
    // Then: the aggregate is InProgress — the running child holds the
    // team open past the terminal sibling (never flip terminal while
    // any child is in-progress).
    assert_eq!(aggregate_of_states(states), TeamStatus::InProgress);
    // And: the same fixture with the collision vocabulary for the
    // blocked child (a write-set collision blocks pre-quarantine).
    assert_eq!(
        aggregate_of_states(["file_touched", "collision", "success"]),
        TeamStatus::InProgress
    );
}

#[test]
fn aggregate_terminal_settles_on_highest_terminal_and_empty_waits() {
    // Given: an all-terminal team (error + cancelled + success).
    // Then: Error outranks Cancelled outranks Success — the team
    // settles terminal but never reports success over a failure.
    assert_eq!(
        aggregate_of_states(["failed", "quarantined", "success"]),
        TeamStatus::Error
    );
    assert_eq!(
        aggregate_of_states(["quarantined", "success"]),
        TeamStatus::Cancelled
    );
    assert_eq!(
        aggregate_of_states(["success", "success"]),
        TeamStatus::Success
    );
    // And: a team with no reported child state is WaitingForEvents.
    assert_eq!(aggregate_of_states([]), TeamStatus::WaitingForEvents);
    // And: an unmapped state never collapses to a terminal aggregate.
    assert_eq!(
        aggregate_of_states(["success", "something-new"]),
        TeamStatus::InProgress
    );
}

#[test]
fn team_status_vocabulary_is_stable_snake_case() {
    assert_eq!(TeamStatus::InProgress.as_str(), "in_progress");
    assert_eq!(TeamStatus::Blocked.as_str(), "blocked");
    assert_eq!(TeamStatus::Error.as_str(), "error");
    assert_eq!(TeamStatus::Cancelled.as_str(), "cancelled");
    assert_eq!(TeamStatus::Success.as_str(), "success");
    assert_eq!(TeamStatus::WaitingForEvents.as_str(), "waiting_for_events");
}

// ---------------------------------------------------------------------------
// (2) compat gate: the new team-budget payloads riding Event frames
//     downgrade to v1-decodable Notifications (frame id preserved) —
//     task 8's gate, applied to the todo-15 variants
// ---------------------------------------------------------------------------

#[test]
fn compat_downgrades_team_budget_events_to_v1_notifications() {
    let frames = vec![
        (
            SessionEvent::TeamBudget {
                spawned: 2,
                tokens_spent: 50,
                fan_out_cap: 4,
                token_budget: 1000,
            },
            "team_budget",
        ),
        (
            SessionEvent::TeamBudgetExhausted {
                spent: 1000,
                budget: 1000,
            },
            "team_budget_exhausted",
        ),
    ];
    for (event, expected_kind) in frames {
        let frame = Frame { id: 11, event };
        let downgraded =
            downgrade_for_v1(ServerMsg::Event { frame }).expect("downgraded, not withheld");
        let ServerMsg::Event { frame } = downgraded else {
            panic!("shape");
        };
        assert_eq!(frame.id, 11, "frame id survives the downgrade");
        match frame.event {
            SessionEvent::Notification { kind, message } => {
                assert_eq!(kind, expected_kind, "v1 kind channel: {message}");
                assert!(!message.is_empty(), "message carries the payload");
            }
            other => panic!("expected notification downgrade, got {other:?}"),
        }
    }
}
