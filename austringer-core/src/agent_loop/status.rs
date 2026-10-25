//! Session status state machine with typed, rejected-illegal transitions.

/// Lifecycle status of a session (graceful-cancel semantics, Warp pattern).
///
/// `Cancelling` is the transient grace state: `cancel()` moves a running
/// session there, in-flight operations get the cancel-grace window to reach
/// a clean boundary, and the loop then lands `Cancelled` (cleanly, or by
/// force when the window expires). `Cancelled` is terminal except for the
/// single sanctioned resume escape back to `InProgress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionStatus {
    /// A run is executing or a turn-level completion left it parked.
    InProgress,
    /// Task completion was refused (open todos / plan gate) and the model
    /// is being re-prompted with the blocking state.
    Blocked,
    /// A cancel was requested; the grace window is running.
    Cancelling,
    /// Terminal: the session was cancelled (resumable).
    Cancelled,
    /// Terminal: task-level completion succeeded.
    Success,
    /// Terminal: the session failed (max turns, exhausted errors, stages).
    Failed,
}

impl SessionStatus {
    /// True for terminal statuses (`Cancelled` included — it only leaves
    /// via the explicit resume escape).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Success | Self::Failed)
    }

    /// Applies a transition, rejecting illegal ones with the typed pair.
    ///
    /// Legal set: `InProgress -> Success | Failed | Blocked | Cancelling`;
    /// `Blocked -> InProgress | Cancelling`; `Cancelling -> Cancelled`;
    /// and the resume escape `Cancelled -> InProgress`. `Success` and
    /// `Failed` are absorbing. Self-transitions are not transitions.
    pub fn transition(self, to: Self) -> Result<Self, IllegalTransition> {
        let legal = matches!(
            (self, to),
            (
                Self::InProgress,
                Self::Success | Self::Failed | Self::Blocked | Self::Cancelling
            ) | (Self::Blocked, Self::InProgress | Self::Cancelling)
                | (Self::Cancelling, Self::Cancelled)
                | (Self::Cancelled, Self::InProgress)
        );
        if legal {
            Ok(to)
        } else {
            Err(IllegalTransition { from: self, to })
        }
    }
}

/// A rejected status transition, carrying the offending pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("illegal session-status transition: {from:?} -> {to:?}")]
pub struct IllegalTransition {
    /// The status being left.
    pub from: SessionStatus,
    /// The status that was attempted.
    pub to: SessionStatus,
}

#[cfg(test)]
mod tests {
    use super::SessionStatus::*;

    #[test]
    fn terminal_statuses_are_reported() {
        assert!(!InProgress.is_terminal());
        assert!(!Blocked.is_terminal());
        assert!(!Cancelling.is_terminal());
        assert!(Cancelled.is_terminal());
        assert!(Success.is_terminal());
        assert!(Failed.is_terminal());
    }

    #[test]
    fn self_transitions_are_rejected() {
        assert!(InProgress.transition(InProgress).is_err());
        assert!(Cancelling.transition(Cancelling).is_err());
    }
}
