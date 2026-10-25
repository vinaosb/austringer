//! Team status aggregation (todo 15): the Warp status-precedence
//! algorithm over the `Child*` team-event vocabulary (receipt: Warp
//! PR#11367 / #12084 — InProgress > Blocked > Error > Cancelled >
//! Success, plus WaitingForEvents when no child has reported state).
//! The aggregate NEVER flips to a terminal status while any child is
//! in-progress: InProgress outranks every terminal status, so a
//! still-running sibling holds the team open no matter what the
//! others did. The daemon, the TUI, and the web SPA all reduce the
//! same `Child*` events; this module is the single source for the
//! state-string mapping (the web port in `austringer-web/assets/ui.js`
//! mirrors it).

/// Aggregate status of a delegation team (Warp's vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamStatus {
    /// At least one child is running (or in an unmapped phase —
    /// unknown never collapses to terminal).
    InProgress,
    /// A child is waiting on something (a surfaced ask, a write-set
    /// collision).
    Blocked,
    /// A child failed.
    Error,
    /// A child was quarantined before it ran.
    Cancelled,
    /// A child succeeded (terminal; loses to every other status).
    Success,
    /// No child has reported state yet.
    WaitingForEvents,
}

impl TeamStatus {
    /// Stable wire/render vocabulary (snake_case; the web port uses
    /// the same strings).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            TeamStatus::InProgress => "in_progress",
            TeamStatus::Blocked => "blocked",
            TeamStatus::Error => "error",
            TeamStatus::Cancelled => "cancelled",
            TeamStatus::Success => "success",
            TeamStatus::WaitingForEvents => "waiting_for_events",
        }
    }
}

/// Precedence rank (higher wins; Warp PR#11367). WaitingForEvents
/// never ranks — it is only the empty-team aggregate.
fn precedence(status: TeamStatus) -> u8 {
    match status {
        TeamStatus::InProgress => 5,
        TeamStatus::Blocked => 4,
        TeamStatus::Error => 3,
        TeamStatus::Cancelled => 2,
        TeamStatus::Success => 1,
        TeamStatus::WaitingForEvents => 0,
    }
}

/// Maps one child's team-event state string (the `ChildStarted`
/// initial state, a `ChildProgress` phase, or a `ChildFinished`
/// outcome) onto [`TeamStatus`]. Unmapped strings stay non-terminal —
/// an unknown phase must never collapse the team to a terminal
/// aggregate.
#[must_use]
pub fn child_status(state: &str) -> TeamStatus {
    match state {
        "success" => TeamStatus::Success,
        "failed" => TeamStatus::Error,
        "quarantined" => TeamStatus::Cancelled,
        "blocked" | "collision" => TeamStatus::Blocked,
        _ => TeamStatus::InProgress,
    }
}

/// The aggregate over child state strings: the highest-precedence
/// child status wins; an empty team is WaitingForEvents.
#[must_use]
pub fn aggregate_of_states<'a>(states: impl IntoIterator<Item = &'a str>) -> TeamStatus {
    let mut best: Option<(u8, TeamStatus)> = None;
    for state in states {
        let status = child_status(state);
        let rank = precedence(status);
        if best.is_none_or(|(current, _)| rank > current) {
            best = Some((rank, status));
        }
    }
    best.map_or(TeamStatus::WaitingForEvents, |(_, status)| status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_child_outranks_every_terminal_sibling() {
        for terminal in ["success", "failed", "quarantined"] {
            assert_eq!(
                aggregate_of_states(["running", terminal]),
                TeamStatus::InProgress,
                "running + {terminal} must stay InProgress"
            );
        }
    }

    #[test]
    fn blocked_outranks_terminals_but_not_in_progress() {
        assert_eq!(
            aggregate_of_states(["blocked", "failed"]),
            TeamStatus::Blocked
        );
        assert_eq!(
            aggregate_of_states(["blocked", "blocked"]),
            TeamStatus::Blocked
        );
        assert_eq!(
            aggregate_of_states(["blocked", "running"]),
            TeamStatus::InProgress
        );
    }

    #[test]
    fn terminal_order_is_error_then_cancelled_then_success() {
        assert_eq!(
            aggregate_of_states(["quarantined", "success"]),
            TeamStatus::Cancelled
        );
        assert_eq!(
            aggregate_of_states(["failed", "quarantined", "success"]),
            TeamStatus::Error
        );
        assert_eq!(aggregate_of_states(["success"]), TeamStatus::Success);
    }

    #[test]
    fn empty_team_waits_for_events() {
        assert_eq!(aggregate_of_states([]), TeamStatus::WaitingForEvents);
    }

    #[test]
    fn unmapped_state_stays_non_terminal() {
        assert_eq!(
            aggregate_of_states(["success", "brand-new-phase"]),
            TeamStatus::InProgress
        );
    }
}
